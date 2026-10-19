# Working on neisora

Read [docs/AI-DEVELOPMENT.md](docs/AI-DEVELOPMENT.md) before changing playback,
queues, authentication, native commands, packaging or platform code. It explains
invariants and the tests that caught previous regressions. Also read any more
specific instructions in the directory you change.

## Project map

- `src/`: Svelte 5 / TypeScript interface. `App.svelte` owns the playback/queue state.
- `src-tauri/src/`: Rust backend, YouTube Music requests, mpv IPC and private storage.
- `tools/`: regression tests, isolated production tests and packaging helpers.
- `docs/FEATURES.md`: agreed feature scope. Release notes describe behavior at a version.
- `main`: shared Linux and Windows implementation with platform adapters and native CI.
  Check the actual branch and CI before assuming platform support.

## Rules that matter

1. Read current code, `git status`, the applicable instructions and relevant tests.
   Preserve unrelated user edits. Keep work within the user's agreed scope.
   Existing authorization applies; do not invent additional approval steps.
2. Never use a real Google profile as a test fixture. Do not copy, log out,
   overwrite or publish cookies, browser storage, signed media URLs or personal logs.
   Use disposable profiles and synthetic music. Windows Known Folders require the
   dedicated disposable-runner test; setting `APPDATA` is not sufficient isolation.
3. YouTube owns recommendations. Keep its radio context and distinguish explicit
   skip from natural EOF, selection, repeat and session restoration. Do not start a
   fresh radio as a fallback for a failed refresh. Preserve manual queue additions.
4. Do not reintroduce automatic duplicate recommendations into a radio tail.
   Ordinary playlist duplicates and explicit manual repeats are intentional.
   Endpoint IDs, reported listening positions and visible row indices are different.
5. Bind asynchronous results to their request, account and playback generation.
   A late reply must not modify another track, queue, login or lyric selection.
6. Validate IDs and commands in Rust before I/O. Keep network responses bounded,
   URL domains constrained, subprocess arguments structured, and remote webviews
   outside local IPC capabilities. Never render provider strings as HTML.
7. Preserve native mpv playback, exact seek behavior and the single queue owner.
   Do not add a YouTube web player as a shortcut. Never fake lyric word timings.
8. Preserve visual quality while optimizing. Release RAF, timers, observers and
   GPU resources. Keep the queue virtualized and avoid background polling when
   an observed native property or an explicit refresh is sufficient.
9. Run checks appropriate to the change. A passing build is not proof of working
   playback, native clipboard, installer or GPU acceleration. State what was tested.
   Do not claim Windows/macOS support from a Linux cross-check alone.
10. Keep profiles, runtime downloads, installers, logs and screenshots out of Git.
    Preserve the `local.kumite.vesper` application identifier; branding changes
    must not silently create a new profile. Keep the GPL-3.0-only license and third-party notices.

## On the maintainer's desktop

If testing on the maintainer's Hyprland session, put **all test windows on workspace
6 without switching focus**. Do not change the compositor settings or interrupt
personal playback. Other contributors should use an isolated display/session.

## Before handing over

Summarize behavior changes, relevant checks and remaining limitations. Keep feature
status and release notes accurate. Installing a local candidate is different from
publishing a GitHub release. Do not mark unfinished platform tests as passed.
