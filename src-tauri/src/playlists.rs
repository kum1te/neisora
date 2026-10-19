//! Typed playlist writes. A fresh server header, not a renderer-provided flag, grants editing.
use crate::{api::Api, library};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Change {
    Create {
        title: String,
        description: String,
        privacy: String,
        video_id: Option<String>,
        video_ids: Option<Vec<String>>,
    },
    Update {
        playlist_id: String,
        title: String,
        description: String,
        privacy: String,
    },
    Delete {
        playlist_id: String,
    },
    Add {
        playlist_id: String,
        video_id: String,
    },
    Remove {
        playlist_id: String,
        video_id: String,
        set_video_id: String,
    },
    Move {
        playlist_id: String,
        set_video_id: String,
        before: Option<String>,
    },
}
fn id(value: &str, max: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > max
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err("Некорректный идентификатор плейлиста или трека".into());
    }
    Ok(())
}
fn video(value: &str) -> Result<(), String> {
    id(value, 11)?;
    if value.len() != 11 {
        return Err("Некорректный трек".into());
    }
    Ok(())
}
fn details(title: &str, description: &str, privacy: &str) -> Result<(), String> {
    if title.trim().is_empty()
        || title.chars().count() > 150
        || title
            .chars()
            .any(|c| c.is_control() || c == '<' || c == '>')
        || description.chars().count() > 5000
    {
        return Err("Название: 1–150 символов без < и >; описание: до 5000 символов".into());
    }
    if !["PRIVATE", "UNLISTED", "PUBLIC"].contains(&privacy) {
        return Err("Неизвестная видимость плейлиста".into());
    }
    Ok(())
}
impl Change {
    fn request(&self) -> Result<(&'static str, Value, Option<&str>), String> {
        let (route, body, playlist): (_, _, Option<&str>) = match self {
            Self::Create {
                title,
                description,
                privacy,
                video_id,
                video_ids,
            } => {
                details(title, description, privacy)?;
                let mut body =
                    json!({"title":title.trim(),"description":description,"privacyStatus":privacy});
                if let Some(ids) = video_ids {
                    if video_id.is_some() || ids.is_empty() || ids.len() > 2000 {
                        return Err("В плейлисте должно быть от 1 до 2000 треков".into());
                    }
                    for value in ids {
                        video(value)?;
                    }
                    body["videoIds"] = json!(ids);
                }
                if let Some(v) = video_id {
                    video(v)?;
                    body["videoIds"] = json!([v]);
                }
                ("playlist/create", body, None)
            }
            Self::Update {
                playlist_id,
                title,
                description,
                privacy,
            } => {
                details(title, description, privacy)?;
                (
                    "browse/edit_playlist",
                    json!({"playlistId":playlist_id,"actions":[
                        {"action":"ACTION_SET_PLAYLIST_NAME","playlistName":title.trim()},
                        {"action":"ACTION_SET_PLAYLIST_DESCRIPTION","playlistDescription":description},
                        {"action":"ACTION_SET_PLAYLIST_PRIVACY","playlistPrivacy":privacy}
                    ]}),
                    Some(playlist_id),
                )
            }
            Self::Delete { playlist_id } => (
                "playlist/delete",
                json!({"playlistId":playlist_id}),
                Some(playlist_id),
            ),
            Self::Add {
                playlist_id,
                video_id,
            } => {
                video(video_id)?;
                (
                    "browse/edit_playlist",
                    json!({"playlistId":playlist_id,"actions":[{"action":"ACTION_ADD_VIDEO","addedVideoId":video_id}]}),
                    Some(playlist_id),
                )
            }
            Self::Remove {
                playlist_id,
                video_id,
                set_video_id,
            } => {
                video(video_id)?;
                id(set_video_id, 512)?;
                (
                    "browse/edit_playlist",
                    json!({"playlistId":playlist_id,"actions":[{"action":"ACTION_REMOVE_VIDEO","removedVideoId":video_id,"setVideoId":set_video_id}]}),
                    Some(playlist_id),
                )
            }
            Self::Move {
                playlist_id,
                set_video_id,
                before,
            } => {
                id(set_video_id, 512)?;
                let mut action =
                    json!({"action":"ACTION_MOVE_VIDEO_BEFORE","setVideoId":set_video_id});
                if let Some(b) = before {
                    id(b, 512)?;
                    if b == set_video_id {
                        return Err("Трек уже на этом месте".into());
                    }
                    action["movedSetVideoIdSuccessor"] = json!(b);
                }
                (
                    "browse/edit_playlist",
                    json!({"playlistId":playlist_id,"actions":[action]}),
                    Some(playlist_id),
                )
            }
        };
        if let Some(p) = playlist {
            id(p, 200)?;
            if p.starts_with("RD") || p == "LM" || p.starts_with("OLAK") {
                return Err("Эту подборку нельзя редактировать".into());
            }
        }
        Ok((route, body, playlist))
    }
}
fn accepted(data: &Value, create: bool) -> Result<Option<String>, String> {
    if create {
        if let Some(p) = data["playlistId"].as_str() {
            id(p, 200)?;
            return Ok(Some(p.into()));
        }
    } else if data["status"] == "STATUS_SUCCEEDED" {
        return Ok(None);
    }
    Err("YouTube не подтвердил изменение плейлиста. Обнови страницу перед повтором.".into())
}
impl Api {
    pub async fn playlist_change(&self, change: Change) -> Result<Option<String>, String> {
        let epoch = self.session_generation();
        let (route, body, playlist) = change.request()?;
        let _write = self.playlist_updates.lock().await;
        if epoch != self.session_generation() {
            return Err("Сессия изменилась. Повтори изменение плейлиста.".into());
        }
        if !self.authenticated().await {
            return Err("Подключи YouTube в настройках".into());
        }
        if let Some(p) = playlist {
            let data = self
                .request_for_session("browse", json!({"browseId":format!("VL{p}")}), epoch)
                .await?;
            let own = library::collection(&data).ok_or("YouTube не вернул права на плейлист")?;
            if !own.editable || own.playlist_id != p {
                return Err("Можно изменять только свои плейлисты".into());
            }
        }
        let response = self.request_for_session(route, body, epoch).await?;
        accepted(&response, matches!(change, Change::Create { .. }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queue_creation_preserves_order_and_rejects_local_or_oversized_payloads() {
        let request = |ids: Value| {
            serde_json::from_value::<Change>(json!({"action":"create","title":"Queue","description":"","privacy":"PRIVATE","videoIds":ids})).unwrap()
        };
        let (_, body, _) = request(json!(["abcdefghijk", "abcdefghijk", "zyxwvutsrqp"]))
            .request()
            .unwrap();
        assert_eq!(
            body["videoIds"],
            json!(["abcdefghijk", "abcdefghijk", "zyxwvutsrqp"])
        );
        for ids in [
            json!([]),
            json!(["local:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]),
            json!(vec!["abcdefghijk"; 2001]),
        ] {
            assert!(request(ids).request().is_err());
        }
        let both:Change=serde_json::from_value(json!({"action":"create","title":"Queue","description":"","privacy":"PRIVATE","videoId":"abcdefghijk","videoIds":["abcdefghijk"]})).unwrap();
        assert!(both.request().is_err());
    }
    #[tokio::test]
    async fn queued_write_is_bound_to_the_account_at_invocation() {
        let dir = std::env::temp_dir().join(format!(
            "neisora-playlist-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let api = Api::new(dir.clone()).unwrap();
        let guard = api.playlist_updates.lock().await;
        let mut pending = Box::pin(api.playlist_change(Change::Create {
            title: "test".into(),
            description: "".into(),
            privacy: "PRIVATE".into(),
            video_id: None,
            video_ids: None,
        }));
        // Poll exactly once: the write must wait behind the current mutation.
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(pending.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        api.logout().await.unwrap();
        drop(guard);
        let error = pending.await.unwrap_err();
        assert!(error.contains("Сессия изменилась"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_untyped_commands_and_invalid_fields() {
        assert!(serde_json::from_value::<Change>(
            json!({"action":"anything","url":"https://evil.test"})
        )
        .is_err());
        assert!(Change::Create {
            title: " ".into(),
            description: "".into(),
            privacy: "PUBLIC".into(),
            video_id: None,
            video_ids: None
        }
        .request()
        .is_err());
        assert!(Change::Add {
            playlist_id: "RDmix".into(),
            video_id: "abcdefghijk".into()
        }
        .request()
        .is_err());
        assert!(Change::Move {
            playlist_id: "PLtest".into(),
            set_video_id: "../../x".into(),
            before: None
        }
        .request()
        .is_err());
        assert!(details("test", "", "WORLD").is_err());
        assert!(details("<script>", "", "PRIVATE").is_err());
    }
    #[test]
    fn duplicate_entries_are_addressed_by_set_id_and_end_has_no_successor() {
        let c = Change::Remove {
            playlist_id: "PLtest".into(),
            video_id: "abcdefghijk".into(),
            set_video_id: "unique-occurrence".into(),
        };
        let (_, b, _) = c.request().unwrap();
        assert_eq!(b["actions"][0]["setVideoId"], "unique-occurrence");
        let c = Change::Move {
            playlist_id: "PLtest".into(),
            set_video_id: "unique-occurrence".into(),
            before: None,
        };
        let (_, b, _) = c.request().unwrap();
        assert!(b["actions"][0].get("movedSetVideoIdSuccessor").is_none());
        assert!(accepted(&json!({}), false).is_err());
        assert!(accepted(&json!({"status":"STATUS_FAILED"}), false).is_err());
        assert!(accepted(&json!({"status":"STATUS_SUCCEEDED"}), false).is_ok());
    }
    #[test]
    fn server_header_proves_ownership_and_collection_identity() {
        let data = json!({"contents":{"musicEditablePlaylistDetailHeaderRenderer":{"playlistId":"PLown","editHeader":{"musicPlaylistEditHeaderRenderer":{"privacy":"UNLISTED"}},"header":{"musicResponsiveHeaderRenderer":{"title":{"runs":[{"text":"Mine"}]},"description":{"musicDescriptionShelfRenderer":{"description":{"runs":[{"text":"Description"}]}}}}}}}});
        let c = library::collection(&data).unwrap();
        assert!(c.editable);
        assert_eq!(c.playlist_id, "PLown");
        assert_eq!(c.browse_id, "VLPLown");
        assert_eq!(c.privacy, "UNLISTED");
        assert_eq!(c.description, "Description");
        assert!(
            !library::collection(&json!({"header":{"watchEndpoint":{"playlistId":"PLother"}}}))
                .unwrap()
                .editable
        );
    }
}
