mod common;
mod gameboy;
mod instance;
mod megadrive;
mod nes;
mod pce;
mod ps1;
mod sega8;
mod snes;

pub use gameboy::{GameBoyAdapter, GameBoyAdvanceAdapter};
pub use instance::CoreInstance;
pub use megadrive::MegaDriveAdapter;
pub use nes::NesAdapter;
pub use pce::PceAdapter;
pub use ps1::Ps1Adapter;
pub use sega8::{MasterSystemAdapter, Sg1000Adapter};
pub use snes::SnesAdapter;

mod n64;
pub use n64::N64Adapter;
