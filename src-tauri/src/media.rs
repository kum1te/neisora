//! Linux MPRIS integration. No keyboard grabs or compositor-specific configuration.
use serde::Deserialize;
#[cfg(any(target_os = "linux", windows))]
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tauri::AppHandle;
#[cfg(target_os = "linux")]
use tauri::Emitter;

#[derive(Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub cover: String,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub paused: bool,
    pub active: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub repeat_mode: String,
}
impl Snapshot {
    #[cfg(target_os = "linux")]
    fn status(&self) -> &str {
        if !self.active {
            "Stopped"
        } else if self.paused {
            "Paused"
        } else {
            "Playing"
        }
    }
    #[cfg(target_os = "linux")]
    fn loop_status(&self) -> &str {
        match self.repeat_mode.as_str() {
            "one" => "Track",
            "all" => "Playlist",
            _ => "None",
        }
    }
    #[cfg(any(target_os = "linux", test))]
    fn track_path(&self) -> String {
        if self.id.is_empty() {
            "/org/mpris/MediaPlayer2/TrackList/NoTrack".into()
        } else {
            format!(
                "/org/mpris/MediaPlayer2/track/{}",
                self.id
                    .bytes()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            )
        }
    }
    fn validate(&self) -> Result<(), String> {
        if !(self.id.is_empty()
            || crate::local_music::valid_id(&self.id)
            || (self.id.len() == 11
                && self
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')))
            || !["", "off", "one", "all"].contains(&self.repeat_mode.as_str())
            || self.title.len() > 2000
            || self.artist.len() > 2000
            || self.cover.len() > 4000
            || [self.position, self.duration, self.volume]
                .iter()
                .any(|v| !v.is_finite())
            || self.position < 0.0
            || self.duration < 0.0
            || !(0.0..=100.0).contains(&self.volume)
        {
            return Err("Некорректное состояние медиаплеера".into());
        }
        Ok(())
    }
}
#[cfg(any(target_os = "linux", windows))]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Command {
    action: String,
    value: Option<f64>,
    track_id: String,
}

#[derive(Default)]
pub struct Media {
    #[cfg(windows)]
    controls: std::sync::Mutex<Option<windows::Controls>>,
    state: Arc<RwLock<Snapshot>>,
}
impl Media {
    pub fn start(&self, app: AppHandle) {
        #[cfg(target_os = "linux")]
        {
            let state = self.state.clone();
            std::thread::Builder::new()
                .name("neisora-mpris".into())
                .spawn(move || {
                    if let Err(e) = linux::serve(app, state) {
                        eprintln!("MPRIS недоступен: {e}");
                    }
                })
                .ok();
        }
        #[cfg(windows)]
        match windows::Controls::new(app, self.state.clone()) {
            Ok(controls) => {
                if let Ok(mut slot) = self.controls.lock() {
                    *slot = Some(controls);
                }
            }
            Err(error) => eprintln!("SMTC недоступен: {error}"),
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        let _ = app;
    }
    pub fn update(&self, next: Snapshot) -> Result<(), String> {
        next.validate()?;
        #[cfg(windows)]
        {
            let old = {
                let mut state = self.state.write().map_err(|e| e.to_string())?;
                std::mem::replace(&mut *state, next.clone())
            };
            if let Ok(slot) = self.controls.lock() {
                if let Some(controls) = slot.as_ref() {
                    controls.update(&old, &next).map_err(|e| e.to_string())?;
                }
            }
        }
        #[cfg(not(windows))]
        {
            *self.state.write().map_err(|e| e.to_string())? = next;
        }

        Ok(())
    }
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use dbus::{
        arg::{PropMap, RefArg, Variant},
        blocking::SyncConnection,
        channel::{MatchingReceiver, Sender},
        message::MatchRule,
        Message, Path,
    };
    use dbus_crossroads::Crossroads;
    use std::time::Duration;
    const PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
    struct Data {
        app: AppHandle,
        state: Arc<RwLock<Snapshot>>,
    }
    impl Data {
        fn snapshot(&self) -> Snapshot {
            self.state.read().map(|s| s.clone()).unwrap_or_default()
        }
        fn command(&self, action: &str, value: Option<f64>) {
            let _ = self.app.emit(
                "media-command",
                Command {
                    action: action.into(),
                    value,
                    track_id: self.snapshot().id,
                },
            );
        }
    }
    fn variant<T: RefArg + 'static>(value: T) -> Variant<Box<dyn RefArg>> {
        Variant(Box::new(value))
    }
    fn metadata(s: &Snapshot) -> PropMap {
        let mut map = PropMap::new();
        if s.id.is_empty() {
            return map;
        }
        map.insert(
            "mpris:trackid".into(),
            variant(Path::new(s.track_path()).expect("validated track ID")),
        );
        map.insert("xesam:title".into(), variant(s.title.clone()));
        map.insert("xesam:artist".into(), variant(vec![s.artist.clone()]));
        map.insert(
            "xesam:url".into(),
            variant(if crate::local_music::valid_id(&s.id) {
                String::new()
            } else {
                format!("https://music.youtube.com/watch?v={}", s.id)
            }),
        );
        map.insert("mpris:length".into(), variant((s.duration * 1e6) as i64));
        if reqwest::Url::parse(&s.cover).is_ok_and(|u| {
            (u.scheme() == "https" || (crate::local_music::valid_id(&s.id) && u.scheme() == "file"))
                && u.username().is_empty()
                && u.password().is_none()
        }) {
            map.insert("mpris:artUrl".into(), variant(s.cover.clone()));
        }
        map
    }
    pub fn changed(c: &SyncConnection, old: &Snapshot, s: &Snapshot) {
        let mut values = PropMap::new();
        if old.id != s.id
            || old.title != s.title
            || old.artist != s.artist
            || old.cover != s.cover
            || old.duration != s.duration
        {
            values.insert("Metadata".into(), variant(metadata(s)));
        }
        if old.status() != s.status() {
            values.insert("PlaybackStatus".into(), variant(s.status().to_owned()));
        }
        if old.loop_status() != s.loop_status() {
            values.insert("LoopStatus".into(), variant(s.loop_status().to_owned()));
        }
        if old.volume != s.volume {
            values.insert("Volume".into(), variant(s.volume / 100.0));
        }
        for (name, before, after) in [
            ("CanGoNext", old.can_next, s.can_next),
            ("CanGoPrevious", old.can_previous, s.can_previous),
            ("CanPlay", !old.id.is_empty(), !s.id.is_empty()),
            ("CanPause", old.active, s.active),
            (
                "CanSeek",
                old.active && old.duration > 0.0,
                s.active && s.duration > 0.0,
            ),
        ] {
            if before != after {
                values.insert(name.into(), variant(after));
            }
        }
        if !values.is_empty() {
            let _ = c.send(
                Message::new_signal(PATH, "org.freedesktop.DBus.Properties", "PropertiesChanged")
                    .unwrap()
                    .append3(PLAYER, values, Vec::<String>::new()),
            );
        }
        if old.id == s.id && (old.position - s.position).abs() > 2.0 {
            let _ = c.send(
                Message::new_signal(PATH, PLAYER, "Seeked")
                    .unwrap()
                    .append1((s.position * 1e6) as i64),
            );
        }
    }
    pub fn serve(
        app: AppHandle,
        state: Arc<RwLock<Snapshot>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let c = SyncConnection::new_session()?;
        // Do not replace another instance's player or queue for its bus name.
        if c.request_name("org.mpris.MediaPlayer2.neisora", false, false, true)?
            != dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply::PrimaryOwner
        {
            return Err("Имя MPRIS уже занято".into());
        }
        let mut cr = Crossroads::new();
        let root = cr.register("org.mpris.MediaPlayer2", |b| {
            b.property::<String, _>("Identity")
                .get(|_, _: &mut Data| Ok("neisora".into()));
            b.property::<String, _>("DesktopEntry")
                .get(|_, _: &mut Data| Ok("neisora".into()));
            b.property::<bool, _>("CanQuit")
                .get(|_, _: &mut Data| Ok(true));
            b.property::<bool, _>("CanRaise")
                .get(|_, _: &mut Data| Ok(true));
            b.property::<bool, _>("HasTrackList")
                .get(|_, _: &mut Data| Ok(false));
            b.property::<Vec<String>, _>("SupportedUriSchemes")
                .get(|_, _: &mut Data| Ok(vec![]));
            b.property::<Vec<String>, _>("SupportedMimeTypes")
                .get(|_, _: &mut Data| Ok(vec![]));
            b.method("Raise", (), (), |_, d: &mut Data, (): ()| {
                use tauri::Manager;
                if let Some(w) = d.app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                Ok(())
            });
            b.method("Quit", (), (), |_, d: &mut Data, (): ()| {
                use tauri::Manager;
                if let Some(w) = d.app.get_webview_window("main") {
                    let _ = w.close();
                }
                Ok(())
            });
        });
        let player = cr.register(PLAYER, |b| {
            b.property::<String, _>("PlaybackStatus")
                .get(|_, d: &mut Data| Ok(d.snapshot().status().into()));
            b.property::<String, _>("LoopStatus")
                .get(|_, d: &mut Data| Ok(d.snapshot().loop_status().into()))
                .set(|_, d, value| {
                    let mode = match value.as_str() {
                        "None" => 0.0,
                        "Playlist" => 1.0,
                        "Track" => 2.0,
                        _ => return Err(dbus::MethodErr::invalid_arg("LoopStatus")),
                    };
                    d.command("repeat", Some(mode));
                    Ok(None)
                });
            b.property::<PropMap, _>("Metadata")
                .get(|_, d: &mut Data| Ok(metadata(&d.snapshot())));
            b.property::<i64, _>("Position")
                .get(|_, d: &mut Data| Ok((d.snapshot().position * 1e6) as i64));
            b.property::<f64, _>("Volume")
                .get(|_, d: &mut Data| Ok(d.snapshot().volume / 100.0))
                .set(|_, d, v| {
                    if v.is_finite() {
                        d.command("volume", Some(v.clamp(0.0, 1.0) * 100.0));
                    }
                    Ok(None)
                });
            b.property::<f64, _>("Rate")
                .get(|_, _: &mut Data| Ok(1.0))
                .set(|_, _, _| Ok(None));
            b.property::<f64, _>("MinimumRate")
                .get(|_, _: &mut Data| Ok(1.0));
            b.property::<f64, _>("MaximumRate")
                .get(|_, _: &mut Data| Ok(1.0));
            b.property::<bool, _>("CanControl")
                .get(|_, _: &mut Data| Ok(true));
            b.property::<bool, _>("CanGoNext")
                .get(|_, d: &mut Data| Ok(d.snapshot().can_next));
            b.property::<bool, _>("CanGoPrevious")
                .get(|_, d: &mut Data| Ok(d.snapshot().can_previous));
            b.property::<bool, _>("CanPlay")
                .get(|_, d: &mut Data| Ok(!d.snapshot().id.is_empty()));
            b.property::<bool, _>("CanPause")
                .get(|_, d: &mut Data| Ok(d.snapshot().active));
            b.property::<bool, _>("CanSeek").get(|_, d: &mut Data| {
                let s = d.snapshot();
                Ok(s.active && s.duration > 0.0)
            });
            for (method, action) in [
                ("PlayPause", "toggle"),
                ("Play", "play"),
                ("Pause", "pause"),
                ("Stop", "stop"),
                ("Next", "next"),
                ("Previous", "previous"),
            ] {
                b.method(method, (), (), move |_, d: &mut Data, (): ()| {
                    d.command(action, None);
                    Ok(())
                });
            }
            b.method(
                "Seek",
                ("Offset",),
                (),
                |_, d: &mut Data, (offset,): (i64,)| {
                    let s = d.snapshot();
                    if s.active && s.duration > 0.0 {
                        let target = s.position + (offset as f64 / 1e6);
                        if target > s.duration {
                            d.command("next", None);
                        } else {
                            d.command("seek", Some(target.max(0.0)));
                        }
                    }
                    Ok(())
                },
            );
            b.method(
                "SetPosition",
                ("TrackId", "Position"),
                (),
                |_, d: &mut Data, (track, position): (Path<'static>, i64)| {
                    let s = d.snapshot();
                    let value = position as f64 / 1e6;
                    if s.active
                        && !s.id.is_empty()
                        && track.to_string() == s.track_path()
                        && value >= 0.0
                        && value <= s.duration
                    {
                        d.command("seek", Some(value));
                    }
                    Ok(())
                },
            );
            b.signal::<(i64,), _>("Seeked", ("Position",));
        });
        cr.insert(
            PATH,
            &[root, player],
            Data {
                app,
                state: state.clone(),
            },
        );
        let cr = std::sync::Mutex::new(cr);
        c.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |msg, conn| {
                if let Ok(mut cr) = cr.lock() {
                    let _ = cr.handle_message(msg, conn);
                }
                true
            }),
        );
        // All connection I/O belongs to this worker. Previously updates queued
        // signals from another thread while process() waited up to 30 seconds;
        // a panel's next property poll accidentally flushed those signals.
        let mut previous = Snapshot::default();
        loop {
            c.process(Duration::from_millis(50))?;
            let next = state.read().map_err(|e| e.to_string())?.clone();
            changed(&c, &previous, &next);
            c.channel().flush();
            previous = next;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_ids_and_nonfinite_positions() {
        let mut s = Snapshot {
            id: "../../unsafe".into(),
            ..Snapshot::default()
        };
        assert!(s.validate().is_err());
        s.id = "abc-def1234".into();
        assert!(s.validate().is_ok());
        assert!(s.track_path().ends_with("6162632d64656631323334"));
        s.position = f64::NAN;
        assert!(s.validate().is_err());
    }
}

#[cfg(windows)]
mod windows;
