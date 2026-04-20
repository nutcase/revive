# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Revive is a unified SDL2 launcher for four separate Rust emulator cores maintained as sibling repositories. The cores are consumed as **path dependencies** from directories adjacent to this workspace:

- `../nes-rust/crates/core` → `nes-emulator`
- `../snes-rust/crates/core` → `snes-core` (crate name `snes_emulator`)
- `../megadrive/crates/core` → `megadrive-core`
- `../pce/crates/core` → `pce-core` (imported as `pce`)

Those sibling checkouts must exist for `cargo build` to succeed. If a core's public API changes, the adapter for that system in `revive-core` has to be updated in lockstep.

## Common commands

```sh
cargo run                           # open file picker
cargo run -- --select               # force file picker
cargo run -- <rom>                  # auto-detect system by extension
cargo run -- run <rom> --system nes # explicit system
cargo run -- run <rom> --cheats cheats/game.json
cargo run -- <rom> --no-audio

cargo build
cargo test                          # workspace tests (revive-core + revive-cheat)
cargo test -p revive-cheat
cargo test -p revive-core detects_megadrive_bin_header
```

SDL2 is vendored via `sdl2 = { features = ["bundled", "static-link"] }`, so no system SDL install is required, but a C/C++ toolchain and CMake are. `.cargo/config.toml` sets `CMAKE_POLICY_VERSION_MINIMUM=3.5` to work around newer CMake versions rejecting the bundled build.

## Architecture

Three-crate workspace, default binary is `revive-cli`:

- **`revive-core`** — Defines the `CoreInstance` enum that wraps each backend (`NesAdapter`, `SnesAdapter`, `MegaDriveAdapter`, `PceAdapter`) and exposes a uniform surface: `load_rom`, `step_frame`, `frame` (returning an `RGB24` `FrameView`), `audio_spec`/`drain_audio_i16`, `set_button`, `memory_regions`/`read_memory`/`write_memory_byte`, `save_state_to_slot`/`load_state_from_slot`, `flush_persistent_save`. `SystemKind` and `VirtualButton` are the two abstractions every adapter translates to/from its native types. `detect_system` handles extension-based routing, with a `SEGA` header check as the only disambiguator for `.bin`.

- **`revive-cheat`** — UI-independent. `CheatSearch` runs incremental RAM scans (`SearchFilter` variants split into snapshot-needing vs. value-only in `needs_snapshot`). `CheatManager` persists `CheatEntry` lists as JSON. No dependency on `revive-core` — entries reference regions by string ID (`"wram"`, `"sram"`, `"cpu_ram"`, `"prg_ram"`, `"cart_ram"`, `"bram"`) which the CLI passes straight through to `CoreInstance::write_memory_byte`.

- **`revive-cli`** — The SDL2 frontend. One ROM at a time, single event loop in `run_sdl_loop`. `FrameClock` paces the main thread using per-system native refresh rates (NES/SNES 60.0988, MD 59.9227, PCE 60). Each frame: poll events → apply cheats → `step_frame` → re-apply cheats → drain audio into the `AudioQueue<i16>` → blit the `FrameView` into a streaming RGB24 texture. Key mapping is per-system (`map_nes_key`, `map_snes_key`, etc.); state slots are handled in `handle_state_key` with Ctrl/Cmd as the save modifier.

### Per-system quirks to be aware of

- **NES**: controllers are bitmasks (`nes_button_mask`), pushed to the core via `set_controller`/`set_controller2` on every press/release.
- **SNES**: audio uses `AUDIO_BACKEND=sdl_callback` — the env var is set around `SnesEmulator::new` and restored afterwards. Audio is pulled per-frame with a 60 Hz remainder accumulator in `drain_audio_i16`. Framebuffer is `u32` ARGB and is re-expanded to RGB24 each frame.
- **Mega Drive**: both pads default to 6-button. `step_frame` loops `step()` until `frame_ready`.
- **PC Engine**: joypad is an active-low byte (`pad_state` starts at `0xFF`). HuCard ROMs (`.pce`) load backup RAM (`.sav`) and BRAM (`.brm`) siblings to the ROM on boot; raw binaries are loaded at `$C000`. `flush_persistent_save` only writes if `hucard`.

### State and save files

Save states live in `states/<system>/<rom-stem>.slot<N>.<ext>` (relative to CWD), created on demand. Extensions: NES uses its own internal path (`nes.save_state(slot, stem)`); SNES `.sns`, MD `.mdst`, PCE `.pcst`. SRAM/backup is separate and flushed on clean exit only (`core.flush_persistent_save()` in the shutdown path — crashes skip it).

## Coding notes

- The `CoreInstance` match arms are exhaustive by design. Adding a method requires adding it on every adapter; adding a system requires a new enum variant plus matches in every `match self` block plus `detect_system`, `SystemKind::parse`/`label`/`state_dir`, and the CLI's `map_key`/`FrameClock`.
- `VirtualButton` is a superset of all systems' buttons. Adapters return `None` from their mapping helper for unsupported buttons and the `set_button` call is a no-op — don't assume every button maps.
- Memory region IDs are `&'static str` in `MemoryRegion` but `String` in `CheatEntry` (deserialized JSON). Keep the canonical IDs in sync between the adapter and the cheat-file documentation in `README.md`.
