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

## Follow-up optimizations (2026-09-10)

Implemented on top of `d254fab`:

- **GBA OBJ:** rasterize each object's visible horizontal span once per scanline,
  decoding OAM metadata outside the pixel loop. Keep the best two OBJ pixels for
  blending and a separate object-window mask. Disabled OBJ/window rendering does
  not allocate or clear a scanline buffer. Affine/double-size bounds, wrapping,
  mosaic, palette modes and OAM ordering retain the existing behavior.
- **GBA text BG:** cache one decoded eight-pixel tile row per text layer per
  scanline. Wrapped tile coordinates and source row identify the cache entry;
  scanline-local caches cannot carry stale VRAM/palette/register data across lines.
- **GBA HALT:** skip CPU dispatch and repeated PPU polling up to the nearest
  HBlank/scanline boundary or timer overflow. All overflows are boundaries,
  including those with IRQ disabled, because they can drive cascade/FIFO DMA.
  Newly enabled or reconfigured timers synchronize before batching. Timer and
  audio mixing still run in their original one-cycle order during HALT, even
  with an experimental audio granularity configured. Both frame-stepping paths
  use the same scheduler.
- **YM2612:** cache phase increments/keycodes using their complete input values,
  and share PM calculation for operators with the same FNUM. FMS, LFO step/sign,
  channel-3 special frequencies, MUL and detune participate in the key. The
  derived cache encodes zero bytes and starts cold after decoding, preserving
  the existing save-state layout. Waveforms, sample rate and envelope processing
  are unchanged.
- **GBA states:** count serialized bytes without allocating a payload on load;
  write header and payload into one exactly sized buffer on save. ROM CRC is
  cached lazily and invalidated when replacing the ROM. Existing and legacy
  payload layouts remain supported.
- **Paused/minimized frontend:** skip unchanged egui generation and GPU
  presentation. SDL input wakes the paused event wait, with the waking event
  retained for processing. egui repaint deadlines handle tooltips, caret and
  animation; HUD expiry requests its own repaint. Actual redraws retain native
  frame pacing so immediate animation requests cannot busy-loop. Minimized
  windows skip frame uploads and presentation; active emulation/audio continue.

### Follow-up measurements

Three sequential release runs, after compilation and with the test app closed;
median times below. Tests run with one test thread. Reference and optimized
paths are compiled into the same test binary: the old pixel renderers, original
FM phase calculation, one-cycle HALT dispatch, and allocating state-size check
serve as references. These are synthetic workload measurements, not overall
application FPS or guarantees for commercial ROMs.

| Workload | Reference | Optimized | Reduction |
| --- | ---: | ---: | ---: |
| OBJ + object-window rasterization, 160 lines | 12.5472 ms | 0.3173 ms | 97.5% |
| One text BG layer, 160 lines | 0.4396 ms | 0.3027 ms | 31.1% |
| HALT/interrupt loop, one frame including audio | 4.8315 ms | 2.1824 ms | 54.8% |
| State payload size calculation only | 2.3797 ms | 0.0024 ms | 99.9% |
| Six active FM channels, 532,670 hardware samples | 104.076 ms | 82.269 ms | 21.0% |

OBJ/BG pixel checksums and FM PCM checksums matched in every run. HALT state
CRC was `EAF351` on both paths. The state-size result measures the eliminated
size-check allocation, not full save/load latency. Cache benefits depend on
scene content and register-write frequency.

### Follow-up validation

- Workspace tests: **1,667 passed, 13 ignored**, including doc tests.
- Pixel-reference comparisons cover OBJ priority, semi-transparency, affine
  matrices/double size, disabled objects, object windows, wrapped coordinates,
  mosaic, 4/8-bit pixels, text BG scroll and all map sizes. Existing scanline
  snapshot/DMA regression tests remain passing.
- HALT comparisons cover both frame paths, all timer prescalers, cascade, FIFO
  DMA and display interrupts; frame cycles, pixels, PCM and serialized state
  match the one-cycle reference.
- FM comparisons cover 8,192 frequency/operator configurations and 16,384 PCM
  samples with register writes, special-mode changes and state restoration.
- State tests compare the original byte layout, legacy loading and ROM CRC
  invalidation after replacing a ROM.
- Release build and targeted clippy completed; existing unrelated warnings
  remain. Formatting and whitespace checks pass.
- Release app on Metal: pause, save, memory edit `00 -> 2A`, load restoring `00`,
  HUD expiry without input, minimize/restore, resume and clean exit checked.
  A settled paused 300-iteration profiler window reported UI and presentation
  mean/p95 `0.0000/0.0000 ms`; resumed core/UI/presentation work was observed.

```sh
cargo test --release -p emulator-gba benchmark_followup_render_halt_and_state -- --ignored --nocapture --test-threads=1
cargo test --release -p megadrive-core benchmark_fm_phase_cache -- --ignored --nocapture --test-threads=1
cargo test --workspace
```
