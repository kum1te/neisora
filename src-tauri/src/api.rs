use crate::model::{self, Feed, Item, Queue};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, Notify, RwLock};

pub const ORIGIN: &str = "https://music.youtube.com";
pub const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct Api {
    pub client: reqwest::Client,
    pub directory: PathBuf,
    cookie: Arc<RwLock<String>>,
    context: Arc<Mutex<Option<(u64, Value)>>>,
    cookie_updates: Arc<Mutex<()>>,
    pub(crate) playlist_updates: Arc<Mutex<()>>,
    session_epoch: Arc<AtomicU64>,
    rejected: Arc<AtomicBool>,
    session_changes: Arc<Notify>,
}

pub fn private_write(path: &std::path::Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let filename = path
        .file_name()
        .ok_or("Некорректный путь настроек")?
        .to_string_lossy();
    let tmp = path.with_file_name(format!(
        ".{filename}.{}.{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    // create_new refuses existing files and symlinks; a replacement always starts at 0600.
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(data).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        crate::platform::replace(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}

pub struct PrivateTempFile(pub PathBuf);
impl Drop for PrivateTempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Remove only our known temporary files, never the live session or another running instance's copies.
pub fn cleanup_temporary_files(directory: &std::path::Path, include_current: bool) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(pid) = name
            .strip_prefix("mpv-")
            .and_then(|v| v.strip_suffix(".sock"))
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|p| *p > 0 && *p <= i32::MAX as u32)
        {
            if pid != std::process::id() && !crate::platform::process_alive(pid) {
                let _ = std::fs::remove_file(entry.path());
            }
            continue;
        }
        let pid = name
            .strip_prefix("stream-session-")
            .or_else(|| name.strip_prefix("browser-session-"))
            .and_then(|v| v.split('-').next())
            .and_then(|v| v.parse::<u32>().ok())
            .or_else(|| {
                if name.starts_with(".cookies.txt.") && name.ends_with(".tmp") {
                    name.rsplit('.').nth(2)?.parse::<u32>().ok()
                } else {
                    None
                }
            });
        let Some(pid) = pid.filter(|pid| *pid > 0 && *pid <= i32::MAX as u32) else {
            continue;
        };
        let stale = if pid == std::process::id() {
            include_current
        } else {
            !crate::platform::process_alive(pid)
        };
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub fn cookie_header(netscape: &str) -> String {
    let mut pairs = std::collections::BTreeMap::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for line in netscape.lines() {
        let line = line.strip_prefix("#HttpOnly_").unwrap_or(line);
        if line.starts_with('#') {
            continue;
        }
        let cols: Vec<_> = line.split('\t').collect();
        if cols.len() < 7 {
            continue;
        }
        let domain = cols[0].trim_start_matches('.');
        if domain != "youtube.com" && domain != "music.youtube.com" {
            continue;
        }
        if !"/".starts_with(cols[2]) {
            continue;
        }
        let expiry = cols[4].parse::<u64>().unwrap_or(0);
        if expiry > 0 && expiry <= now {
            continue;
        }
        if cols[5].bytes().any(|c| c <= 32 || c == b';')
            || cols[6].bytes().any(|c| c == b'\r' || c == b'\n')
        {
            continue;
        }
        pairs.insert(cols[5], cols[6]);
    }
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn merge_cookies(raw: &str, headers: &[String]) -> String {
    let now = cookie::time::OffsetDateTime::now_utc();
    let mut rows: Vec<String> = raw
        .lines()
        .filter(|line| !line.starts_with('#') || line.starts_with("#HttpOnly_"))
        .map(str::to_owned)
        .collect();
    for header in headers {
        let Ok(c) = cookie::Cookie::parse(header.as_str()) else {
            continue;
        };
        let domain = c.domain().unwrap_or("music.youtube.com");
        if !["youtube.com", "music.youtube.com"].contains(&domain.trim_start_matches('.')) {
            continue;
        }
        let path = c.path().unwrap_or("/");
        if !"/".starts_with(path) || c.value().contains(['\t', '\r', '\n']) {
            continue;
        }
        rows.retain(|row| {
            let cols: Vec<_> = row.trim_start_matches("#HttpOnly_").split('\t').collect();
            cols.len() >= 7
                && !(cols[0].trim_start_matches('.') == domain.trim_start_matches('.')
                    && cols[2] == path
                    && cols[5] == c.name())
        });
        let expiry = c
            .max_age()
            .map(|age| now.unix_timestamp() + age.whole_seconds())
            .or_else(|| c.expires_datetime().map(|t| t.unix_timestamp()))
            .unwrap_or(0);
        if (c.max_age().is_some_and(|age| age.whole_seconds() <= 0))
            || (expiry > 0 && expiry <= now.unix_timestamp())
        {
            continue;
        }
        let scope = if domain.trim_start_matches('.') == "youtube.com" {
            ".youtube.com"
        } else {
            "music.youtube.com"
        };
        rows.push(format!(
            "{}{}\t{}\t{}\t{}\t{}\t{}\t{}",
            if c.http_only().unwrap_or(false) {
                "#HttpOnly_"
            } else {
                ""
            },
            scope,
            if scope.starts_with('.') {
                "TRUE"
            } else {
                "FALSE"
            },
            path,
            if c.secure().unwrap_or(false) {
                "TRUE"
            } else {
                "FALSE"
            },
            expiry,
            c.name(),
            c.value()
        ));
    }
    format!("# Netscape HTTP Cookie File\n{}\n", rows.join("\n"))
}

fn has_auth(cookie: &str) -> bool {
    cookie.split("; ").any(|p| {
        p.starts_with("SAPISID=")
            || p.starts_with("__Secure-3PAPISID=")
            || p.starts_with("__Secure-1PAPISID=")
    })
}

fn auth_header(cookie: &str) -> Option<String> {
    let pairs: Vec<_> = cookie
        .split("; ")
        .filter_map(|p| p.split_once('='))
        .collect();
    let sid = ["SAPISID", "__Secure-3PAPISID", "__Secure-1PAPISID"]
        .iter()
        .find_map(|name| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| *v))?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let digest = Sha1::digest(format!("{timestamp} {sid} {ORIGIN}").as_bytes());
    Some(format!("SAPISIDHASH {timestamp}_{digest:x}"))
}

impl Api {
    pub fn new(directory: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        crate::platform::private_directory(&directory).map_err(|e| e.to_string())?;
        cleanup_temporary_files(&directory, false);
        let raw = std::fs::read_to_string(directory.join("cookies.txt")).unwrap_or_default();
        let client = reqwest::Client::builder()
            .user_agent(UA)
            .https_only(true)
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(25))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            client,
            directory,
            cookie: Arc::new(RwLock::new(cookie_header(&raw))),
            context: Arc::new(Mutex::new(None)),
            cookie_updates: Arc::new(Mutex::new(())),
            playlist_updates: Arc::new(Mutex::new(())),
            session_epoch: Arc::new(AtomicU64::new(0)),
            rejected: Arc::new(AtomicBool::new(false)),
            session_changes: Arc::new(Notify::new()),
        })
    }

    pub async fn authenticated(&self) -> bool {
        !self.rejected.load(Ordering::SeqCst) && has_auth(&self.cookie.read().await)
    }

    pub fn session_generation(&self) -> u64 {
        self.session_epoch.load(Ordering::SeqCst)
    }

    pub async fn session_changed(&self, expected: u64) {
        loop {
            let changed = self.session_changes.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.session_generation() != expected {
                return;
            }
            changed.await;
        }
    }

    pub async fn save_cookies_if_current(&self, raw: &str, expected: u64) -> Result<u64, String> {
        let header = cookie_header(raw);
        if !has_auth(&header) {
            return Err("В сессии нет входа в YouTube. Сначала войди в music.youtube.com в выбранном браузере.".into());
        }
        let guard = self.cookie_updates.lock().await;
        if self.session_epoch.load(Ordering::SeqCst) != expected {
            return Err("Вход отменён: сессия изменилась".into());
        }
        private_write(&self.directory.join("cookies.txt"), raw.as_bytes())?;
        let generation = self.session_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        self.session_changes.notify_waiters();
        *self.cookie.write().await = header;
        self.rejected.store(false, Ordering::SeqCst);
        drop(guard);
        if let Ok(mut context) = self.context.try_lock() {
            *context = None;
        }
        Ok(generation)
    }

    pub async fn logout(&self) -> Result<(), String> {
        let guard = self.cookie_updates.lock().await;
        self.session_epoch.fetch_add(1, Ordering::SeqCst);
        self.session_changes.notify_waiters();
        let path = self.directory.join("cookies.txt");
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        *self.cookie.write().await = String::new();
        self.rejected.store(false, Ordering::SeqCst);
        drop(guard);
        if let Ok(mut context) = self.context.try_lock() {
            *context = None;
        }
        Ok(())
    }

    async fn update_cookies(&self, response: &reqwest::Response, epoch: u64) -> Result<(), String> {
        let headers: Vec<String> = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok().map(str::to_owned))
            .collect();
        if headers.is_empty() {
            return Ok(());
        }
        let _guard = self.cookie_updates.lock().await;
        // A response from before login/logout must not recreate or overwrite that session.
        if epoch != self.session_epoch.load(Ordering::SeqCst) {
            return Ok(());
        }
        let path = self.directory.join("cookies.txt");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Ok(());
        };
        let merged = merge_cookies(&raw, &headers);
        if merged != raw {
            private_write(&path, merged.as_bytes())?;
            *self.cookie.write().await = cookie_header(&merged);
        }
        Ok(())
    }

    async fn context(&self) -> Result<Value, String> {
        let epoch = self.session_generation();
        let mut cached = self.context.lock().await;
        if let Some((_, ctx)) = cached.as_ref().filter(|(version, _)| *version == epoch) {
            return Ok(ctx.clone());
        }
        let cookie = self.cookie.read().await.clone();
        let response = self
            .client
            .get(ORIGIN)
            .header("Cookie", cookie)
            .send()
            .await
            .map_err(|e| format!("YouTube недоступен: {e}"))?;
        self.update_cookies(&response, epoch).await?;
        let html = crate::network::text(response, crate::network::PAGE_LIMIT).await?;
        let mut ctx = None;
        for (index, _) in html.match_indices("ytcfg.set(") {
            let rest = &html[index + "ytcfg.set(".len()..];
            if let Some(Ok(value)) = serde_json::Deserializer::from_str(rest)
                .into_iter::<Value>()
                .next()
            {
                if let Some(context) = value.get("INNERTUBE_CONTEXT") {
                    ctx = Some(context.clone());
                    break;
                }
            }
        }
        let mut ctx = ctx.ok_or_else(|| {
            "Не удалось прочитать конфигурацию YouTube Music. Проверь подключение к YouTube."
                .to_owned()
        })?;
        ctx["client"]["hl"] = json!("ru");
        ctx["client"]["timeZone"] = json!("Europe/Moscow");
        if epoch != self.session_epoch.load(Ordering::SeqCst) {
            return Err("Сессия изменилась. Повтори запрос.".into());
        }
        *cached = Some((epoch, ctx.clone()));
        Ok(ctx)
    }

    pub async fn request(&self, route: &str, payload: Value) -> Result<Value, String> {
        self.request_client(route, payload, false, None).await
    }

    pub(crate) async fn request_for_session(
        &self,
        route: &str,
        payload: Value,
        epoch: u64,
    ) -> Result<Value, String> {
        self.request_client(route, payload, false, Some(epoch))
            .await
    }

    pub async fn mobile_lyrics(&self, endpoint: Value) -> Result<Value, String> {
        self.request_client("browse", endpoint, true, None).await
    }

    async fn request_client(
        &self,
        route: &str,
        payload: Value,
        mobile: bool,
        expected_epoch: Option<u64>,
    ) -> Result<Value, String> {
        if ![
            "browse",
            "search",
            "next",
            "player",
            "like/like",
            "like/removelike",
            "account/account_menu",
            "playlist/create",
            "playlist/delete",
            "browse/edit_playlist",
        ]
        .contains(&route)
        {
            return Err("Неизвестная команда YouTube".into());
        }
        let epoch = self.session_epoch.load(Ordering::SeqCst);
        if expected_epoch.is_some_and(|expected| expected != epoch) {
            return Err("Сессия изменилась. Повтори запрос.".into());
        }
        let mut context = self.context().await?;
        if mobile {
            context["client"]["clientName"] = json!("ANDROID_MUSIC");
            context["client"]["clientVersion"] = json!("7.21.50");
        }
        let cookie = self.cookie.read().await.clone();
        if epoch != self.session_generation() {
            return Err("Сессия изменилась. Повтори запрос.".into());
        }
        let mut body = payload;
        if !body.is_object() {
            return Err("Некорректная команда YouTube".into());
        }
        body["context"] = context.clone();
        let mut req = self
            .client
            .post(format!("{ORIGIN}/youtubei/v1/{route}?prettyPrint=false"))
            .header("Origin", ORIGIN)
            .header("Referer", format!("{ORIGIN}/"))
            .header("X-Youtube-Client-Name", if mobile { "21" } else { "67" })
            .header(
                "X-Youtube-Client-Version",
                context["client"]["clientVersion"]
                    .as_str()
                    .unwrap_or_default(),
            )
            .header("X-Goog-AuthUser", "0")
            .header("Cookie", &cookie)
            .json(&body);
        if let Some(authorization) = auth_header(&cookie) {
            req = req
                .header("Authorization", authorization)
                .header("X-Origin", ORIGIN);
        }
        if let Some(visitor) = context["client"]["visitorData"].as_str() {
            req = req.header("X-Goog-Visitor-Id", visitor);
        }
        let response = req
            .send()
            .await
            .map_err(|e| format!("Запрос к YouTube: {e}"))?;
        if !mobile {
            self.update_cookies(&response, epoch).await?;
        }
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            if !mobile && epoch == self.session_epoch.load(Ordering::SeqCst) {
                self.rejected.store(true, Ordering::SeqCst);
            }
            return Err("Сессия YouTube устарела. Обнови вход в настройках аккаунта.".into());
        }
        if !status.is_success() {
            return Err(format!(
                "YouTube вернул HTTP {status}. Попробуй обновить сессию или повторить запрос."
            ));
        }
        let data: Value = crate::network::json(response, crate::network::API_LIMIT).await?;
        if epoch != self.session_generation() {
            return Err("Сессия изменилась. Обнови страницу перед повтором операции.".into());
        }
        if let Some(error) = data.get("error") {
            return Err(model::text(&error["message"]));
        }
        Ok(data)
    }

    pub async fn browse(&self, endpoint: Value) -> Result<Feed, String> {
        let payload = endpoint.get("browseEndpoint").cloned().unwrap_or(endpoint);
        if payload.get("browseId").is_none() && payload.get("continuation").is_none() {
            return Err("У подборки нет команды открытия".into());
        }
        let data = self.request("browse", payload).await?;
        Ok(model::feed(&data, self.authenticated().await))
    }

    pub async fn search(&self, query: String) -> Result<Feed, String> {
        let data = self.request("search", json!({"query":query})).await?;
        Ok(model::feed(&data, self.authenticated().await))
    }

    pub async fn next(&self, endpoint: Value) -> Result<Queue, String> {
        let mut payload = model::watch_payload(&endpoint).unwrap_or(endpoint);
        if payload.get("videoId").is_none()
            && payload.get("playlistId").is_none()
            && payload.get("continuation").is_none()
        {
            return Err("У подборки нет команды воспроизведения".into());
        }
        payload["enablePersistentPlaylistPanel"] = json!(true);
        let mut anchor: Option<Item> = None;
        for _ in 0..3 {
            let data = self.request("next", payload.clone()).await?;
            let queue = model::queue(&data, &payload);
            // An individual track comes with one item plus a server-provided automix endpoint.
            // Follow that endpoint before returning, just as the YouTube Music player does.
            let automix = model::find(&data, "automixPlaylistVideoRenderer")
                .and_then(|a| a.get("navigationEndpoint"))
                .and_then(model::watch_payload);
            if queue.as_ref().is_some_and(|q| {
                !q.items.is_empty()
                    && (q.continuation.is_some()
                        || automix.is_none()
                        || payload.get("playlistId").is_some())
            }) {
                return Ok(model::keep_anchor(queue.unwrap(), anchor));
            }
            if let Some(next) = automix {
                if anchor.is_none() {
                    anchor = queue
                        .as_ref()
                        .and_then(|q| q.items.get(q.selected))
                        .cloned();
                }
                payload = next;
                continue;
            }
            return queue.ok_or_else(|| "YouTube не вернул очередь для этой подборки".into());
        }
        Err("Не удалось открыть очередь микса".into())
    }

    pub async fn like(&self, video_id: String, liked: bool) -> Result<(), String> {
        if !self.authenticated().await {
            return Err("Подключи YouTube в настройках".into());
        }
        if video_id.len() != 11
            || !video_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("Некорректный трек".into());
        }
        self.request(
            if liked {
                "like/like"
            } else {
                "like/removelike"
            },
            json!({"target":{"videoId":video_id}}),
        )
        .await?;
        Ok(())
    }

    pub async fn refresh_queue(
        &self,
        context: Value,
        endpoint: Value,
        signals: Value,
    ) -> Result<Option<Queue>, String> {
        let payload = skip_payload(context, endpoint, signals)?;
        let data = self.request("next", payload.clone()).await?;
        // Some accounts receive only metadata: retaining the current tail is correct.
        // Never recreate a radio as a fallback for a missing queue update.
        Ok(model::queue(&data, &payload))
    }
}

