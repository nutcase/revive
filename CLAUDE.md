# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Revive is a unified SDL2/OpenGL/egui launcher for the emulator cores vendored into this repository under `crates/cores/`. The frontend runs one active ROM at a time and routes system-specific behavior through `revive-core` adapters.

Vendored core crates:

- `crates/cores/nes` → `nes-emulator`
- `crates/cores/snes` → `snes-core` (crate name `snes_emulator`)
- `crates/cores/sg1000` → `sg1000-core`
- `crates/cores/mastersystem` → `mastersystem-core`
- `crates/cores/megadrive` → `megadrive-core`
- `crates/cores/pce` → `pce-core` (imported as `pce`)
- `crates/cores/gameboy/core` → `emulator-core`
- `crates/cores/gameboy/gb` → `emulator-gb`
- `crates/cores/gameboy/gba` → `emulator-gba`

If a core's public API changes, the adapter for that system in `revive-core` has to be updated in lockstep. Upstream sibling repositories are no longer required for a local build; sync core changes into `crates/cores/` explicitly.

## Common commands

```sh
cargo run                           # open file picker
cargo run -- --select               # force file picker
cargo run -- <rom>                  # auto-detect system by extension
cargo run -- run <rom> --system nes # explicit system
cargo run -- run <rom> --cheats cheats/game.json
cargo run -- <rom> --no-audio

cargo build
cargo test                          # workspace tests
cargo test -p revive-cheat
cargo test -p revive-core detects_megadrive_bin_header
cargo test -p sg1000-core
cargo test -p mastersystem-core
cargo test -p nes-emulator save_state
cargo test -p snes-core
```

SDL2 is vendored via `sdl2 = { features = ["bundled", "static-link"] }`, so no system SDL install is required, but a C/C++ toolchain and CMake are. `.cargo/config.toml` sets `CMAKE_POLICY_VERSION_MINIMUM=3.5` to work around newer CMake versions rejecting the bundled build.

## Architecture

Workspace layout, default binary is `revive-cli`:

- **`revive-core`** — Defines the `CoreInstance` enum that wraps each backend (`NesAdapter`, `SnesAdapter`, `Sg1000Adapter`, `MasterSystemAdapter`, `MegaDriveAdapter`, `PceAdapter`) and exposes a uniform surface: `load_rom`, `step_frame`, `frame` (returning an `RGB24` `FrameView`), `audio_spec`/`drain_audio_i16`, `set_button`, `memory_regions`/`read_memory`/`write_memory_byte`, `save_state_to_slot`/`load_state_from_slot`, `flush_persistent_save`. `SystemKind` and `VirtualButton` are the two abstractions every adapter translates to/from its native types. `detect_system` handles extension-based routing, with a `SEGA` header check as the only disambiguator for `.bin`.

- **`revive-cheat`** — UI-independent. `CheatSearch` runs incremental RAM scans (`SearchFilter` variants split into snapshot-needing vs. value-only in `needs_snapshot`). `CheatManager` persists `CheatEntry` lists as JSON. No dependency on `revive-core` — entries reference regions by string ID (`"wram"`, `"sram"`, `"cpu_ram"`, `"prg_ram"`, `"cart_ram"`, `"bram"`) which the CLI passes straight through to `CoreInstance::write_memory_byte`.

- **`revive-cli`** — The SDL2/OpenGL/egui frontend. One ROM at a time, single event loop in `run_sdl_loop`. `FrameClock` paces the main thread using per-system native refresh rates (NES/SNES 60.0988, MD 59.9227, PCE 60, GB/GBA 59.7275). Each frame: poll events → apply cheats → `step_frame` → re-apply cheats → drain audio into the `AudioQueue<i16>` → upload `RGB24` frame data into an OpenGL texture → render the optional egui cheat panel. Key mapping is per-system (`nes_keycode_button`, `snes_keycode_button`, etc.); state slots are handled in `handle_state_key` with Cmd+1-9 for load and Cmd+Shift+1-9 for save on macOS, and Ctrl+1-9 / Ctrl+Shift+1-9 on Windows/Linux.

- **`crates/cores/*`** — Vendored emulator cores copied from the original standalone projects. Keep local fixes here when they are required by Revive.

### Per-system quirks to be aware of

- **NES**: controllers are bitmasks (`nes_button_mask`), pushed to the core via `set_controller`/`set_controller2` on every press/release.
- **SNES**: audio uses `AUDIO_BACKEND=sdl_callback` — the env var is set around `SnesEmulator::new` and restored afterwards. Audio is pulled per-frame with a 60 Hz remainder accumulator in `drain_audio_i16`. Framebuffer is `u32` ARGB and is re-expanded to RGB24 each frame.
- **SG-1000**: implemented as a separate `sg1000-core` crate, not as a Mega Drive mode. The core has a Z80 CPU, TMS9918A-style VDP, SN76489 PSG, 1 KiB mirrored work RAM, and active-low two-button controller ports. ROM auto-detection uses `.sg`/`.sg1000`; `.bin` remains explicit unless it has a Mega Drive `SEGA` header.
- **Master System**: implemented as a separate `mastersystem-core` crate, not as a Mega Drive mode. It shares the Z80/SN76489 family shape with SG-1000, but has its own SMS Mode 4 VDP path, CRAM, 8 KiB mirrored work RAM, standard 16 KiB bank mapper, and `.sms`/`.mk3` detection.
- **Mega Drive**: both pads default to 6-button. `step_frame` loops `step()` until `frame_ready`.
- **PC Engine**: joypad is an active-low byte (`pad_state` starts at `0xFF`). HuCard ROMs (`.pce`) load backup RAM (`.sav`) and BRAM (`.brm`) siblings to the ROM on boot; raw binaries are loaded at `$C000`. `flush_persistent_save` only writes if `hucard`.

### State and save files

Save states live in `states/<system>/<rom-stem>/slot<N>.<ext>` (relative to CWD), created on demand. Extensions: NES `.sav`, SNES `.sns`, SG-1000 `.sgs`, Master System `.smsst`, MD `.mdst`, PCE `.pcst`, GBA `.gbas`. SRAM/backup is separate and flushed on clean exit only (`core.flush_persistent_save()` in the shutdown path — crashes skip it). Cheats default to `cheats/<system>/<rom-stem>/cheats.json`.

## Coding notes

- The `CoreInstance` match arms are exhaustive by design. Adding a method requires adding it on every adapter; adding a system requires a new enum variant plus matches in every `match self` block plus `detect_system`, `SystemKind::parse`/`label`/`state_dir`, and the CLI's `map_key`/`FrameClock`.
- `VirtualButton` is a superset of all systems' buttons. Adapters return `None` from their mapping helper for unsupported buttons and the `set_button` call is a no-op — don't assume every button maps.
- Memory region IDs are `&'static str` in `MemoryRegion` but `String` in `CheatEntry` (deserialized JSON). Keep the canonical IDs in sync between the adapter and the cheat-file documentation in `README.md`.
- Do not add title-specific hack code. Avoid ROM-name, ROM-hash, or single-game conditionals in adapters or cores; fix the underlying hardware behavior, timing, rendering, input, save-state, or mapper/emulation rule and cover it with a focused regression test where practical.
