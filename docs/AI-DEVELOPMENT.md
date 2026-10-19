# AI contributor guide

This is a practical guide for coding agents and people extending neisora. Read it
with the current code; old audit reports and release notes are historical evidence,
not a guarantee about the current tree. User instructions and the task's established
authorization take precedence over this guide.

`AGENTS.md` is the entry point for agents that discover repository instructions
automatically. If your assistant does not read it automatically, explicitly ask it
to read `AGENTS.md` and this document before editing; no specific AI vendor or
plugin is required.

## Start here

Run `git status --short` and check the branch. Read `AGENTS.md`, the agreed scope in
`docs/FEATURES.md`, and the files/tests concerned. Preserve other work in progress.
Use the versions in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`
and `src-tauri/tauri.conf.json`; a README badge may describe the published stable
version while the working tree is a newer candidate.

Svelte 5 renders a custom interface. Rust makes YouTube Music requests and controls
an external mpv process over local IPC. The ordinary playback path does not embed
the YouTube web application. A separate restricted webview handles Google login.

| Area | Main files | Contract |
| --- | --- | --- |
| App state / navigation | `src/App.svelte` | One authoritative queue and current track; native playback is asynchronous. |
| Radio / queue edits | `src/queue.ts`, `src-tauri/src/api.rs`, `model.rs` | Preserve YouTube context, occurrence identity and manual additions. |
| Playback / recovery | `src-tauri/src/player.rs`, `src/SeekBar.svelte` | Validate inputs, cancel superseded work, confirm mpv opened audio. |
| Session restore | `src/session.ts`, `src-tauri/src/session.rs` | Native private checkpoint is authoritative; restore paused. |
| Lyrics | `lyrics.rs`, `karaoke.rs`, `src/ExpandedPlayer.svelte` | Timings come from actual providers/files, never estimated word durations. |
| Local collection | `local_music.rs`, `src/LocalLibrary.svelte` | Opaque local IDs, bounded images, safe roots and file watcher cleanup. |
| Media integration | `media.rs` and Windows adapters | Metadata changes should signal immediately without panel polling. |
| Graphics | `src/Visualizer.svelte`, `visualizerRenderer.ts`, `visualizer.rs` | Display resolution, GPU path, loss recovery, explicit lifecycle. |
| Privacy / desktop actions | `diagnostics.rs`, `sharing.rs`, `desktop_actions.rs` | No raw private data in reports; construct allowed service links. |
| Packaging | `tools/prepare-*.py`, `tools/install.sh`, CI | Package exact helpers/licenses, test the actual installed artifact. |

New features may add files; locate their commands and tests rather than assuming
this table is exhaustive. `main` contains shared code and platform adapters. Keep Linux and Windows
configuration paths intact when changing shared code.

## Radio: the easiest place to cause a subtle regression

A manual Next is not the same as natural EOF. Explicit skips carry
`WATCH_NEXT_TYPE_SKIP_VIDEO`, interaction signals and the latest queue context.
Natural EOF advances the existing queue. Repeat-one is applied only at EOF;
manual Next must still advance. Restoring a session is not a skip.

- Do not use a new playlist request to imitate refreshing a supermix. Preserve
  `queueContextParams`; do not reintroduce the initial mix `params` into a skip.
- `refreshChain` serializes skip responses. Even an obsolete response may advance
  the server context, but it must not replace a newer visible queue.
- `QUEUE_UPDATE_SECTION_QUEUE` contains future items. Do not find the first
  matching video ID and slice everything before it: repeated IDs make this wrong.
- A response can echo the current or skipped item or overlap a continuation page.
  Normalize **automatic radio recommendations**, not every playlist. Do not append
  an old page again, discard new items before an overlap, or drop user-added repeats.
- An endpoint's item ID, `queuePosition` reported to YouTube, and a visible row index
  have different lifetimes. Removing/pruning/reordering rows must not rewrite the
  wrong identifier. Keep fresh endpoint IDs for replacement tails.
- Preserve manual/local additions, removal exclusions, their order and current
  playback. Shuffle is for finite queues, not a replacement recommendation algorithm.

Run the queue unit tests and `tools/radio_skip_check.py` in the fixture suite after
changing any of these rules. The latter reproduces initial duplicates, echoed
current/skipped items, overlapping continuation pages, natural EOF and context
continuity through the real UI. A live account still needs separate verification;
synthetic responses do not prove every YouTube response shape.

## Playback and asynchronous work

Use generations/request counters and account epochs already present in the code.
Check them after awaits, not just before starting a request. A logout, new track,
new queue or new lyric choice invalidates relevant old results. Do not let late
cache fills restore a preference that the user reset in the meantime.

The mpv `loadfile` command returning successfully does not prove audio opened.
Keep the entry-ID/file-loaded confirmation and bounded timeouts. Network recovery
must remain limited per playback generation, preserve position and avoid restarting
another track. Treat local file failure separately from remote stream failure.

Rapid seeks are serialized and the newest target wins. Test actual mpv position,
not only the slider. Stream URLs expire; do not persist them in sessions or reuse
one forever. A prefetched URL alone does not implement gapless playback.

Sleep timers run natively. They override repeat/automatic advancement, cancel a
pending load and keep the user's volume. Preserve this when changing EOF handling.
Do not await a player transition while retaining the playback-state lock that the
transition needs. Keep EOF ownership checks and release guards before other awaits.

Audio device changes belong to mpv's native output configuration. Device IDs are
not executable arguments or arbitrary IPC property names. Enumerate actual devices,
validate selection, serialize changes and preserve the desired device across a
temporary fallback. Do not change the system-wide default sink in app tests.

## Lyrics and interface behavior

Plain text is a legitimate fallback. Word animation requires real word timestamps.
Do not stretch studio timings over a live/remix version just to increase match rate.
Manual version selection must be bound to the selected track and persist without
allowing an old automatic reply to override it. Keep lyric offset behavior clear
when changing versions and preserve local LRC support.

The user wants Share beside Like in the bottom player, not extra Share buttons on
home/search/mix cards or the ordinary browsing interface. Keep narrow layouts usable
from 420 px, including long metadata. Do not add a row of tiny permanent controls
to each queue item; removal and drag handles appear on hover/focus.

Test scrolling with the wheel as well as the scrollbar. Do not stack a JS smooth
scroll loop on native momentum. Cancel RAF promptly at the target and on interaction.
Release listeners, observers, timeouts and WebGL resources on unmount/hidden views.
A hidden workspace can be compositor-throttled: its FPS is not an active-window
benchmark. Xvfb/software rendering is not proof of hardware acceleration.

In Svelte components, avoid a local variable named `state` alongside `$state`: the
store shorthand can shadow the rune. Use a descriptive name such as `outputState`.
Use `untrack` for deliberately capturing an initial reactive prop value.

When editing the real Svelte DOM in a test, changing a reactive element's whole
`textContent` can detach nodes Svelte has cached. For long-title geometry tests,
change the existing text node's `nodeValue`, then restore it.

## Security and personal data

Never copy or mutate the maintainer's Google profile to make a test convenient.
Do not print process environments, cookies, network headers, signed stream URLs
or profile dumps. Prefer disposable profiles and public/synthetic test material.
Use a real account only within explicit authorization; do not publish its data.

On Linux use isolated `XDG_CONFIG_HOME` and `XDG_DATA_HOME`. The internal app ID is
`local.kumite.vesper`; changing it breaks profile continuity. Windows resolves its
profile via Known Folders: overriding `APPDATA` alone does not isolate it. The
Windows GUI harness refuses to run outside a disposable GitHub Actions VM.

Provider content is untrusted. Keep response-size/time limits, strict native ID
validation, constrained URL schemes/hosts, structured subprocess arguments and
safe local-root resolution. The main local webview's IPC capability must not be
granted to login/remote origins. Do not weaken CSP to solve a UI issue or use
`{@html}` for titles, lyrics or errors.

Persist private settings through `api::private_write`, which uses a private,
exclusive temporary file and atomic replacement. Respect the Windows ACL adapter.
Do not hide security advisories with exclusions; record the actual mitigation and
remaining risk. Diagnostic exports must use allowlisted fields/enums, not a regex
that attempts to scrub arbitrary raw logs afterward.

## Checks: use the existing harnesses

Install pinned dependencies with `pnpm install --frozen-lockfile`. Use the supported
Node/pnpm/Rust versions and system prerequisites described in `docs/LINUX.md` and CI.
The maintainer may have an Ubuntu baseline build wrapper; it is machine-local,
not a repository requirement. Do not tell another contributor to depend on `/tmp`
scripts or paths from a previous agent session.

Basic checks:

```sh
pnpm check
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --locked --manifest-path src-tauri/Cargo.toml
cargo clippy --locked --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
node --experimental-strip-types --test tools/queue.test.ts tools/playback.test.ts tools/session.test.ts
node --experimental-strip-types tools/sharing_unit.mjs
python3 tools/check-release.py
```

Commands needing the native GTK/WebKit build must run in a compatible Linux build
environment. Read ignored tests' reasons before claiming all integrations passed.
Run relevant checks; do not add implementation-mirroring tests or repeatedly rerun
unrelated heavy suites without a failure/change that justifies it.

The WebKit fixture suite uses the same components with synthetic provider responses:

```sh
pnpm exec vite build --mode test
cargo build --locked --manifest-path src-tauri/Cargo.toml
NEISORA_TEST_BINARY="$(pwd)/src-tauri/target/debug/neisora" \
  dbus-run-session -- xvfb-run -a -s '-screen 0 1920x1080x24' sh tools/gui-checks.sh
