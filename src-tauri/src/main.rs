#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod api;
mod appearance;
mod audio_output;
mod catalog;
mod desktop_actions;
mod diagnostics;
mod history;
mod instance;
mod karaoke;
mod library;
mod local_music;
mod lyric_choices;
mod lyrics;
mod media;
mod model;
mod mpv_ipc;
mod network;
mod platform;
mod player;
mod playlists;
mod process;
mod runtime;
mod session;
mod sharing;
mod sleep_timer;
mod visualizer;
mod wallpaper;

use api::Api;
use model::{Feed, Queue};
use player::Player;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

struct Backend {
    api: Api,
    player: Player,
    appearance: appearance::Appearance,
    lyrics: lyrics::LyricsCache,
    lyric_choices: lyric_choices::Store,
    karaoke: karaoke::Cache,
    catalog: catalog::Cache,
    history: history::History,
    session: session::Session,
    sleep: Arc<sleep_timer::SleepTimer>,
    media: media::Media,
    local: Arc<local_music::LocalMusic>,
}
type BackendState<'a> = State<'a, Arc<Backend>>;

fn release_login(window: &tauri::WebviewWindow) {
    // A cached WebKit context can retain the Google page's process after the window
    // closes. The login context is isolated from the app; release its process now.
    #[cfg(target_os = "linux")]
    let _ = window.with_webview(|webview| {
        use webkit2gtk::WebViewExt;
        webview.inner().terminate_web_process();
    });
    let _ = window.destroy();
}

fn login_url_allowed(url: &reqwest::Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && (host == "google.com"
            || host.ends_with(".google.com")
            || host == "youtube.com"
            || host.ends_with(".youtube.com"))
}

#[derive(Serialize)]
struct Status {
    version: &'static str,
    authenticated: bool,
    browser: Vec<String>,
    mpv: bool,
    extractor: bool,
    volume: f64,
    #[serde(rename = "safeRenderer")]
    safe_renderer: bool,
}

#[tauri::command]
async fn status(state: BackendState<'_>) -> Result<Status, String> {
    #[cfg(unix)]
    let home = dirs::home_dir().ok_or("Не найден домашний каталог")?;
    #[cfg(unix)]
    let browser = [
        (
            "firefox",
            vec![
                ".mozilla/firefox",
                ".config/mozilla/firefox",
                "snap/firefox/common/.mozilla/firefox",
                ".var/app/org.mozilla.firefox/.mozilla/firefox",
            ],
        ),
        (
            "brave",
            vec![
                ".config/BraveSoftware/Brave-Browser",
                ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
            ],
        ),
        (
            "chromium",
            vec![
                ".config/chromium",
                "snap/chromium/common/chromium",
                ".var/app/org.chromium.Chromium/config/chromium",
            ],
        ),
        (
            "chrome",
            vec![
                ".config/google-chrome",
                ".var/app/com.google.Chrome/config/google-chrome",
            ],
        ),
        ("vivaldi", vec![".config/vivaldi"]),
    ]
    .into_iter()
    .filter(|(_, paths)| paths.iter().any(|p| home.join(p).is_dir()))
    .map(|(n, _)| n.to_owned())
    .collect();
    #[cfg(windows)]
    let browser = [
        ("firefox", "APPDATA", "Mozilla/Firefox"),
        (
            "brave",
            "LOCALAPPDATA",
            "BraveSoftware/Brave-Browser/User Data",
        ),
        ("chromium", "LOCALAPPDATA", "Chromium/User Data"),
        ("chrome", "LOCALAPPDATA", "Google/Chrome/User Data"),
        ("vivaldi", "LOCALAPPDATA", "Vivaldi/User Data"),
    ]
    .into_iter()
    .filter(|(_, variable, path)| {
        std::env::var_os(variable)
            .is_some_and(|root| std::path::Path::new(&root).join(path).is_dir())
    })
    .map(|(name, _, _)| name.to_owned())
    .collect();
    Ok(Status {
        version: env!("CARGO_PKG_VERSION"),
        authenticated: state.api.authenticated().await,
        browser,
        mpv: runtime::resolve("mpv").is_ok(),
        extractor: runtime::resolve("yt-dlp").is_ok() && runtime::resolve("node").is_ok(),
        volume: state.player.snapshot().await.volume,
        safe_renderer: runtime::safe_renderer(),
    })
}

