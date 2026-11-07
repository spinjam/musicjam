# MusicJam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A macOS player for local MP3/FLAC/WAV files: fast search, double-click/Enter loads into a "Next" deck, headphone preview on a separate output, crossfade to the speakers.

**Architecture:** One binary crate. Pure logic, unit-tested: library scan and tag fallback, search scoring, transport state machine, crossfade gains, waveform peaks, settings. An audio engine owns one render thread per logical output. Each thread decodes with symphonia, resamples to the device rate, mixes, and pushes into an `rtrb` ring that a `cpal` callback drains. The UI talks to the engine through a command channel and gets `EngineEvent`s back.

**Tech Stack:** Rust 2024, gpui-pre =0.3.9 + gpui-component =0.8.0 (same as filejam/dbdelve, from crates.io, no vendor patches), cpal 0.15, symphonia 0.5, rubato 0.16, rtrb 0.3, lofty 0.22, rusqlite 0.41 (bundled), walkdir, deunicode, serde+toml, crossbeam-channel.

**Spec:** `doc/2026-11-07-musicjam-design.md` (§10 holds the post-review decisions)

## Global Constraints

- macOS only; `cargo run`; no bundle.
- Reads only MP3/FLAC/WAV and never touches Qobuz folders.
- Nothing except an explicit user action, or the auto-crossfade at the end of a track, changes what plays on Master.
- Preview audio only ever goes to the Cue output. Output devices are opened for output only.
- Support dir: `~/Library/Application Support/MusicJam/` (`settings.toml`, `library.sqlite`).
- Default crossfade: 5 s.
- Exact pins and a committed `Cargo.lock`. Dependencies are built at opt-level 3 in dev.

## Review Focus

1. Search for "the narcissist blur" and "blur narcissist": *The Narcissist* by Blur comes first, ahead of covers or other "Narcissist" titles → `search::tests::narcissist_ranks_first`.
2. A file without tags (WAV) takes artist/album/title from the path → `library::tests::fallback_from_path`.
3. The auto-crossfade starts so that it ends exactly when On Air ends; it waits while paused; it fires during preview and stops the preview → `transport::tests::*`.
4. Next is empty when On Air ends → the player stops and does not pick a track itself → `transport::tests::stops_when_next_empty`.
5. Library folder missing at rescan → the index is kept → `library::tests::missing_root_keeps_index`.

---

### Task 1: Scaffold and settings
**Files:** `Cargo.toml`, `.gitignore`, `src/main.rs` (empty window titled "MusicJam"), `src/settings.rs`
**Produces:** `Settings { library_root: Option<PathBuf>, master_device: Option<String>, cue_device: Option<String>, crossfade_secs: f32 }`, `settings::load() -> Settings`, `settings::save(&Settings) -> io::Result<()>`, `settings::support_dir() -> PathBuf`.
- [ ] Test `settings_roundtrip` (TOML through a temp dir); a missing file gives the defaults (`crossfade_secs == 5.0`).
- [ ] `git init`, build, test, commit.

### Task 2: Library (scan, tags, index)
**Files:** `src/library/{mod.rs,track.rs,scan.rs,store.rs}`
**Produces:** `Track { id: i64, path: PathBuf, title, artist, album: String, track_no: Option<u32>, duration_secs: f32 }`, `scan::meta_from_path(root, path) -> (artist, album, title, track_no)`, `Library::open(db_path) -> Result<Library>`, `Library::rescan(&mut self, root) -> Result<ScanReport>` (no-op + `ScanReport::RootMissing` if root absent), `Library::tracks() -> &[Track]`.
- [ ] Tests: `fallback_from_path` (`Blur/The Ballad of Darren/03 - The Narcissist.wav` → Blur / The Ballad of Darren / The Narcissist / 3), `missing_root_keeps_index`, `rescan_detects_added_and_removed`.
- [ ] Implement: lofty for tags, path fallback per field, mtime+size to skip unchanged files, SQLite table `tracks`.