```

This requires `tauri-driver`, a compatible WebKitWebDriver, Xvfb, xdotool,
ImageMagick and Python GTK bindings. Override `NEISORA_TEST_BINARY` if using an
external Cargo target directory. The suite's sequence has fixtures shared between
checks; do not reorder it without reading their setup/cleanup. Keep each test's
provider hook isolated and restore it. Svelte `$state` values can be proxies;
fixture wire snapshots should serialize like IPC rather than `structuredClone`
an arbitrary proxy.

**Never run a test-mode Vite build while a production build is compiling/packaging.**
Tauri embeds `dist`; mixing modes can ship test hooks. Complete the fixture build,
then build production assets explicitly and verify the hook is absent in production.

Production artifact checks include:

- `tools/installed-checks.sh` with `NEISORA_TEST_BINARY` and an isolated profile;
  `NEISORA_SMOKE_LOCAL=1` tests real mpv/local artwork/LRC/history/seek and full restart.
- `tools/playerbar_check.py` tests responsive controls and passive MPRIS signals.
  Polling Metadata during a latency measurement can conceal a broken signal path.
- `tools/security.py` tests real IPC, CSP, escaping and the remote-origin boundary.
- `tools/appimage-check.py` checks bundled helpers/licenses. `tools/appimage-checks.sh`
  uses a WebKit driver matching the image for real playback/restart checks.
- Windows has its own native runtime, GUI, IPC and installer checks in
  `.github/workflows/windows.yml` and `tools/windows-*.py` in the shared source tree.

A fresh native driver process is required between production GUI sessions; an
older WebKit driver can exit after its first session. Let `installed-checks.sh`
manage the restart and terminate only its own process group. Never use `killall`
on the maintainer's app, browser, mpv or system audio server.

The Linux audio output integration test uses a **private** PulseAudio server and
null sinks, never the system default sink:

```sh
NEISORA_TEST_BINARY="$(pwd)/src-tauri/target/debug/neisora" \
  dbus-run-session -- xvfb-run -a sh tools/audio-output-checks.sh
