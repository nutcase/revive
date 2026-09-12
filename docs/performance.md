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

## Third performance pass

- GB/GBC clears the LCD-off framebuffer once per transition. Reset and state
  restoration invalidate the derived flag as needed; the serialized PPU layout
  is unchanged.
- GBA advances timers/audio in batches ending at sample or timer boundaries.
  Pre-overflow cycles finish before FIFO/DMA latching, and the overflow cycle's
  audio is mixed afterwards. The experimental coarse mode remains opt-in.
- Mega Drive allocates debug scanline VRAM history on first capture. Adjacent
  identical lines share immutable storage, with copy-on-write for updates.
  With capture disabled, the former 240 × 64 KiB = **15 MiB** allocation is absent.
  Custom serialization preserves the old vector/array bytes in both directions.
- RGB24 frames upload directly as packed R8 textures. The shader retrieves the
  three channels and decodes sRGB, eliminating the per-frame CPU RGBA expansion
  and reducing source texture bytes from four to three per pixel. Other pixel
  formats keep their existing path. This trades CPU conversion for GPU work;
  synchronized render measurements accompany the CPU upload measurements.
- `load_rom_with_audio(..., false)` now reaches NES, GB/GBC, GBA, SG-1000,
  Master System, Mega Drive and PCE, in addition to the existing SNES setting.
  Hardware clocks, filters, oscillators and interrupts continue to advance;
  host PCM accumulation is suppressed. GBA retains at most one stereo pair
  for the existing slew limiter and clears it at the usual per-frame drain.
  Adapters retain the host setting and reapply it before stepping after state
  restoration. The setting adds no save-state bytes.

### Third-pass verification and measurement

Regression coverage includes LCD disable/re-enable and state restoration;
GBA one-cycle timer/FIFO-DMA/PCM/state comparisons across prescalers and sound
sample rates; legacy Mega Drive history bytes; muted/resumed synthesis state
and PCM for each audio backend;
and synthetic ROMs exercising every newly connected no-audio adapter.
The explicit GPU readback test covers all 256 channel values, odd row widths,
multiple rows and format changes (one code-value tolerance for sRGB rounding).

Manual benchmarks are ignored tests, run explicitly after compilation:

```sh
cargo test --release -p emulator-gb benchmark_lcd_off_steps -- --ignored --nocapture --test-threads=1
cargo test --release -p emulator-gba benchmark_timer_event_batches -- --ignored --nocapture --test-threads=1
cargo test --release -p revive-cli benchmark_rgb_upload_and_render -- --ignored --nocapture --test-threads=1
```

Three sequential runs on Apple M2 / Metal, with compilation finished and the
verification app closed. Medians below; all references and optimized paths
are in the same binary. These are synthetic component measurements, not game
FPS claims. The GPU benchmark warms up 20 frames, measures 100, and waits for
GPU completion on every frame, so its total includes driver/scheduling latency.

| Workload | Reference | Optimized | Change |
| --- | ---: | ---: | ---: |
| GB LCD off, 100,000 steps | 150.544 ms | 0.764 ms | 99.5% less |
| GBA timers/audio, 2M cycles in 4-cycle calls | 35.137 ms | 14.192 ms | 59.6% less |
| GBA timers/audio, 2M cycles in 1000-cycle calls | 34.386 ms | 1.169 ms | 96.6% less |
| RGB CPU conversion/upload, 320×240 target | 150.795 µs/frame | 80.189 µs/frame | 46.8% less |
| RGB CPU conversion/upload, 1280×960 target | 238.538 µs/frame | 163.312 µs/frame | 31.5% less |
| RGB synchronized total, 320×240 target | 1.407 ms/frame | 1.323 ms/frame | 5.9% less |
| RGB synchronized total, 1280×960 target | 1.635 ms/frame | 1.664 ms/frame | 1.7% more |

One-cycle timer calls measured 36.779 ms versus 35.105 ms; they use the original
per-cycle operations without event scheduling. Treat the small difference as
measurement/compiler variation. RGB upload saves main-thread work and texture
storage; an end-to-end improvement at enlarged window sizes was **not**
established (individual 1280×960 runs varied in both directions). No claim is
made that moving conversion to the GPU accelerates every GPU or window size.

### Evaluated but not adopted: SNES shared OBJ lookup

The main/sub-screen sharing candidate passed window/time-over pixel comparisons,
but worsened the synthetic 256,000-pixel composition benchmark. The first
implementation measured 132.582 ms versus 170.041 ms (three-run medians).
A second version passed resolved sprite tuples directly into composition and
used identical opaque coordinate inputs on both paths; it measured 193.761 ms
versus 255.158 ms in a standalone release run. These two builds used different
feature-unified dependency graphs, so compare **within** each pair only.
The SNES runtime changes were removed; the remaining five candidates were kept.

The rejected second experiment is preserved in
[`experiments/snes-shared-obj.patch`](experiments/snes-shared-obj.patch) for
reproduction in a disposable worktree. Apply it there and run:

```sh
cargo test --release -p snes-core --lib benchmark_shared_obj_screens -- --ignored --nocapture --test-threads=1
```

Native verification used a synthetic NES RGB24 ROM with `--no-audio`: rendered
color, panel toggling, save/load HUD notifications, rendering after restore,
and clean exit were observed.

Final workspace tests: **1,677 passed, 0 failed, 16 ignored**. The ignored GPU
readback test also passed when run explicitly. Workspace Clippy completed with
warnings, including existing HUD float-literal warnings and chunk-iteration
style suggestions in the new GPU tests. The release build also completed.

## PCE sprite span rendering

