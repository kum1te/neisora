# neisora. 1.0.0

**The first stable release — for Linux and Windows.**

Your YouTube Music recommendations, your local collection, and a player you can
make your own. Thank you to everyone who helped test neisora along the way.

## Make yourself at home

- **Your music, together.** Explore YouTube Music recommendations, supermixes,
  albums and playlists, or add folders from your own collection.
- **A player worth opening.** Expand the artwork and lyrics, turn on an
  audio-reactive GPU visualizer, and choose your colors — including a palette
  inspired by your wallpaper.
- **Lyrics that follow along.** Synchronized lines, word highlighting when real
  timestamps are available, timing adjustments, and a version picker when the
  automatic match needs a hand.
- **The everyday essentials.** Likes, playlist editing, drag-to-reorder queues,
  shuffle, repeat, media keys, listening history and a sleep timer. Restart the
  app and pick up your saved queue from pause.
- **Your preferred audio output.** Choose headphones or speakers in Settings.
  neisora remembers the device and handles disconnecting and reconnecting it.
- **A fresh Windows welcome.** A per-user installer with neisora artwork and
  English/Russian setup pages that follow your Windows language.

## Pick your download

All builds below are **64-bit x86**. You only need the package for your system.

| System | Download |
| :--- | :--- |
| Linux — run without installing | `neisora_1.0.0_amd64.AppImage` |
| Debian / Ubuntu | `neisora_1.0.0_amd64.deb` |
| Fedora / RPM-based distributions | `neisora-1.0.0-1.x86_64.rpm` |
| Arch / EndeavourOS | `neisora-1.0.0-1-x86_64.pkg.tar.zst` |
| Linux — portable archive | `neisora-linux-x86_64.tar.gz` |
| Windows 10 / 11 | `neisora_1.0.0_x64-setup.exe` |

For AppImage, allow execution before launching:

```sh
chmod +x neisora_1.0.0_amd64.AppImage
./neisora_1.0.0_amd64.AppImage
```

Linux builds target **Ubuntu 22.04 / glibc 2.35 or newer**. AppImage includes
its GUI and playback libraries; native packages and the portable archive need
system dependencies. Windows setup needs **internet access** to fetch verified
playback components and WebView2 when needed. The packages are currently unsigned;
Windows may show an unknown-publisher warning. Use `SHA256SUMS` to check downloads.

[Installation guide](https://github.com/kum1te/neisora/blob/v1.0.0/README.md#installation)
· [Русская документация](https://github.com/kum1te/neisora/blob/v1.0.0/README.ru.md)

## A few things to know

- The application interface is currently **Russian**.
- Sign in through **Настройки → Аккаунт** for your personal recommendations,
  library and likes. Add local music folders in Settings.
- Some tracks only have plain lyrics; word highlighting requires actual word
  timestamps. It is never generated from guessed timings.
- neisora is an independent client using unofficial YouTube Music endpoints.
  Changes on YouTube's side may affect compatibility.
- Mini-player, gapless playback and macOS support remain future work.

## Open source, from the first note

neisora is released under **GPLv3** (`GPL-3.0-only`). Third-party components keep
their own licenses and notices. This release includes the exact application source,
AppImage dependency sources and their inventory; these are for developers and are
not needed to install the player.

[Source availability](https://github.com/kum1te/neisora/blob/v1.0.0/docs/SOURCES.md)
· [Report a bug](https://github.com/kum1te/neisora/issues)

Enjoy the music. **neisora.**
