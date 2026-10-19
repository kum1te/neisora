use crate::player::PlayerState;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex, Notify},
};
pub const FILTER: &str = "@neisora:astats=metadata=1:reset=1:measure_perchannel=none:measure_overall=RMS_level+Peak_level";
#[derive(Clone, Serialize)]
pub struct Level {
    pub rms: f64,
    pub peak: f64,
}
fn energy(value: &Value) -> f64 {
    let db = value
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| value.as_f64())
        .unwrap_or(f64::NEG_INFINITY);
    if db.is_finite() {
        ((db + 60.0) / 60.0).clamp(0.0, 1.0).powi(2)
    } else {
        0.0
    }
}
pub fn spawn(
    socket: PathBuf,
    enabled: Arc<AtomicBool>,
    state: Arc<Mutex<PlayerState>>,
    wake: Arc<Notify>,
    app: AppHandle,
) {
    tokio::spawn(async move {
        let Ok(stream) = crate::mpv_ipc::connect(&socket).await else {
            return;
        };
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();
        let mut clock = tokio::time::interval(Duration::from_millis(50));
        clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            clock.tick().await;
            let s = state.lock().await.clone();
            if !enabled.load(Ordering::Relaxed) || s.paused || !s.active || s.buffering {
                tokio::select! {
                    _=wake.notified()=>continue,
                    line=lines.next_line()=>match line { Ok(Some(_))=>continue, _=>break }
                }
            }
            let request = format!(
                "{}\n",
                json!({"command":["get_property","af-metadata/neisora"],"request_id":91})
            );
            if writer.write_all(request.as_bytes()).await.is_err() {
                break;
            }
            let result = tokio::time::timeout(Duration::from_millis(500), async {
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if v["request_id"] == 91 {
                            return Some(v);
                        }
                    }
                }
                None
            })
            .await;
            let Ok(Some(v)) = result else {
                break;
            };
            if enabled.load(Ordering::Relaxed) {
                let gain = (s.volume / 100.0).clamp(0.0, 1.0).sqrt();
                let _ = app.emit(
                    "audio-level",
                    Level {
                        rms: energy(&v["data"]["lavfi.astats.Overall.RMS_level"]) * gain,
                        peak: energy(&v["data"]["lavfi.astats.Overall.Peak_level"]) * gain,
                    },
                );
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silence_and_invalid_metadata_are_zero() {
        for v in [json!("-inf"), json!("NaN"), Value::Null, json!(-90)] {
            assert_eq!(energy(&v), 0.0);
        }
        assert!(energy(&json!("-12")) > energy(&json!("-30")));
        assert_eq!(energy(&json!(20)), 1.0);
    }
}
