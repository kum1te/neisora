use crate::{
    api::Api,
    lyrics::{self, Line, Lyrics, Word},
};
use base64::Engine;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

pub struct Cache {
    values: Mutex<HashMap<String, (Instant, Option<Lyrics>)>>,
    gate: Semaphore,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
            gate: Semaphore::new(2),
        }
    }
}

fn aliases(title: &str, artist: &str) -> Vec<String> {
    let clean = lyrics::clean_title(title, artist);
    let mut variants = vec![clean.clone()];
    // Japanese video titles often contain the same name in two scripts.
    if clean
        .chars()
        .any(|c| ('\u{3040}'..='\u{9fff}').contains(&c))
        && clean.chars().any(|c| c.is_ascii_alphabetic())
    {
        let split = regex::Regex::new(r"\s*[-–—|]\s*").unwrap();
        for name in split.split(&clean) {
            if !name.trim().is_empty() {
                variants.push(name.trim().to_owned());
            }
        }
        variants.sort_by_key(|v| {
            (
                !v.chars().any(|c| ('\u{3040}'..='\u{9fff}').contains(&c)),
                v.len(),
            )
        });
    }
    variants.dedup();
    variants
}

fn parse_krc(text: &str) -> Vec<Line> {
    let line_tag = regex::Regex::new(r"^\[(\d+),(\d+)\]").unwrap();
    let word_tag = regex::Regex::new(r"<(\d+),(\d+),(\d+)>").unwrap();
    let offset = text
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("[offset:")?
                .strip_suffix(']')?
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(0.0)
        / 1000.0;
    let mut lines = Vec::new();
    for raw in text.lines() {
        let Some(line) = line_tag.captures(raw) else {
            continue;
        };
        let time = line[1].parse::<f64>().unwrap_or(0.0) / 1000.0 + offset;
        let tags: Vec<_> = word_tag.captures_iter(raw).collect();
        let mut words = Vec::new();
        for (i, tag) in tags.iter().enumerate() {
            let start = time + tag[1].parse::<f64>().unwrap_or(0.0) / 1000.0;
            let length = tag[2].parse::<f64>().unwrap_or(0.0) / 1000.0;
            let from = tag.get(0).unwrap().end();
            let to = tags
                .get(i + 1)
                .map(|t| t.get(0).unwrap().start())
                .unwrap_or(raw.len());
            if length <= 0.0 || raw[from..to].is_empty() {
                continue;
            }
            words.push(Word {
                time: start.max(0.0),
                end: (start + length).max(0.0),
                text: raw[from..to].to_owned(),
            });
        }
        if !words.is_empty() {
            lines.push(Line {
                time: time.max(0.0),
                text: words.iter().map(|w| w.text.as_str()).collect(),
                words,
            });
        }
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}
fn decode(raw: &str) -> Option<Vec<Line>> {
    if raw.len() > 1_000_000 {
        return None;
    }
    let data = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    if !data.starts_with(b"krc1") {
        return None;
    }
    // KRC's public file format uses a repeating 16-byte XOR before zlib.
    const KEY: [u8; 16] = [
        0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e,
        0x69,
    ];
    let bytes: Vec<_> = data[4..]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % 16])
        .collect();
    let mut text = String::new();
    flate2::read::ZlibDecoder::new(bytes.as_slice())
        .take(1_000_000)
        .read_to_string(&mut text)
        .ok()?;
    let lines = parse_krc(&text);
    (!lines.is_empty()).then_some(lines)
}
impl Cache {
    pub async fn invalidate(&self, id: &str) {
        self.values.lock().await.remove(id);
    }
    pub async fn get(
        &self,
        api: &Api,
        id: String,
        title: String,
        artist: String,
        duration: f64,
    ) -> Option<Lyrics> {
        if let Some((_, value)) = self.values.lock().await.get(&id).filter(|(at, v)| {
            at.elapsed() < Duration::from_secs(if v.is_some() { 43200 } else { 180 })
        }) {
            return value.clone();
        }
        let _permit = tokio::time::timeout(Duration::from_secs(2), self.gate.acquire())
            .await
            .ok()?
            .ok()?;
        if let Some((_, value)) = self.values.lock().await.get(&id).filter(|(at, v)| {
            at.elapsed() < Duration::from_secs(if v.is_some() { 43200 } else { 180 })
        }) {
            return value.clone();
        }
        let value = tokio::time::timeout(
            Duration::from_secs(13),
            self.fetch(api, &title, &artist, duration),
        )
        .await
        .ok()
        .flatten();
        let mut cache = self.values.lock().await;
        if cache.len() >= 100 {
            if let Some(key) = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(id, _)| id.clone())
            {
                cache.remove(&key);
            }
        }
        cache.insert(id, (Instant::now(), value.clone()));
        value
    }
    async fn fetch(&self, api: &Api, title: &str, artist: &str, duration: f64) -> Option<Lyrics> {
        let names = aliases(title, artist);
        let primary = lyrics::primary_artist(artist);
        let normalized: Vec<_> = names.iter().map(|s| lyrics::normalized(s)).collect();
        for name in names.iter().take(2) {
            let query = format!("{primary} - {name}");
            // Metadata only: this client has no default Google Cookie/Authorization headers.
            let response = api
                .client
                .get("https://lyrics.kugou.com/search")
                .query(&[
                    ("ver", "1"),
                    ("man", "yes"),
                    ("client", "pc"),
                    ("keyword", query.as_str()),
                    ("duration", "0"),
                ])
                .timeout(Duration::from_secs(6))
                .send()
                .await
                .ok()?;
            let result: Value = crate::network::json(response, crate::network::LYRICS_LIMIT)
                .await
                .ok()?;
            let mut candidates: Vec<_> = result["candidates"]
                .as_array()?
                .iter()
                .filter(|c| {
                    normalized.contains(&lyrics::normalized(c["song"].as_str().unwrap_or_default()))
                        && lyrics::artist_matches(
                            artist.split('•').next().unwrap_or(artist),
                            c["singer"].as_str().unwrap_or_default(),
                        )
                        && (c["duration"].as_f64().unwrap_or(0.0) / 1000.0 - duration).abs() <= 6.0
                })
                .collect();
            candidates.sort_by(|a, b| {
                ((a["duration"].as_f64().unwrap_or(0.0) / 1000.0 - duration).abs())
                    .total_cmp(&((b["duration"].as_f64().unwrap_or(0.0) / 1000.0 - duration).abs()))
            });
            for candidate in candidates.into_iter().take(2) {
                let response = api
                    .client
                    .get("https://lyrics.kugou.com/download")
                    .query(&[
                        ("ver", "1"),
                        ("client", "pc"),
                        ("id", candidate["id"].as_str()?),
                        ("accesskey", candidate["accesskey"].as_str()?),
                        ("fmt", "krc"),
                        ("charset", "utf8"),
                    ])
                    .timeout(Duration::from_secs(6))
                    .send()
                    .await
                    .ok()?;
                let data: Value = crate::network::json(response, crate::network::LYRICS_LIMIT)
                    .await
                    .ok()?;
                if let Some(lines) = data["content"].as_str().and_then(decode) {
                    return Some(Lyrics {
                        lines,
                        plain: String::new(),
                        source: "KuGou".into(),
                        synced: true,
                    });
                }
            }
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_and_oversized_krc() {
        assert!(decode("not base64!").is_none());
        assert!(decode(&"A".repeat(1_000_001)).is_none());
        assert!(
            decode(&base64::engine::general_purpose::STANDARD.encode(b"krc1garbage")).is_none()
        );
    }
    #[test]
    fn krc_uses_relative_word_offsets_and_keeps_the_spaces() {
        let lines = parse_krc("[offset:-100]\n[1000,2000]<0,600,0>Hello <800,900,0>world");
        assert_eq!(lines[0].text, "Hello world");
        assert!((lines[0].time - 0.9).abs() < 1e-8);
        assert!((lines[0].words[1].time - 1.7).abs() < 1e-8);
        assert!((lines[0].words[1].end - 2.6).abs() < 1e-8);
    }
    #[test]
    fn video_aliases_do_not_strip_slowed_or_live_versions() {
        assert!(
            aliases("BABYMETAL - メギツネ - MEGITSUNE (OFFICIAL)", "BABYMETAL")
                .contains(&"メギツネ".into())
        );
        assert_eq!(aliases("Song - slowed", "Artist"), vec!["Song - slowed"]);
        assert!(decode("not a KRC file").is_none());
    }
}
