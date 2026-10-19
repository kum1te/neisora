use crate::diagnostics::{Diagnostics, Outcome, Stage};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Child,
    sync::{watch, Mutex, Notify, Semaphore},
};

#[derive(Clone, Serialize)]
pub struct PlayerState {
    pub position: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub buffering: bool,
    pub active: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            position: 0.0,
            duration: 0.0,
            paused: true,
            volume: 70.0,
            buffering: false,
            active: false,
        }
    }
}

#[derive(Clone)]
struct StreamInfo {
    url: String,
    headers: Vec<String>,
    user_agent: String,
}
#[derive(Clone)]
struct ActivePlayback {
    id: String,
    generation: u64,
    entry: i64,
}
#[derive(Clone, Serialize)]
struct StreamFailure {
    id: String,
    generation: u64,
    position: f64,
}

pub struct Player {
    output: Arc<crate::audio_output::Output>,
    pub diagnostics: Arc<Diagnostics>,
    sleep: Arc<crate::sleep_timer::SleepTimer>,
    directory: PathBuf,
    socket: PathBuf,
    child: Mutex<Option<Child>>,
    state: Arc<Mutex<PlayerState>>,
    streams: Mutex<HashMap<String, (Instant, StreamInfo)>>,
    flights: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    generation: Arc<AtomicU64>,
    recovered_generation: AtomicU64,
    mpv_epoch: Arc<AtomicU64>,
    playback: Arc<Mutex<Option<ActivePlayback>>>,
    session_epoch: AtomicU64,
    transition: Mutex<()>,
    extraction_limit: Semaphore,
    play_change: Notify,
    visualizer: Arc<AtomicBool>,
    visualizer_wake: Arc<Notify>,
    seek_serial: AtomicU64,
    seek_lock: Mutex<()>,
    current_track: watch::Sender<String>,
}

fn resume_position(position: f64) -> Result<f64, String> {
    if position.is_finite() && (0.0..=604800.0).contains(&position) {
        Ok(position)
    } else {
        Err("Некорректная позиция продолжения".into())
    }
}

