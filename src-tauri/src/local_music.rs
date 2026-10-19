//! Authorized music folders, an incremental on-disk index and lazy artwork.
use crate::{api::private_write, model::Item};
use base64::Engine;
use lofty::{
    config::ParseOptions,
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::Accessor,
};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha1::{Digest, Sha1};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Notify, Semaphore};
const MAX_TRACKS: usize = 50_000;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    #[serde(default)]
    pub volume: Option<u64>,
    pub id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub available: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    item: Item,
    path: PathBuf,
    folder: String,
    artist: String,
    album: String,
    stamp: u64,
    size: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    roots: Vec<Folder>,
    #[serde(default)]
    tracks: Vec<Record>,
    #[serde(default)]
    revision: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub folders: Vec<Folder>,
    pub total: usize,
    pub scanning: bool,
    pub warning: String,
}
#[derive(Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: usize,
    pub artists: Vec<String>,
    pub albums: Vec<String>,
    pub status: Status,
}
pub struct LocalMusic {
    pub folder_dialog: tokio::sync::Mutex<()>,
    directory: PathBuf,
    state: RwLock<Index>,
    scanning: AtomicBool,
    wake: Arc<Notify>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    warning: RwLock<String>,
    art_gate: Arc<Semaphore>,
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 46 && id.starts_with("local:") && id[6..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn volume(path: &Path) -> Option<u64> {
    crate::platform::volume(path)
}
fn online(root: &Folder) -> bool {
    root.path.is_dir()
        && root
            .volume
            .is_none_or(|expected| volume(&root.path) == Some(expected))
}
fn key(path: &Path) -> String {
    format!("{:x}", Sha1::digest(path.as_os_str().as_encoded_bytes()))
}
fn stamp(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|t| t.as_nanos() as u64)
        .unwrap_or(0)
}
fn supported(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|e| {
        [
            "mp3", "flac", "m4a", "mp4", "ogg", "opus", "wav", "aac", "aif", "aiff", "wv", "ape",
            "mpc",
        ]
        .contains(&e.to_ascii_lowercase().as_str())
    })
}
fn trim(s: Option<std::borrow::Cow<'_, str>>) -> String {
    s.unwrap_or_default().trim().chars().take(500).collect()
}
impl LocalMusic {
    pub fn new(directory: PathBuf) -> Self {
        let path = directory.join("local-music.json");
        let mut state = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= 40 * 1024 * 1024)
            .and_then(|_| std::fs::read(path).ok())
            .and_then(|s| serde_json::from_slice::<Index>(&s).ok())
            .unwrap_or_default();
        state.roots.truncate(32);
        state
            .roots
            .retain(|r| r.path.is_absolute() && r.id == key(&r.path));
        state.tracks.truncate(MAX_TRACKS);
        state.tracks.retain(|r| {
            valid_id(&r.item.id)
                && r.item.id == format!("local:{}", key(&r.path))
                && state
                    .roots
                    .iter()
                    .any(|f| f.id == r.folder && r.path.starts_with(&f.path))
        });
        Self {
            folder_dialog: tokio::sync::Mutex::new(()),
            directory,
            state: RwLock::new(state),
            scanning: AtomicBool::new(false),
            wake: Arc::new(Notify::new()),
            watcher: Mutex::new(None),
            warning: RwLock::new(String::new()),
            art_gate: Arc::new(Semaphore::new(2)),
        }
    }
    pub fn status(&self) -> Status {
        let state = self.state.read().unwrap();
        Status {
            folders: state.roots.clone(),
            total: state.tracks.len(),
            scanning: self.scanning.load(Ordering::Relaxed),
            warning: self.warning.read().unwrap().clone(),
        }
    }
    pub fn page(&self, query: String, artist: String, album: String, offset: usize) -> Page {
        let state = self.state.read().unwrap();
        let q = query.to_lowercase();
        let matches = state.tracks.iter().filter(|r| {
            (artist.is_empty() || r.artist == artist)
                && (album.is_empty() || r.album == album)
                && (q.is_empty()
                    || format!("{} {} {}", r.item.title, r.artist, r.album)
                        .to_lowercase()
                        .contains(&q))
        });
        let total = matches.clone().count();
        let items = matches
            .skip(offset)
            .take(100)
            .map(|r| r.item.clone())
            .collect();
        let mut artists = state
            .tracks
            .iter()
            .map(|r| r.artist.clone())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        artists.sort();
        artists.dedup();
        artists.truncate(2000);
        let mut albums = state
            .tracks
            .iter()
            .filter(|r| artist.is_empty() || r.artist == artist)
            .map(|r| r.album.clone())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        albums.sort();
        albums.dedup();
        albums.truncate(2000);
        drop(state);
        Page {
            items,
            total,
            artists,
            albums,
            status: self.status(),
        }
    }
    pub fn add_folder(&self, path: PathBuf) -> Result<(), String> {
        let path = std::fs::canonicalize(path).map_err(|_| "Папка недоступна")?;
        if !path.is_dir() {
            return Err("Выбери папку с музыкой".into());
        }
        let mut state = self.state.write().unwrap();
        if state
            .roots
            .iter()
            .any(|r| path.starts_with(&r.path) || r.path.starts_with(&path))
        {
            return Err("Эта папка или её подпапка уже подключена".into());
        }
        if state.roots.len() >= 32 {
            return Err("Можно подключить до 32 папок".into());
        }
        let mut next = state.clone();
        next.roots.push(Folder {
            volume: volume(&path),
            id: key(&path),
            path,
            available: true,
        });
        next.revision += 1;
        self.save(&next)?;
        *state = next;
        drop(state);
        self.configure_watcher();
        self.request_scan();
        Ok(())
    }
    pub fn remove_folder(&self, id: &str) -> Result<(), String> {
        if id.len() != 40 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Некорректная папка".into());
        }
        let mut state = self.state.write().unwrap();
        let mut next = state.clone();
        next.roots.retain(|r| r.id != id);
        next.tracks.retain(|r| r.folder != id);
        next.revision += 1;
        self.save(&next)?;
        *state = next;
        drop(state);
        self.configure_watcher();
        self.request_scan();
        Ok(())
    }
    fn save(&self, state: &Index) -> Result<(), String> {
        private_write(
            &self.directory.join("local-music.json"),
            &serde_json::to_vec(state).map_err(|e| e.to_string())?,
        )
    }
    pub fn request_scan(&self) {
        self.wake.notify_one();
    }
    fn configure_watcher(&self) {
        let wake = self.wake.clone();
        let profile = self.directory.clone();
        let result =
            notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
                if event.is_err()
                    || event.is_ok_and(|e| {
                        !matches!(e.kind, notify::EventKind::Access(_))
                            && !e.paths.iter().all(|p| p.starts_with(&profile))
                    })
                {
                    wake.notify_one();
                }
            });
        if let Ok(mut watcher) = result {
            for root in &self.state.read().unwrap().roots {
                let _ = watcher.watch(&root.path, RecursiveMode::Recursive);
            }
            *self.watcher.lock().unwrap() = Some(watcher);
        }
    }
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        self.configure_watcher();
        let library = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut ticks = 0u32;
            loop {
                library.scanning.store(true, Ordering::Relaxed);
                let _ = app.emit("local-library-updated", ());
                let scan = library.clone();
                let result = tokio::task::spawn_blocking(move || scan.scan()).await;
                if let Err(e) = result.unwrap_or_else(|e| Err(e.to_string())) {
                    *library.warning.write().unwrap() = e;
                }
                library.scanning.store(false, Ordering::Relaxed);
                let _ = app.emit("local-library-updated", ());
                loop {
                    tokio::select! { _=library.wake.notified()=>break, _=tokio::time::sleep(Duration::from_secs(30))=>{} }
                    ticks += 1;
                    let changed = library
                        .state
                        .read()
                        .unwrap()
                        .roots
                        .iter()
                        .any(|r| r.available != online(r));
                    if changed || ticks.is_multiple_of(10) {
                        library.configure_watcher();
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(650)).await;
            }
        });
    }
    fn scan(&self) -> Result<(), String> {
        let old = self.state.read().unwrap().clone();
        if old.roots.is_empty() {
            return Ok(());
        }
        let by_path: HashMap<_, _> = old.tracks.iter().map(|r| (r.path.clone(), r)).collect();
        let mut next = old.clone();
        next.tracks.clear();
        let mut seen = HashSet::new();
        let mut warning = String::new();
        for root in &mut next.roots {
            root.available = online(root) && std::fs::read_dir(&root.path).is_ok();
            if !root.available {
                for r in old.tracks.iter().filter(|r| r.folder == root.id) {
                    let mut r = r.clone();
                    r.item.available = Some(false);
                    next.tracks.push(r);
                }
                continue;
            }
            if root.available && root.volume.is_none() {
                root.volume = volume(&root.path);
            }
            let mut stack = vec![(root.path.clone(), 0)];
            let mut incomplete = false;
            while let Some((dir, depth)) = stack.pop() {
                if next.tracks.len() >= MAX_TRACKS {
                    warning = "Достигнут лимит 50 000 локальных треков".into();
                    break;
                }
                if !std::fs::canonicalize(&dir).is_ok_and(|p| p == dir && p.starts_with(&root.path))
                {
                    incomplete = true;
                    continue;
                }
                let entries = match std::fs::read_dir(&dir) {
                    Ok(v) => v,
                    Err(_) => {
                        incomplete = true;
                        continue;
                    }
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Ok(kind) = entry.file_type() else {
                        continue;
                    };
                    if kind.is_dir() && depth < 64 {
                        stack.push((path, depth + 1));
                        continue;
                    }
                    if !kind.is_file()
                        || !supported(&path)
                        || path.to_str().is_none()
                        || next.tracks.len() >= MAX_TRACKS
                    {
                        continue;
                    }
                    if !std::fs::canonicalize(&path)
                        .is_ok_and(|p| p == path && p.starts_with(&root.path))
                    {
                        continue;
                    }
                    let Ok(meta) = entry.metadata() else {
                        continue;
                    };
                    if meta.len() > 4 * 1024 * 1024 * 1024 {
                        continue;
                    }
                    seen.insert(path.clone());
                    let modified = stamp(&meta);
                    if let Some(r) = by_path
                        .get(&path)
                        .filter(|r| r.stamp == modified && r.size == meta.len())
                    {
                        let mut r = (*r).clone();
                        r.item.available = Some(true);
                        next.tracks.push(r);
                        continue;
                    }
                    if let Some(record) = read_record(&path, &root.id, modified, meta.len()) {
                        next.tracks.push(record);
                    }
                }
            }
            // A transient permission/read error must not erase indexed music.
            if incomplete {
                warning =
                    "Некоторые подпапки недоступны. Сохранённые записи оставлены в коллекции."
                        .into();
                for r in old
                    .tracks
                    .iter()
                    .filter(|r| r.folder == root.id && !seen.contains(&r.path))
                {
                    let mut r = r.clone();
                    r.item.available = Some(false);
                    next.tracks.push(r);
                }
            }
        }
        next.tracks.truncate(MAX_TRACKS);
        next.tracks.sort_by_cached_key(|r| {
            (
                r.artist.to_lowercase(),
                r.album.to_lowercase(),
                r.item.title.to_lowercase(),
            )
        });
        let mut state = self.state.write().unwrap();
        if state.revision != old.revision {
            self.request_scan();
            return Ok(());
        }
        self.save(&next)?;
        *state = next;
        *self.warning.write().unwrap() = warning;
        Ok(())
    }
    pub fn resolve(&self, id: &str) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Некорректный локальный трек".into());
        }
        let state = self.state.read().unwrap();
        let r = state
            .tracks
            .iter()
            .find(|r| r.item.id == id)
            .ok_or("Трек больше не входит в локальную медиатеку")?;
        let root = state
            .roots
            .iter()
            .find(|f| f.id == r.folder)
            .ok_or("Папка отключена")?;
        if !online(root) {
            return Err("Папка или подключённый диск сейчас недоступны".into());
        }
        let canonical = std::fs::canonicalize(&r.path)
            .map_err(|_| "Файл недоступен. Проверь, подключён ли диск.")?;
        if !canonical.starts_with(&root.path)
            || !std::fs::symlink_metadata(&r.path).is_ok_and(|m| m.file_type().is_file())
            || !canonical.is_file()
        {
            return Err("Файл находится вне подключённой папки".into());
        }
        Ok(canonical)
    }
    fn art_path(&self, id: &str, large: bool) -> Result<PathBuf, String> {
        if !valid_id(id) {
            return Err("Некорректный трек".into());
        }
        let state = self.state.read().unwrap();
        let record = state
            .tracks
            .iter()
            .find(|r| r.item.id == id)
            .ok_or("Трек не найден")?;
        Ok(self.directory.join("local-art").join(format!(
            "{}-{}-{}.jpg",
            &id[6..],
            record.stamp,
            if large { 800 } else { 240 }
        )))
    }
    pub fn cover_url(&self, id: &str) -> Option<String> {
        self.art_path(id, true)
            .ok()
            .filter(|p| p.is_file())
            .and_then(|p| reqwest::Url::from_file_path(p).ok())
            .map(|u| u.to_string())
    }
    pub async fn artwork(
        self: &Arc<Self>,
        id: String,
        large: bool,
    ) -> Result<Option<String>, String> {
        let permit = self
            .art_gate
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        let library = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let target = library.art_path(&id, large)?;
            if let Ok(bytes) = std::fs::metadata(&target)
                .ok()
                .filter(|m| m.len() <= 1024 * 1024)
                .ok_or(())
                .and_then(|_| std::fs::read(&target).map_err(|_| ()))
            {
                return Ok(Some(format!(
                    "data:image/jpeg;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                )));
            }
            lofty::config::apply_global_options(
                lofty::config::GlobalOptions::new().allocation_limit(16 * 1024 * 1024),
            );
            let source = library.resolve(&id)?;
            let file = Probe::open(&source)
                .map_err(|e| e.to_string())?
                .options(ParseOptions::new().read_properties(false))
                .read()
                .map_err(|e| e.to_string())?;
            let Some(picture) = file
                .tags()
                .iter()
                .flat_map(|t| t.pictures())
                .find(|p| p.data().len() <= 12 * 1024 * 1024)
            else {
                return Ok(None);
            };
            let mut reader = image::ImageReader::new(std::io::Cursor::new(picture.data()))
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            let image = reader
                .decode()
                .map_err(|e| e.to_string())?
                .thumbnail(if large { 800 } else { 240 }, if large { 800 } else { 240 })
                .to_rgb8();
            let mut bytes = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
                .encode_image(&image)
                .map_err(|e| e.to_string())?;
            std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            private_write(&target, &bytes)?;
            // Bound generated artwork on disk independently of the music collection.
            if let Ok(entries) = std::fs::read_dir(target.parent().unwrap()) {
                let mut files = entries
                    .flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "jpg"))
                    .filter_map(|e| e.metadata().ok().map(|m| (m.modified().ok(), e.path())))
                    .collect::<Vec<_>>();
                files.sort_by_key(|f| f.0);
                let remove = files.len().saturating_sub(256);
                for (_, file) in files.into_iter().take(remove) {
                    if file != target {
                        let _ = std::fs::remove_file(file);
                    }
                }
            }
            Ok(Some(format!(
                "data:image/jpeg;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )))
        })
        .await
        .map_err(|e| e.to_string())?
    }
    pub fn lyrics(&self, id: &str) -> Result<crate::lyrics::Lyrics, String> {
        let path = self.resolve(id)?;
        let lrc = path.with_extension("lrc");
        let text = std::fs::symlink_metadata(&lrc)
            .ok()
            .filter(|m| m.is_file() && !m.file_type().is_symlink() && m.len() <= 512 * 1024)
            .and_then(|_| std::fs::read_to_string(lrc).ok());
        if let Some(plain) = text {
            let lines = crate::lyrics::parse_lrc(&plain);
            return Ok(crate::lyrics::Lyrics {
                synced: !lines.is_empty(),
                lines,
                plain,
                source: "Локальный LRC".into(),
            });
        }
        Err("Рядом с файлом нет текста .lrc".into())
    }
}
fn read_record(path: &Path, folder: &str, stamp: u64, size: u64) -> Option<Record> {
    lofty::config::apply_global_options(
        lofty::config::GlobalOptions::new().allocation_limit(16 * 1024 * 1024),
    );
    let parsed = Probe::open(path)
        .ok()?
        .options(ParseOptions::new().read_cover_art(false))
        .read()
        .ok()?;
    let tag = parsed.primary_tag().or_else(|| parsed.first_tag());
    let title = trim(tag.and_then(|t| t.title()));
    let artist = trim(tag.and_then(|t| t.artist()));
    let album = trim(tag.and_then(|t| t.album()));
    let duration = parsed.properties().duration().as_secs();
    let id = format!("local:{}", key(path));
    let item:Item=serde_json::from_value(json!({"id":id,"title":if title.is_empty(){path.file_stem()?.to_string_lossy().chars().take(500).collect::<String>()}else{title},"subtitle":([artist.as_str(),album.as_str()].into_iter().filter(|s|!s.is_empty()).collect::<Vec<_>>().join(" • ")),"image":"","kind":"song","duration":format!("{}:{:02}",duration/60,duration%60),"selected":false,"endpoint":{"localId":id},"play":null,"source":"local","album":album,"available":true,"localRevision":stamp})).ok()?;
    Some(Record {
        item,
        path: path.to_owned(),
        folder: folder.into(),
        artist,
        album,
        stamp,
        size,
    })
}

#[cfg(target_os = "linux")]
pub async fn choose_folder(window: tauri::WebviewWindow) -> Result<Option<PathBuf>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    window
        .with_webview(move |webview| {
            use gtk::prelude::*;
            let parent = webview
                .inner()
                .toplevel()
                .and_then(|w| w.downcast::<gtk::Window>().ok());
            let dialog = gtk::FileChooserNative::new(
                Some("Папка с музыкой — neisora"),
                parent.as_ref(),
                gtk::FileChooserAction::SelectFolder,
                Some("Подключить"),
                Some("Отмена"),
            );
            if let Some(folder) = dirs::audio_dir()
                .filter(|p| p.is_dir())
                .or_else(dirs::home_dir)
            {
                dialog.set_current_folder(folder);
            }
            dialog.set_modal(true);
            dialog.set_local_only(true);
            let keep_alive = std::rc::Rc::new(std::cell::RefCell::new(Some(dialog.clone())));
            let sender = std::cell::RefCell::new(Some(tx));
            dialog.connect_response(move |dialog, response| {
                if let Some(tx) = sender.borrow_mut().take() {
                    let _ = tx.send(if response == gtk::ResponseType::Accept {
                        dialog.filename()
                    } else {
                        None
                    });
                }
                dialog.destroy();
                keep_alive.borrow_mut().take();
            });
            dialog.show();
        })
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Выбор папки отменён".into())
}
#[cfg(windows)]
pub async fn choose_folder(window: tauri::WebviewWindow) -> Result<Option<PathBuf>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let parent = window.clone();
    window
        .run_on_main_thread(move || {
            let mut dialog = rfd::FileDialog::new()
                .set_title("Папка с музыкой")
                .set_parent(&parent);
            if let Some(folder) = dirs::audio_dir() {
                dialog = dialog.set_directory(folder);
            }
            let _ = tx.send(dialog.pick_folder());
        })
        .map_err(|e| e.to_string())?;
    rx.await.map_err(|_| "Выбор папки отменён".into())
}
#[cfg(not(any(target_os = "linux", windows)))]
pub async fn choose_folder(_: tauri::WebviewWindow) -> Result<Option<PathBuf>, String> {
    Err("Выбор папок на этой платформе пока не поддерживается".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wav(path: &Path) {
        let size = 16000u32;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + size).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(8000u32.to_le_bytes());
        bytes.extend(16000u32.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(16u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(size.to_le_bytes());
        bytes.resize(bytes.len() + size as usize, 0);
        std::fs::write(path, bytes).unwrap();
    }
    #[test]
    fn incremental_index_keeps_unmounted_music_and_rejects_symlink_escape() {
        let base = std::env::temp_dir().join(format!("neisora-local-index-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("profile")).unwrap();
        std::fs::create_dir_all(base.join("music/sub")).unwrap();
        wav(&base.join("music/sub/Трек.wav"));
        wav(&base.join("outside.wav"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("outside.wav"), base.join("music/escape.wav"))
            .unwrap();
        let library = LocalMusic::new(base.join("profile"));
        library.add_folder(base.join("music")).unwrap();
        assert!(library.add_folder(base.join("music/sub")).is_err());
        library.scan().unwrap();
        let page = library.page(String::new(), String::new(), String::new(), 0);
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].title, "Трек");
        assert_eq!(page.items[0].source.as_deref(), Some("local"));
        let id = page.items[0].id.clone();
        assert!(library.resolve(&id).is_ok());
        assert!(library.resolve("../../outside.wav").is_err());
        std::fs::write(
            base.join("music/sub/Трек.lrc"),
            "[00:00.50]Локальный текст\n[00:00.90]Вторая строка",
        )
        .unwrap();
        assert!(library.lyrics(&id).unwrap().synced);
        library.scan().unwrap();
        assert_eq!(
            library
                .page("Трек".into(), String::new(), String::new(), 0)
                .total,
            1
        );
        let original_volume = library.state.read().unwrap().roots[0].volume;
        library.state.write().unwrap().roots[0].volume = Some(u64::MAX);
        library.scan().unwrap();
        assert_eq!(library.status().total, 1);
        assert!(!library.status().folders[0].available);
        assert!(library.resolve(&id).is_err());
        library.state.write().unwrap().roots[0].volume = original_volume;
        library.scan().unwrap();
        std::fs::rename(base.join("music"), base.join("offline")).unwrap();
        library.scan().unwrap();
        assert_eq!(library.status().total, 1);
        assert_eq!(
            library
                .page(String::new(), String::new(), String::new(), 0)
                .items[0]
                .available,
            Some(false)
        );
        assert!(library.resolve(&id).is_err());
        std::fs::rename(base.join("offline"), base.join("music")).unwrap();
        library.scan().unwrap();
        assert!(library.resolve(&id).is_ok());
        std::fs::remove_file(base.join("music/sub/Трек.wav")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("outside.wav"), base.join("music/sub/Трек.wav"))
            .unwrap();
        assert!(library.resolve(&id).is_err());
        let folder = library.status().folders[0].id.clone();
        library.remove_folder(&folder).unwrap();
        assert!(library.status().folders.is_empty());
        assert!(base.join("outside.wav").exists());
        assert!(library.resolve(&id).is_err());
        drop(library);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[tokio::test]
    async fn reads_tags_and_resizes_embedded_art_without_exporting_file_paths() {
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::tag::{Tag, TagExt, TagType};
        let base = std::env::temp_dir().join(format!("neisora-local-art-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("profile")).unwrap();
        std::fs::create_dir_all(base.join("music")).unwrap();
        let file = base.join("music/source.wav");
        wav(&file);
        let image = image::RgbImage::from_pixel(1024, 1024, image::Rgb([90, 60, 150]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode_image(&image)
            .unwrap();
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("Tagged Song".into());
        tag.set_artist("Tagged Artist".into());
        tag.set_album("Tagged Album".into());
        tag.push_picture(
            Picture::unchecked(bytes)
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Jpeg)
                .build(),
        );
        tag.save_to_path(&file, lofty::config::WriteOptions::default())
            .unwrap();
        let library = Arc::new(LocalMusic::new(base.join("profile")));
        library.add_folder(base.join("music")).unwrap();
        library.scan().unwrap();
        let page = library.page(
            String::new(),
            "Tagged Artist".into(),
            "Tagged Album".into(),
            0,
        );
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].title, "Tagged Song");
        assert!(!serde_json::to_string(&page.items[0])
            .unwrap()
            .contains("source.wav"));
        for (large, width) in [(false, 240), (true, 800)] {
            let encoded = library
                .artwork(page.items[0].id.clone(), large)
                .await
                .unwrap()
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded.split_once(',').unwrap().1)
                .unwrap();
            let image = image::load_from_memory(&bytes).unwrap();
            assert_eq!(image.width(), width);
            assert_eq!(image.height(), width);
        }
        assert!(library
            .cover_url(&page.items[0].id)
            .unwrap()
            .starts_with("file://"));
        drop(library);
        std::fs::remove_dir_all(base).unwrap();
    }
}
