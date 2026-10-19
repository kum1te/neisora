//! Explicit lyric choices are private, bounded preferences; evict text, never the choice.
use crate::lyrics::{Candidate, Lyrics};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::Mutex;
const LIMIT: usize = 4 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
pub struct Selection {
    pub record: Candidate,
    pub lyrics: Option<Lyrics>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    track_id: String,
    selection: Selection,
}
pub struct Store {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn valid_selection(s: &Selection) -> bool {
    s.record.valid() && s.lyrics.as_ref().is_none_or(valid_lyrics)
}
pub fn valid_lyrics(lyrics: &Lyrics) -> bool {
    lyrics.source.len() <= 100
        && lyrics.plain.len() <= 256_000
        && lyrics.lines.len() <= 5000
        && lyrics.synced != lyrics.lines.is_empty()
        && lyrics.lines.iter().all(|l| {
            l.time.is_finite()
                && (0.0..=604800.0).contains(&l.time)
                && l.text.len() <= 4000
                && l.words.len() <= 400
                && l.words.iter().all(|w| {
                    w.time.is_finite()
                        && w.end.is_finite()
                        && w.time >= 0.0
                        && w.end >= w.time
                        && w.end <= 604800.0
                        && w.text.len() <= 2000
                })
        })
        && lyrics.lines.windows(2).all(|w| w[0].time <= w[1].time)
        && serde_json::to_vec(lyrics).is_ok_and(|raw| raw.len() <= 384_000)
}
impl Store {
    pub fn new(directory: PathBuf) -> Self {
        let path = directory.join("lyric-choices.json");
        let entries = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= LIMIT as u64)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|b| serde_json::from_slice::<Vec<Entry>>(&b).ok())
            .filter(|entries| {
                entries.len() <= 2000
                    && entries
                        .iter()
                        .all(|e| valid_id(&e.track_id) && valid_selection(&e.selection))
            })
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }
    pub async fn get(&self, id: &str) -> Result<Option<Selection>, String> {
        if !valid_id(id) {
            return Err("Некорректный ID трека".into());
        }
        Ok(self
            .entries
            .lock()
            .await
            .iter()
            .rev()
            .find(|e| e.track_id == id)
            .map(|e| e.selection.clone()))
    }
    async fn write(&self, next: Vec<Entry>, entries: &mut Vec<Entry>) -> Result<(), String> {
        let mut next = next;
        if next.len() > 2000 {
            return Err("Сохранено слишком много версий текста".into());
        }
        let mut raw =
            serde_json::to_vec(&next).map_err(|_| "Не удалось сохранить версию текста")?;
        for index in 0..next.len().saturating_sub(1) {
            if raw.len() <= LIMIT {
                break;
            }
            next[index].selection.lyrics = None;
            raw = serde_json::to_vec(&next).map_err(|_| "Не удалось сохранить версию текста")?;
        }
        if raw.len() > LIMIT {
            return Err("Сохранённая версия текста слишком большая".into());
        }
        crate::api::private_write(&self.path, &raw)
            .map_err(|_| "Не удалось сохранить версию текста")?;
        *entries = next;
        Ok(())
    }
    pub async fn set(&self, id: String, selection: Option<Selection>) -> Result<(), String> {
        if !valid_id(&id) || selection.as_ref().is_some_and(|s| !valid_selection(s)) {
            return Err("Некорректная версия текста".into());
        }
        let mut entries = self.entries.lock().await;
        let mut next: Vec<Entry> = entries
            .iter()
            .filter(|e| e.track_id != id)
            .cloned()
            .collect();
        if let Some(selection) = selection {
            next.push(Entry {
                track_id: id,
                selection,
            });
        }
        self.write(next, &mut entries).await
    }
    pub async fn cache_if_current(&self, id: &str, selection: Selection) -> Result<(), String> {
        if !valid_selection(&selection) {
            return Err("Некорректная версия текста".into());
        }
        let mut entries = self.entries.lock().await;
        if !entries
            .iter()
            .any(|e| e.track_id == id && e.selection.record.id == selection.record.id)
        {
            return Ok(());
        }
        let mut next: Vec<Entry> = entries
            .iter()
            .filter(|e| e.track_id != id)
            .cloned()
            .collect();
        next.push(Entry {
            track_id: id.into(),
            selection,
        });
        self.write(next, &mut entries).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn choices_survive_restart_and_late_fetch_cannot_restore_reset_selection() {
        let directory =
            std::env::temp_dir().join(format!("neisora-lyric-choice-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let selection = Selection {
            record: Candidate {
                id: 1,
                title: "Song".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                duration: 30.,
                synced: true,
                word_timed: false,
            },
            lyrics: Some(Lyrics {
                lines: crate::lyrics::parse_lrc("[00:00.00]Selected line"),
                plain: "".into(),
                synced: true,
                source: "LRCLIB".into(),
            }),
        };
        let store = Store::new(directory.clone());
        store
            .set("abcdefghijk".into(), Some(selection.clone()))
            .await
            .unwrap();
        let reopened = Store::new(directory.clone());
        assert_eq!(
            reopened
                .get("abcdefghijk")
                .await
                .unwrap()
                .unwrap()
                .record
                .id,
            1
        );
        reopened.set("abcdefghijk".into(), None).await.unwrap();
        reopened
            .cache_if_current("abcdefghijk", selection)
            .await
            .unwrap();
        assert!(reopened.get("abcdefghijk").await.unwrap().is_none());
        assert!(reopened.get("../../etc/passwd").await.is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
