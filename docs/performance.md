# Performance changes and validation

Measured on 2026-09-10 against `46d0d95` (the SDL3/wgpu frontend).

## Changes

- GBA shares immutable BG/OBJ VRAM snapshots between unchanged scanlines.
  CPU/DMA and mirrored writes, mutable VRAM access, clear/reset and state load
  invalidate the affected snapshots. Historical scanline reads and the expanded
  save-state byte layout remain available to both rendering paths.
- The cheat panel captures only the active tab, swaps reusable current/previous
  buffers, and skips automatic capture when paused without a memory change.
  Manual Refresh and Auto-off behavior are retained.
- SNES/PCE BGRA bytes upload directly to BGRA textures. Texture recreation checks
  both size and pixel format. RGB24 conversion and RGBA direct upload remain.
- NES/MD/SG-1000/SMS drain audio into reusable buffers. Existing allocating core
  APIs remain available for callers outside the frontend.
- Pause skips core stepping, audio draining and unchanged frame uploads. State
  keys, memory edits and changed cheat values invalidate cached output. Audio
  queued before pausing is cleared once.
- `sega8-common` now uses dev `opt-level=3`, matching the other core crates.
- `REVIVE_PERF=1` reports frontend stage mean/p95 CPU wall times every 300 frames.

## Synthetic GBA measurements

The same ignored benchmark ran on the original GBA/core source in an isolated
workspace and the modified source. Both used the same release profile and
`.cargo/config.toml`. Three runs alternated before/after order after builds
finished; values below are medians of those runs. Each frame run warms up for
30 frames, then measures 300 frames of a synthetic Mode 3 ROM with audio drained.

| Measurement | Before (ms) | After (ms) |
| --- | ---: | ---: |
| Frame mean | 4.3966 | 4.1585 |
| Frame p95 | 5.4127 | 5.3587 |
| Snapshot work: one BG VRAM write per frame | 0.3202 | 0.0150 |
| Snapshot work: BG/OBJ VRAM writes every scanline | 0.3225 | 0.3532 |

Frame mean improved by about 5%; low-update snapshot work dropped by about 95%.
The pixel checksum was `1C03E00` in every run. In the every-scanline-write stress
case, sharing cannot avoid copies and snapshot work increased by about 0.03 ms
(about 10%). These are synthetic results, not performance guarantees for games;
profile representative scenes with and without the cheat panel before drawing
conclusions about an individual title.

## Validation

- `cargo test --workspace`: 1,660 passed, 11 ignored; doc tests completed.
- The ignored real-GPU upload/readback test passed for RGB24, RGBA and BGRA,
  including format changes at the same dimensions.
- `cargo clippy -p revive-cli -p revive-core -p revive-cheat --all-targets --no-deps`
  passed with warnings.
- `cargo build --release -p revive-cli` passed. The existing `hud.rs` float-literal
  warning remains.
- Release app on Metal with a generated GBA ROM: game display, Pause, state save,
  paused memory edit (00 → 2A), state load (2A → 00), Auto off, manual Refresh,
  search filtering to one matching address, resume, and clean exit confirmed.
- The profiler confirmed near-zero core/audio/upload time while paused and
  resumption of those stages after unpausing.

## Reproduce

```sh
cargo test --release -p emulator-gba benchmark_gba_frames_and_snapshots -- --ignored --nocapture
cargo test -p revive-cli gpu_upload_preserves_rgb_rgba_bgra_colors_and_format_changes -- --ignored
REVIVE_PERF=1 cargo run-native -- <rom>
```

Presentation timing includes surface acquisition/VSync waits, not GPU execution
time. Frame pacing is measured separately. Avoid overlapping builds/tests when
collecting measurements or running workspace doc tests.
