use crate::{
    api::Api,
    model::{self, Feed},
};
use serde::Serialize;
use serde_json::{json, Value};
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub browse_id: String,
    pub playlist_id: String,
    pub saved: Option<bool>,
    pub editable: bool,
    pub description: String,
    pub privacy: String,
}
pub fn like_status(node: &Value) -> Option<bool> {
    model::find(node, "likeStatus")
        .and_then(Value::as_str)
        .and_then(|s| match s {
            "LIKE" => Some(true),
            "DISLIKE" | "INDIFFERENT" => Some(false),
            _ => None,
        })
}
pub fn collection(data: &Value) -> Option<Collection> {
    let editable = model::find(data, "musicEditablePlaylistDetailHeaderRenderer");
    let header = editable
        .and_then(|h| h.get("header"))
        .or_else(|| data.get("header"))
        .or_else(|| model::find(data, "musicResponsiveHeaderRenderer"))
        .or(editable)?;
    let playlist = editable
        .and_then(|h| h.get("playlistId"))
        .or_else(|| {
            model::find(header, "likeEndpoint").and_then(|v| v.pointer("/target/playlistId"))
        })
        .or_else(|| model::find(header, "watchPlaylistEndpoint").and_then(|v| v.get("playlistId")))
        .or_else(|| model::find(header, "watchEndpoint").and_then(|v| v.get("playlistId")))?
        .as_str()?
        .to_owned();
    if playlist.starts_with("RD") || playlist == "LM" {
        return None;
    }
    let saved = like_status(header).or_else(|| library_toggle(header));
    Some(Collection {
        browse_id: format!("VL{playlist}"),
        playlist_id: playlist,
        saved,
        editable: editable.is_some(),
        description: model::find(header, "musicDescriptionShelfRenderer")
            .map(|d| model::text(&d["description"]))
            .unwrap_or_default(),
        privacy: editable
            .and_then(|h| h.pointer("/editHeader/musicPlaylistEditHeaderRenderer/privacy"))
            .and_then(Value::as_str)
            .unwrap_or("PRIVATE")
            .into(),
    })
}
fn library_toggle(node: &Value) -> Option<bool> {
    if let Some(b) = node.get("toggleButtonRenderer") {
        let icon = b
            .pointer("/defaultIcon/iconType")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if ["LIBRARY_ADD", "BOOKMARK_BORDER", "PLAYLIST_ADD"].contains(&icon) {
            return b.get("isToggled").and_then(Value::as_bool);
        }
        if ["LIBRARY_SAVED", "BOOKMARK"].contains(&icon) {
            return b.get("isToggled").and_then(Value::as_bool).map(|v| !v);
        }
    }
    if let Some(b) = node.get("musicLibraryAddToButtonRenderer") {
        return b.get("isAddedToLibrary").and_then(Value::as_bool);
    }
    match node {
        Value::Object(m) => m.values().find_map(library_toggle),
        Value::Array(a) => a.iter().find_map(library_toggle),
        _ => None,
    }
}
fn valid_id(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl Api {
    async fn require_account(&self) -> Result<(), String> {
        if !self.authenticated().await {
            Err("Подключи YouTube в настройках".into())
        } else {
            Ok(())
        }
    }
    pub async fn track_like(&self, id: String) -> Result<Option<bool>, String> {
        self.require_account().await?;
        if !valid_id(&id, 11) || id.len() != 11 {
            return Err("Некорректный трек".into());
        }
        let data = self
            .request(
                "next",
                json!({"videoId":id,"isAudioOnly":true,"enablePersistentPlaylistPanel":true}),
            )
            .await?;
        Ok(model::find(&data, "likeButtonRenderer")
            .and_then(like_status)
            .or_else(|| {
                model::queue(&data, &json!({"videoId":id})).and_then(|q| {
                    q.items
                        .into_iter()
                        .find(|i| i.id == id)
                        .and_then(|i| i.liked)
                })
            }))
    }
    pub async fn library(&self, category: String) -> Result<Feed, String> {
        self.require_account().await?;
        let id = match category.as_str() {
            "playlists" => "FEmusic_liked_playlists",
            "albums" => "FEmusic_liked_albums",
            "songs" => "FEmusic_liked_videos",
            _ => return Err("Неизвестный раздел медиатеки".into()),
        };
        let data = self.request("browse", json!({"browseId":id})).await?;
        let mut feed = model::feed(&data, self.authenticated().await);
        if feed.continuation.is_none() {
            feed.continuation = [
                "gridRenderer",
                "musicShelfRenderer",
                "musicPlaylistShelfRenderer",
            ]
            .iter()
            .find_map(|key| {
                model::find(&data, key)
                    .and_then(|s| s.get("continuations"))
                    .and_then(model::continuation)
            })
            .map(|token| json!({"continuation":token}));
        }
        Ok(feed)
    }
    pub async fn save_collection(&self, browse_id: String, saved: bool) -> Result<(), String> {
        self.require_account().await?;
        if !valid_id(&browse_id, 200)
            || !(browse_id.starts_with("VL") || browse_id.starts_with("MPRE"))
        {
            return Err("Эту подборку нельзя сохранить".into());
        }
        let generation = self.session_generation();
        let data = self
            .request("browse", json!({"browseId":browse_id}))
            .await?;
        if generation != self.session_generation() {
            return Err("Сессия изменилась. Повтори сохранение.".into());
        }
        let target =
            collection(&data).ok_or("YouTube не вернул команду сохранения этой подборки")?;
        if !valid_id(&target.playlist_id, 200) {
            return Err("Некорректная подборка".into());
        }
        self.request(
            if saved {
                "like/like"
            } else {
                "like/removelike"
            },
            json!({"target":{"playlistId":target.playlist_id}}),
        )
        .await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_collection_state_without_using_a_tracks_like() {
        let data = json!({"header":{"musicDetailHeaderRenderer":{"buttons":[{"toggleButtonRenderer":{"defaultIcon":{"iconType":"LIBRARY_ADD"},"isToggled":true}},{"musicPlayButtonRenderer":{"playNavigationEndpoint":{"watchPlaylistEndpoint":{"playlistId":"OLAKtest"}}}}]}},"contents":{"likeStatus":"DISLIKE"}});
        let c = collection(&data).unwrap();
        assert_eq!(c.playlist_id, "OLAKtest");
        assert_eq!(c.saved, Some(true));
        assert!(collection(&json!({"header":{"watchEndpoint":{"playlistId":"RDmix"}}})).is_none());
    }
    #[tokio::test]
    #[ignore = "Requires an explicit disposable copy of YouTube cookies"]
    async fn live_account_library() {
        let source =
            std::env::var("NEISORA_LIBRARY_COOKIES").expect("temporary cookie copy path required");
        let dir = std::env::temp_dir().join(format!("neisora-library-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        crate::api::private_write(&dir.join("cookies.txt"), &std::fs::read(source).unwrap())
            .unwrap();
        let api = Api::new(dir).unwrap();
        for category in ["playlists", "albums", "songs"] {
            let feed = api.library(category.into()).await.unwrap();
            eprintln!(
                "library {category}: sections={}, items={}",
                feed.sections.len(),
                feed.sections.iter().map(|s| s.items.len()).sum::<usize>()
            );
            if let Some(item) = feed
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .find(|i| i.endpoint.get("browseEndpoint").is_some())
            {
                let data = api
                    .request("browse", item.endpoint["browseEndpoint"].clone())
                    .await
                    .unwrap();
                eprintln!(
                    "collection {:?}: {:?}",
                    item.kind,
                    collection(&data).map(|c| c.saved)
                );
                fn inspect(v: &Value) {
                    match v {
                        Value::Object(m) => {
                            for (k, v) in m {
                                if [
                                    "toggleButtonRenderer",
                                    "musicLibraryAddToButtonRenderer",
                                    "likeButtonRenderer",
                                ]
                                .contains(&k.as_str())
                                {
                                    eprintln!("header button {k} keys={:?}, icon={}, toggled={}, added={}, like={}",v.as_object().map(|m|m.keys().collect::<Vec<_>>()),v.pointer("/defaultIcon/iconType").unwrap_or(&Value::Null),v["isToggled"],v["isAddedToLibrary"],v["likeStatus"]);
                                }
                                inspect(v);
                            }
                        }
                        Value::Array(a) => a.iter().for_each(inspect),
                        _ => (),
                    }
                }
                if let Some(header) = data
                    .get("header")
                    .or_else(|| model::find(&data, "musicResponsiveHeaderRenderer"))
                {
                    inspect(header);
                }
            }
            if category == "songs" {
                if let Some(item) = feed
                    .sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .find(|i| i.id.len() == 11 && model::watch_payload(&i.endpoint).is_some())
                {
                    let value = api.track_like(item.id.clone()).await.unwrap();
                    eprintln!("track like state: {value:?}, item={:?}", item.liked);
                    assert!(value.is_some(), "Missing account like state");
                }
            }
        }
    }
}
