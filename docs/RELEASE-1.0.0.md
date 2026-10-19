# neisora. 1.0.0

Linux x86_64 and Windows x64 share one source tree. Application code is now
GPL-3.0-only; third-party notices are retained. English is the primary README;
[Russian documentation](../README.ru.md) remains available.

## Included

- YouTube Music recommendations, dynamic mixes, search, likes and library.
- Playlist creation/editing, manual queue edits, repeat/shuffle, local music,
  paused session restoration, local history and a native sleep timer.
- Expanded player, optional lyrics, GPU visualizations, themes and wallpaper colors.
- Manual LRCLIB lyric version preview and selection, saved per track, with a reset
  to automatic matching and independent timing offsets for selected versions.
- Native audio output selection. The preferred device persists; disconnecting it
  temporarily selects the system output and reconnecting restores the preference.
- Share beside Like in the bottom player. Browsing cards and menus have no Share
  buttons. Bounded diagnostic export is available in Settings.
- Linux MPRIS and Windows SMTC adapters; per-user Windows NSIS installer.

## Checks and limits

Source checks include Svelte/TypeScript, Rust unit tests, formatting and Clippy.
The isolated Linux WebKit suite exercises playback state, radio skip context,
responsive controls, lyrics, queue/playlist editing, sleep, diagnostics and hostile
IPC inputs. A private Pulse server checks actual mpv routing, disconnect/reconnect,
and pause/seek/volume preservation without accessing personal audio devices.
Production native/AppImage checks use real local playback and fresh profiles.
Windows CI builds and installs NSIS, tests native primitives and exercises the
installed application through WebView2 with synthetic local music.

A GitHub release is published only after the Linux distribution and Windows native
jobs pass. Automated Windows GUI checks use a null audio backend: hardware audio,
media keys, Google login and GPU behavior still need checks on users' machines.
Packages are unsigned; SHA256SUMS checks file integrity, not publisher identity.
Windows installation requires internet for mpv and possibly WebView2.

Mini-player and gapless are deferred. Offline playback covers local files; YouTube
downloads and Pear plugins are not included. The UI is currently Russian.

## Sources and history

1.0.0 starts a new public Git history at the maintainer's request. Earlier history
was backed up privately before rewriting; earlier MIT grants are not revoked.
See [source availability](SOURCES.md). The application identifier stays
`local.kumite.vesper` so existing profiles survive the update.
