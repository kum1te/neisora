//! Bounded, in-memory diagnostics. Raw provider errors and media metadata never enter it.
use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex, time::Instant};
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Resolve,
    OpenAudio,
    Playback,
    Recovery,
    Control,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Started,
    Ready,
    Failed,
}
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    Timeout,
    AccessDenied,
    Unavailable,
    Component,
    Unknown,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub seconds_ago: u64,
    pub stage: Stage,
    pub outcome: Outcome,
    pub code: Option<Code>,
}
struct Entry {
    time: Instant,
    stage: Stage,
    outcome: Outcome,
    code: Option<Code>,
}
#[derive(Default)]
pub struct Diagnostics {
    events: Mutex<VecDeque<Entry>>,
}
impl Diagnostics {
    pub fn record(&self, stage: Stage, outcome: Outcome, error: Option<&str>) {
        let code = error.map(classify);
        let mut events = self.events.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() == 32 {
            events.pop_front();
        }
        events.push_back(Entry {
            time: Instant::now(),
            stage,
            outcome,
            code,
        });
    }
    pub fn failure<T>(&self, stage: Stage, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = &result {
            self.record(stage, Outcome::Failed, Some(error));
        }
        result
    }
    pub fn clear(&self) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    pub fn snapshot(&self) -> Vec<Event> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|e| Event {
                seconds_ago: e.time.elapsed().as_secs(),
                stage: e.stage,
                outcome: e.outcome,
                code: e.code,
            })
            .collect()
    }
}
fn classify(error: &str) -> Code {
    // The string is inspected only to produce an enum; never retained or serialized.
    let error = error.to_lowercase();
    if error.contains("timeout") || error.contains("вовремя") || error.contains("ожидания")
    {
        Code::Timeout
    } else if ["403", "401", "sign in", "вход", "denied"]
        .iter()
        .any(|s| error.contains(s))
    {
        Code::AccessDenied
    } else if ["not available", "недоступ", "не отдал"]
        .iter()
        .any(|s| error.contains(s))
    {
        Code::Unavailable
    } else if ["не найден", "запустить", "завершился"]
        .iter()
        .any(|s| error.contains(s))
    {
        Code::Component
    } else {
        Code::Unknown
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_is_bounded_and_drops_private_error_details() {
        let log = Diagnostics::default();
        for _ in 0..100 {
            log.record(Stage::Resolve,Outcome::Failed,Some("HTTP 403 https://googlevideo.com/?sig=PRIVATE_COOKIE /home/user/secret Song Name"));
        }
        let events = log.snapshot();
        assert_eq!(events.len(), 32);
        let text = serde_json::to_string(&events).unwrap();
        assert!(text.contains("access_denied"));
        for secret in ["PRIVATE", "googlevideo", "secret", "Song", "HTTP"] {
            assert!(!text.contains(secret));
        }
        log.clear();
        assert!(log.snapshot().is_empty());
    }
}
