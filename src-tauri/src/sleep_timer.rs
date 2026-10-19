//! Playback timers run natively, including when the webview is hidden or throttled.
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, Notify};
#[derive(Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Off {},
    Track {},
    Minutes { minutes: u32 },
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub mode: String,
    pub deadline_ms: Option<u64>,
}
struct Data {
    snapshot: Snapshot,
    revision: u64,
    hold: bool,
}
pub struct SleepTimer {
    data: Mutex<Data>,
    changed: Notify,
}
impl Default for SleepTimer {
    fn default() -> Self {
        Self {
            data: Mutex::new(Data {
                snapshot: Snapshot {
                    mode: "off".into(),
                    deadline_ms: None,
                },
                revision: 0,
                hold: false,
            }),
            changed: Notify::new(),
        }
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn plan(request: Request, time: u64) -> Result<Snapshot, String> {
    match request {
        Request::Off {} => Ok(Snapshot {
            mode: "off".into(),
            deadline_ms: None,
        }),
        Request::Track {} => Ok(Snapshot {
            mode: "track".into(),
            deadline_ms: None,
        }),
        Request::Minutes { minutes } if (1..=1440).contains(&minutes) => Ok(Snapshot {
            mode: "minutes".into(),
            deadline_ms: Some(time + u64::from(minutes) * 60_000),
        }),
        _ => Err("Таймер: от 1 до 1440 минут".into()),
    }
}
impl SleepTimer {
    pub async fn snapshot(&self) -> Snapshot {
        self.data.lock().await.snapshot.clone()
    }
    pub async fn set(&self, app: &AppHandle, request: Request) -> Result<Snapshot, String> {
        let snapshot = plan(request, now())?;
        {
            let mut data = self.data.lock().await;
            data.revision = data.revision.wrapping_add(1);
            data.snapshot = snapshot.clone();
        }
        self.changed.notify_one();
        let _ = app.emit("sleep-timer-state", snapshot.clone());
        Ok(snapshot)
    }
    async fn pause_and_finish(&self, app: &AppHandle) {
        if let Some(state) = app.try_state::<Arc<crate::Backend>>() {
            if let Err(error) = state.player.pause_for_sleep().await {
                let _ = app.emit("sleep-timer-state", self.snapshot().await);
                let _ = app.emit(
                    "player-error",
                    format!("Таймер сна: не удалось остановить музыку. {error}"),
                );
                return;
            }
        }
        let _ = app.emit("sleep-timer-state", self.snapshot().await);
        let _ = app.emit("sleep-timer-fired", ());
    }
    /// EOF suppression happens before the frontend can apply repeat or radio Next.
    pub async fn on_track_end(&self, app: &AppHandle) -> bool {
        let mut data = self.data.lock().await;
        if data.hold {
            return true;
        }
        if data.snapshot.mode != "track"
            && data.snapshot.deadline_ms.is_none_or(|time| time > now())
        {
            return false;
        }
        data.hold = true;
        data.snapshot = Snapshot {
            mode: "off".into(),
            deadline_ms: None,
        };
        data.revision = data.revision.wrapping_add(1);
        drop(data);
        self.changed.notify_one();
        self.pause_and_finish(app).await;
        true
    }
    pub async fn allow_play(&self, automatic: bool) -> bool {
        let mut data = self.data.lock().await;
        if automatic && data.hold {
            return false;
        }
        data.hold = false;
        true
    }
    /// Reserve a playback generation while checking the expiration gate atomically.
    pub async fn begin_play(
        &self,
        automatic: bool,
        generation: &std::sync::atomic::AtomicU64,
    ) -> Option<u64> {
        let mut data = self.data.lock().await;
        if automatic && data.hold {
            return None;
        }
        data.hold = false;
        Some(generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1)
    }
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        let timer = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let changed = timer.changed.notified();
                tokio::pin!(changed);
                changed.as_mut().enable();
                let (deadline, revision) = {
                    let data = timer.data.lock().await;
                    (data.snapshot.deadline_ms, data.revision)
                };
                let Some(deadline) = deadline else {
                    changed.await;
                    continue;
                };
                tokio::select! {_=tokio::time::sleep(Duration::from_millis(deadline.saturating_sub(now()).min(1000)))=>{},_=changed=>continue}
                let mut data = timer.data.lock().await;
                if data.revision != revision
                    || data.snapshot.deadline_ms.is_none_or(|time| time > now())
                {
                    continue;
                }
                data.hold = true;
                data.snapshot = Snapshot {
                    mode: "off".into(),
                    deadline_ms: None,
                };
                data.revision = data.revision.wrapping_add(1);
                drop(data);
                timer.pause_and_finish(&app).await;
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn expiration_gate_blocks_automatic_replay_until_manual_play() {
        let timer = SleepTimer::default();
        {
            let mut data = timer.data.lock().await;
            data.hold = true;
        }
        let generation = std::sync::atomic::AtomicU64::new(4);
        assert!(timer.begin_play(true, &generation).await.is_none());
        assert_eq!(generation.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert!(!timer.allow_play(true).await);
        assert!(timer.allow_play(false).await);
        assert!(timer.allow_play(true).await);
        assert_eq!(timer.snapshot().await.mode, "off");
    }
    #[test]
    fn validates_and_calculates_deadlines() {
        assert!(plan(Request::Minutes { minutes: 0 }, 0).is_err());
        assert!(plan(Request::Minutes { minutes: 1441 }, 0).is_err());
        assert_eq!(
            plan(Request::Minutes { minutes: 15 }, 1000)
                .unwrap()
                .deadline_ms,
            Some(901000)
        );
        assert!(plan(Request::Track {}, 0).unwrap().deadline_ms.is_none());
        for raw in [
            r#"{"mode":"run"}"#,
            r#"{"mode":"off","minutes":3}"#,
            r#"{"mode":"minutes","minutes":-1}"#,
        ] {
            assert!(serde_json::from_str::<Request>(raw).is_err());
        }
    }
}
