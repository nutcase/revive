# Mupen64Plus-Next integration

Source: https://github.com/libretro/mupen64plus-libretro-nx

Pinned commit: `6752836de8b224febfd5708444755b77712ac939`

The `vendor/` tree is a source snapshot, including upstream license notices.
The three unused upstream prebuilt GLideNHQ libraries (`libpng.a`, `libz.a`,
`libdxtn.a`) are omitted. Upstream `.gitignore` files are also omitted so
source files tracked by upstream (such as GLSM and generated assembly headers)
remain included when this snapshot is committed. No cartridge or firmware image is included.
Mupen64Plus-Next is GPL-2.0-or-later; preserve all dependency licenses as well.
The embedded shared-library packaging does not remove the source-distribution
requirements applicable to the bundled emulator.

## Build and host

- `build.rs` builds the vendored source with GNU make and a C/C++ compiler in
  Cargo's OUT_DIR, never in the checkout. No build-time network access or
  sibling repositories are needed (ordinary Cargo dependencies aside).
- Cached-interpreter R4300 CPU, CXD4 LLE RSP, Angrylion software RDP with four
  workers. No CPU/RSP JIT, Vulkan device, or external firmware is required.
  The unused GLideN64 path remains linked against the OS OpenGL library.
- zlib comes from the target system SDK (`SYSTEM_ZLIB=1`); the remaining
  selected native dependencies are vendored. Linux needs OpenGL development
  libraries and zlib; Windows uses MinGW as does Revive's PS1 build. macOS
  Apple Silicon is the validation target; Linux/MinGW paths are unverified.
- A private shared image is embedded into the Rust executable. On load it is
  written into an owned temporary directory and loaded with local symbol
  scope. This isolates the libretro and dependency symbols from the statically
  linked PS1 core without maintaining thousands of symbol renames. The image
  unloads before its directory is removed. Signed/hardened distribution builds
  must validate their library-signing policy separately.
- A process-wide lock and a !Send/!Sync marker enforce a single thread-confined
  N64 instance. C callbacks copy the software frame and stereo 44.1 kHz audio
  into owned buffers; no native worker accesses a Rust borrow.
- 8 MiB RDRAM (Expansion Pak enabled), one controller with Controller Pak,
  alternate libretro button mapping and host-managed analog input. Persistent
  storage is the upstream packed EEPROM/Controller Paks/SRAM/FlashRAM buffer.
  It is kept separate from state slots and flushed atomically by the adapter.
- Host RAM views expose guest big-endian byte addresses, reversing each native
  word. Writes use the inverse mapping. The existing 8-bit cheat format needs
  no change. GameShark parsing, ZIP loading, 64DD, Transfer Pak and rumble are
  outside this initial integration.
- States have a versioned Revive envelope, normalized-ROM SHA-256 and payload
  checksum. Reject wrong sizes, incompatible native headers, foreign ROMs and
  corrupt payloads before calling native deserialization. Native failures
  restore a pre-load snapshot. These are local emulator states, not a sandbox
  for executing hostile cartridge/state data.

## Local upstream changes

- `custom/dependencies/libpng/pngpriv.h`: use modern `<math.h>` on Darwin
  instead of the classic-Mac `<fp.h>` branch selected by TARGET_OS_MAC.
- `libretro-common/libco/aarch64.c`: preserve d8-d15, FPCR and FPSR across
  coroutine switches; initialize their context storage. The old path violated
  AAPCS64 and could corrupt host floating-point calculations.
- `libretro/libretro.c`: validate the native state buffer size, return the
  actual load result instead of unconditional success, and delete the inactive
  coroutine at teardown. These changes do not select individual game titles.
- The build injects `bridge.c` into the private library and pins the revision
  string. Native objects are invalidated on build-script reruns because the
  upstream make rules do not track all nested C/header dependencies.

## Verification

`cargo test -p n64-core` checks ROM byte ordering, malformed state envelopes
and coroutine floating-point register preservation without retail assets.
The coroutine test deliberately keeps a distinct d8 value live on each stack.

A local cartridge can be tested without modifying its normal save directory:

```sh
cargo run --release -p n64-core --example n64_probe -- \
  "roms/nintendo64/Super Mario 64 (USA).z64" 6000 /tmp/revive-n64-frames
REVIVE_N64_TEST_ROM="$PWD/roms/nintendo64/Super Mario 64 (USA).z64" \
  cargo test --release -p revive-core --test n64_local_rom -- --ignored --nocapture
```

The probe produces optional PPM screenshots, frame timing, audio evidence,
and RAM/state/persistent-buffer round trips in a temporary save directory.
Its timed input is a reproducible manual probe, not a per-title core hack or
proof that arbitrary games work. The adapter test also runs an original PS1
homebrew program in the same process to check symbol and memory isolation.
It resumes execution after state loading, rejects corrupt/foreign/truncated/
oversized slots without changing RAM, and checks that an invalid battery-save
size causes a recoverable load error without replacing the file.
