//! Resolve video metadata to a matching catalog recording without changing playback.
use crate::{api::Api, lyrics, model};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Debug, Serialize)]
pub struct Recording {
    pub id: Option<String>,
    pub title: String,
    pub artist: String,
    pub image: String,
    pub duration: f64,
}
pub struct Cache {
    values: Mutex<HashMap<String, (Instant, Option<Recording>)>>,
    gate: Semaphore,
    flights: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            values: Mutex::new(HashMap::new()),
            gate: Semaphore::new(2),
            flights: Mutex::new(HashMap::new()),
        }
    }
}
fn seconds(value: &str) -> Option<f64> {
    let parts = value
        .split(':')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    Some(parts.into_iter().fold(0.0, |sum, n| sum * 60.0 + n as f64))
}
fn safe_image(raw: &str, deezer: bool) -> Option<String> {
    let url = reqwest::Url::parse(raw).ok()?;
    let host = url.host_str()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return None;
    }
    if deezer {
        return (host == "cdn-images.dzcdn.net").then(|| raw.to_owned());
    }
    if host.starts_with("yt3.")
        || (!host.ends_with(".googleusercontent.com") && !host.ends_with(".ggpht.com"))
    {
        return None;
    }
    let size = regex::Regex::new(r"=w\d+-h\d+").unwrap();
    Some(size.replace(raw, "=w800-h800").into_owned())
}
fn matches(
    title: &str,
    artist: &str,
    duration: f64,
    actual_title: &str,
    actual_artist: &str,
    actual_duration: f64,
    music_video: bool,
) -> bool {
    lyrics::normalized(&lyrics::clean_title(title, artist))
        == lyrics::normalized(&lyrics::clean_title(actual_title, actual_artist))
        && lyrics::artist_matches(artist, actual_artist)
        && actual_duration > 0.0
        && duration > 0.0
        && ((duration - actual_duration).abs() <= 12.0
            || (music_video && duration >= actual_duration && duration - actual_duration <= 30.0))
}
impl Cache {
    pub async fn get(
        &self,
        api: &Api,
        title: &str,
        artist: &str,
        duration: f64,
        music_video: bool,
    ) -> Option<Recording> {
        if !duration.is_finite() || duration <= 0.0 || title.len() > 1000 || artist.len() > 1000 {
            return None;
        }
        let key = format!(
            "{}|{}|{}|{}",
            lyrics::normalized(title),
            lyrics::normalized(artist),
            duration.round(),
            music_video
        );
        let cached = || async {
            self.values
                .lock()
                .await
                .get(&key)
                .filter(|(at, v)| {
                    at.elapsed() < Duration::from_secs(if v.is_some() { 43200 } else { 120 })
                })
                .map(|(_, v)| v.clone())
        };
        if let Some(value) = cached().await {
            return value;
        }
        let flight = {
            let mut flights = self.flights.lock().await;
            flights.retain(|_, f| Arc::strong_count(f) > 1);
            flights
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _flight = tokio::time::timeout(Duration::from_secs(6), flight.lock())
            .await
            .ok()?;
        if let Some(value) = cached().await {
            return value;
        }
        let _permit = tokio::time::timeout(Duration::from_secs(1), self.gate.acquire())
            .await
            .ok()?
            .ok()?;
        if let Some(value) = cached().await {
            return value;
        }
        let value = tokio::time::timeout(
            Duration::from_secs(5),
            self.fetch(api, title, artist, duration, music_video),
        )
        .await
        .ok()
        .flatten();
        let mut values = self.values.lock().await;
        if values.len() >= 128 {
            if let Some(oldest) = values
                .iter()
                .min_by_key(|(_, v)| v.0)
                .map(|(k, _)| k.clone())
            {
                values.remove(&oldest);
            }
        }
        values.insert(key, (Instant::now(), value.clone()));
        value
    }
    async fn fetch(
        &self,
        api: &Api,
        title: &str,
        artist: &str,
        duration: f64,
        music_video: bool,
    ) -> Option<Recording> {
        let identities = lyrics::identities(title, artist);
        let mut identified = None;
        for (name, performer) in identities.iter().take(2) {
            let query = format!("{} {}", lyrics::primary_artist(performer), name);
            let feed = api.search(query).await.ok();
            if let Some(feed) = feed {
                let found = feed
                    .sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .filter_map(|item| {
                        let actual = seconds(&item.duration)
                            .or_else(|| item.subtitle.split('•').find_map(|s| seconds(s.trim())))?;
                        if !matches(
                            name,
                            performer,
                            duration,
                            &item.title,
                            &item.subtitle,
                            actual,
                            music_video,
                        ) {
                            return None;
                        }
                        let id = model::watch_payload(&item.endpoint)?
                            .get("videoId")?
                            .as_str()?
                            .to_owned();
                        let image = safe_image(&item.image, false).unwrap_or_default();
                        Some((
                            item.is_music_video,
                            (actual - duration).abs(),
                            Recording {
                                id: Some(id),
                                title: lyrics::clean_title(&item.title, &item.subtitle),
                                artist: item
                                    .subtitle
                                    .split('•')
                                    .find(|part| lyrics::artist_matches(performer, part))
                                    .unwrap_or(performer)
                                    .trim()
                                    .into(),
                                image,
                                duration: actual,
                            },
                        ))
                    })
                    .min_by(|a, b| {
                        a.2.image
                            .is_empty()
                            .cmp(&b.2.image.is_empty())
                            .then_with(|| a.0.cmp(&b.0))
                            .then_with(|| a.1.total_cmp(&b.1))
                    });
                if let Some((_, _, recording)) = found {
                    if !recording.image.is_empty() {
                        return Some(recording);
                    }
                    identified = Some(recording);
                }
            }
        }
        let (name, performer) = identities.last()?;
        let external = self
            .external(api, name, performer, duration, music_video)
            .await;
        match (identified, external) {
            (Some(mut recording), Some(artwork)) => {
                recording.image = artwork.image;
                if music_video {
                    recording.duration = artwork.duration;
                }
                Some(recording)
            }
            (Some(recording), None) => Some(recording),
            (None, artwork) => artwork,
        }
    }
    // Only public metadata, never Google cookies.
    async fn external(
        &self,
        api: &Api,
        name: &str,
        performer: &str,
        duration: f64,
        music_video: bool,
    ) -> Option<Recording> {
        let query = format!("{} {}", lyrics::primary_artist(performer), name);
        let response = api
            .client
            .get("https://api.deezer.com/search")
            .query(&[("q", query.as_str()), ("limit", "10")])
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()?;
        let data: serde_json::Value = crate::network::json(response, crate::network::LYRICS_LIMIT)
            .await
            .ok()?;
        data["data"].as_array()?.iter().find_map(|item| {
            let actual_title = item["title"].as_str()?;
            let actual_artist = item["artist"]["name"].as_str()?;
            let actual = item["duration"].as_f64()?;
            if !matches(
                name,
                performer,
                duration,
                actual_title,
                actual_artist,
                actual,
                music_video,
            ) {
                return None;
            }
            Some(Recording {
                id: None,
                title: actual_title.into(),
                artist: actual_artist.into(),
                image: safe_image(item["album"]["cover_xl"].as_str()?, true)?,
                duration: actual,
            })
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_matches_credits_but_preserves_versions_and_duration() {
        assert!(matches(
            "VILLIAN, madk1d — ДИНАСТИЯ (official music video)",
            "madk1d",
            165.0,
            "ДИНАСТИЯ",
            "VILLIAN и madk1d",
            150.0,
            true
        ));
        assert!(!matches(
            "Song (Live)",
            "Artist",
            200.0,
            "Song",
            "Artist",
            200.0,
            false
        ));
        assert!(!matches(
            "Song", "Artist", 220.0, "Song", "Artist", 200.0, false
        ));
        assert!(!matches(
            "Song", "Artist", 200.0, "Song", "Other", 200.0, false
        ));
    }
    #[test]
    fn clip_tail_tolerance_requires_music_video_and_does_not_accept_shortened_audio() {
        assert!(matches(
            "ДИНАСТИЯ",
            "madk1d и VILLIAN",
            165.0,
            "ДИНАСТИЯ",
            "VILLIAN, madk1d",
            150.0,
            true
        ));
        assert!(!matches(
            "ДИНАСТИЯ",
            "madk1d и VILLIAN",
            165.0,
            "ДИНАСТИЯ",
            "VILLIAN, madk1d",
            150.0,
            false
        ));
        assert!(!matches(
            "Song", "Artist", 150.0, "Song", "Artist", 180.0, true
        ));
    }
    #[tokio::test]
    #[ignore = "live public provider probe; requires network"]
    async fn live_lyrics_examples() {
        let dir = std::env::temp_dir().join(format!("neisora-lyrics-probe-{}", std::process::id()));
        let api = Api::new(dir.clone()).unwrap();
        let cache = Cache::default();
        let library = lyrics::LyricsCache::default();
        for (title, artist, duration) in [
            ("Sellit!", "Niko B", 93.0),
            (
                "VILLIAN, madk1d - ДИНАСТИЯ (official music video)",
                "VILLIAN",
                165.0,
            ),
        ] {
            if let Ok(feed) = api
                .search(format!(
                    "{} {}",
                    lyrics::primary_artist(artist),
                    lyrics::clean_title(title, artist)
                ))
                .await
            {
                for i in feed.sections.iter().flat_map(|s| &s.items).take(6) {
                    println!(
                        "catalog candidate: {} | {} | duration={} musicVideo={}",
                        i.title, i.subtitle, i.duration, i.is_music_video
                    );
                }
            }
            let recording = cache
                .get(&api, title, artist, duration, artist != "Niko B")
                .await;
            println!("catalog {title}: {recording:?}");
            assert!(
                recording.as_ref().is_some_and(|r| !r.image.is_empty()),
                "No matching artwork"
            );
            let id = recording
                .as_ref()
                .and_then(|r| r.id.clone())
                .unwrap_or_else(|| {
                    if artist == "Niko B" {
                        "2P7Cm1GDd6I"
                    } else {
                        "J7NFL-eOxiQ"
                    }
                    .into()
                });
            let value = library
                .get_with_recording(&api, id, title.into(), artist.into(), duration, recording)
                .await;
            println!(
                "lyrics {title}: {:?}",
                value
                    .as_ref()
                    .map(|v| (&v.source, v.synced, v.lines.len(), v.plain.len()))
            );
            let value = value.expect("Example lyrics not found");
            if artist != "Niko B" {
                assert!(value.synced, "Expected timed lyrics for the official clip");
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn artwork_sources_are_restricted_and_resolution_is_upgraded() {
        assert_eq!(
            safe_image(
                "https://lh3.googleusercontent.com/cover=w120-h120-l90",
                false
            )
            .unwrap(),
            "https://lh3.googleusercontent.com/cover=w800-h800-l90"
        );
        for url in [
            "file:///etc/passwd",
            "https://example.org/cover",
            "https://cdn-images.dzcdn.net.evil.test/x",
            "https://user:pass@cdn-images.dzcdn.net/x",
        ] {
            assert!(safe_image(url, true).is_none());
        }
    }
}