```

The tagged `release.yml` workflow waits for both native Windows checks and the Linux
distribution matrix before publishing. Keep the source archive and AppImage
corresponding-source inventory with binary assets.

## Handing work to another assistant

Leave a short, factual handoff when a task spans sessions or tools:

- The requested outcome, accepted decisions and work still pending.
- The branch and modified files; distinguish existing edits from your changes.
- Invariants affected, checks actually run and exact failures still unresolved.
- The source version, installed version and any built artifact, separately.
- Only disposable test paths and useful reproduction commands; never credentials
  or profile dumps. Machine-local helpers are optional, not project requirements.

Update these instructions when architecture or test commands change. Keep them
about durable contracts; put version-specific results in release notes and audits.
An assistant should not infer a completed feature or successful release from a
previous agent’s intention or an unfinished working-tree change.

## Building, installing and publishing

Build for the baseline platform, not just the current host. Linux release packages
use the Ubuntu/glibc baseline described in `docs/LINUX.md`; Arch-built binaries do
not prove compatibility with older distributions. Prepare runtime and notices with
the existing scripts; never commit their downloaded outputs.

The native build must use production `dist`. Copy/install the native binary **before**
Tauri's AppImage bundler stamps it with the AppImage bundle type. Installing that
stamped binary as an ordinary desktop build can break resource discovery. Use
`tools/install.sh` and its versioned atomic installation instead of overwriting a
running binary. Do not force-restart personal playback to activate an update.

Test the actual AppImage with its own mpv and matching WebKit driver, not an unrelated
host mpv/driver. Windows installers download pinned components and verify hashes;
keep their integrity checks, licenses and negative installer tests. A Linux GNU
cross-check is useful but does not replace native Windows/WebView2/audio tests.
macOS also needs a native build/test; do not advertise it as released prematurely.

Distinguish source version, installed local candidate, CI artifact and public release.
Update version files together when preparing a version; do not retag/publish a
release merely because a local candidate works. Follow the user's actual release
authorization and avoid redundant permission requests when it is already present.

Keep logs, screenshots, audio fixtures, installers, runtime downloads, profiles and
credentials out of Git (`artifacts/`, `release/` and runtime directories are ignored).
Preserve the project's GPL-3.0-only license and bundled dependency notices. Finish with an
accurate summary of behavior, tests and remaining platform/hardware verification.
