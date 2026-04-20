# Revive

Unified launcher for the adjacent NES, SNES, Mega Drive, PC Engine, and Game Boy
emulator cores.

This workspace currently keeps the existing cores as path dependencies:

- `../nes-rust/crates/core`
- `../snes-rust/crates/core`
- `../megadrive/crates/core`
- `../pce/crates/core`
- `../gameboy/crates/core`
- `../gameboy/crates/gb`
- `../gameboy/crates/gba`

The local crates are split by responsibility:

- `crates/revive-core`: system detection and per-console adapters.
- `crates/revive-cheat`: UI-independent cheat search and cheat entry persistence.
- `crates/revive-cli`: SDL2 frontend for one active title at a time.

## Run

```sh
cargo run
cargo run -- --select
cargo run -- <rom>
cargo run -- run <rom> --system nes
cargo run -- run <rom> --system snes
cargo run -- run <rom> --system megadrive
cargo run -- run <rom> --system pce
cargo run -- run <rom> --system gb
cargo run -- run <rom> --system gbc
cargo run -- run <rom> --system gba
cargo run -- run <rom> --cheats cheats/game.json
```

When no ROM path is passed, or when `--select` is passed, Revive opens a local
file selection dialog.

Supported automatic extensions:

- `.nes` -> NES
- `.sfc`, `.smc` -> SNES
- `.md`, `.gen` -> Mega Drive
- `.pce` -> PC Engine
- `.gb` -> Game Boy
- `.gbc` -> Game Boy Color
- `.gba` -> Game Boy Advance
- `.bin` -> Mega Drive only when a `SEGA` header is detected

## Controls

State slots:

- `Ctrl/Cmd + 0..9`: save state
- `0..9`: load state

GB/GBC save states are not exposed by the current `../gameboy` core API yet.
GBA save states are supported.

NES:

- Arrows: D-pad
- `Z`: A
- `X`: B
- `Return`: Start
- `Space` or Shift: Select

SNES:

- Arrows: D-pad
- `D`: A
- `S`: B
- `W`: X
- `A`: Y
- `E`: L
- `Q`: R
- `Return` or `Space`: Start
- Shift: Select

Mega Drive:

- Arrows: D-pad
- `A`: A
- `Z`: B
- `X`: C
- `S`: X
- `D`: Y
- `F`: Z
- `Q`: Mode
- `Return` or `Space`: Start

PC Engine:

- Arrows: D-pad
- `Z`: I
- `X`: II
- `Return` or `Space`: Run
- Shift: Select

Game Boy / Game Boy Color:

- Arrows: D-pad
- `X`: A
- `Z`: B
- `Return` or `Space`: Start
- Backspace or Shift: Select

Game Boy Advance:

- Arrows: D-pad
- `X`: A
- `Z`: B
- `A`: L
- `S`: R
- `Return` or `Space`: Start
- Backspace or Shift: Select

## Cheat File Format

`--cheats` loads JSON entries handled by `revive-cheat`.

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

Common region ids:

- NES: `cpu_ram`, `prg_ram`
- SNES: `wram`, `sram`
- Mega Drive: `wram`
- PC Engine: `wram`, `cart_ram`, `bram`
- Game Boy / Game Boy Color: `cart_ram` (read-only through the current core API)
- Game Boy Advance: cheat memory regions are not exposed by the current core API
