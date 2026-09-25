# Revive

Revive is a Rust emulator development workspace and integrated SDL3/wgpu/egui
frontend for several vendored emulator cores. It is built for working on classic
console emulation, save states, memory maps, controller input, audio/video
timing, cheat search, and frontend integration in one local repository.

Only one title runs at a time. Revive selects the target system from the ROM
extension or an explicit `--system` option, and `revive-core` hides the API
differences between the underlying emulator cores.

The repository is useful for people searching for Rust emulator projects,
multi-system emulator frontends, NES/SNES/Game Boy/GBA emulator development,
Sega 8-bit and Mega Drive emulation, PC Engine emulation, SDL3 emulators,
wgpu frame presentation, egui tooling, save-state serialization, cartridge
mappers, PPU/video timing, APU/audio timing, and cheat or RAM search tools.

## Supported Systems

| System | `--system` values | ROM extensions |
| --- | --- | --- |
| NES / Famicom | `nes`, `fc`, `famicom` | `.nes` |
| SNES / Super Famicom | `snes`, `sfc`, `super-famicom`, `superfamicom` | `.sfc`, `.smc` |
| SG-1000 | `sg1000`, `sg-1000`, `sega-sg1000` | `.sg`, `.sg1000` |
| Master System / Mark III | `sms`, `mastersystem`, `master-system`, `sega-master-system`, `markiii`, `mark-iii` | `.sms`, `.mk3` |
| Mega Drive / Genesis | `md`, `genesis`, `megadrive`, `mega-drive` | `.md`, `.gen`, `.genesis`, `.bin` with a Mega Drive `SEGA` header |
| PC Engine / TurboGrafx-16 | `pce`, `pcengine`, `pc-engine`, `tg16`, `turbografx`, `turbografx-16` | `.pce` |
| Game Boy | `gb`, `gameboy`, `game-boy` | `.gb` |
| Game Boy Color | `gbc`, `gameboycolor`, `game-boy-color`, `gameboy-color` | `.gbc` |
| Game Boy Advance | `gba`, `gameboyadvance`, `game-boy-advance`, `gameboy-advance` | `.gba` |
| Nintendo 64 | `n64`, `nintendo64`, `nintendo-64` | `.z64`, `.n64`, `.v64` |
| PlayStation | `ps1`, `psx`, `playstation` | `.cue`, `.zip` containing one CUE and its tracks; raw `.bin` requires `--system ps1` |

## Emulator Development

Revive is intentionally organized so emulator development work can happen at
the hardware-core layer while still being exercised through a shared desktop
frontend.

Good entry points:

- Emulator cores: `crates/cores/*`
- System adapters and common runtime API: `crates/revive-core`
- Cheat search and memory editing model: `crates/revive-cheat`
- SDL3/wgpu/egui frontend loop: `crates/revive-cli`
- Save-state examples: `crates/cores/gameboy/gb/src/state.rs` and `crates/cores/gameboy/gba/src/state.rs`
- Input, frame, audio, memory, save-state, and persistent-save integration:
  `crates/revive-core/src/adapters/`

Common emulator topics represented in the codebase include CPU stepping,
PPU/VDP rendering, APU/PSG audio, cartridge mappers, RAM and VRAM regions,
save-state formats, ROM detection, controller mapping, frame pacing, and
debugging through cheat/memory tools.

## Workspace Layout

The emulator cores are vendored into this repository under `crates/cores/`.
Sibling repositories are not required at build time.

```text
crates/cores/nes
crates/cores/snes
crates/cores/sg1000
crates/cores/mastersystem
crates/cores/megadrive
crates/cores/pce
crates/cores/ps1
crates/cores/gameboy/core
crates/cores/gameboy/gb
crates/cores/gameboy/gba
```

Revive-specific crates are split by responsibility.

- `crates/revive-core`: system detection, emulator adapters, and the common runtime API
- `crates/revive-cheat`: UI-independent cheat search, cheat definitions, and JSON persistence
- `crates/revive-cli`: SDL3 + wgpu + egui frontend

## Requirements

- Rust toolchain
- C/C++ toolchain
- CMake
- GNU make (for the vendored PS1 C core)
- Apple Silicon native builds are recommended on macOS

