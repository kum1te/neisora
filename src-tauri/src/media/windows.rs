//! Native SMTC, with event registrations owned and removed with the service.
use super::{Command, Snapshot};
use std::sync::{Arc, RwLock};
use tauri::{AppHandle, Emitter, Manager};
use windows::{
    core::{factory, Result, HSTRING},
    Foundation::{TimeSpan, TypedEventHandler},
    Media::{
        MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
        SystemMediaTransportControls, SystemMediaTransportControlsButton,
        SystemMediaTransportControlsButtonPressedEventArgs,
        SystemMediaTransportControlsTimelineProperties,
    },
    Win32::{Foundation::HWND, System::WinRT::ISystemMediaTransportControlsInterop},
};

pub struct Controls {
    controls: SystemMediaTransportControls,
    buttons: i64,
    position: i64,
}
fn emit(app: &AppHandle, state: &RwLock<Snapshot>, action: &str, value: Option<f64>) {
    let id = state.read().map(|s| s.id.clone()).unwrap_or_default();
    let _ = app.emit(
        "media-command",
        Command {
            action: action.into(),
            value,
            track_id: id,
        },
    );
}
impl Controls {
    pub fn new(app: AppHandle, state: Arc<RwLock<Snapshot>>) -> Result<Self> {
        let window = app
            .get_webview_window("main")
            .expect("main window exists at setup");
        let hwnd = window
            .hwnd()
            .map_err(|_| windows::core::Error::from_thread())?;
        let interop: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, _>()?;
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd.0)) }?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsStopEnabled(true)?;
        let callback_app = app.clone();
        let callback_state = state.clone();
        let buttons = controls.ButtonPressed(&TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.as_ref() {
                let action = match args.Button()? {
                    SystemMediaTransportControlsButton::Play => "play",
                    SystemMediaTransportControlsButton::Pause => "pause",
                    SystemMediaTransportControlsButton::Stop => "stop",
                    SystemMediaTransportControlsButton::Next => "next",
                    SystemMediaTransportControlsButton::Previous => "previous",
                    _ => return Ok(()),
                };
                emit(&callback_app, &callback_state, action, None);
            }
            Ok(())
        }))?;
        let position =
            match controls.PlaybackPositionChangeRequested(&TypedEventHandler::<
                SystemMediaTransportControls,
                PlaybackPositionChangeRequestedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref() {
                    emit(
                        &app,
                        &state,
                        "seek",
                        Some(args.RequestedPlaybackPosition()?.Duration as f64 / 10_000_000.0),
                    );
                }
                Ok(())
            })) {
                Ok(token) => token,
                Err(error) => {
                    let _ = controls.RemoveButtonPressed(buttons);
                    return Err(error);
                }
            };
        Ok(Self {
            controls,
            buttons,
            position,
        })
    }
    pub fn update(&self, old: &Snapshot, next: &Snapshot) -> Result<()> {
        self.controls.SetIsEnabled(next.active)?;
        self.controls.SetIsNextEnabled(next.can_next)?;
        self.controls.SetIsPreviousEnabled(next.can_previous)?;
        self.controls.SetPlaybackStatus(if !next.active {
            MediaPlaybackStatus::Stopped
        } else if next.paused {
            MediaPlaybackStatus::Paused
        } else {
            MediaPlaybackStatus::Playing
        })?;
        if old.id != next.id || old.title != next.title || old.artist != next.artist {
            let display = self.controls.DisplayUpdater()?;
            display.SetType(MediaPlaybackType::Music)?;
            let music = display.MusicProperties()?;
            music.SetTitle(&HSTRING::from(&next.title))?;
            music.SetArtist(&HSTRING::from(&next.artist))?;
            display.Update()?;
        }
        let timeline = SystemMediaTransportControlsTimelineProperties::new()?;
        let span = |seconds: f64| TimeSpan {
            Duration: (seconds * 10_000_000.0).min(i64::MAX as f64) as i64,
        };
        timeline.SetStartTime(span(0.0))?;
        timeline.SetMinSeekTime(span(0.0))?;
        timeline.SetEndTime(span(next.duration))?;
        timeline.SetMaxSeekTime(span(next.duration))?;
        timeline.SetPosition(span(next.position.min(next.duration)))?;
        self.controls.UpdateTimelineProperties(&timeline)
    }
}
impl Drop for Controls {
    fn drop(&mut self) {
        let _ = self.controls.RemoveButtonPressed(self.buttons);
        let _ = self
            .controls
            .RemovePlaybackPositionChangeRequested(self.position);
        let _ = self.controls.SetIsEnabled(false);
    }
}