The sprite renderer now visits each accepted sprite's clipped horizontal span
instead of searching the sprite list at every display pixel. Each 16-pixel
pattern row loads its four VRAM words once per span. A scanline-local occupancy
mask preserves the first opaque sprite's ownership even when the background
hides that pixel; transparent pixels do not claim ownership. Sprite selection,
cell-slot limits and overflow reporting retain the prior behavior. No persistent
cache, public API, or save-state layout changes are introduced.

The pre-change pixel-first renderer is retained only in tests. Differential
coverage compares pixels, line counts, VDC status and serialized bus state for
256 combinations of scenes and rendering options: horizontal/vertical flips,
all sprite sizes, clipping and per-line display offsets/widths, background
priority, overlap/transparency, VRAM wrapping, current/snapshotted VRAM,
programmed vertical windows, CG plane selection, row interleaving, raw pattern
indices, reverse priority and enabled/disabled sprite limits. A separate
regression verifies that an opaque sprite behind BG still blocks a later
sprite above BG. Existing sprite/SATB/save-state tests also pass.

### Measurements

Apple M2, release profile; reference and optimized renderers in the same test
binary. Three sequential runs after compilation, each with 20 warm-up frames
and six paired batches of 200 calls, alternating reference/optimized order.
The table gives the median of each run's batch medians. Each call renders the
sprite pass for a 512-by-240 framebuffer with the normal sprite limit. The
fixtures use deterministic synthetic patterns and SATB data. Source setup,
allocation and assertions are outside the timed region.

| Sprite workload | Reference (µs/frame) | Spans (µs/frame) | Reduction |
| --- | ---: | ---: | ---: |
| Spread, including clipped edges | 1058.385 | 209.461 | 80.2% |
| Dense rows | 1013.654 | 221.445 | 78.2% |
| Overlapping sprites | 217.968 | 50.896 | 76.6% |
| Offscreen sprites | 660.178 | 47.317 | 92.8% |
| Transparent overlapping sprites | 288.631 | 62.534 | 78.3% |

These are sprite-pass times, including sprite selection, not complete emulated
frame times or game FPS. CPU, background rendering, audio and presentation are
outside this benchmark. Whole-game mean/p95 improvements remain unmeasured.

```sh
cargo test -p pce-core
cargo test --release -p pce-core benchmark_pce_sprite_spans -- --ignored --nocapture --test-threads=1
```

Validation: `cargo test --workspace --no-fail-fast` passed with **1,679 passed,
0 failed, 17 ignored**. Targeted Clippy (`pce-core`, `revive-core`, `revive-cli`,
all targets) and `cargo build --release` completed with existing warnings.
Formatting checks for changed Rust files and whitespace checks pass.

## PCE palette RGB lookup

`Vce::palette_rgb` now indexes a 512-entry RGB table with the current palette
word's low nine bits. The default table is computed at compile time and uses
2 KiB shared across VCE instances. With `runtime-debug-flags`, the table is
initialized once using `PCE_FORCE_BRIGHTNESS`, retaining the previous first-use,
process-wide setting and parsing behavior. Both truncating divisions and channel
saturation remain identical to the former arithmetic.

The key is the color value itself, so byte writes, CRAM DMA, direct debug writes,
reset and restored palette values need no cache invalidation. No VCE fields or
serialized bytes change. Regression tests compare all 65,536 raw words against
the original arithmetic at all 16 brightness levels, check intermediate byte
writes and address wrapping, and round-trip the pre-change serialized layout.
Separate processes also verify the feature-enabled lookup at all brightness
levels, with an unset override, masked `FF`, and invalid input.

### Measurements

Apple M2, release profile. The isolated lookup benchmark measures 122,880 color
reads/output stores, with both paths in one binary, alternating order, two
warm-up batches and six measured batches of 100 iterations. The median was
**155.752 µs → 76.822 µs (50.7% less)**.

Rendering measurements use two standalone copies of the PCE crate with identical
fixtures, dependencies and release settings (`opt-level=3`, thin LTO, one codegen
unit). The reference uses `vce.rs` from `2d7e09f`; the optimized copy uses the RGB
lookup. The additional VCE unit-test module is omitted from the optimized copy
to retain the same test inventory in both binaries. Both builds finish before
measurement, and before/after run order alternates across three pairs. Each
workload warms up for 30 calls and times 300 calls to `render_frame_from_vram`.
The table reports medians of the three per-run means and p95s, in microseconds.

| Synthetic 256-pixel-wide rendering workload | Mean before | Mean after | Reduction | p95 before | p95 after |
| --- | ---: | ---: | ---: | ---: | ---: |
| Background only | 255.519 | 203.812 | 20.2% | 297.208 | 248.500 |
| Background and sprites | 427.318 | 380.009 | 11.1% | 503.750 | 436.875 |
| Background/sprites, 64 palette entries updated per call | 445.549 | 375.950 | 15.6% | 501.666 | 437.667 |

Final serialized-bus checksums (including the framebuffer) agree in all runs:
`6EE4A0FA`, `E3CCBE58`, and `CB2FADCF`, respectively. Palette-animation writes are
inside the timed interval; fixture allocation and checksumming are outside.
These measure CPU rendering, not CPU emulation, audio, presentation or game FPS.
Absolute times and gains vary with workload and machine.

```sh
cargo test -p pce-core
cargo test --release -p pce-core benchmark_pce_palette_lookup -- --ignored --nocapture --test-threads=1
cargo test --release -p pce-core benchmark_pce_palette_frame_rendering -- --ignored --nocapture --test-threads=1
cargo test -p pce-core --features runtime-debug-flags vce::tests::configured_palette_lookup_matches_reference
```

Workspace validation: **1,682 passed, 0 failed, 19 ignored**. Targeted Clippy
(`pce-core`, `revive-core`, `revive-cli`, all targets), release build, changed-file
Rust formatting and whitespace checks pass. Existing Clippy/build warnings remain.
