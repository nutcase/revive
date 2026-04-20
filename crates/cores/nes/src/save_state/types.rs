use crate::apu::ApuState;
use crate::cartridge::CartridgeState;
use serde::{Deserialize, Serialize};

/// Opaque serialized emulator snapshot.
///
/// The fields are crate-private to keep the on-disk schema separate from the
/// public API. Front-ends should create and restore snapshots through `Nes`.
#[derive(Clone, Serialize, Deserialize)]
pub struct SaveState {
    // CPU state
    pub(crate) cpu_a: u8,
    pub(crate) cpu_x: u8,
    pub(crate) cpu_y: u8,
    pub(crate) cpu_pc: u16,
    pub(crate) cpu_sp: u8,
    pub(crate) cpu_status: u8,
    pub(crate) cpu_cycles: u64,

    // PPU state
    pub(crate) ppu_control: u8,
    pub(crate) ppu_mask: u8,
    pub(crate) ppu_status: u8,
    pub(crate) ppu_oam_addr: u8,
    pub(crate) ppu_scroll_x: u8,
    pub(crate) ppu_scroll_y: u8,
    pub(crate) ppu_addr: u16,
    pub(crate) ppu_data_buffer: u8,
    pub(crate) ppu_w: bool,
    pub(crate) ppu_t: u16,
    pub(crate) ppu_v: u16,
    pub(crate) ppu_x: u8,
    pub(crate) ppu_scanline: i16,
    pub(crate) ppu_cycle: u16,
    pub(crate) ppu_frame: u64,

    // PPU memory
    pub(crate) ppu_palette: [u8; 32],
    pub(crate) ppu_nametable: Vec<u8>, // Flattened nametable data
    pub(crate) ppu_oam: Vec<u8>,

    // Main RAM
    pub(crate) ram: Vec<u8>,

    // Cartridge state
    pub(crate) cartridge_prg_bank: u8,
    pub(crate) cartridge_chr_bank: u8,
    #[serde(default)]
    pub(crate) cartridge_state: Option<CartridgeState>,

    // APU state (basic)
    pub(crate) apu_frame_counter: u8,
    pub(crate) apu_frame_interrupt: bool,
    #[serde(default)]
    pub(crate) apu_state: Option<ApuState>,

    // Additional metadata
    pub(crate) rom_filename: String,
    pub(crate) timestamp: u64,
    #[serde(default)]
    pub(crate) cpu_halted: bool,
    #[serde(default)]
    pub(crate) bus_dma_cycles: u32,
    #[serde(default)]
    pub(crate) bus_dma_in_progress: bool,
    #[serde(default)]
    pub(crate) bus_dmc_stall_cycles: u32,
    #[serde(default)]
    pub(crate) ppu_frame_complete: bool,
}

impl SaveState {
    /// Current explicit save-state file format version.
    pub const FORMAT_VERSION: u16 = 4;

    /// ROM stem stored in the snapshot metadata.
    pub fn rom_filename(&self) -> &str {
        &self.rom_filename
    }

    /// UNIX timestamp stored in the snapshot metadata.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}
