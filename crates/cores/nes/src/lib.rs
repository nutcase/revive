//! Core NES emulation library.
//!
//! Front-ends should normally use [`Nes`] as the runtime facade and avoid
//! depending on CPU, PPU, APU, bus, or mapper internals.

mod apu;
mod audio_ring;
mod bus;
mod cartridge;
mod cpu;
mod error;
mod memory;
mod nes;
mod ppu;
pub mod save_state;
mod sram;

mod nes_state;

pub use apu::AudioDiagFull;
pub use audio_ring::SpscRingBuffer;
pub use error::{Error, Result};
pub use nes::Nes;
pub use save_state::SaveState;

/// Approximate NTSC CPU cycles per rendered frame used by front-end pacing.
pub const CPU_CYCLES_PER_FRAME: u32 = 29830;
