# Changelog

## 1.0.0 - 2026-05-11

### Added

- **Repeat / loop mode** - Cycle between Off, All, and One with the `r` key. Repeat All wraps the queue; Repeat One restarts the current track. MPRIS `LoopStatus` is fully wired.
- **Keyboard volume control** - Adjust playback volume with `+` / `-` in 5% steps. Volume is displayed in the footer and persisted across sessions.
- **Queue auto-extension** - When playing from Browse > All Songs, the queue automatically fetches the next page as you approach the end, preventing playback from stopping unexpectedly.
- **Queue follows playback** - The Queue view now auto-scrolls to keep the currently playing track visible.
- **Signal handling** - SIGINT and SIGTERM are caught for graceful shutdown, ensuring queue persistence, MPV cleanup, PipeWire rate restoration, and terminal recovery always run.

### Fixed

- **Clippy warnings** - All 30 clippy warnings resolved; CI now gates on `cargo clippy -- -D warnings` and `cargo fmt --check`.
- **Cava temp path conflict** - Cava config now uses a PID-unique temp directory, preventing collisions when running multiple ferrosonic instances.
- **Production unwrap calls** - Replaced remaining production `unwrap()` calls with safe error handling.
- **PipeWire rate restore on shutdown** - The original sample rate is now always restored on exit, even when the process is terminated by a signal.

### Changed

- Queue persistence now saves and restores `repeat_mode` and `volume` alongside the queue and position.
- Footer now displays repeat mode and volume status.
