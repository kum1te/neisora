# Licensing and corresponding source

The neisora application is GPL-3.0-only; see [LICENSE](../LICENSE) and
[COPYRIGHT](../COPYRIGHT). Dependencies retain their own licenses. Do not replace
third-party notices when changing the application's license.

Each release includes a `neisora-<version>-source.tar.gz` archive of its exact source
commit, including the lockfiles, patched GLib source and build/packaging scripts.
The same source is available through the release's Git tag. Build instructions:
[Linux](LINUX.md), [Windows](WINDOWS.md), [contributor guide](AI-DEVELOPMENT.md).
Cargo and pnpm resolve the pinned dependencies; `tools/prepare-notices.py` records
their versions, source repositories and license texts in binary distributions.

Runtime versions and SHA-256 hashes are pinned in `tools/runtime-manifest.json`:

- Node.js source: `https://nodejs.org/dist/v<VERSION>/node-v<VERSION>.tar.gz`.
- yt-dlp source: `https://github.com/yt-dlp/yt-dlp/releases/tag/<VERSION>`.
- Windows Python source: `https://www.python.org/downloads/source/` for the pinned
  version. Khronos Vulkan loader/headers are built from the pinned commits using
  `tools/prepare-vulkan.py`.
- The Windows installer downloads mpv directly from its upstream release and
  verifies its hash; the installer does not redistribute that mpv/FFmpeg binary.

For AppImage, `licenses/appimage/system-components.txt` records the exact Ubuntu
binary and source package versions. The release's `appimage-sources.json` index
provides exact source archive locations and SHA-256 hashes, with an accompanying
`neisora-<version>-appimage-sources.tar` containing those archives. This includes
mpv and the transitive Ubuntu components staged into the image. Use the Ubuntu
source package build instructions and the recorded baseline to rebuild components;
`tools/prepare-appimage.py` and `tools/prepare-appimage-sources.py` generate the
runtime/source inventory. Native Linux packages use system mpv and GUI libraries.

Release source archives and downloaded dependencies are build outputs and must
not be committed to Git. Earlier versions distributed under MIT keep their original
terms; the new license and rewritten history do not revoke those permissions.
