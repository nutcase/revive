mod common;
mod gameboy;
mod instance;
mod megadrive;
mod nes;
mod pce;
mod sega8;
mod snes;

pub use gameboy::{GameBoyAdapter, GameBoyAdvanceAdapter};
pub use instance::CoreInstance;
pub use megadrive::MegaDriveAdapter;
pub use nes::NesAdapter;
pub use pce::PceAdapter;
pub use sega8::{MasterSystemAdapter, Sg1000Adapter};
pub use snes::SnesAdapter;
