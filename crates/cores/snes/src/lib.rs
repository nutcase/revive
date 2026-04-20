#![cfg_attr(not(feature = "dev"), allow(dead_code))]
#![allow(clippy::new_without_default)]

pub mod audio;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod debug;
pub use debug::debugger;
pub use debug::flags as debug_flags;
pub mod dma;
pub mod emulator;
pub mod hud_toast;
pub mod input;
pub mod ppu;
pub mod savestate;
pub mod shutdown;
