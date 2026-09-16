# PCSX ReARMed integration

Source: https://github.com/libretro/pcsx_rearmed

Pinned commit: `8625c395a24411f8c77e69802b516df9c613a712`

The tracked `vendor/` source snapshot contains the upstream AUTHORS, COPYING,
README, Makefiles, core, frontend, plugins, headers, and dependency sources.
No Sony BIOS, disc image, prebuilt library, or game data is included.
PCSX ReARMed is GPL-2.0-or-later; retain its notices and the licenses in the
vendored dependencies. Distribution of a linked Revive executable must comply
with the applicable GPL source and license requirements as well.

Local upstream changes:

- `frontend/libretro.c`: restrict the legacy iOS `ptrace` workaround to iOS
  builds with a dynamic recompiler. Revive uses the interpreter and must not
  trace its own macOS process.
- `Makefile`: pin the generated revision string to `8625c39-revive` instead of
  accidentally identifying the parent Revive repository as the upstream core.
- `libpcsxcore/sio.c`: explicitly zero-initialize the memory-card buffers so
  Mach-O emits defined zero-fill storage instead of common symbols whose sizes
  cause the linker to infer an excessive 32 KiB alignment on macOS.

Build choices:

- Static libretro core, interpreter CPU, software GPU (NEON/SIMD where supported).
- Firmware option is explicitly `HLE`; the host exposes no BIOS directory.
- Synchronous CD/GPU/SPU execution. No native worker can call into a borrowed
  host buffer; the Rust wrapper also enforces a single, thread-confined instance.
- No CHD, physical CD drive, libretro VFS, or JIT dependency. CUE/BIN is the
  initial supported disc workflow. ZIP handling belongs to `revive-core`.
- The core always generates 44.1 kHz stereo; SDL handles device conversion.
- Memory card 1 uses host persistence; card 2 is disabled.

`build.rs` copies sources to Cargo's OUT_DIR and builds there. The checkout is
not modified during builds and no network or sibling repository is required.
GNU make and a C compiler are required. macOS/Apple Silicon is validated;
Linux and MinGW build paths are provided but not yet verified. MSVC is not
supported by this upstream Makefile integration.

`cargo test -p ps1-core` runs an original tiny MIPS program without retail
assets, covering HLE startup, singleton ownership, frame/audio progression,
RAM and state restoration, audio disabling, card validation, and reopening.
The `probe` example can capture PPM frames and send timed pad input to a user's
locally supplied disc for manual compatibility verification.