fn valid_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn valid_media_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port_or_known_default() == Some(443)
            && u.host_str()
                .is_some_and(|h| h.ends_with(".googlevideo.com"))
    })
}
async fn connect_mpv(child: &mut Child, socket: &Path) -> Result<crate::mpv_ipc::Stream, String> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("mpv завершился при запуске: {status}"));
        }
        if let Ok(stream) = crate::mpv_ipc::connect(socket).await {
            return Ok(stream);
        }
        if started.elapsed() >= Duration::from_secs(5) {
            return Err("mpv не отвечает: истекло время ожидания IPC".into());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
impl Player {
    pub fn new(directory: PathBuf, sleep: Arc<crate::sleep_timer::SleepTimer>) -> Self {
        let mut state = PlayerState::default();
        if let Ok(raw) = std::fs::read(directory.join("volume.json")) {
            if let Ok(volume) = serde_json::from_slice::<f64>(&raw) {
                if volume.is_finite() {
                    state.volume = volume.clamp(0.0, 100.0);
                }
            }
        }
        Self {
            output: Arc::new(crate::audio_output::Output::new(directory.clone())),
            diagnostics: Arc::new(Diagnostics::default()),
            sleep,
            socket: crate::mpv_ipc::endpoint(&directory),
            directory,
            child: Mutex::new(None),
            state: Arc::new(Mutex::new(state)),
            streams: Mutex::new(HashMap::new()),
            flights: Mutex::new(HashMap::new()),
            generation: Arc::new(AtomicU64::new(0)),
            recovered_generation: AtomicU64::new(0),
            mpv_epoch: Arc::new(AtomicU64::new(0)),
            playback: Arc::new(Mutex::new(None)),
            session_epoch: AtomicU64::new(0),
            transition: Mutex::new(()),
            extraction_limit: Semaphore::new(2),
            play_change: Notify::new(),
            visualizer: Arc::new(AtomicBool::new(false)),
            visualizer_wake: Arc::new(Notify::new()),
            seek_serial: AtomicU64::new(0),
            seek_lock: Mutex::new(()),
            current_track: watch::channel(String::new()).0,
        }
    }

    async fn ensure(&self, app: &AppHandle) -> Result<(), String> {
        let mut child = self.child.lock().await;
        if let Some(c) = child.as_mut() {
            if c.try_wait().map_err(|e| e.to_string())?.is_none() {
                if crate::mpv_ipc::present(&self.socket) {
                    return Ok(());
                }
                let _ = c.kill().await;
                let _ = c.wait().await;
            }
        }
        if crate::mpv_ipc::present(&self.socket) {
            crate::mpv_ipc::cleanup(&self.socket);
        }
        let instance = self.mpv_epoch.fetch_add(1, Ordering::SeqCst) + 1;
        let epoch = self.mpv_epoch.clone();
        let mut cmd = crate::runtime::command("mpv")?;
        // neisora owns the queue and SMTC session; mpv must not register a second
        // media session or consume a key before the frontend advances its queue.
        #[cfg(windows)]
        cmd.args(["--input-media-keys=no", "--media-controls=no"]);
        cmd.args([
            "--no-config",
            "--load-scripts=no",
            "--idle=yes",
            "--no-video",
            "--no-terminal",
            "--ytdl=no",
            "--access-references=no",
            "--audio-client-name=neisora",
            "--audio-fallback-to-null=yes",
            "--cache=yes",
            "--demuxer-max-bytes=16MiB",
            "--network-timeout=12",
        ])
        .arg(format!("--volume={}", self.state.lock().await.volume))
        .arg(format!("--input-ipc-server={}", self.socket.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
        if let Ok(ao) = std::env::var("VESPER_MPV_AO") {
            cmd.arg(format!("--ao={ao}"));
        }
        if self.visualizer.load(Ordering::Relaxed) {
            cmd.arg(format!("--af={}", crate::visualizer::FILTER));
        }
        crate::process::parent_death(&mut cmd);
        *child = Some(
            cmd.spawn()
                .map_err(|e| format!("Не удалось запустить mpv: {e}"))?,
        );
        let mut stream = connect_mpv(child.as_mut().unwrap(), &self.socket).await?;
        for (id, name) in [
            "time-pos",
            "duration",
            "pause",
            "volume",
            "paused-for-cache",
            "idle-active",
            "audio-device-list",
            "current-ao",
        ]
        .iter()
        .enumerate()
        {
            stream
                .write_all(
                    format!("{}\n", json!({"command":["observe_property",id,name]})).as_bytes(),
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        crate::visualizer::spawn(
            self.socket.clone(),
            self.visualizer.clone(),
            self.state.clone(),
            self.visualizer_wake.clone(),
            app.clone(),
        );
        let state = self.state.clone();
        let playback = self.playback.clone();
        let visualizer_wake = self.visualizer_wake.clone();
        let generation = self.generation.clone();
        let app = app.clone();
        let sleep = self.sleep.clone();
        let diagnostics = self.diagnostics.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            let mut last_emit = Instant::now() - Duration::from_secs(1);
            while let Ok(Some(line)) = lines.next_line().await {
                if epoch.load(Ordering::SeqCst) != instance {
                    break;
                }
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if event["event"] == "property-change"
                    && matches!(
                        event["name"].as_str(),
                        Some("audio-device-list" | "current-ao")
                    )
                {
                    let app = app.clone();
                    tokio::spawn(async move {
                        use tauri::Manager;
                        if let Some(state) = app.try_state::<Arc<crate::Backend>>() {
                            let _ = state.player.reconcile_audio(&app).await;
                        }
                    });
                }
                if event["event"] == "end-file" {
                    let track = {
                        let mut active = playback.lock().await;
                        let found = active
                            .as_ref()
                            .filter(|track| {
                                event["playlist_entry_id"].as_i64() == Some(track.entry)
                                    && generation.load(Ordering::SeqCst) == track.generation
                            })
                            .cloned();
                        if found.is_some() {
                            *active = None;
                        }
                        found
                    };
                    if let Some(track) = track {
                        let snapshot = state.lock().await.clone();
                        let broken = event["reason"] == "error"
                            || (event["reason"] == "eof"
                                && snapshot.duration > 0.0
                                && snapshot.position + 2.0 < snapshot.duration);
                        if broken {
                            diagnostics.record(Stage::Playback, Outcome::Failed, None);
                        }
                        if broken && crate::local_music::valid_id(&track.id) {
                            let _ = app.emit(
                                "player-error",
                                "Не удалось дочитать локальный файл. Проверь доступность диска.",
                            );
                        } else if broken {
                            let _ = app.emit(
                                "player-stream-error",
                                StreamFailure {
                                    id: track.id.clone(),
                                    generation: track.generation,
                                    position: snapshot.position,
                                },
                            );
                        } else if event["reason"] == "eof" && !sleep.on_track_end(&app).await {
                            let _ = app.emit("track-ended", ());
                        }
                    }
                }
                if event["event"] != "property-change" {
                    continue;
                }
                let mut s = state.lock().await;
                let value = &event["data"];
                match event["name"].as_str().unwrap_or_default() {
                    "time-pos" => s.position = value.as_f64().unwrap_or(0.0),
                    "duration" => s.duration = value.as_f64().unwrap_or(0.0),
                    "pause" => s.paused = value.as_bool().unwrap_or(true),
                    "volume" => s.volume = value.as_f64().unwrap_or(70.0),
                    "paused-for-cache" => s.buffering = value.as_bool().unwrap_or(false),
                    "idle-active" => s.active = !value.as_bool().unwrap_or(true),
                    _ => (),
                }
                if matches!(
                    event["name"].as_str(),
                    Some("pause" | "paused-for-cache" | "idle-active")
                ) {
                    visualizer_wake.notify_one();
                }
                // A paused seek can be the final time-pos event. Throttling it away
                // would leave the UI at its old position until playback resumes.
                if s.paused
                    || last_emit.elapsed() > Duration::from_millis(250)
                    || event["name"] != "time-pos"
                {
                    let _ = app.emit("player-state", s.clone());
                    last_emit = Instant::now();
                }
            }
            if epoch.load(Ordering::SeqCst) == instance {
                *playback.lock().await = None;
                let mut s = state.lock().await;
                let unexpected = s.active;
                s.active = false;
                s.paused = true;
                s.buffering = false;
                let _ = app.emit("player-state", s.clone());
                visualizer_wake.notify_one();
                if unexpected {
                    diagnostics.record(Stage::Playback, Outcome::Failed, Some("плеер завершился"));
                    let _ = app.emit(
                        "player-error",
                        "Аудиоплеер завершился. Нажми воспроизведение, чтобы запустить его снова.",
                    );
                }
            }
        });
        Ok(())
    }

    async fn send(&self, command: Value) -> Result<Value, String> {
        let mut stream = crate::mpv_ipc::connect(&self.socket)
            .await
            .map_err(|_| "Плеер ещё не запущен".to_owned())?;
        stream
            .write_all(format!("{}\n", json!({"command":command,"request_id":1})).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(stream).lines();
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
                if value["request_id"] == 1 {
                    if value["error"] != "success" {
                        return Err(format!("mpv: {}", value["error"]));
                    }
                    return Ok(value["data"].clone());
                }
            }
            Err("Соединение с плеером закрыто".to_owned())
        })
        .await
        .map_err(|_| "mpv не ответил вовремя".to_owned())?
    }

    pub async fn resolve(&self, id: &str) -> Result<String, String> {
        Ok(self.resolve_stream(id, false).await?.url)
    }
    async fn resolve_stream(&self, id: &str, retry: bool) -> Result<StreamInfo, String> {
        // A click on the prefetched song joins that extraction instead of starting
        // a second yt-dlp process and competing for network/CPU.
        let flight = {
            let mut flights = self.flights.lock().await;
            flights.retain(|_, gate| Arc::strong_count(gate) > 1);
            flights
                .entry(id.to_owned())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = flight.lock().await;
        self.resolve_once(id, retry).await
    }

    async fn resolve_once(&self, id: &str, retry: bool) -> Result<StreamInfo, String> {
        let epoch = self.session_epoch.load(Ordering::SeqCst);
        if !valid_video_id(id) {
            return Err("Некорректный ID трека".into());
        }
        {
            let mut cache = self.streams.lock().await;
            cache.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(1200));
            if retry {
                cache.remove(id);
            }
            if let Some((_, stream)) = cache.get(id) {
                return Ok(stream.clone());
            }
        }
        let _permit = self
            .extraction_limit
            .acquire()
            .await
            .map_err(|e| e.to_string())?;
        let mut output = self
            .extract(
                id,
                retry && self.directory.join("cookies.txt").exists(),
                retry,
            )
            .await?;
        if !output.status.success() && !retry && self.directory.join("cookies.txt").exists() {
            let error = String::from_utf8_lossy(&output.stderr).to_lowercase();
            if [
                "sign in",
                "confirm",
                "private",
                "age",
                "403",
                "not available",
            ]
            .iter()
            .any(|word| error.contains(word))
            {
                output = self.extract(id, true, retry).await?;
            }
        }
        if !output.status.success() {
            // Keep extractor output out of logs: it can contain signed URLs and account details.
            let stderr = String::from_utf8_lossy(&output.stderr);
            let reason = if stderr.contains("Sign in") || stderr.contains("confirm") {
                "YouTube требует вход или проверку. Обнови сессию в настройках."
            } else if stderr.contains("403") {
                "YouTube отклонил поток (403). Попробуй обновить yt-dlp или сессию."
            } else {
                "Не удалось получить аудио от YouTube. Проверь сеть и актуальность yt-dlp."
            };
            return Err(reason.into());
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|_| "yt-dlp вернул некорректный ответ".to_owned())?;
        let url = value["url"]
            .as_str()
            .ok_or("YouTube не вернул адрес потока")?
            .to_owned();
        if !valid_media_url(&url) {
            return Err("Неожиданный источник аудиопотока".into());
        }
        let headers = value["http_headers"]
            .as_object()
            .map(|headers| {
                headers
                    .iter()
                    .filter(|(key, _)| {
                        ["accept", "accept-language", "referer", "origin"]
                            .contains(&key.to_ascii_lowercase().as_str())
                    })
                    .filter_map(|(key, value)| {
                        value
                            .as_str()
                            .filter(|v| !v.contains(['\r', '\n']))
                            .map(|v| format!("{key}: {v}"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let user_agent = value["http_headers"]["User-Agent"]
            .as_str()
            .filter(|v| !v.contains(['\r', '\n']))
            .unwrap_or(crate::api::UA)
            .to_owned();
        let stream = StreamInfo {
            url,
            headers,
            user_agent,
        };
        let mut cache = self.streams.lock().await;
        if epoch != self.session_epoch.load(Ordering::SeqCst) {
            return Err("Сессия изменилась. Запусти трек снова.".into());
        }
        if cache.len() >= 32 {
            if let Some(oldest) = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(id, _)| id.clone())
            {
                cache.remove(&oldest);
            }
        }
        cache.insert(id.to_owned(), (Instant::now(), stream.clone()));
        Ok(stream)
    }

    async fn extract(
        &self,
        id: &str,
        authenticated: bool,
        retry: bool,
    ) -> Result<std::process::Output, String> {
        let mut cmd = crate::runtime::command("yt-dlp")?;
        let javascript = format!("node:{}", crate::runtime::resolve("node")?.display());
        cmd.args([
            "--ignore-config",
            "--no-plugin-dirs",
            "--no-remote-components",
            "--no-playlist",
            "--skip-download",
            "--no-warnings",
            "--no-progress",
            "--socket-timeout",
            "12",
            "--retries",
            "1",
            "--no-js-runtimes",
            "--js-runtimes",
            &javascript,
            "-f",
            if retry {
                "bestaudio[ext=m4a]/bestaudio/best"
            } else {
                "bestaudio/best"
            },
            "--dump-single-json",
        ])
        .kill_on_drop(true);
        // yt-dlp saves its cookie jar on exit. Give each extraction its own short-lived
        // copy, so prefetch cannot recreate the main session after the user logs out.
        let raw = if authenticated {
            std::fs::read(self.directory.join("cookies.txt")).ok()
        } else {
            None
        };
        let _cookie_snapshot = if let Some(raw) = raw {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = self
                .directory
                .join(format!("stream-session-{}-{stamp}.txt", std::process::id()));
            crate::api::private_write(&path, &raw)?;
            cmd.arg("--cookies").arg(&path);
            Some(CookieSnapshot(path))
        } else {
            None
        };
        cmd.arg(format!("https://music.youtube.com/watch?v={id}"));
        crate::process::output(cmd, Duration::from_secs(30), 8 * 1024 * 1024).await
    }

    fn claim_recovery(&self, generation: u64) -> bool {
        generation == self.generation.load(Ordering::SeqCst)
            && self.recovered_generation.swap(generation, Ordering::SeqCst) != generation
    }

    async fn load_confirmed(&self, stream: &StreamInfo) -> Result<i64, String> {
        self.diagnostics
            .record(Stage::OpenAudio, Outcome::Started, None);
        let result = self.load_inner(stream).await;
        self.diagnostics.record(
            Stage::OpenAudio,
            if result.is_ok() {
                Outcome::Ready
            } else {
                Outcome::Failed
            },
            result.as_ref().err().map(String::as_str),
        );
        result
    }
    async fn load_inner(&self, stream: &StreamInfo) -> Result<i64, String> {
        self.send(json!(["set_property", "user-agent", stream.user_agent]))
            .await?;
        self.send(json!([
            "set_property",
            "referrer",
            "https://music.youtube.com/"
        ]))
        .await?;
        self.send(json!([
            "set_property",
            "http-header-fields",
            stream.headers
        ]))
        .await?;
        let mut connection = crate::mpv_ipc::connect(&self.socket)
            .await
            .map_err(|e| e.to_string())?;
        connection
            .write_all(
                format!(
                    "{}\n",
                    json!({"command":["loadfile",stream.url,"replace"],"request_id":77})
                )
                .as_bytes(),
            )
            .await
            .map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(connection).lines();
        tokio::time::timeout(Duration::from_secs(18), async {
            let mut wanted = None;
            let mut current = None;
            let mut loaded = None;
            let mut failures = Vec::new();
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if event["request_id"] == 77 {
                    if event["error"] != "success" {
                        return Err("mpv отклонил команду загрузки".to_owned());
                    }
                    wanted = event["data"]["playlist_entry_id"].as_i64();
                }
                match event["event"].as_str() {
                    Some("start-file") => current = event["playlist_entry_id"].as_i64(),
                    Some("file-loaded") => loaded = current,
                    Some("end-file") if event["reason"] == "error" => {
                        if let Some(id) = event["playlist_entry_id"].as_i64() {
                            failures.push(id);
                        }
                    }
                    _ => (),
                }
                if let Some(id) = wanted {
                    if failures.contains(&id) {
                        return Err("YouTube не отдал рабочий аудиопоток".into());
                    }
                    if loaded == Some(id) {
                        return Ok(id);
                    }
                }
            }
            Err("Соединение с mpv закрыто".into())
        })
        .await
        .map_err(|_| "Аудиосервер не ответил вовремя".to_owned())?
    }

    async fn start_stream(
        &self,
        id: &str,
        generation: u64,
        retry: bool,
        position: f64,
    ) -> Result<(), String> {
        let changed = self.play_change.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.diagnostics.record(
            if retry {
                Stage::Recovery
            } else {
                Stage::Resolve
            },
            Outcome::Started,
            None,
        );
        let result = tokio::select! { stream=self.resolve_stream(id, retry)=>stream, _=changed=>return Ok(()) };
        self.diagnostics.record(
            Stage::Resolve,
            if result.is_ok() {
                Outcome::Ready
            } else {
                Outcome::Failed
            },
            result.as_ref().err().map(String::as_str),
        );
        let stream = result?;
        let _guard = self.transition.lock().await;
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        let changed = self.play_change.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        *self.playback.lock().await = None;
        self.send(json!(["set_property", "pause", true])).await?;
        let entry =
            tokio::select! { entry=self.load_confirmed(&stream)=>entry?, _=changed=>return Ok(()) };
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        *self.playback.lock().await = Some(ActivePlayback {
            id: id.to_owned(),
            generation,
            entry,
        });
        if position.is_finite() && position > 0.0 {
            self.send(json!(["seek", position, "absolute+exact"]))
                .await?;
        }
        self.send(json!(["set_property", "pause", false])).await?;
        Ok(())
    }

    pub async fn play(
        &self,
        app: &AppHandle,
        id: String,
        position: f64,
        automatic: bool,
    ) -> Result<(), String> {
        if !valid_video_id(&id) {
            return Err("Некорректный ID трека".into());
        }
        let position = resume_position(position)?;
        let Some(generation) = self.sleep.begin_play(automatic, &self.generation).await else {
            return Ok(());
        };
        self.play_change.notify_waiters();
        self.current_track.send_replace(id.clone());
        self.ensure(app).await?;
        self.reconcile_audio(app).await?;
        {
            let _guard = self.transition.lock().await;
            if generation != self.generation.load(Ordering::SeqCst) {
                return Ok(());
            }
            *self.playback.lock().await = None;
            self.send(json!(["stop"])).await?;
        }
        match self.start_stream(&id, generation, false, position).await {
            Ok(()) => Ok(()),
            Err(_) if self.claim_recovery(generation) => {
                self.start_stream(&id, generation, true, position).await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn play_local(
        &self,
        app: &AppHandle,
        id: String,
        path: PathBuf,
        position: f64,
        automatic: bool,
    ) -> Result<(), String> {
        if !crate::local_music::valid_id(&id) {
            return Err("Некорректный локальный трек".into());
        }
        let position = resume_position(position)?;
        let url = reqwest::Url::from_file_path(path)
            .map_err(|_| "Некорректный путь аудиофайла")?
            .to_string();
        let Some(generation) = self.sleep.begin_play(automatic, &self.generation).await else {
            return Ok(());
        };
        self.play_change.notify_waiters();
        self.current_track.send_replace(id.clone());
        self.ensure(app).await?;
        self.reconcile_audio(app).await?;
        let _guard = self.transition.lock().await;
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        *self.playback.lock().await = None;
        self.send(json!(["stop"])).await?;
        self.send(json!(["set_property", "pause", true])).await?;
        let changed = self.play_change.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        let stream = StreamInfo {
            url,
            headers: vec![],
            user_agent: String::new(),
        };
        let entry = tokio::select! {result=self.load_confirmed(&stream)=>result.map_err(|_|"mpv не смог открыть локальный аудиофайл. Проверь файл и подключение диска.".to_owned())?, _=changed=>return Ok(())};
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        *self.playback.lock().await = Some(ActivePlayback {
            id,
            generation,
            entry,
        });
        if position > 0.0 {
            self.send(json!(["seek", position, "absolute+exact"]))
                .await?;
        }
        self.send(json!(["set_property", "pause", false])).await?;
        Ok(())
    }

    pub async fn recover(&self, id: String, generation: u64, position: f64) -> Result<(), String> {
        if generation != self.generation.load(Ordering::SeqCst)
            || self.current_track.borrow().as_str() != id
        {
            return Ok(());
        }
        self.streams.lock().await.remove(&id);
        if !self.claim_recovery(generation) {
            return Err("YouTube прервал поток повторно. Попробуй запустить трек ещё раз.".into());
        }
        self.start_stream(&id, generation, true, position).await
    }

    pub async fn snapshot(&self) -> PlayerState {
        self.state.lock().await.clone()
    }

    pub async fn audio_outputs(
        &self,
        app: &AppHandle,
    ) -> Result<crate::audio_output::Snapshot, String> {
        self.ensure(app).await?;
        self.reconcile_audio(app).await
    }
    async fn reconcile_audio(
        &self,
        app: &AppHandle,
    ) -> Result<crate::audio_output::Snapshot, String> {
        let _guard = self.output.gate.lock().await;
        self.audio_snapshot_locked(app).await
    }
    async fn audio_snapshot_locked(
        &self,
        app: &AppHandle,
    ) -> Result<crate::audio_output::Snapshot, String> {
        let devices = crate::audio_output::devices(
            self.send(json!(["get_property", "audio-device-list"]))
                .await?,
        )?;
        let preferred = self.output.preferred().await;
        let selected = crate::audio_output::desired(&preferred, &devices).to_owned();
        let current = self.send(json!(["get_property", "audio-device"])).await?;
        if current.as_str() != Some(&selected) {
            self.send(json!(["set_property", "audio-device", selected]))
                .await?;
        }
        let silent = self
            .send(json!(["get_property", "current-ao"]))
            .await
            .ok()
            .and_then(|v| v.as_str().map(|s| s == "null"))
            .unwrap_or(false)
            && self.state.lock().await.active;
        let snapshot = crate::audio_output::Snapshot {
            fallback: preferred != selected,
            preferred,
            selected,
            silent,
            devices,
        };
        let _ = app.emit("audio-output-state", snapshot.clone());
        Ok(snapshot)
    }
    pub async fn set_audio_output(
        &self,
        app: &AppHandle,
        name: String,
    ) -> Result<crate::audio_output::Snapshot, String> {
        self.ensure(app).await?;
        let _guard = self.output.gate.lock().await;
        let devices = crate::audio_output::devices(
            self.send(json!(["get_property", "audio-device-list"]))
                .await?,
        )?;
        if !devices.iter().any(|d| d.name == name) {
            return Err("Этот аудиовыход больше недоступен. Обнови список устройств.".into());
        }
        let previous = self.send(json!(["get_property", "audio-device"])).await?;
        self.send(json!(["set_property", "audio-device", name]))
            .await?;
        if let Err(error) = self.output.save(name).await {
            let _ = self
                .send(json!(["set_property", "audio-device", previous]))
                .await;
            return Err(error);
        }
        self.audio_snapshot_locked(app).await
    }
    pub async fn set_visualizer(&self, enabled: bool) -> Result<(), String> {
        // Serialize configuration with mpv startup AND other filter changes.
        let mut child = self.child.lock().await;
        if self.visualizer.load(Ordering::SeqCst) == enabled {
            return Ok(());
        }
        let alive = child
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if alive && crate::mpv_ipc::present(&self.socket) {
            if enabled {
                self.send(json!(["af", "add", crate::visualizer::FILTER]))
                    .await?;
            } else {
                self.send(json!(["af", "remove", "@neisora"])).await?;
            }
        }
        self.visualizer.store(enabled, Ordering::SeqCst);
        self.visualizer_wake.notify_one();
        Ok(())
    }
    pub async fn pause_for_sleep(&self) -> Result<(), String> {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.play_change.notify_waiters();
        let _guard = self.transition.lock().await;
        if generation != self.generation.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(active) = self.playback.lock().await.as_mut() {
            active.generation = generation;
        }
        if crate::mpv_ipc::present(&self.socket) {
            self.send(json!(["set_property", "pause", true])).await?;
        }
        Ok(())
    }
    pub async fn control(&self, action: String, value: Option<f64>) -> Result<(), String> {
        if action == "toggle" {
            self.sleep.allow_play(false).await;
        }
        if action == "seek" {
            let sequence = self.seek_serial.fetch_add(1, Ordering::SeqCst) + 1;
            let generation = self.generation.load(Ordering::SeqCst);
            let _guard = self.seek_lock.lock().await;
            let _transition = self.transition.lock().await;
            if sequence != self.seek_serial.load(Ordering::SeqCst)
                || generation != self.generation.load(Ordering::SeqCst)
            {
                return Ok(());
            }
            let target = value.filter(|v| v.is_finite()).unwrap_or(0.0).max(0.0);
            self.send(json!(["seek", target, "absolute+exact"])).await?;
            return Ok(());
        }
        if action == "volume" || action == "volume_commit" {
            let volume = value
                .filter(|v| v.is_finite())
                .unwrap_or(70.0)
                .clamp(0.0, 100.0);
            self.state.lock().await.volume = volume;
            if action == "volume_commit" {
                crate::api::private_write(
                    &self.directory.join("volume.json"),
                    volume.to_string().as_bytes(),
                )?;
            }
            if !crate::mpv_ipc::present(&self.socket) {
                return Ok(());
            }
        }
        let command = match action.as_str() {
            "toggle" => json!(["cycle", "pause"]),
            "pause" => json!(["set_property", "pause", true]),
            "seek" => json!(["seek", value.unwrap_or(0.0).max(0.0), "absolute+exact"]),
            "volume" | "volume_commit" => json!([
                "set_property",
                "volume",
                value.unwrap_or(70.0).clamp(0.0, 100.0)
            ]),
            "stop" => {
                self.generation.fetch_add(1, Ordering::SeqCst);
                self.play_change.notify_waiters();
                self.current_track.send_replace(String::new());
                json!(["stop"])
            }
            _ => return Err("Неизвестная команда плеера".into()),
        };
        self.send(command).await?;
        Ok(())
    }

    pub async fn clear(&self) {
        self.current_track.send_replace(String::new());
        self.play_change.notify_waiters();
        self.session_epoch.fetch_add(1, Ordering::SeqCst);
        self.streams.lock().await.clear();
    }

    pub async fn shutdown(&self) {
        self.mpv_epoch.fetch_add(1, Ordering::SeqCst);
        crate::process::terminate_all();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.play_change.notify_waiters();
        self.current_track.send_replace(String::new());
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        crate::mpv_ipc::cleanup(&self.socket);
    }

    pub async fn prefetch(&self, id: &str) -> Result<(), String> {
        let mut track = self.current_track.subscribe();
        let resolved = self.resolve(id);
        tokio::pin!(resolved);
        loop {
            tokio::select! {
                result=&mut resolved=>return result.map(|_|()),
                _=track.changed()=>{if track.borrow().as_str()!=id{return Ok(());}}
            }
        }
    }
}

struct CookieSnapshot(PathBuf);
impl Drop for CookieSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        crate::mpv_ipc::cleanup(&self.socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resume_position_rejects_invalid_values() {
        for value in [-1.0, f64::NAN, f64::INFINITY, 604801.0] {
            assert!(resume_position(value).is_err());
        }
        assert_eq!(resume_position(72.5).unwrap(), 72.5);
        assert_eq!(resume_position(0.0).unwrap(), 0.0);
    }
    #[cfg(unix)]
    use tokio::process::Command;
    #[test]
    fn rejects_command_payloads_and_non_google_media() {
        assert!(valid_video_id("cK3NMZAUKGw"));
        for id in [
            "--exec=sh",
            "../secret",
            "https://x/y",
            "abc;touch x",
            "cK3NMZAUKG\n",
        ] {
            assert!(!valid_video_id(id));
        }
        assert!(valid_media_url(
            "https://rr1.googlevideo.com/videoplayback?sig=test"
        ));
        for url in [
            "file:///etc/passwd",
            "http://rr1.googlevideo.com/x",
            "https://googlevideo.com.evil.org/x",
            "https://localhost/x",
            "https://user:pass@rr1.googlevideo.com/x",
            "https://rr1.googlevideo.com:123/x",
        ] {
            assert!(!valid_media_url(url));
        }
    }
    #[test]
    fn recovery_is_once_per_listen_and_ignores_superseded_tracks() {
        let player = Player::new(
            std::env::temp_dir(),
            Arc::new(crate::sleep_timer::SleepTimer::default()),
        );
        player.generation.store(1, Ordering::SeqCst);
        assert!(player.claim_recovery(1));
        assert!(!player.claim_recovery(1));
        player.generation.store(2, Ordering::SeqCst);
        assert!(!player.claim_recovery(1));
        assert!(player.claim_recovery(2));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn load_waits_for_real_audio_and_reports_failed_open() {
        let dir = std::env::temp_dir().join(format!("vesper-mpv-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let player = Player::new(
            dir.clone(),
            Arc::new(crate::sleep_timer::SleepTimer::default()),
        );
        let mut child = Command::new("mpv")
            .args([
                "--no-config",
                "--idle=yes",
                "--ao=null",
                "--no-video",
                "--really-quiet",
            ])
            .arg(format!("--input-ipc-server={}", player.socket.display()))
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        drop(connect_mpv(&mut child, &player.socket).await.unwrap());
        let invalid = StreamInfo {
            url: dir.join("missing.wav").to_string_lossy().into_owned(),
            user_agent: crate::api::UA.into(),
            headers: vec!["Accept: */*".into()],
        };
        assert!(player.load_confirmed(&invalid).await.is_err());
        // Two seconds of silent PCM, generated without a network or audio device.
        let size = 8000u32 * 2 * 2;
        let mut wav = b"RIFF".to_vec();
        wav.extend((size + 36).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(8000u32.to_le_bytes());
        wav.extend(16000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(size.to_le_bytes());
        wav.resize(44 + size as usize, 0);
        let file = dir.join("silent.wav");
        std::fs::write(&file, wav).unwrap();
        let valid = StreamInfo {
            url: file.to_string_lossy().into_owned(),
            ..invalid
        };
        assert!(player.load_confirmed(&valid).await.unwrap() > 0);
        child.kill().await.unwrap();
        child.wait().await.unwrap();
        drop(player);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