#[tauri::command]
async fn browse(endpoint: Value, state: BackendState<'_>) -> Result<Feed, String> {
    let id = endpoint
        .get("browseEndpoint")
        .unwrap_or(&endpoint)
        .get("browseId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut feed = state.api.browse(endpoint).await?;
    if let Some(collection) = &mut feed.collection {
        collection.browse_id = id;
    }
    Ok(feed)
}
#[tauri::command]
async fn search(query: String, state: BackendState<'_>) -> Result<Feed, String> {
    state.api.search(query).await
}
#[tauri::command]
async fn queue(endpoint: Value, state: BackendState<'_>) -> Result<Queue, String> {
    state.api.next(endpoint).await
}
#[tauri::command]
async fn refresh_queue(
    context: Value,
    endpoint: Value,
    signals: Value,
    state: BackendState<'_>,
) -> Result<Option<Queue>, String> {
    state.api.refresh_queue(context, endpoint, signals).await
}
#[tauri::command]
async fn set_visualizer(enabled: bool, state: BackendState<'_>) -> Result<(), String> {
    state.player.set_visualizer(enabled).await
}
#[tauri::command]
async fn get_lyrics(
    id: String,
    title: String,
    artist: String,
    duration: f64,
    music_video: Option<bool>,
    refresh: Option<bool>,
    state: BackendState<'_>,
) -> Result<lyrics::Lyrics, String> {
    if refresh.unwrap_or(false) {
        state.lyrics.invalidate(&id).await;
    }
    let recording = state
        .catalog
        .get(
            &state.api,
            &title,
            &artist,
            duration,
            music_video.unwrap_or(false),
        )
        .await;
    state
        .lyrics
        .get_with_recording(&state.api, id, title, artist, duration, recording)
        .await
}
#[tauri::command]
async fn lyric_versions(
    title: String,
    artist: String,
    query: String,
    state: BackendState<'_>,
) -> Result<Vec<lyrics::Candidate>, String> {
    state
        .lyrics
        .versions(&state.api, title, artist, query)
        .await
}
#[tauri::command]
async fn preview_lyric_version(
    record_id: u64,
    state: BackendState<'_>,
) -> Result<lyric_choices::Selection, String> {
    state.lyrics.version(&state.api, record_id).await
}
#[tauri::command]
async fn get_lyric_choice(
    id: String,
    state: BackendState<'_>,
) -> Result<Option<lyric_choices::Selection>, String> {
    let mut selection = state.lyric_choices.get(&id).await?;
    if let Some(value) = selection.as_ref().filter(|v| v.lyrics.is_none()) {
        if let Ok(loaded) = state.lyrics.version(&state.api, value.record.id).await {
            let _ = state
                .lyric_choices
                .cache_if_current(&id, loaded.clone())
                .await;
            selection = Some(loaded);
        }
    }
    Ok(selection)
}
#[tauri::command]
async fn select_lyric_version(
    id: String,
    record_id: Option<u64>,
    state: BackendState<'_>,
) -> Result<Option<lyric_choices::Selection>, String> {
    if !lyric_choices::valid_id(&id) {
        return Err("Некорректный ID трека".into());
    }
    let selection = if let Some(record) = record_id {
        Some(state.lyrics.version(&state.api, record).await?)
    } else {
        None
    };
    state.lyric_choices.set(id, selection.clone()).await?;
    Ok(selection)
}
#[tauri::command]
async fn get_word_lyrics(
    id: String,
    title: String,
    artist: String,
    duration: f64,
    music_video: Option<bool>,
    refresh: Option<bool>,
    state: BackendState<'_>,
) -> Result<Option<lyrics::Lyrics>, String> {
    if refresh.unwrap_or(false) {
        state.karaoke.invalidate(&id).await;
    }
    let recording = state
        .catalog
        .get(
            &state.api,
            &title,
            &artist,
            duration,
            music_video.unwrap_or(false),
        )
        .await;
    let (title, artist, duration) = recording
        .map(|r| (r.title, r.artist, r.duration))
        .unwrap_or((title, artist, duration));
    Ok(state
        .karaoke
        .get(&state.api, id, title, artist, duration)
        .await)
}
#[tauri::command]
async fn get_recording(
    title: String,
    artist: String,
    duration: f64,
    music_video: Option<bool>,
    state: BackendState<'_>,
) -> Result<Option<catalog::Recording>, String> {
    Ok(state
        .catalog
        .get(
            &state.api,
            &title,
            &artist,
            duration,
            music_video.unwrap_or(false),
        )
        .await)
}
#[tauri::command]
async fn get_appearance(state: BackendState<'_>) -> Result<appearance::AppearanceState, String> {
    Ok(state.appearance.state().await)
}
#[tauri::command]
async fn set_appearance(
    preferences: appearance::Preferences,
    state: BackendState<'_>,
) -> Result<(), String> {
    state.appearance.save(preferences).await
}
#[tauri::command]
async fn wallpaper_theme(state: BackendState<'_>) -> Result<appearance::Theme, String> {
    state.appearance.wallpaper_theme().await
}
#[tauri::command]
fn share_link(target: sharing::Target) -> Result<String, String> {
    sharing::url(target)
}
#[tauri::command]
async fn copy_share_link(target: sharing::Target, app: AppHandle) -> Result<(), String> {
    desktop_actions::copy(&app, sharing::url(target)?).await
}
#[tauri::command]
async fn open_share_link(target: sharing::Target, app: AppHandle) -> Result<(), String> {
    desktop_actions::open(&app, sharing::url(target)?).await
}
async fn diagnostic_data(state: &BackendState<'_>) -> Value {
    let player = state.player.snapshot().await;
    serde_json::json!({"schema":1,"version":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "package":if std::env::var_os("APPIMAGE").is_some(){"appimage"}else{"desktop"},
        "components":{"mpv":runtime::resolve("mpv").is_ok(),"extractor":runtime::resolve("yt-dlp").is_ok(),"javascript":runtime::resolve("node").is_ok()},
        "graphics":{"safeRenderer":runtime::safe_renderer()},
        "player":{"active":player.active,"paused":player.paused,"buffering":player.buffering},
        "events":state.player.diagnostics.snapshot()})
}
#[tauri::command]
async fn diagnostic_report(state: BackendState<'_>) -> Result<Value, String> {
    Ok(diagnostic_data(&state).await)
}
#[tauri::command]
async fn copy_diagnostic_report(state: BackendState<'_>, app: AppHandle) -> Result<Value, String> {
    let report = diagnostic_data(&state).await;
    desktop_actions::copy(
        &app,
        serde_json::to_string_pretty(&report).map_err(|_| "Не удалось подготовить отчёт")?,
    )
    .await?;
    Ok(report)
}
#[tauri::command]
fn clear_diagnostics(state: BackendState<'_>) {
    state.player.diagnostics.clear();
}
#[tauri::command]
async fn audio_outputs(
    state: BackendState<'_>,
    app: AppHandle,
) -> Result<audio_output::Snapshot, String> {
    state.player.audio_outputs(&app).await
}
#[tauri::command]
async fn set_audio_output(
    name: String,
    state: BackendState<'_>,
    app: AppHandle,
) -> Result<audio_output::Snapshot, String> {
    state.player.set_audio_output(&app, name).await
}
#[tauri::command]
async fn get_sleep_timer(state: BackendState<'_>) -> Result<sleep_timer::Snapshot, String> {
    Ok(state.sleep.snapshot().await)
}
#[tauri::command]
async fn set_sleep_timer(
    request: sleep_timer::Request,
    state: BackendState<'_>,
    app: AppHandle,
) -> Result<sleep_timer::Snapshot, String> {
    state.sleep.set(&app, request).await
}
#[tauri::command]
async fn playback_session(state: BackendState<'_>) -> Result<Option<String>, String> {
    Ok(state.session.read().await)
}
#[tauri::command]
async fn save_playback_session(
    value: Option<String>,
    state: BackendState<'_>,
) -> Result<(), String> {
    state.session.save(value).await
}
#[tauri::command]
async fn play(
    id: String,
    position: Option<f64>,
    automatic: Option<bool>,
    state: BackendState<'_>,
    app: AppHandle,
) -> Result<(), String> {
    let result = state
        .player
        .play(
            &app,
            id,
            position.unwrap_or(0.0),
            automatic.unwrap_or(false),
        )
        .await;
    state
        .player
        .diagnostics
        .failure(diagnostics::Stage::Playback, result)
}
#[tauri::command]
async fn recover_playback(
    id: String,
    generation: u64,
    position: f64,
    state: BackendState<'_>,
) -> Result<(), String> {
    state.player.recover(id, generation, position).await
}
#[tauri::command]
async fn prefetch(id: String, state: BackendState<'_>) -> Result<(), String> {
    state.player.prefetch(&id).await
}
#[tauri::command]
async fn control(
    action: String,
    value: Option<f64>,
    state: BackendState<'_>,
) -> Result<(), String> {
    let result = state.player.control(action, value).await;
    state
        .player
        .diagnostics
        .failure(diagnostics::Stage::Control, result)
}
#[tauri::command]
async fn like(id: String, liked: bool, state: BackendState<'_>) -> Result<(), String> {
    state.api.like(id, liked).await
}

#[tauri::command]
async fn track_like(id: String, state: BackendState<'_>) -> Result<Option<bool>, String> {
    state.api.track_like(id).await
}
#[tauri::command]
async fn library(category: String, state: BackendState<'_>) -> Result<Feed, String> {
    state.api.library(category).await
}
#[tauri::command]
async fn save_collection(
    browse_id: String,
    saved: bool,
    state: BackendState<'_>,
) -> Result<(), String> {
    state.api.save_collection(browse_id, saved).await
}
#[tauri::command]
async fn playlist_change(
    change: playlists::Change,
    state: BackendState<'_>,
) -> Result<Option<String>, String> {
    state.api.playlist_change(change).await
}
#[tauri::command]
async fn listening_history(state: BackendState<'_>) -> Result<Vec<history::Entry>, String> {
    Ok(state.history.list().await)
}
#[tauri::command]
async fn record_history(
    item: model::Item,
    playback: String,
    state: BackendState<'_>,
) -> Result<(), String> {
    state.history.record(item, playback).await
}
#[tauri::command]
async fn clear_history(state: BackendState<'_>) -> Result<(), String> {
    state.history.clear().await
}
#[tauri::command]
async fn sync_media(mut snapshot: media::Snapshot, state: BackendState<'_>) -> Result<(), String> {
    if local_music::valid_id(&snapshot.id) {
        snapshot.cover = state.local.cover_url(&snapshot.id).unwrap_or_default();
    }
    state.media.update(snapshot)
}

#[tauri::command]
async fn local_status(state: BackendState<'_>) -> Result<local_music::Status, String> {
    Ok(state.local.status())
}
#[tauri::command]
async fn local_library(
    query: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    offset: Option<usize>,
    state: BackendState<'_>,
) -> Result<local_music::Page, String> {
    Ok(state.local.page(
        query.unwrap_or_default(),
        artist.unwrap_or_default(),
        album.unwrap_or_default(),
        offset.unwrap_or(0),
    ))
}
#[tauri::command]
async fn choose_music_folder(
    window: tauri::WebviewWindow,
    state: BackendState<'_>,
) -> Result<bool, String> {
    let _dialog = state
        .local
        .folder_dialog
        .try_lock()
        .map_err(|_| "Выбор папки уже открыт")?;
    if let Some(path) = local_music::choose_folder(window).await? {
        state.local.add_folder(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
#[tauri::command]
async fn remove_music_folder(id: String, state: BackendState<'_>) -> Result<(), String> {
    state.local.remove_folder(&id)
}
#[tauri::command]
async fn scan_local_music(state: BackendState<'_>) -> Result<(), String> {
    state.local.request_scan();
    Ok(())
}
#[tauri::command]
async fn play_local(
    id: String,
    position: Option<f64>,
    automatic: Option<bool>,
    state: BackendState<'_>,
    app: AppHandle,
) -> Result<(), String> {
    let path = state
        .player
        .diagnostics
        .failure(diagnostics::Stage::OpenAudio, state.local.resolve(&id))?;
    let result = state
        .player
        .play_local(
            &app,
            id,
            path,
            position.unwrap_or(0.0),
            automatic.unwrap_or(false),
        )
        .await;
    state
        .player
        .diagnostics
        .failure(diagnostics::Stage::Playback, result)
}
#[tauri::command]
async fn local_artwork(
    id: String,
    large: Option<bool>,
    state: BackendState<'_>,
) -> Result<Option<String>, String> {
    state.local.artwork(id, large.unwrap_or(false)).await
}
#[tauri::command]
async fn local_lyrics(id: String, state: BackendState<'_>) -> Result<lyrics::Lyrics, String> {
    state.local.lyrics(&id)
}

#[tauri::command]
async fn logout(state: BackendState<'_>, app: AppHandle) -> Result<(), String> {
    state.api.logout().await?;
    state.sleep.set(&app, sleep_timer::Request::Off {}).await?;
    state.session.save(None).await?;
    state.player.clear().await;
    state.player.diagnostics.clear();
    process::terminate_active();
    let _ = state.player.control("stop".into(), None).await;
    api::cleanup_temporary_files(&state.api.directory, true);
    if let Some(w) = app.get_webview_window("login") {
        release_login(&w);
    }
    // Clear the login webview profile too, so signing out cannot silently sign back in.
    let profile = state.api.directory.join("login");
    if profile.exists() {
        tokio::fs::remove_dir_all(profile)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn import_browser(browser: String, state: BackendState<'_>) -> Result<(), String> {
    if !["firefox", "brave", "chromium", "chrome", "vivaldi"].contains(&browser.as_str()) {
        return Err("Этот браузер не поддерживается".into());
    }
    let generation = state.api.session_generation();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = state.api.directory.join(format!(
        "browser-session-{}-{stamp}.tmp",
        std::process::id()
    ));
    api::private_write(&path, b"")?;
    let _temporary = api::PrivateTempFile(path.clone());
    // Read only on an explicit UI action. Only YouTube cookies are exported, never the browser's entire cookie jar.
    let script = r#"
import os, sys, http.cookiejar
sys.path.insert(0, sys.argv[3])
from yt_dlp.cookies import extract_cookies_from_browser
os.umask(0o077)
source = extract_cookies_from_browser(sys.argv[1])
target = http.cookiejar.MozillaCookieJar(sys.argv[2])
for c in source:
    if c.domain.lstrip('.') in ('youtube.com', 'music.youtube.com'):
        target.set_cookie(c)
target.save(ignore_discard=True, ignore_expires=True)
"#;
    let mut cmd = runtime::command("python3")?;
    cmd.args(["-c", script, &browser])
        .arg(&path)
        .arg(runtime::resolve("yt-dlp")?)
        .kill_on_drop(true);
    let result = tokio::select! {
        result=crate::process::output(cmd, std::time::Duration::from_secs(40), 256 * 1024)=>result,
        _=state.api.session_changed(generation)=>return Err("Вход отменён: сессия изменилась".into())
    };
    match result {
        Ok(output) if output.status.success() => {
            let raw = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| e.to_string())?;
            let _ = tokio::fs::remove_file(path).await;
            state.api.save_cookies_if_current(&raw, generation).await?;
            state.player.clear().await;
            state.player.diagnostics.clear();
            Ok(())
        }
        _ => {
            let _ = tokio::fs::remove_file(path).await;
            Err("Не удалось импортировать сессию. Войди в YouTube Music в выбранном браузере. Для Chromium может понадобиться разблокированный системный кошелёк.".into())
        }
    }
}

#[tauri::command]
async fn open_login(app: AppHandle, state: BackendState<'_>) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("login") {
        return w.set_focus().map_err(|e| e.to_string());
    }
    let handle = app.clone();
    let generation = state.api.session_generation();
    let directory = state.api.directory.join("login");
    WebviewWindowBuilder::new(&app, "login", WebviewUrl::External("https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fmusic.youtube.com".parse().unwrap()))
        .title("Вход в YouTube Music — neisora").inner_size(520.0, 740.0).data_directory(directory)
        .user_agent(api::UA)
        .on_navigation(move |url| {
            let host = url.host_str().unwrap_or_default();
            let allowed = login_url_allowed(url);
            if allowed && host == "music.youtube.com" {
                let app = handle.clone();
                tauri::async_runtime::spawn(async move {
                    let mut expected = generation;
                    // Let Google's cookie transfer finish before copying the session.
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    for _ in 0..6 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let backend = app.state::<Arc<Backend>>();
                        if backend.api.session_generation() != expected { return; }
                        let Some(w) = app.get_webview_window("login") else { return; };
                        let Ok(cookies) = w.cookies_for_url("https://music.youtube.com".parse().unwrap()) else { continue; };
                        let mut jar = "# Netscape HTTP Cookie File\n".to_owned();
                        for c in cookies {
                            let domain = c.domain().unwrap_or("music.youtube.com");
                            if !["youtube.com", "music.youtube.com"].contains(&domain.trim_start_matches('.')) { continue; }
                            let expiry = c.expires_datetime().map(|d| d.unix_timestamp().to_string()).unwrap_or_else(|| "0".into());
                            jar.push_str(&format!("{}{}\t{}\t{}\t{}\t{}\t{}\t{}\n", if c.http_only().unwrap_or(false) { "#HttpOnly_" } else { "" }, domain, if domain.starts_with('.') { "TRUE" } else { "FALSE" }, c.path().unwrap_or("/"), if c.secure().unwrap_or(false) { "TRUE" } else { "FALSE" }, expiry, c.name(), c.value()));
                        }
                        let Ok(saved) = backend.api.save_cookies_if_current(&jar, expected).await else { continue; };
                        expected = saved;
                        if backend.api.browse(serde_json::json!({"browseId":"FEmusic_mixed_for_you"})).await.is_ok()
                            && backend.api.session_generation() == expected {
                            backend.player.clear().await;
                            let _ = app.emit("auth-updated", ());
                            release_login(&w);
                            return;
                        }
                    }
                });
            }
            allowed
        }).build().map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    runtime::configure_graphics();
    if std::env::args().any(|a| a == "--probe") {
        let result = tokio::runtime::Runtime::new().unwrap().block_on(probe());
        if let Err(e) = result {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let context = tauri::generate_context!();
    let profile = dirs::config_dir()
        .expect("Не найден каталог настроек")
        .join(&context.config().identifier);
    std::fs::create_dir_all(&profile).expect("Не удалось создать каталог профиля");
    let profile = std::fs::canonicalize(profile).expect("Не удалось открыть каталог профиля");
    platform::private_directory(&profile).expect("Не удалось защитить каталог профиля");
    let mut instance = match instance::Instance::acquire(&profile) {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        if let Some(state) = app.try_state::<Arc<Backend>>() {
                            state.player.shutdown().await;
                            api::cleanup_temporary_files(&state.api.directory, true);
                        }
                        app.exit(0);
                    });
                }
            }
        })
        .setup(move |app| {
            #[cfg(target_os = "linux")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.with_webview(|webview| {
                    use webkit2gtk::{SettingsExt, WebViewExt};
                    if let Some(settings) = webview.inner().settings() {
                        settings.set_enable_smooth_scrolling(true);
                    }
                });
            }
            let directory = app.path().app_config_dir()?;
            runtime::init(app.path().resource_dir()?);
            instance.listen(app.handle().clone());
            app.manage(instance);
            let api = Api::new(directory.clone()).map_err(std::io::Error::other)?;
            let sleep = Arc::new(sleep_timer::SleepTimer::default());
            app.manage(Arc::new(Backend {
                api,
                appearance: appearance::Appearance::new(directory.clone()),
                lyrics: lyrics::LyricsCache::default(),
                lyric_choices: lyric_choices::Store::new(directory.clone()),
                karaoke: karaoke::Cache::default(),
                catalog: catalog::Cache::default(),
                history: history::History::new(directory.clone()),
                session: session::Session::new(directory.clone()),
                sleep: sleep.clone(),
                media: media::Media::default(),
                local: Arc::new(local_music::LocalMusic::new(directory.clone())),
                player: Player::new(directory, sleep.clone()),
            }));
            sleep.start(app.handle().clone());
            app.state::<Arc<Backend>>()
                .local
                .start(app.handle().clone());
            app.state::<Arc<Backend>>()
                .media
                .start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            share_link,
            copy_share_link,
            open_share_link,
            diagnostic_report,
            copy_diagnostic_report,
            clear_diagnostics,
            audio_outputs,
            set_audio_output,
            get_sleep_timer,
            set_sleep_timer,
            playback_session,
            save_playback_session,
            status,
            browse,
            search,
            queue,
            refresh_queue,
            get_lyrics,
            get_word_lyrics,
            lyric_versions,
            preview_lyric_version,
            get_lyric_choice,
            select_lyric_version,
            set_visualizer,
            get_appearance,
            get_recording,
            set_appearance,
            wallpaper_theme,
            play,
            prefetch,
            recover_playback,
            control,
            like,
            track_like,
            library,
            save_collection,
            playlist_change,
            listening_history,
            record_history,
            clear_history,
            sync_media,
            local_status,
            local_library,
            choose_music_folder,
            remove_music_folder,
            scan_local_music,
            play_local,
            local_artwork,
            local_lyrics,
            logout,
            import_browser,
            open_login
        ])
        .run(context)
        .expect("Не удалось запустить neisora");
}