### Task 3: Search and folder tree
**Files:** `src/library/search.rs`, `src/library/tree.rs`
**Produces:** `search::Index::new(&[Track])`, `Index::query(&self, q: &str, scope: Option<&Path>) -> Vec<usize>` (indices into tracks, best first, at most 500); `tree::FolderNode { path, name, children }`, `tree::build(root, &[Track]) -> FolderNode`.
- [ ] Tests: `narcissist_ranks_first` (fixtures: Blur/The Narcissist, Some Cover Band/The Narcissist (Blur cover), Blur/The Ballad, Other/Narcissist; queries "the narcissist blur" and "blur narcissist"), `scope_includes_subfolders`, `diacritics_ignored` ("beyonce" finds "Beyoncé").
- [ ] Scoring: every query token must prefix-match some token of title/artist/album/path; weights title 3, artist 2, album 1, path 0.5; +1 for a whole-word match; +4 when the matched tokens cover the entire title; ties sorted by artist, album, track_no.

### Task 4: Decoding, resampling, waveform
**Files:** `src/audio/{mod.rs,source.rs,waveform.rs}`
**Produces:** `Source::open(path, out_rate: u32, start_secs: f32) -> Result<Source>`, `Source::read(&mut self, out: &mut [f32]) -> usize` (interleaved stereo; returns frames, 0 at EOF), `Source::duration_secs()`, `waveform::peaks(path, buckets: usize) -> Result<Vec<f32>>`.
- [ ] Tests on a generated WAV (1 s, 44.1 kHz sine): `reads_all_frames_at_native_rate`, `resamples_to_48k` (frame count ≈ 48000 ± 1%), `start_offset_skips`, `peaks_len_and_range`.

### Task 5: Transport state machine (pure)
**Files:** `src/audio/transport.rs`
**Produces:** `Transport::new(crossfade_secs)`; methods `load_next(id, duration, start)`, `set_start(secs)`, `play()`, `pause()`, `stop()`, `crossfade_now()`, `seek(secs)`, `toggle_preview()`, `advance(dt_secs) -> Vec<Effect>`; `Effect = StartOnAir{id,start} | StartIncoming{id,start} | FinishCrossfade | StopAll | StopPreview | StartPreview{id,start}`; `gains(&self) -> (f32 /*on air*/, f32 /*incoming*/)` using an equal-power curve.
- [ ] Tests: `auto_crossfade_ends_with_track`, `paused_holds_auto`, `auto_stops_preview`, `stops_when_next_empty`, `play_with_nothing_on_air_starts_next_without_fade`, `crossfade_promotes_next_and_clears_slot`.

### Task 6: Output devices and engine
**Files:** `src/audio/{output.rs,engine.rs,tone.rs}`
**Produces:** `output::list_devices() -> Vec<String>`, `output::default_device() -> Option<String>`; `Engine::start() -> (Engine, Receiver<EngineEvent>)`; `Engine::send(Command)`; `Command = SetDevices{master,cue} | SetCrossfade(f32) | LoadNext{track} | SetStart(f32) | TogglePreview | Play | Pause | Stop | Crossfade | Seek(f32) | TestTone(Role)`; `EngineEvent = State(Snapshot) | DeviceLost(Role) | DeviceMissing(Role) | Error(String)`; `Snapshot { on_air: Option<DeckInfo>, next: Option<DeckInfo>, playing, previewing, crossfading }`.
- [ ] Master render thread: owns the Transport, two `Source`s (on air + incoming), mixes with `gains()`, ~10 ms blocks, publishes a Snapshot every 100 ms. Cue render thread: one preview `Source`. Each feeds its ring; the cpal error callback → `DeviceLost`.
- [ ] Test tone: 440 Hz, 0.4 s left then 0.4 s right, on that output only.
- [ ] Manual check: `cargo run --example engine_smoke -- <file>`.

### Task 7: UI
**Files:** `src/ui/{mod.rs,workspace.rs,tree_view.rs,track_list.rs,decks.rs,waveform_view.rs,settings_view.rs}`, `src/actions.rs`
- [ ] Three columns per mockup C1. Tree (click selects scope), search input + virtualised list (`uniform_list`), double-click/Enter → `LoadNext`, arrows move the selection; deck panel with waveforms (click on the On Air waveform = seek, click on the Next waveform = set start, ←/→ nudge 1 s, Shift ±0.1 s), buttons Play/Pause, Stop, Preview, Crossfade, Rescan; device list and alerts.
- [ ] Settings window: folder picker (`cx.prompt_for_paths`), two device menus with Test buttons, same-device warning, crossfade seconds.
- [ ] Verify: `cargo clippy --all-targets -- -D warnings`, `cargo test`, launch and play a track.
