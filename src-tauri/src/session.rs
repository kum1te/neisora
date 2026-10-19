//! Bounded private playback checkpoint, independent of WebKit's delayed storage flush.
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::Mutex;
const MAX_BYTES: usize = 3_000_000;
pub struct Session {
    path: PathBuf,
    gate: Mutex<()>,
}
fn clean(raw: &str) -> Result<String, String> {
    if raw.len() > MAX_BYTES {
        return Err("Слишком большая сохранённая очередь".into());
    }
    let data: Value = serde_json::from_str(raw).map_err(|_| "Некорректная сохранённая очередь")?;
    let queue = &data["queue"];
    let items = queue["items"]
        .as_array()
        .filter(|i| !i.is_empty() && i.len() <= 2000)
        .ok_or("Некорректная очередь")?;
    let selected = queue["selected"]
        .as_u64()
        .filter(|n| (*n as usize) < items.len())
        .ok_or("Некорректная позиция очереди")?;
    if data["version"] != 1 {
        return Err("Неизвестная версия очереди".into());
    }
    let bounded = |key: &str| {
        data[key]
            .as_f64()
            .filter(|v| v.is_finite() && (0.0..=604800.0).contains(v))
            .ok_or_else(|| "Некорректная позиция трека".to_owned())
    };
    let dynamic = queue["dynamic"].as_bool().unwrap_or(false);
    let radio_id = data["radioId"].as_str().filter(|id| {
        dynamic
            && id.starts_with("RD")
            && id.len() <= 200
            && id.len() > 2
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    });
    let mut cleaned = Vec::with_capacity(items.len());
    for value in items {
        let item: crate::model::Item =
            serde_json::from_value(value.clone()).map_err(|_| "Некорректный трек очереди")?;
        let mut item = crate::history::clean(item)?;
        item.set_video_id = None;
        item.liked = None;
        item.source = if item.source.as_deref() == Some("local") {
            Some("local".into())
        } else {
            None
        };
        if let Ok(mut url) = reqwest::Url::parse(&item.image) {
            url.set_query(None);
            url.set_fragment(None);
            item.image = url.to_string();
        }
        let mut item = serde_json::to_value(item).map_err(|e| e.to_string())?;
        item["manual"] = json!(value["manual"].as_bool().unwrap_or(false));
        if let Some(order) = value["shuffleOrder"].as_u64().filter(|n| *n <= 100_000) {
            item["shuffleOrder"] = json!(order);
        }
        if let Some(key) = value["queueKey"].as_str().filter(|s| {
            s.len() <= 80
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            item["queueKey"] = json!(key);
        }
        cleaned.push(item);
    }
    let title: String = queue["title"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();
    let partial = data["partial"].as_bool().unwrap_or(false);
    let result=json!({"version":1,"queue":{"title":title,"items":cleaned,"selected":selected,"dynamic":dynamic,"shuffled":!dynamic&&queue["shuffled"].as_bool().unwrap_or(false),"context":{"restored":true,"restoredPartial":partial,"playlistId":radio_id},"continuation":null},"position":bounded("position")?,"duration":bounded("duration")?,"radioId":radio_id,"partial":partial}).to_string();
    if result.len() > MAX_BYTES {
        return Err("Слишком большая сохранённая очередь".into());
    }
    Ok(result)
}
impl Session {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            path: directory.join("playback-session.json"),
            gate: Mutex::new(()),
        }
    }
    pub async fn read(&self) -> Option<String> {
        let _guard = self.gate.lock().await;
        let size = std::fs::metadata(&self.path).ok()?.len();
        if size > MAX_BYTES as u64 {
            return None;
        }
        clean(&std::fs::read_to_string(&self.path).ok()?).ok()
    }
    pub async fn save(&self, raw: Option<String>) -> Result<(), String> {
        let raw = raw.map(|s| clean(&s)).transpose()?;
        let _guard = self.gate.lock().await;
        match raw {
            Some(raw) => crate::api::private_write(&self.path, raw.as_bytes()),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e.to_string()),
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        json!({"version":1,"queue":{"title":"Test","items":[{"id":"abcdefghijk","title":"Track","subtitle":"Artist","image":"https://i.ytimg.com/x?secret=token","kind":"song","duration":"3:20","selected":false,"endpoint":{"watchEndpoint":{"queueContextParams":"secret"}},"play":{"url":"private"},"manual":true,"shuffleOrder":2}],"selected":0,"dynamic":true},"position":12.5,"duration":200,"radioId":"RDtest"})
    }
    #[test]
    fn strips_commands_and_tokens_and_rejects_invalid_checkpoints() {
        let raw = clean(&fixture().to_string()).unwrap();
        assert!(!raw.contains("secret"));
        assert!(!raw.contains("private"));
        let data: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(data["queue"]["items"][0]["manual"], true);
        assert_eq!(data["radioId"], "RDtest");
        let mut bad = fixture();
        bad["position"] = json!(-1);
        assert!(clean(&bad.to_string()).is_err());
        let mut bad = fixture();
        bad["queue"]["items"][0]["id"] = json!("../../etc/passwd");
        assert!(clean(&bad.to_string()).is_err());
        let mut bad = fixture();
        bad["queue"]["selected"] = json!(99);
        assert!(clean(&bad.to_string()).is_err());
        assert!(clean(&"x".repeat(MAX_BYTES + 1)).is_err());
        assert!(clean("null").is_err());
    }
    #[tokio::test]
    async fn persists_private_checkpoint_across_store_instances_and_clears() {
        let dir = std::env::temp_dir().join(format!(
            "neisora-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let store = Session::new(dir.clone());
        store.save(Some(fixture().to_string())).await.unwrap();
        assert!(Session::new(dir.clone()).read().await.is_some());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.join("playback-session.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        store.save(None).await.unwrap();
        assert!(store.read().await.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