SDL3 is built through the `sdl3` crate's `build-from-source-static` feature, so
a system SDL3 installation is not required.

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
cargo run -- run <rom> --system sg1000
cargo run -- run <rom> --system sms
cargo run -- run <rom> --system megadrive
cargo run -- run <rom> --system pce
cargo run -- run <rom> --system gb
cargo run -- run <rom> --system gbc
cargo run -- run <rom> --system gba
cargo run -- run <disc.cue> --system ps1
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

## Performance measurements

Set `REVIVE_PERF=1` to log mean/p95 CPU wall time every 300 frames:

```sh
REVIVE_PERF=1 cargo run-native -- <rom>
cargo test --release -p emulator-gba benchmark_gba_frames_and_snapshots -- --ignored --nocapture
cargo test -p revive-cli gpu_upload_preserves_rgb_rgba_bgra_colors_and_format_changes -- --ignored
```

The frontend separates core/cheats, audio, frame upload, UI, presentation and
frame pacing. Presentation includes surface/VSync waits; it does not measure GPU
execution time. Compare the same ROM/scene, audio settings and panel state in
release builds, without another build running in the background. The ignored
GBA benchmark uses a synthetic Mode 3 ROM and includes both unchanged VRAM and
VRAM writes on every scanline; it is not a commercial-game performance estimate.

GBA scanlines share immutable VRAM snapshots until the corresponding memory
changes. CPU/DMA writes, mirrored writes, external mutable memory access, reset
and state load invalidate that cache. Save states retain the original expanded
snapshot layout and historical scanline contents. SNES/PCE BGRA frames upload
directly to BGRA textures; NES/MD/SG-1000/SMS reuse audio output buffers. The
cheat panel captures only its active tab and swaps reusable current/previous
buffers. Pausing skips core/audio work and unchanged frame uploads; state keys,
memory edits and cheats invalidate the cached frame and memory view.

Audio playback initially buffers 50 ms, increasing up to 100 ms if scheduling
stalls empty the queue. Pausing and recovery also refill before resuming.
Brief frame overruns retain the original cadence so they do not accumulate into
an audio deficit. Set `REVIVE_AUDIO_DEBUG=1` to log playback underruns. An opt-in
real-device diagnostic uses a temporary save directory (run it on its own):

```sh
REVIVE_AUDIO_TEST_ROM="$PWD/roms/nintendo64/Super Mario 64 (USA).z64" \
  cargo test --release -p revive-cli local_rom_audio_delivery -- --ignored --nocapture
```

`REVIVE_AUDIO_TEST_FRAMES` overrides the default 900 frames. Avoid concurrent
builds during this diagnostic; it plays sound through the default audio device.

## ROM Detection

Revive detects systems from file extensions.

- `.nes`: NES
- `.sfc`, `.smc`: SNES
- `.sg`, `.sg1000`: SG-1000
- `.sms`, `.mk3`: Master System
- `.md`, `.gen`, `.genesis`: Mega Drive
- `.pce`: PC Engine
- `.gb`: Game Boy
- `.gbc`: Game Boy Color
- `.gba`: Game Boy Advance
- `.z64`, `.n64`, `.v64`: Nintendo 64
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

SG-1000:

- Arrow keys: D-pad
- `Z` / `J`: Button 1
- `X` / `K`: Button 2

Master System:

- Arrow keys: D-pad
- `Z` / `J`: Button 1
- `X` / `K`: Button 2

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
- SG-1000: `wram`, `vram`
- Master System: `wram`, `cart_ram`, `vram`
- Mega Drive: `wram`
- PC Engine: `wram`, `cart_ram`, `bram`
- Game Boy: `wram`, `vram`, `oam`, `hram`, `cart_ram`
- Game Boy Color: `wram`, `vram`, `oam`, `hram`, `cart_ram`
- Game Boy Advance: `ewram`, `iwram`, `pram`, `vram`, `oam`

Game Boy / Game Boy Color `cart_ram` appears only when the cartridge exposes
backup RAM.

## Save States

Save states are also separated per game.

