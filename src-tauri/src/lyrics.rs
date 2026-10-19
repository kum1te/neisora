use crate::{api::Api, model};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Word {
    pub time: f64,
    pub end: f64,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub time: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<Word>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<Line>,
    pub plain: String,
    pub source: String,
    pub synced: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
    pub synced: bool,
    pub word_timed: bool,
}
impl Candidate {
    pub fn valid(&self) -> bool {
        self.id > 0
            && self.id <= 9_007_199_254_740_991
            && self.duration.is_finite()
            && (0.0..=604800.0).contains(&self.duration)
            && [&self.title, &self.artist, &self.album]
                .iter()
                .all(|s| s.len() <= 600)
            && !self.title.trim().is_empty()
    }
}
fn candidate(record: &Value) -> Option<Candidate> {
    let item = Candidate {
        id: record["id"].as_u64()?,
        title: record["trackName"].as_str()?.into(),
        artist: record["artistName"].as_str()?.into(),
        album: record["albumName"].as_str().unwrap_or_default().into(),
        duration: record["duration"].as_f64()?,
        synced: record["syncedLyrics"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
            || record["hasWordSync"] == true,
        word_timed: record["hasWordSync"] == true,
    };
    item.valid().then_some(item)
}
pub struct LyricsCache {
    versions: Mutex<HashMap<u64, (Instant, crate::lyric_choices::Selection)>>,
    cache: Mutex<HashMap<String, (Instant, Lyrics)>>,
    gate: Semaphore,
    remote_after: Mutex<Instant>,
}
impl Default for LyricsCache {
    fn default() -> Self {
        Self {
            versions: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            gate: Semaphore::new(2),
            remote_after: Mutex::new(Instant::now()),
        }
    }
}

pub fn parse_lrc(lrc: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut offset = 0.0;
    for raw in lrc.lines() {
        if let Some(v) = raw
            .trim()
            .strip_prefix("[offset:")
            .and_then(|s| s.strip_suffix(']'))
            .and_then(|s| s.parse::<f64>().ok())
        {
            offset = v / 1000.0;
        }
    }
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(start) = rest.strip_prefix('[') {
            let Some((stamp, tail)) = start.split_once(']') else {
                break;
            };
            let Some((m, s)) = stamp.split_once(':') else {
                break;
            };
            let (Ok(m), Ok(s)) = (m.parse::<f64>(), s.parse::<f64>()) else {
                break;
            };
            stamps.push((m * 60.0 + s + offset).max(0.0));
            rest = tail;
        }
        for time in stamps {
            lines.push(Line {
                time,
                text: rest.trim().to_owned(),
                words: vec![],
            });
        }
    }
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    lines
}
fn lyricsfile_lines(raw: &str) -> Option<Vec<Line>> {
    if raw.len() > 512_000 {
        return None;
    }
    let data: Value = serde_saphyr::from_str(raw).ok()?;
    let mut lines: Vec<Line> = data["lines"]
        .as_array()?
        .iter()
        .filter_map(|line| {
            let time = line["start_ms"].as_f64()? / 1000.0;
            if !time.is_finite() || time < 0.0 {
                return None;
            }
            let words: Vec<Word> = line["words"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|word| {
                    let start = word["start_ms"].as_f64()? / 1000.0;
                    let end = word["end_ms"].as_f64()? / 1000.0;
                    if !start.is_finite() || !end.is_finite() || start < time || end <= start {
                        return None;
                    }
                    Some(Word {
                        time: start,
                        end,
                        text: word["text"].as_str()?.to_owned(),
                    })
                })
                .collect();
            let text = line["text"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| words.iter().map(|w| w.text.as_str()).collect());
            // Reject invalid or partial word segmentation rather than dropping part of a line.
            let joined: String = words.iter().map(|w| w.text.as_str()).collect();
            let words = if joined.split_whitespace().collect::<Vec<_>>()
                == text.split_whitespace().collect::<Vec<_>>()
            {
                words
            } else {
                vec![]
            };
            Some(Line { time, text, words })
        })
        .collect();
    lines.sort_by(|a, b| a.time.total_cmp(&b.time));
    (!lines.is_empty()).then_some(lines)
}
fn word_synced(lyrics: &Lyrics) -> bool {
    lyrics.lines.iter().any(|l| !l.words.is_empty())
}
fn youtube_lyrics(data: &Value) -> Option<Lyrics> {
    if let Some(lines) = model::find(data, "timedLyricsData").and_then(Value::as_array) {
        let lines: Vec<_> = lines
            .iter()
            .filter_map(|v| {
                let time = v
                    .pointer("/cueRange/startTimeMilliseconds")
                    .and_then(|t| t.as_f64().or_else(|| t.as_str()?.parse().ok()))?
                    / 1000.0;
                Some(Line {
                    time,
                    text: v["lyricLine"].as_str()?.trim().to_owned(),
                    words: vec![],
                })
            })
            .collect();
        if !lines.is_empty() {
            return Some(Lyrics {
                plain: String::new(),
                lines,
                source: "YouTube Music".into(),
                synced: true,
            });
        }
    }
    let plain = model::find(data, "musicDescriptionShelfRenderer")
        .map(|s| model::text(&s["description"]))
        .filter(|s| !s.is_empty())
        .or_else(|| {
            model::find(data, "timedLyricsData")
                .and_then(Value::as_array)
                .map(|lines| {
                    lines
                        .iter()
                        .filter_map(|v| v["lyricLine"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        })
        .unwrap_or_default();
    if plain.is_empty() {
        None
    } else {
        Some(Lyrics {
            plain,
            lines: vec![],
            source: "YouTube Music".into(),
            synced: false,
        })
    }
}
fn lyrics_endpoint(node: &Value) -> Option<Value> {
    if let Some(e) = node.get("browseEndpoint") {
        if e.pointer(
            "/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType",
        )
        .and_then(Value::as_str)
            == Some("MUSIC_PAGE_TYPE_TRACK_LYRICS")
            || e["browseId"]
                .as_str()
                .is_some_and(|id| id.starts_with("MPLYt"))
        {
            return Some(e.clone());
        }
    }
    match node {
        Value::Object(m) => m.values().find_map(lyrics_endpoint),
        Value::Array(a) => a.iter().find_map(lyrics_endpoint),
        _ => None,
    }
}
pub(crate) fn normalized(value: &str) -> String {
    value
        .to_lowercase()
        .replace('ё', "е")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub(crate) fn artists(artist: &str) -> Vec<String> {
    let split =
        regex::Regex::new(r"(?i)\s+(?:&|и|і|and|x|feat\.?|ft\.?|with)\s+|[,;•]+\s*").unwrap();
    split
        .split(artist)
        .map(|s| {
            s.trim()
                .trim_end_matches(" - Topic")
                .trim_end_matches("VEVO")
                .trim()
                .to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect()
}
pub(crate) fn primary_artist(artist: &str) -> String {
    artists(artist.split('•').next().unwrap_or(artist))
        .into_iter()
        .next()
        .unwrap_or_default()
}
pub(crate) fn clean_title(title: &str, artist: &str) -> String {
    let labels = regex::Regex::new(r"(?i)[\[(](?:official[^\])]*|lyrics?|lyric video|music video|audio|video|клип|официальный[^\])]*|текст песни|(?:feat\.?|ft\.?)\s+[^\])]+)[\])]").unwrap();
    let cleaned = labels.replace_all(title, "");
    let suffix = regex::Regex::new(
        r"(?i)\s*[-–—|]\s*(?:official(?: music)? (?:video|audio)|lyrics?|клип)\s*$",
    )
    .unwrap();
    let cleaned = suffix.replace(&cleaned, "");
    for delimiter in [" — ", " – ", " - ", " –", " -"] {
        if let Some((prefix, song)) = cleaned.split_once(delimiter) {
            if artist_matches(artist, prefix) {
                return song.trim().to_owned();
            }
        }
    }
    cleaned.trim().to_owned()
}
pub(crate) fn artist_matches(expected: &str, actual: &str) -> bool {
    let expected = artists(expected);
    let actual = artists(actual);
    expected.iter().any(|a| {
        actual
            .iter()
            .any(|b| !normalized(a).is_empty() && normalized(a) == normalized(b))
    })
}
pub(crate) fn identities(title: &str, artist: &str) -> Vec<(String, String)> {
    let artist = artist.split('•').next().unwrap_or(artist).trim();
    let mut pairs = vec![(clean_title(title, artist), artist.to_owned())];
    // A video uploader may not be the performer. Treat its title prefix as an
    // alternate identity only; providers still have to confirm title AND artist.
    for delimiter in [" — ", " – ", " - "] {
        if let Some((prefix, song)) = title.split_once(delimiter) {
            let version = normalized(song);
            if ![
                "slowed",
                "sped up",
                "live",
                "remix",
                "instrumental",
                "acoustic",
                "nightcore",
            ]
            .iter()
            .any(|v| version.starts_with(v))
            {
                let pair = (clean_title(song, prefix), prefix.trim().to_owned());
                if !pair.0.is_empty() && !pair.1.is_empty() && !pairs.contains(&pair) {
                    pairs.push(pair);
                }
            }
            break;
        }
    }
    pairs
}
fn best_result(results: &[Value], title: &str, artist: &str, duration: f64) -> Option<Lyrics> {
    let title = normalized(&clean_title(title, artist));
    results
        .iter()
        .filter_map(|r| {
            let name = normalized(&clean_title(
                r["trackName"].as_str()?,
                r["artistName"].as_str()?,
            ));
            if name != title || !artist_matches(artist, r["artistName"].as_str()?) {
                return None;
            }
            let diff = (r["duration"].as_f64()? - duration).abs();
            // Do not stretch a studio recording's timestamps onto a different live/remix version.
            if duration > 0.0 && diff > 12.0 {
                return None;
            }
            let lines = r["hasWordSync"]
                .as_bool()
                .filter(|v| *v)
                .and_then(|_| r["lyricsfile"].as_str())
                .and_then(lyricsfile_lines)
                .unwrap_or_else(|| parse_lrc(r["syncedLyrics"].as_str().unwrap_or_default()));
            let plain = r["plainLyrics"].as_str().unwrap_or_default().to_owned();
            if lines.is_empty() && plain.is_empty() {
                return None;
            }
            Some((
                (
                    lines.iter().any(|line| !line.words.is_empty()),
                    !lines.is_empty(),
                    -(diff * 100.0) as i64,
                ),
                Lyrics {
                    synced: !lines.is_empty(),
                    lines,
                    plain,
                    source: "LRCLIB".into(),
                },
            ))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, lyrics)| lyrics)
}
fn genius_plain(html: &str) -> Option<String> {
    let document = dom_query::Document::from(html);
    let plain = document
        .select("[data-lyrics-container='true']")
        .iter()
        .map(|part| part.formatted_text().to_string())
        .filter(|text| text.lines().filter(|l| !l.trim().is_empty()).count() >= 3)
        .collect::<Vec<_>>()
        .join("\n\n");
    (!plain.trim().is_empty()).then(|| plain.trim().to_owned())
}
async fn genius(api: &Api, title: &str, artist: &str) -> Option<Lyrics> {
    for (name, performer) in identities(title, artist).into_iter().take(2) {
        let query = format!("{} {name}", primary_artist(&performer));
        let agent = concat!(
            "neisora/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/kum1te/neisora)"
        );
        let response = api
            .client
            .get("https://genius.com/api/search/multi")
            .query(&[("q", query.as_str())])
            .header("User-Agent", agent)
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .ok()?;
        let data: Value = crate::network::json(response, crate::network::LYRICS_LIMIT)
            .await
            .ok()?;
        for hit in data["response"]["sections"]
            .as_array()?
            .iter()
            .flat_map(|s| s["hits"].as_array().into_iter().flatten())
        {
            let song = &hit["result"];
            let actual_artist = song["artist_names"]
                .as_str()
                .or_else(|| song["primary_artist"]["name"].as_str())
                .unwrap_or_default();
            if normalized(&clean_title(
                song["title"].as_str().unwrap_or_default(),
                actual_artist,
            )) != normalized(&name)
                || !artist_matches(&performer, actual_artist)
            {
                continue;
            }
            let url = reqwest::Url::parse(song["url"].as_str()?).ok()?;
            if url.scheme() != "https"
                || url.host_str() != Some("genius.com")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port_or_known_default() != Some(443)
                || !url.path().ends_with("-lyrics")
            {
                continue;
            }
            let response = api
                .client
                .get(url)
                .header("User-Agent", agent)
                .timeout(Duration::from_secs(3))
                .send()
                .await
                .ok()?;
            let html = crate::network::text(response, crate::network::LYRICS_LIMIT)
                .await
                .ok()?;
            if let Some(plain) = genius_plain(&html) {
                return Some(Lyrics {
                    plain,
                    lines: vec![],
                    source: "Genius".into(),
                    synced: false,
                });
            }
        }
    }
    None
}
impl LyricsCache {
    pub async fn versions(
        &self,
        api: &Api,
        title: String,
        artist: String,
        query: String,
    ) -> Result<Vec<Candidate>, String> {
        if title.len() > 2000 || artist.len() > 2000 || query.chars().count() > 200 {
            return Err("Слишком длинный поисковый запрос".into());
        }
        let title = clean_title(&title, &artist);
        let artist = primary_artist(&artist);
        let query = query.trim();
        let fields = if query.is_empty() {
            vec![
                ("track_name", title.as_str()),
                ("artist_name", artist.as_str()),
            ]
        } else {
            vec![("q", query)]
        };
        let results = self
            .search(api, &fields)
            .await
            .ok_or("Не удалось получить версии текста. Попробуй позже.")?;
        let mut seen = std::collections::HashSet::new();
        let mut records: Vec<_> = results
            .iter()
            .filter(|r| {
                r["plainLyrics"]
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty())
                    || r["syncedLyrics"]
                        .as_str()
                        .is_some_and(|s| !s.trim().is_empty())
                    || r["hasWordSync"] == true
            })
            .filter_map(candidate)
            .filter(|c| seen.insert(c.id))
            .take(40)
            .collect();
        records.sort_by_key(|c| (!c.word_timed, !c.synced));
        Ok(records)
    }
    pub async fn version(
        &self,
        api: &Api,
        id: u64,
    ) -> Result<crate::lyric_choices::Selection, String> {
        if id == 0 || id > 9_007_199_254_740_991 {
            return Err("Некорректный номер версии текста".into());
        }
        if let Some((_, value)) = self
            .versions
            .lock()
            .await
            .get(&id)
            .filter(|(time, _)| time.elapsed() < Duration::from_secs(1800))
        {
            return Ok(value.clone());
        }
        let _permit = self
            .gate
            .acquire()
            .await
            .map_err(|_| "Поиск текста недоступен")?;
        let record = self
            .library_request(api, &format!("get/{id}"), &[])
            .await
            .ok_or("Не удалось загрузить выбранную версию. Попробуй позже.")?;
        let mut item = candidate(&record)
            .filter(|c| c.id == id)
            .ok_or("Сервис вернул другую версию текста")?;
        let raw = record["syncedLyrics"].as_str().unwrap_or_default();
        let plain = record["plainLyrics"].as_str().unwrap_or_default();
        if raw.len() > 256_000 || plain.len() > 256_000 {
            return Err("Версия текста слишком большая".into());
        }
        let lines = record["hasWordSync"]
            .as_bool()
            .filter(|v| *v)
            .and_then(|_| record["lyricsfile"].as_str())
            .and_then(lyricsfile_lines)
            .unwrap_or_else(|| parse_lrc(raw));
        let lyrics = Lyrics {
            synced: !lines.is_empty(),
            lines,
            plain: plain.into(),
            source: "LRCLIB".into(),
        };
        if !crate::lyric_choices::valid_lyrics(&lyrics)
            || (lyrics.lines.is_empty() && lyrics.plain.is_empty())
        {
            return Err("В выбранной версии нет корректного текста".into());
        }
        item.synced = lyrics.synced;
        item.word_timed = lyrics.lines.iter().any(|l| !l.words.is_empty());
        let value = crate::lyric_choices::Selection {
            record: item,
            lyrics: Some(lyrics),
        };
        let mut cache = self.versions.lock().await;
        if cache.len() >= 12 {
            if let Some(old) = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(id, _)| *id)
            {
                cache.remove(&old);
            }
        }
        cache.insert(id, (Instant::now(), value.clone()));
        Ok(value)
    }
    pub async fn invalidate(&self, id: &str) {
        self.cache.lock().await.remove(id);
    }
    async fn youtube(&self, api: &Api, id: &str) -> Option<Lyrics> {
        let data = api.request("next", json!({"videoId":id})).await.ok()?;
        let endpoint = lyrics_endpoint(&data)?;
        // Only browseId is portable to Android. Web-specific endpoint configs cause HTTP 400.
        let mobile = api
            .mobile_lyrics(json!({"browseId":endpoint["browseId"]}))
            .await
            .ok()
            .as_ref()
            .and_then(youtube_lyrics);
        if mobile.as_ref().is_some_and(|v| v.synced) {
            return mobile;
        }
        if mobile.is_some() {
            return mobile;
        }
        api.request("browse", endpoint)
            .await
            .ok()
            .as_ref()
            .and_then(youtube_lyrics)
    }
    async fn library_request(
        &self,
        api: &Api,
        route: &str,
        query: &[(&str, &str)],
    ) -> Option<Value> {
        {
            let mut after = self.remote_after.lock().await;
            let now = Instant::now();
            if *after > now + Duration::from_secs(2) {
                return None;
            }
            if *after > now {
                tokio::time::sleep(*after - now).await;
            }
            *after = Instant::now() + Duration::from_millis(400);
        }
        // This request has its own headers; Google cookies are never sent to lyric providers.
        let response = api
            .client
            .get(format!("https://lrclib.net/api/{route}"))
            .query(query)
            .header(
                "Lrclib-Client",
                concat!("neisora/", env!("CARGO_PKG_VERSION")),
            )
            .timeout(Duration::from_secs(6))
            .send()
            .await
            .ok()?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            *self.remote_after.lock().await = Instant::now() + Duration::from_secs(60);
            return None;
        }
        crate::network::json(response, crate::network::LYRICS_LIMIT)
            .await
            .ok()
    }
    async fn search(&self, api: &Api, query: &[(&str, &str)]) -> Option<Vec<Value>> {
        self.library_request(api, "search", query)
            .await?
            .as_array()
            .cloned()
    }
    async fn lrclib(&self, api: &Api, title: &str, artist: &str, duration: f64) -> Option<Lyrics> {
        let title = clean_title(title, artist);
        let full_artist = artist.split('•').next().unwrap_or(artist).trim();
        let primary = primary_artist(artist);
        let mut candidates = Vec::new();
        if let Some(results) = self
            .search(api, &[("track_name", &title), ("artist_name", full_artist)])
            .await
        {
            candidates.extend(results);
            if let Some(result) =
                best_result(&candidates, &title, full_artist, duration).filter(|v| v.synced)
            {
                return Some(result);
            }
        }
        if normalized(full_artist) != normalized(&primary) {
            if let Some(results) = self
                .search(api, &[("track_name", &title), ("artist_name", &primary)])
                .await
            {
                candidates.extend(results);
                if let Some(result) =
                    best_result(&candidates, &title, full_artist, duration).filter(|v| v.synced)
                {
                    return Some(result);
                }
            }
        }
        // A broader title search finds entries whose artist spelling includes extra credits.
        if let Some(results) = self.search(api, &[("track_name", &title)]).await {
            candidates.extend(results);
        }
        if let Some(result) =
            best_result(&candidates, &title, full_artist, duration).filter(|v| v.synced)
        {
            return Some(result);
        }
        if let Some(record) = self
            .library_request(
                api,
                "get",
                &[
                    ("track_name", title.as_str()),
                    ("artist_name", full_artist),
                    ("duration", &duration.round().to_string()),
                ],
            )
            .await
        {
            candidates.push(record);
        }
        best_result(&candidates, &title, full_artist, duration)
    }
    pub async fn get(
        &self,
        api: &Api,
        id: String,
        title: String,
        artist: String,
        duration: f64,
    ) -> Result<Lyrics, String> {
        self.get_with_recording(api, id, title, artist, duration, None)
            .await
    }
    pub async fn get_with_recording(
        &self,
        api: &Api,
        id: String,
        title: String,
        artist: String,
        duration: f64,
        recording: Option<crate::catalog::Recording>,
    ) -> Result<Lyrics, String> {
        let cached = || async {
            self.cache
                .lock()
                .await
                .get(&id)
                .filter(|(at, v)| {
                    at.elapsed() < Duration::from_secs(if v.synced { 43200 } else { 120 })
                })
                .map(|(_, v)| v.clone())
        };
        if let Some(value) = cached().await {
            return Ok(value);
        }
        let _permit = self.gate.acquire().await.map_err(|e| e.to_string())?;
        if let Some(value) = cached().await {
            return Ok(value);
        }
        let native = tokio::time::timeout(Duration::from_secs(12), async {
            let original = self.youtube(api, &id).await;
            if original.as_ref().is_some_and(|l| l.synced) {
                return original;
            }
            if let Some(other) = recording
                .as_ref()
                .and_then(|r| r.id.as_deref())
                .filter(|other| *other != id)
            {
                let alternate = self.youtube(api, other).await;
                if alternate.as_ref().is_some_and(|l| l.synced) {
                    return alternate;
                }
                return original.or(alternate);
            }
            original
        });
        let library = tokio::time::timeout(
            Duration::from_secs(12),
            self.lrclib(
                api,
                recording.as_ref().map_or(title.as_str(), |r| &r.title),
                recording.as_ref().map_or(artist.as_str(), |r| &r.artist),
                recording.as_ref().map_or(duration, |r| r.duration),
            ),
        );
        tokio::pin!(native, library);
        let result = tokio::select! {
            value = &mut native => {
                let value = value.ok().flatten();
                if value.as_ref().is_some_and(|v| v.synced) {
                    let other = tokio::time::timeout(Duration::from_secs(2), &mut library).await.ok().and_then(Result::ok).flatten();
                    other.filter(word_synced).or(value)
                }
                else { let other = library.await.ok().flatten(); other.as_ref().filter(|v| v.synced).cloned().or(value).or(other) }
            }
            value = &mut library => {
                let value = value.ok().flatten();
                if value.as_ref().is_some_and(|v| v.synced) { value }
                else { let other = native.await.ok().flatten(); other.as_ref().filter(|v| v.synced).cloned().or(value).or(other) }
            }
        };
        let result = match result {
            Some(value) => value,
            None => tokio::time::timeout(
                Duration::from_secs(6),
                genius(
                    api,
                    recording.as_ref().map_or(title.as_str(), |r| &r.title),
                    recording.as_ref().map_or(artist.as_str(), |r| &r.artist),
                ),
            )
            .await
            .ok()
            .flatten()
            .ok_or("Текст для этого трека пока не найден. Попробуй ещё раз позже")?,
        };
        let mut cache = self.cache.lock().await;
        if cache.len() >= 150 {
            if let Some(key) = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(id, _)| id.clone())
            {
                cache.remove(&key);
            }
        }
        cache.insert(id.clone(), (Instant::now(), result.clone()));
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_oversized_and_recursive_remote_yaml() {
        assert!(lyricsfile_lines(&"x".repeat(512_001)).is_none());
        assert!(lyricsfile_lines("lines: &loop [*loop]").is_none());
        assert!(lyricsfile_lines("lines: [ {start_ms: -1, text: nope} ]").is_none());
    }
    #[test]
    fn lrc_handles_repeated_timestamps_and_offset() {
        let lines = parse_lrc("[offset:-500]\n[00:12.25][00:22.50]Same line\n[01:02.003]Next");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time, 11.75);
        assert_eq!(lines[1].time, 22.0);
        assert_eq!(lines[2].text, "Next");
    }
    #[test]
    fn lyricsfile_preserves_real_word_timing_and_spaces() {
        let lines = lyricsfile_lines("version: '1.0'\nlines:\n  - text: 'Hello world'\n    start_ms: 1000\n    words:\n      - {text: 'Hello ', start_ms: 1000, end_ms: 1600}\n      - {text: 'world', start_ms: 1800, end_ms: 2300}\n").unwrap();
        assert_eq!(lines[0].words.len(), 2);
        assert_eq!(lines[0].words[1].time, 1.8);
        assert_eq!(lines[0].words[0].text, "Hello ");
        assert_eq!(lines[0].text, "Hello world");
    }
    #[test]
    fn matching_prefers_real_words_over_a_closer_line_only_record() {
        let records = vec![
            json!({"trackName":"Song","artistName":"Artist","duration":200,"syncedLyrics":"[00:01]Hello"}),
            json!({"trackName":"Song","artistName":"Artist","duration":202,"hasWordSync":true,"lyricsfile":"lines:\n  - text: Hello\n    start_ms: 1000\n    words:\n      - {text: Hello, start_ms: 1000, end_ms: 1800}\n"}),
        ];
        assert!(word_synced(
            &best_result(&records, "Song", "Artist", 200.0).unwrap()
        ));
    }
    #[test]
    fn partial_word_timings_fall_back_to_the_complete_line() {
        let lines = lyricsfile_lines("lines:\n  - text: 'Hello world'\n    start_ms: 1000\n    words:\n      - {text: 'Hello', start_ms: 1000, end_ms: 1600}\n").unwrap();
        assert!(lines[0].words.is_empty());
        assert_eq!(lines[0].text, "Hello world");
    }
    #[test]
    fn mobile_cues_are_read_without_fabricating_missing_timing() {
        let timed = youtube_lyrics(&json!({"contents":{"timedLyricsData":[
            {"lyricLine":"A","cueRange":{"startTimeMilliseconds":"1250"}},
            {"lyricLine":"B","cueRange":{"startTimeMilliseconds":3000}}
        ]}}))
        .unwrap();
        assert!(timed.synced);
        assert_eq!(timed.lines[0].time, 1.25);
        let plain =
            youtube_lyrics(&json!({"timedLyricsData":[{"lyricLine":"A"},{"lyricLine":"B"}]}))
                .unwrap();
        assert!(!plain.synced);
        assert_eq!(plain.plain, "A\nB");
    }
    #[test]
    fn genius_html_is_text_only_and_keeps_line_breaks() {
        let html = r#"<script>bad()</script><div data-lyrics-container="true">Heading</div><div data-lyrics-container="true"><div>Line &amp; one<br/>Line <a href="https://example.org">two</a><br/>Line three</div></div>"#;
        let plain = genius_plain(html).unwrap();
        assert!(plain.contains("Line & one"));
        assert!(plain.contains("Line two"));
        assert!(plain.contains("Line three"));
        assert!(!plain.contains("<a"));
        assert!(!plain.contains("bad()"));
        assert!(!plain.contains("Heading"));
    }
    #[test]
    fn joint_credits_video_prefixes_and_endpoint_variants() {
        assert_eq!(
            clean_title(
                "VILLIAN, madk1d - ДИНАСТИЯ (official music video)",
                "madk1d"
            ),
            "ДИНАСТИЯ"
        );
        assert!(artist_matches("madk1d и VILLIAN", "VILLIAN, madk1d"));
        assert!(!artist_matches("Niko B", "Niko B Tribute"));
        assert_eq!(primary_artist("AC/DC"), "AC/DC");
        assert!(identities("Artist - Song (Official Video)", "Uploader")
            .contains(&("Song".into(), "Artist".into())));
        assert_eq!(identities("Song - slowed", "Artist").len(), 1);
        assert!(lyrics_endpoint(&json!({"browseEndpoint":{"browseId":"MPLYt_public"}})).is_some());
        let records = vec![
            json!({"trackName":"ДИНАСТИЯ","artistName":"VILLIAN, madk1d","duration":150,"syncedLyrics":"[00:01]line"}),
        ];
        assert!(
            best_result(
                &records,
                "VILLIAN, madk1d - ДИНАСТИЯ (official music video)",
                "madk1d и VILLIAN",
                151.0
            )
            .unwrap()
            .synced
        );
    }
    #[test]
    fn normalizes_video_titles_and_multiple_artists() {
        assert_eq!(clean_title("GONE.Fludd — БАЛАНС", "GONE.Fludd"), "БАЛАНС");
        assert_eq!(
            clean_title("Never Gonna Give You Up (OFFICIAL VIDEO)", "Rick Astley"),
            "Never Gonna Give You Up"
        );
        assert!(artist_matches("Madvillain, Madlib и MF Doom", "Madvillain"));
        assert!(artist_matches("azrxel и abxddon", "azrxel"));
        assert_eq!(clean_title("lilacs - slowed", "azrxel"), "lilacs - slowed");
    }
    #[test]
    fn matching_prefers_timed_lyrics_and_rejects_wrong_versions() {
        let data = vec![
            json!({"trackName":"Song","artistName":"Artist","duration":200,"plainLyrics":"plain"}),
            json!({"trackName":"Song","artistName":"Artist","duration":203,"syncedLyrics":"[00:10]timed"}),
            json!({"trackName":"Song (Live)","artistName":"Artist","duration":201,"syncedLyrics":"[00:01]wrong"}),
            json!({"trackName":"Song","artistName":"Other","duration":200,"syncedLyrics":"[00:01]wrong"}),
        ];
        let result = best_result(&data, "Song", "Artist", 200.0).unwrap();
        assert!(result.synced);
        assert_eq!(result.lines[0].time, 10.0);
    }
    #[test]
    fn missing_timestamps_are_not_fabricated() {
        assert!(parse_lrc("Plain lyrics\n[ar:Artist]").is_empty());
    }
}
