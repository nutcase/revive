# Revive

Revive is an integrated SDL2 frontend for several vendored Rust emulator cores.

Only one title runs at a time. Revive selects the target system from the ROM
extension or an explicit `--system` option, and `revive-core` hides the API
differences between the underlying emulator cores.

## Supported Systems

- NES / Famicom
- SNES / Super Famicom
- Mega Drive / Genesis
- PC Engine / TurboGrafx-16
- Game Boy
- Game Boy Color
- Game Boy Advance

## Workspace Layout

The emulator cores are vendored into this repository under `crates/cores/`.
Sibling repositories are not required at build time.

```text
crates/cores/nes
crates/cores/snes
crates/cores/megadrive
crates/cores/pce
crates/cores/gameboy/core
crates/cores/gameboy/gb
crates/cores/gameboy/gba
```

Revive-specific crates are split by responsibility.

- `crates/revive-core`: system detection, emulator adapters, and the common runtime API
- `crates/revive-cheat`: UI-independent cheat search, cheat definitions, and JSON persistence
- `crates/revive-cli`: SDL2 + OpenGL + egui frontend

## Requirements

- Rust toolchain
- C/C++ toolchain
- CMake
- Apple Silicon native builds are recommended on macOS

SDL2 is built through the `sdl2` crate's `bundled` / `static-link` features, so
a system SDL2 installation is usually not required.

## Running

If no ROM is passed, Revive opens a local file picker.

```sh
cargo run
cargo run -- --select
cargo run -- <rom>
```

To select a system explicitly:

```sh
cargo run -- run <rom> --system nes
cargo run -- run <rom> --system snes
cargo run -- run <rom> --system megadrive
cargo run -- run <rom> --system pce
cargo run -- run <rom> --system gb
cargo run -- run <rom> --system gbc
cargo run -- run <rom> --system gba
```

To run without audio:

```sh
cargo run -- <rom> --no-audio
```

To use an explicit cheat file:

```sh
cargo run -- <rom> --cheats cheats/custom.json
```

## Apple Silicon Builds

Revive keeps development and play builds separate. The default `cargo run`
path is still useful while editing, but the `release-native` profile is the
intended executable build: `opt-level = 3`, thin LTO, one codegen unit, and
stripped debug info. On Apple Silicon, the Apple aliases target
`aarch64-apple-darwin` with `target-cpu=native`.

```sh
cargo run-native -- <rom>
cargo build-native
cargo run-apple -- --select
cargo run-apple -- <rom>
cargo build-apple
```

The normal `cargo run` path already builds core crates with `opt-level = 3` in
the dev profile, but the release-oriented aliases are better for actual play.

Runtime debug and trace environment flags are disabled in normal optimized
builds for hot-path performance. Enable them only for investigations:

```sh
cargo run-apple -p revive-cli --features snes-runtime-debug-flags -- <rom>
cargo run-apple -p revive-cli --features megadrive-runtime-debug-flags -- <rom>
cargo run-apple -p revive-cli --features gba-runtime-debug-trace -- <rom>
```

## ROM Detection

Revive detects systems from file extensions.

- `.nes`: NES
- `.sfc`, `.smc`: SNES
- `.md`, `.gen`: Mega Drive
- `.pce`: PC Engine
- `.gb`: Game Boy
- `.gbc`: Game Boy Color
- `.gba`: Game Boy Advance
- `.bin`: Mega Drive only when a `SEGA` header is present

If detection fails, pass `--system` explicitly.

## Controls

Common controls:

- `Esc`: quit
- `Tab`: show or hide the cheat panel
- macOS: `Cmd + 1..9` loads state slots 1..9
- macOS: `Cmd + Shift + 1..9` saves state slots 1..9
- Windows/Linux: `Ctrl + 1..9` loads state slots 1..9
- Windows/Linux: `Ctrl + Shift + 1..9` saves state slots 1..9

NES:

- Arrow keys: D-pad
- `Z` / `J`: A
- `X` / `K`: B
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

SNES:

- Arrow keys: D-pad
- `D`: A
- `S`: B
- `W`: X
- `A`: Y
- `E`: L
- `Q`: R
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

Mega Drive:

- Arrow keys: D-pad
- `A`: A
- `Z`: B
- `X`: C
- `S`: X
- `D`: Y
- `F`: Z
- `Q`: Mode
- `Return` / `Space`: Start

PC Engine:

- Arrow keys: D-pad
- `Z` / `J`: I
- `X` / `K`: II
- `Return` / `Space`: Run
- `Backspace` / Shift: Select

Game Boy / Game Boy Color:

- Arrow keys: D-pad
- `X` / `J`: A
- `Z` / `K`: B
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

Game Boy Advance:

- Arrow keys: D-pad
- `X` / `J`: A
- `Z` / `K`: B
- `A`: L
- `S`: R
- `Return` / `Space`: Start
- `Backspace` / Shift: Select

## Cheat Panel

Press `Tab` to open a right-side cheat panel similar to the SNES frontend. While
the panel is open, game input is released and text/UI input takes priority.

The panel has two tabs.