pub fn skip_payload(context: Value, endpoint: Value, signals: Value) -> Result<Value, String> {
    let target = model::watch_payload(&endpoint).unwrap_or(endpoint);
    let video = target
        .get("videoId")
        .cloned()
        .ok_or("Не выбран следующий трек")?;
    let mut payload = json!({
        "videoId":video, "isAudioOnly":true, "enablePersistentPlaylistPanel":true,
        "tunerSettingValue":"AUTOMIX_SETTING_NORMAL",
        "watchNextType":"WATCH_NEXT_TYPE_SKIP_VIDEO", "responsiveSignals":signals
    });
    for key in ["playlistId", "index"] {
        if let Some(value) = target.get(key).or_else(|| context.get(key)) {
            payload[key] = value.clone();
        }
    }
    if let Some(value) = context.get("queueContextParams") {
        payload["queueContextParams"] = value.clone();
    }
    // Initial mix params (wAEB) start a fresh radio. Queue-item params request only
    // metadata for the persistent panel. Neither belongs in a responsive skip.
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_preserves_live_session_and_removes_only_owned_temporary_files() {
        let dir =
            std::env::temp_dir().join(format!("neisora-cleanup-audit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let temp = dir.join(format!("stream-session-{}-123.txt", std::process::id()));
        let stale_socket = dir.join("mpv-2147483647.sock");
        let live_socket = dir.join(format!("mpv-{}.sock", std::process::id()));
        std::fs::write(&stale_socket, b"stale").unwrap();
        std::fs::write(&live_socket, b"live").unwrap();
        for path in [&temp, &dir.join("cookies.txt"), &dir.join("notes.txt")] {
            std::fs::write(path, b"fixture").unwrap();
        }
        cleanup_temporary_files(&dir, false);
        assert!(!stale_socket.exists());
        assert!(live_socket.exists());
        assert!(temp.exists());
        cleanup_temporary_files(&dir, true);
        assert!(live_socket.exists());
        assert!(!temp.exists());
        assert!(dir.join("cookies.txt").exists());
        assert!(dir.join("notes.txt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn late_login_cannot_restore_logged_out_session() {
        let dir = std::env::temp_dir().join(format!("neisora-auth-race-{}", std::process::id()));
        let api = Api::new(dir.clone()).unwrap();
        let old = api.session_generation();
        let context_guard = api.context.lock().await;
        tokio::time::timeout(Duration::from_millis(50), api.logout())
            .await
            .unwrap()
            .unwrap();
        drop(context_guard);
        let jar = ".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tfake-audit-session\n";
        assert!(api.save_cookies_if_current(jar, old).await.is_err());
        assert!(!dir.join("cookies.txt").exists());
        assert!(!api.authenticated().await);
        let saved = api
            .save_cookies_if_current(jar, api.session_generation())
            .await
            .unwrap();
        assert!(api.authenticated().await);
        api.logout().await.unwrap();
        tokio::time::timeout(Duration::from_millis(50), api.session_changed(saved))
            .await
            .unwrap();
        assert!(api.save_cookies_if_current(jar, saved).await.is_err());
        assert!(!dir.join("cookies.txt").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn private_replace_does_not_follow_symlinks_and_enforces_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = std::env::temp_dir().join(format!("neisora-security-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let secret = dir.join("cookies.txt");
        let target = dir.join("unrelated");
        std::fs::write(&target, b"untouched").unwrap();
        symlink(&target, &secret).unwrap();
        symlink(&target, secret.with_extension("tmp")).unwrap();
        private_write(&secret, b"private-test-data").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
        assert_eq!(std::fs::read(&secret).unwrap(), b"private-test-data");
        assert_eq!(
            std::fs::metadata(&secret).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!std::fs::symlink_metadata(&secret)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 3);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn imports_httponly_and_scopes_cookies_to_youtube() {
        let jar = "# Netscape HTTP Cookie File\n#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tsecret\n.google.com\tTRUE\t/\tTRUE\t0\tSID\tother\n.youtube.com.evil.org\tTRUE\t/\tTRUE\t0\tHSID\tbad\n.youtube.com\tTRUE\t/\tTRUE\t1\tOLD\texpired\n";
        assert_eq!(cookie_header(jar), "SAPISID=secret");
        assert!(auth_header(&cookie_header(jar))
            .unwrap()
            .starts_with("SAPISIDHASH "));
    }

    #[test]
    fn rotates_cookies_without_exporting_foreign_domains() {
        let raw = ".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\told\n.youtube.com\tTRUE\t/\tTRUE\t0\tSIDCC\told-session\n";
        let merged = merge_cookies(
            raw,
            &[
                "SAPISID=new; Domain=.youtube.com; Path=/; Secure; HttpOnly; Max-Age=3600".into(),
                "SIDCC=; Domain=.youtube.com; Path=/; Max-Age=0".into(),
                "foreign=secret; Domain=example.com; Path=/".into(),
            ],
        );
        assert_eq!(cookie_header(&merged), "SAPISID=new");
        assert!(!merged.contains("foreign"));
        assert!(merged.contains("#HttpOnly_.youtube.com"));
    }

    #[test]
    fn skip_preserves_session_and_reports_the_skipped_video() {
        let signals = json!({"videoInteraction":[{"videoId":"old_video11","queueIndex":3,"playbackSkip":{}}]});
        let payload = skip_payload(
            json!({"playlistId":"RDlive","params":"wAEB","queueContextParams":"session-token","continuation":"old"}),
            json!({"watchEndpoint":{"videoId":"next_video1","index":4,"params":"reuse-panel"}}),
            signals.clone(),
        ).unwrap();
        assert_eq!(payload["playlistId"], "RDlive");
        assert_eq!(payload["videoId"], "next_video1");
        assert_eq!(payload["index"], 4);
        assert_eq!(payload["watchNextType"], "WATCH_NEXT_TYPE_SKIP_VIDEO");
        assert_eq!(payload["queueContextParams"], "session-token");
        assert_eq!(payload["responsiveSignals"], signals);
        assert!(payload.get("params").is_none());
        assert!(payload.get("continuation").is_none());
    }
}
