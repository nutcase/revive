mod adapters;
mod paths;
mod ps1_disc;
mod system;

pub use adapters::{
    CoreInstance, GameBoyAdapter, GameBoyAdvanceAdapter, MasterSystemAdapter, MegaDriveAdapter,
    N64Adapter, NesAdapter, PceAdapter, Ps1Adapter, Sg1000Adapter, SnesAdapter,
};
pub use system::{
    detect_system, AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemInfo, SystemKind,
    VirtualButton, ALL_SYSTEMS, ROM_EXTENSIONS,
};