```text
states/<system>/<rom>/slot<N>.<ext>
```

Examples:

```text
states/snes/Super F1 Circus Gaiden (Japan)/slot1.sns
states/sg1000/Champion Boxing/slot1.sgs
states/mastersystem/Alex Kidd in Miracle World/slot1.smsst
states/megadrive/Sonic The Hedgehog/slot1.mdst
states/nes/Super Mario Bros/slot1.sav
states/pce/Adventure Island/slot1.pcst
states/gb/Tetris/slot1.gbst
states/gbc/Dragon Warrior Monsters/slot1.gbcst
states/gba/Example/slot1.gbas
states/n64/Example/slot1.n64st
```

Legacy save-state files are used as load-only fallbacks.

```text
states/<system>/<rom>.slot<N>.<ext>
states/<rom>.slot<N>.sav  # legacy NES format
```

## Persistent Saves

SRAM and backup RAM are handled according to each core's API.

- SNES: `.srm`
- PC Engine: `.sav`, `.brm`
- Game Boy / Game Boy Color: `.sav`
- Game Boy Advance: `.sav`
- NES: SRAM persistence is handled by the core
- Nintendo 64: `states/n64/<rom-stem>/cartridge.srm` (cartridge and Controller Pak)

Persistent saves are usually flushed on normal exit. A crash may prevent the
latest save data from being written.

## Design

Revive is not an emulator that runs multiple cores at the same time. It is a
frontend that selects the right core for one ROM and presents a common UI.

### `revive-core`

`revive-core` hides emulator-specific differences behind adapters. The
`CoreInstance` enum wraps NES, SNES, SG-1000, Master System, Mega Drive, PC
Engine, Game Boy, and GBA implementations, and exposes only the common
operations to the CLI.

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

`revive-cli` is the SDL3 + wgpu + egui frontend.

Main flow:

1. Read the ROM path from CLI arguments or the file picker.
2. Detect the system from the extension or `--system`.
3. Start the target core with `CoreInstance::load_rom`.
4. In the event loop, process input, save states, and cheat panel actions.
5. Each frame, run `apply_cheats -> step_frame -> apply_cheats`.
6. Feed samples into the audio queue.
7. Upload the frame using its RGB24/RGBA/BGRA format to a wgpu texture.
8. Draw the egui cheat panel on the right side.

When the panel is open, the viewport reserves the panel width on the right and
renders the game screen into the remaining area while preserving aspect ratio.

## Development Commands

```sh
cargo fmt
cargo clippy -p revive-core -p revive-cheat -p revive-cli --all-targets --no-deps -- -D warnings
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
cargo test -p sg1000-core
cargo test -p mastersystem-core
cargo test -p megadrive-core
cargo test -p pce-core
cargo test -p emulator-gb
cargo test -p emulator-gba
```

## Known Limitations

- Only one ROM can run at a time.
- Without `--cheats`, the Save action writes to `cheats/<system>/<rom>/cheats.json`.
- Cores are vendored under `crates/cores/`, so upstream core repository changes must be synced manually.
- Emulator accuracy, timing, and mapper behavior are still validated system by system; prefer focused regression tests for hardware fixes.


## PlayStation (HLE BIOS)

```sh
cargo run -- "roms/ps1/Momotarou Densetsu (Japan).zip"
cargo run -- "path/to/disc.cue"
```

PS1 uses the vendored PCSX ReARMed interpreter and software renderer with HLE
BIOS explicitly enabled. No Sony BIOS is included, downloaded, or required.
HLE compatibility varies by game; starting one title does not establish full
PS1 compatibility. The core's native frame rate is used for NTSC/PAL pacing.

A ZIP must contain exactly one CUE and every referenced track, with relative
paths preserved (for example, `disc/game.cue` can reference
`tracks/track.bin` beside it under `disc/`). Track paths are relative to the
CUE directory, independent of the launcher working directory. The ZIP is
extracted to an owned temporary directory and removed
when the core closes. Select an extracted CUE when an archive has multiple
CUEs. CHD, disc swapping, analog pads, and a second memory card are not part of
this initial integration. Raw BIN requires an explicit system to preserve
Mega Drive BIN detection.