async fn probe() -> Result<(), String> {
    use serde_json::json;
    let directory = dirs::config_dir()
        .ok_or("Нет каталога настроек")?
        .join("local.kumite.vesper");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let _lock = instance::ProfileLock::acquire(&directory).map_err(|e| e.to_string())?;
    let api = Api::new(directory.clone())?;
    let start = std::time::Instant::now();
    let home = api.browse(json!({"browseId":"FEmusic_home"})).await?;
    println!(
        "{}",
        json!({"check":"home","ms":start.elapsed().as_millis(),"authenticated":home.authenticated,"sections":home.sections.iter().map(|s|json!({"title":s.title,"items":s.items.len()})).collect::<Vec<_>>()})
    );
    if let Ok(path) = std::env::var("VESPER_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec(&home).unwrap()).map_err(|e| e.to_string())?;
    }
    if home.sections.is_empty() {
        return Err("Главная не содержит подборок".into());
    }
    if home.authenticated {
        let mixes = api
            .browse(json!({"browseId":"FEmusic_mixed_for_you"}))
            .await?;
        println!(
            "{}",
            json!({"check":"personal_mixes","sections":mixes.sections.iter().map(|s|json!({"title":s.title,"items":s.items.len()})).collect::<Vec<_>>()})
        );
        let is_supermix = |i: &&model::Item| {
            let title = i.title.to_lowercase();
            title.contains("supermix") || title.contains("супермикс")
        };
        let mut supermix = mixes
            .sections
            .iter()
            .flat_map(|s| &s.items)
            .find(is_supermix)
            .cloned()
            .or_else(|| {
                home.sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .find(is_supermix)
                    .cloned()
            });
        let mut continuation = home.continuation.clone();
        for _ in 0..8 {
            if supermix.is_some() {
                break;
            }
            let Some(endpoint) = continuation else {
                break;
            };
            let more = api.browse(endpoint).await?;
            supermix = more
                .sections
                .iter()
                .flat_map(|s| &s.items)
                .find(is_supermix)
                .cloned();
            continuation = more.continuation;
        }
        if let Some(item) = supermix {
            let endpoint = if let Some(play) = item.play {
                play
            } else if model::watch_payload(&item.endpoint).is_some() {
                item.endpoint
            } else {
                let detail = api.browse(item.endpoint).await?;
                detail
                    .sections
                    .iter()
                    .flat_map(|s| &s.items)
                    .find_map(|i| {
                        i.play.clone().or_else(|| {
                            model::watch_payload(&i.endpoint).map(|_| i.endpoint.clone())
                        })
                    })
                    .ok_or("Супермикс не вернул команду воспроизведения")?
            };
            let q = api.next(endpoint).await?;
            println!(
                "{}",
                json!({"check":"supermix","items":q.items.len(),"continuation":q.continuation.is_some()})
            );
            if std::env::args().any(|a| a == "--features") {
                let target = q
                    .items
                    .get(q.selected + 1)
                    .ok_or("В супермиксе нет следующего трека")?;
                let refreshed = api
                    .refresh_queue(
                        q.context.clone(),
                        target.endpoint.clone(),
                        serde_json::json!({"videoInteraction":[]}),
                    )
                    .await?;
                let refreshed = refreshed.unwrap_or_else(|| q.clone());
                let selected = refreshed
                    .items
                    .get(refreshed.selected)
                    .map(|t| t.id.as_str());
                let old: Vec<_> = q.items.iter().skip(q.selected + 2).map(|t| &t.id).collect();
                let new: Vec<_> = refreshed
                    .items
                    .iter()
                    .skip(refreshed.selected + 1)
                    .map(|t| &t.id)
                    .collect();
                println!(
                    "{}",
                    json!({"check":"supermix_skip","selected_matches":selected==Some(target.id.as_str()),"queue_replaced":old!=new,"items":refreshed.items.len(),"dynamic":refreshed.dynamic})
                );
                if selected != Some(target.id.as_str()) {
                    return Err("При пропуске выбран другой трек".into());
                }
            }
            if let Some(next) = q.continuation {
                let more = api.next(next).await?;
                println!(
                    "{}",
                    json!({"check":"supermix_continuation","items":more.items.len(),"continuation":more.continuation.is_some()})
                );
            }
        } else {
            println!("{}", json!({"check":"supermix","found":false}));
        }
    }
    if std::env::args().any(|a| a == "--features") {
        let appearance = appearance::Appearance::new(directory.clone());
        let theme = appearance.wallpaper_theme().await?;
        println!(
            "{}",
            json!({"check":"wallpaper","accent":theme.accent,"background":theme.background})
        );
        let cache = lyrics::LyricsCache::default();
        match cache
            .get(
                &api,
                "dQw4w9WgXcQ".into(),
                "Never Gonna Give You Up".into(),
                "Rick Astley".into(),
                213.0,
            )
            .await
        {
            Ok(lyrics) => println!(
                "{}",
                json!({"check":"lyrics","lines":lyrics.lines.len(),"synced":lyrics.synced,"source":lyrics.source})
            ),
            Err(error) => println!("{}", json!({"check":"lyrics","error":error})),
        }
    }
    let search = api.search("Massive Attack Teardrop".into()).await?;
    let track = search
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .find(|i| model::watch_payload(&i.endpoint).is_some())
        .ok_or("Поиск не вернул треков")?;
    println!(
        "{}",
        json!({"check":"search","sections":search.sections.len(),"track":track.title})
    );
    let q = api
        .next(track.play.clone().unwrap_or_else(|| track.endpoint.clone()))
        .await?;
    println!(
        "{}",
        json!({"check":"queue","items":q.items.len(),"continuation":q.continuation.is_some()})
    );
    if q.items.is_empty() {
        return Err("Пустая очередь".into());
    }
    if q.items[q.selected].id != track.id {
        return Err("Очередь пропустила выбранный трек".into());
    }
    if let Some(next) = q.continuation {
        let continued = api.next(next).await?;
        println!(
            "{}",
            json!({"check":"continuation","items":continued.items.len(),"continuation":continued.continuation.is_some()})
        );
    }
    if std::env::args().any(|a| a == "--audio") {
        let player = Player::new(directory, Arc::new(sleep_timer::SleepTimer::default()));
        let start = std::time::Instant::now();
        let url = player.resolve(&q.items[q.selected].id).await?;
        let cold_ms = start.elapsed().as_millis();
        let warm = std::time::Instant::now();
        let _ = player.resolve(&q.items[q.selected].id).await?;
        println!(
            "{}",
            json!({"check":"audio_cache","cold_ms":cold_ms,"warm_ms":warm.elapsed().as_millis()})
        );
        let response = api
            .client
            .get(url)
            .header("Range", "bytes=0-65535")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("Аудиопоток: HTTP {status}"));
        }
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        println!(
            "{}",
            json!({"check":"audio","ms":start.elapsed().as_millis(),"status":status.as_u16(),"bytes":bytes.len()})
        );
    }
    Ok(())
}

