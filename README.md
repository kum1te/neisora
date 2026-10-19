<div align="center">
  <img src="docs/assets/hero.svg" alt="neisora. — Your music, in a new light." width="100%" />
  <br /><br />
  <a href="https://github.com/kum1te/neisora/releases"><img src="https://img.shields.io/badge/version-1.0.0-cdb4fa?style=flat-square&amp;labelColor=19151f" alt="Version 1.0.0" /></a>
  <img src="https://img.shields.io/badge/desktop-Linux_%2B_Windows-cdb4fa?style=flat-square&amp;labelColor=19151f" alt="Linux and Windows" />
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPLv3-cdb4fa?style=flat-square&amp;labelColor=19151f" alt="GNU GPLv3" /></a>
  <br /><br />
  <strong>Your YouTube Music recommendations. A custom interface. Native audio.</strong>
  <br /><br />
  <a href="#features">Features</a> · <a href="#installation">Download</a> · <a href="#development">Development</a> · <a href="docs/SECURITY.md">Security</a>
  <br /><br />
  <strong>English</strong> · <a href="README.ru.md">Русский</a>
</div>

<br />

> [!IMPORTANT]
> **Linux and Windows x64 are the 1.0.0 targets. macOS is planned.**
>
> **Possibly**, support for other music services will follow. This is an idea, not a commitment.

**neisora.** is an independent YouTube Music desktop client built with Svelte, Tauri and Rust. Your home feed, mixes, supermix and playlists come from YouTube Music. mpv plays the audio; ordinary listening does not load the YouTube web application.

## Features

| | |
| :--- | :--- |
| **Made for your music** | Personalized recommendations, search, albums, playlists and dynamic mixes from YouTube Music. Manual skips preserve the service's recommendation context. |
| **An expanded player** | Artwork, timeline, controls and optional lyrics, with an animated layout when lyrics are hidden. |
| **Lyrics that follow along** | Line synchronization and word highlighting when actual timestamps exist. Preview and save a different LRCLIB version for a track, or return to automatic matching. |
| **A reactive background** | GPU visualizations driven by real audio: ribbons, rings and waves, with adjustable appearance and intensity. |
| **Your colors** | Preset palettes, custom themes and wallpaper colors in supported desktop environments. |
| **Your collection** | YouTube Music likes, library categories, album saves and playlist creation/editing. |
| **Local music** | Add folders, search tags, view embedded artwork and play files offline. Mix local and YouTube tracks in one queue. |
| **Queue control** | Drag to reorder, play next, remove, shuffle finite queues, repeat a track or queue, and save YouTube queue entries as a playlist. |
| **Pick your output** | Choose a native audio device, keep the preference across restarts, and temporarily fall back when it disconnects. |
| **Everyday essentials** | Media keys, a sleep timer, local listening history and paused session restoration. Share the current track beside the Like button. |
| **Private diagnostics** | Preview, copy and clear a bounded diagnostic report without cookies, stream URLs, local paths or listening metadata. No automatic upload. |

The current interface is in Russian. Plain lyrics remain plain when timestamps are unavailable. YouTube downloads, Pear plugin compatibility, a mini-player and gapless playback are not included in 1.0.0.

## Installation

Download from [GitHub Releases](https://github.com/kum1te/neisora/releases) and verify the file against `SHA256SUMS`.

### Linux x86_64

Choose **AppImage**, **.deb**, **.rpm**, **Arch package**, or the **portable archive**. The baseline is Ubuntu 22.04 / glibc 2.35.

```sh
# AppImage: make executable, then launch
chmod +x neisora_1.0.0_amd64.AppImage
./neisora_1.0.0_amd64.AppImage

# Debian / Ubuntu
sudo apt install ./neisora_1.0.0_amd64.deb

# Fedora
sudo dnf install ./neisora-1.0.0-1.x86_64.rpm

# Arch / EndeavourOS
sudo pacman -U ./neisora-1.0.0-1-x86_64.pkg.tar.zst
```

AppImage includes mpv, Python and the GUI libraries. Native packages install the required system dependencies; the portable archive also needs system libraries. Node.js and yt-dlp are pinned and included. ImageMagick is optional for wallpaper colors. See [Linux requirements and troubleshooting](docs/LINUX.md).

### Windows x64

Download **`neisora_1.0.0_x64-setup.exe`** for Windows 10/11. Installation is per user and requires internet: the installer downloads mpv with a pinned SHA-256 check, and WebView2 when needed. Python and Node.js are included. The installer is unsigned, so Windows may display a publisher warning.

See [Windows setup and limitations](docs/WINDOWS.md) and the [testing checklist](docs/WINDOWS-TESTING.md). Automated checks exercise the native installer, WebView2 and mpv; real audio hardware, Google login and media keys also need verification on users' machines.

### Sign in and make it yours

Open **Настройки → Аккаунт → Войти через Google**. Sign-in uses a separate Google webview. Guest browsing is available; your personal mixes, library and likes require an account. Playback has been used without Premium.

Add music folders under **Настройки → Локальная музыка**, choose an output under **Аудиовыход**, and customize colors and visualizations under **Оформление**. The expanded player opens from the bottom bar. Its lyric version button offers previews and a reset to automatic matching.

The local history keeps the latest 500 listens and can be cleared. It is separate from Google listening history. See [local music](docs/LOCAL-MUSIC.md) and [lyrics and artwork](docs/LYRICS.md).

## Development

Read [AGENTS.md](AGENTS.md) and the [AI and contributor guide](docs/AI-DEVELOPMENT.md) before editing. They cover queue invariants, asynchronous work, isolated profiles, security boundaries and the actual test harnesses. Shared Linux and Windows sources live in `main`.

For Linux, install Node.js 22.12+, pnpm 10, Rust 1.96, the GTK3/WebKitGTK 4.1 development packages, D-Bus/OpenSSL development libraries, mpv and Python 3:

```sh
git clone https://github.com/kum1te/neisora.git
cd neisora
pnpm install --frozen-lockfile
pnpm release
sh tools/install.sh
```

Use [the Linux build guide](docs/LINUX.md) for distribution packages and [the Windows guide](docs/WINDOWS.md) for an NSIS build. Pinned runtime versions and hashes are in `tools/runtime-manifest.json`. The historical `local.kumite.vesper` app identifier is preserved so upgrades retain existing profiles.

## What's next

macOS, a mini-player and gapless playback are future work. Other music services are **possibly** coming later. See the [feature plan](docs/FEATURES.md) and [1.0.0 release notes](docs/RELEASE-1.0.0.md).

YouTube Music access uses unofficial endpoints; service changes can affect compatibility. Executable runtime components are updated with application releases, not downloaded automatically during normal use.

## License

neisora application code is licensed under **[GNU GPL version 3](LICENSE)** (`GPL-3.0-only`). Third-party components retain their licenses and notices, included with binary distributions. Build scripts and dependency versions are included in the source tree; see [source availability](docs/SOURCES.md).

<div align="center"><sub>Independent project. Not affiliated with Google or YouTube.</sub></div>
