use crate::{api::private_write, model::Item};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
const LIMIT: usize = 500;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub item: Item,
    pub played_at: u64,
}
struct Data {
    entries: Vec<Entry>,
    last_playback: String,
}
pub struct History {
    path: PathBuf,
    data: Mutex<Data>,
}
pub(crate) fn clean(mut item: Item) -> Result<Item, String> {
    let local = crate::local_music::valid_id(&item.id) && item.source.as_deref() == Some("local");
    if (!local
        && (item.id.len() != 11
            || !item
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')))
        || item.title.is_empty()
        || item.title.len() > 2000
        || item.subtitle.len() > 2000
        || item.image.len() > 4000
        || item.duration.len() > 32
    {
        return Err("Некорректная запись истории".into());
    }
    if !reqwest::Url::parse(&item.image).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port_or_known_default() == Some(443)
            && u.host_str().is_some_and(|h| {
                h == "cdn-images.dzcdn.net"
                    || h == "i.ytimg.com"
                    || h.ends_with(".googleusercontent.com")
                    || h.ends_with(".ggpht.com")
            })
    }) {
        item.image.clear();
    }
    // Never persist radio/session context or arbitrary commands from the original queue.
    item.endpoint = if local {
        json!({"localId":item.id})
    } else {
        json!({"watchEndpoint":{"videoId":item.id}})
    };
    item.play = None;
    item.selected = false;
    item.kind = "song".into();
    Ok(item)
}
impl History {
    pub fn new(directory: PathBuf) -> Self {
        let path = directory.join("history.json");
        let entries = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= 4 * 1024 * 1024)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|raw| serde_json::from_slice::<Vec<Entry>>(&raw).ok())
            .unwrap_or_default()
            .into_iter()
            .take(LIMIT)
            .filter_map(|e| {
                clean(e.item).ok().map(|item| Entry {
                    item,
                    played_at: e.played_at,
                })
            })
            .collect();
        Self {
            path,
            data: Mutex::new(Data {
                entries,
                last_playback: String::new(),
            }),
        }
    }
    pub async fn list(&self) -> Vec<Entry> {
        self.data.lock().await.entries.clone()
    }
    pub async fn record(&self, item: Item, playback: String) -> Result<(), String> {
        if playback.is_empty() || playback.len() > 100 {
            return Err("Некорректный сеанс воспроизведения".into());
        }
        let item = clean(item)?;
        let mut data = self.data.lock().await;
        if data.last_playback == playback {
            return Ok(());
        }
        let mut entries = data.entries.clone();
        entries.insert(
            0,
            Entry {
                item,
                played_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
        );
        entries.truncate(LIMIT);
        private_write(
            &self.path,
            &serde_json::to_vec(&entries).map_err(|e| e.to_string())?,
        )?;
        data.entries = entries;
        data.last_playback = playback;
        Ok(())
    }
    pub async fn clear(&self) -> Result<(), String> {
        let mut data = self.data.lock().await;
        private_write(&self.path, b"[]")?;
        data.entries.clear();
        // Keep the current playback token: clearing must not re-add the same song.
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn persists_bounded_history_and_strips_radio_context() {
        let dir = std::env::temp_dir().join(format!("neisora-history-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let h = History::new(dir.clone());
        let item:Item=serde_json::from_value(json!({"id":"abc12345678","title":"Track","subtitle":"Artist","image":"file:///etc/passwd","kind":"song","duration":"3:00","endpoint":{"watchEndpoint":{"videoId":"abc12345678","queueContextParams":"private-session"}},"play":null,"selected":true,"isMusicVideo":false})).unwrap();
        h.record(item.clone(), "one".into()).await.unwrap();
        h.record(item.clone(), "one".into()).await.unwrap();
        assert_eq!(h.list().await.len(), 1);
        assert!(h.list().await[0].item.image.is_empty());
        assert!(!std::fs::read_to_string(dir.join("history.json"))
            .unwrap()
            .contains("private-session"));
        for i in 0..505 {
            h.record(item.clone(), format!("play-{i}")).await.unwrap();
        }
        assert_eq!(h.list().await.len(), LIMIT);
        assert_eq!(History::new(dir.clone()).list().await.len(), LIMIT);
        h.clear().await.unwrap();
        h.record(item, "play-504".into()).await.unwrap();
        assert!(h.list().await.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