- `Hex Viewer`: inspect memory, jump to addresses, and edit values directly
- `Cheat Search`: take snapshots, filter candidate addresses, add cheats, and manage active cheats

Active Cheats supports:

- toggling entries on and off
- editing the value to write
- editing labels
- deleting entries
- saving and loading

By default, cheats are stored per game.

```text
cheats/<system>/<rom>/cheats.json
```

Examples:

```text
cheats/snes/Super F1 Circus Gaiden (Japan)/cheats.json
cheats/megadrive/Sonic The Hedgehog/cheats.json
```

If `--cheats <path>` is specified, that path is used instead. If an old
`cheats/<rom>.json` file exists, Revive uses it as a load fallback.

## Cheat JSON Format

`revive-cheat` reads and writes the following JSON format.

```json
[
  {
    "region": "wram",
    "offset": 4660,
    "value": 153,
    "enabled": true,
    "label": "Example"
  }
]
```

Main region IDs:

- NES: `cpu_ram`, `prg_ram`
- SNES: `wram`, `sram`
- Mega Drive: `wram`
- PC Engine: `wram`, `cart_ram`, `bram`
- Game Boy / Game Boy Color: `cart_ram`
- Game Boy Advance: cheat target memory is not currently exposed by the core API

Game Boy / Game Boy Color `cart_ram` is currently read-only through the core API.

## Save States

Save states are also separated per game.

```text
states/<system>/<rom>/slot<N>.<ext>
```

Examples:

```text
states/snes/Super F1 Circus Gaiden (Japan)/slot1.sns
states/megadrive/Sonic The Hedgehog/slot1.mdst
states/nes/Super Mario Bros/slot1.sav
states/pce/Adventure Island/slot1.pcst
states/gba/Example/slot1.gbas
```

Legacy save-state files are used as load-only fallbacks.

```text
states/<system>/<rom>.slot<N>.<ext>
states/<rom>.slot<N>.sav  # legacy NES format
```

Limitations:

- GB/GBC save states are not currently exposed by the Game Boy core API.
- GBA save states are supported.

## Persistent Saves

SRAM and backup RAM are handled according to each core's API.

- SNES: `.srm`
- PC Engine: `.sav`, `.brm`
- Game Boy / Game Boy Color: `.sav`
- Game Boy Advance: `.sav`
- NES: SRAM persistence is handled by the core

Persistent saves are usually flushed on normal exit. A crash may prevent the
latest save data from being written.

## Design

Revive is not an emulator that runs multiple cores at the same time. It is a
frontend that selects the right core for one ROM and presents a common UI.

### `revive-core`

`revive-core` hides emulator-specific differences behind adapters. The
`CoreInstance` enum wraps NES, SNES, Mega Drive, PC Engine, Game Boy, and GBA
implementations, and exposes only the common operations to the CLI.

- `load_rom`
- `step_frame`
- `frame`
- `audio_spec`
- `drain_audio_i16`
- `set_button`
- `memory_regions`
- `read_memory`
- `write_memory_byte`
- `save_state_to_slot`
- `load_state_from_slot`
- `flush_persistent_save`

To add another system, extend `SystemKind`, `CoreInstance`, ROM detection, input
mapping, state paths, and memory regions.

### `revive-cheat`

`revive-cheat` has no UI dependency. It only owns cheat searching and cheat
definition persistence.

- `CheatSearch`: candidate narrowing through RAM snapshots and filters
- `SearchFilter`: search conditions such as equal, changed, and increased
- `CheatManager`: add/remove `CheatEntry` values and save/load JSON

Cheats are represented as `region + offset + value`. The actual write target is
resolved by the `revive-core` adapter for the active system.

### `revive-cli`

`revive-cli` is the SDL2 + OpenGL + egui frontend.

Main flow:

1. Read the ROM path from CLI arguments or the file picker.
2. Detect the system from the extension or `--system`.
3. Start the target core with `CoreInstance::load_rom`.
4. In the event loop, process input, save states, and cheat panel actions.
5. Each frame, run `apply_cheats -> step_frame -> apply_cheats`.
6. Feed samples into the audio queue.
7. Upload the RGB24 frame to an OpenGL texture.
8. Draw the egui cheat panel on the right side.

When the panel is open, the viewport reserves the panel width on the right and
renders the game screen into the remaining area while preserving aspect ratio.

## Development Commands

```sh
cargo fmt
cargo check -p revive-cli
cargo check --target aarch64-apple-darwin -p revive-cli
cargo test -p revive-core
cargo test -p revive-cheat
cargo build -p revive-cli
```

When a core changes, run the relevant package tests as well.

Examples:

```sh
cargo test -p nes-emulator save_state
cargo test -p snes-core
cargo test -p megadrive-core
cargo test -p pce-core
cargo test -p emulator-gb
cargo test -p emulator-gba
```

## Known Limitations

- Only one ROM can run at a time.
- GB/GBC save states are not supported yet.
- GBA cheat memory regions are not currently exposed.
- Without `--cheats`, the Save action writes to `cheats/<system>/<rom>/cheats.json`.
- Cores are vendored under `crates/cores/`, so upstream core repository changes must be synced manually.