#[cfg(test)]
mod security_tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn glib_string_iterator_security_backport() {
        use webkit2gtk::glib::prelude::*;
        let value = ["zero", "один", "二", "last"].to_variant();
        assert_eq!(
            value.array_iter_str().unwrap().collect::<Vec<_>>(),
            vec!["zero", "один", "二", "last"]
        );
        let mut iter = value.array_iter_str().unwrap();
        assert_eq!(iter.next(), Some("zero"));
        assert_eq!(iter.next_back(), Some("last"));
        assert_eq!(iter.nth(1), Some("二"));
        assert_eq!(iter.next(), None);
        assert_eq!(value.array_iter_str().unwrap().last(), Some("last"));
        assert_eq!(value.array_iter_str().unwrap().nth_back(1), Some("二"));
    }
    use super::*;
    #[test]
    fn login_accepts_only_https_google_and_youtube_origins() {
        for url in [
            "https://accounts.google.com/",
            "https://music.youtube.com/",
            "https://www.youtube.com/",
        ] {
            assert!(login_url_allowed(&url.parse().unwrap()));
        }
        for url in [
            "http://accounts.google.com/",
            "https://accounts.google.com.evil.org/",
            "https://user@accounts.google.com/",
            "https://accounts.google.com:8443/",
            "file:///etc/passwd",
            "javascript:alert(1)",
        ] {
            assert!(!login_url_allowed(&url.parse().unwrap()));
        }
    }
}