| PS1 control | Keyboard |
| --- | --- |
| D-pad | Arrow keys |
| Cross / Circle | Z / X |
| Square / Triangle | A / S |
| L1 / R1 | Q / W |
| L2 / R2 | E / R |
| Start | Enter or Space |
| Select | Shift or Backspace |

Memory card 1 is stored as `states/ps1/<original-file-stem>/memory-card.mcd`.
Changed card data is saved atomically every 60 emulated frames and on clean
exit. ZIP extraction paths never determine save names. Save slots use the
existing shortcuts: **Cmd+Shift+1–9** to save and **Cmd+1–9** to load on macOS;
**Ctrl+Shift+1–9** / **Ctrl+1–9** on Windows/Linux. Files are `slot<N>.psst` in
the same directory. Rejected malformed state loads restore the previous state.
Main RAM is available to the cheat panel as `ram` (2 MiB).

See [PS1 upstream and build notes](crates/cores/ps1/UPSTREAM.md) for the pinned
source, local patches, build requirements, and GPL distribution obligations.
The macOS/Apple Silicon build is verified; other native targets require
validation, and this Makefile integration does not support MSVC.

Local verification with Momotarou Densetsu (Japan) covers boot/title, dialogue,
indoor/outdoor movement, menus, audio, and state/card reopening. This is not a
full playthrough or a compatibility guarantee for other games.

Automated tests require no retail assets. An additional opt-in local disc test
can exercise ZIP loading (including nested CUE/track paths), HLE boot,
audio/video, save states and reopening:

```sh
cargo test -p ps1-core -p revive-core -p revive-cli
REVIVE_PS1_TEST_ROM="roms/ps1/Momotarou Densetsu (Japan).zip" \
  cargo test -p revive-core --test ps1_local_disc -- --ignored --nocapture
```

## Nintendo 64

```sh
cargo run --release -- "roms/nintendo64/Super Mario 64 (USA).z64"
cargo run --release -- run game.v64 --system n64
```

N64 uses the bundled Mupen64Plus-Next cached interpreter, CXD4 RSP and
Angrylion software renderer. No external emulator or BIOS installation is
required. `.z64`, `.n64`, and `.v64` cartridge dumps are validated and normalized
by their headers. N64 archives must first be extracted; PS1 CUE ZIP handling is
unchanged. Output stays 4:3, including resolution changes, with the cartridge's
NTSC/PAL pacing and stereo audio. Release builds are recommended.

Initial scope: one player, 8 MiB Expansion Pak, Controller Pak and cartridge
saves. 64DD, Transfer Pak, rumble, online play and high-resolution rendering
are not exposed.

| N64 control | Keyboard | SDL gamepad |
| --- | --- | --- |
| Analog stick | Arrow keys | Left stick |
| D-pad | W / S / A / D | D-pad |
| A / B | Z / X | South / West face buttons |
| Z trigger | Shift | Left trigger |
| L / R | Q / E | Left / right shoulder |
| C up / down / left / right | I / K / J / L | Right stick |
| Start | Enter | Start |

The gamepad connects automatically and can be unplugged/reconnected. Analog
input has a radial deadzone; keyboard diagonals have a normalized magnitude.
Focus loss and UI keyboard capture release both buttons and the stick.

Cartridge EEPROM/SRAM/FlashRAM and Controller Pak data share
`states/n64/<rom-stem>/cartridge.srm`, written atomically every 60 frames when
changed and on clean exit. Existing saves are loaded at startup; invalid save
sizes report an error instead of silently replacing the file. State slots use
`slot<N>.n64st` in the same directory and the existing platform save/load keys.
States are tied to the ROM contents and include an integrity checksum.

The memory panel exposes `rdram` in N64 guest byte-address order; existing
1-byte searches and frozen cheats work with
`cheats/n64/<rom-stem>/cheats.json`. Offsets start at zero, not at the virtual
CPU address `0x80000000`. GameShark-code import is not implemented.

See [N64 upstream/build/validation notes](crates/cores/n64/UPSTREAM.md) for the
pinned source, native fixes, build dependencies, and opt-in local-ROM tests.
