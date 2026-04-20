use std::cell::Cell;

mod audio;
mod banking;
mod irq;

pub(super) const NAMCO163_WRAM_LEN: usize = 0x2000;
pub(super) const NAMCO163_INTERNAL_RAM_LEN: usize = 0x80;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Namco163 {
    pub(in crate::cartridge) chr_banks: [u8; 12],
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) sound_disable: bool,
    pub(in crate::cartridge) chr_nt_disabled_low: bool,
    pub(in crate::cartridge) chr_nt_disabled_high: bool,
    pub(in crate::cartridge) wram_write_enable: bool,
    pub(in crate::cartridge) wram_write_protect: u8,
    pub(in crate::cartridge) internal_addr: Cell<u8>,
    pub(in crate::cartridge) internal_auto_increment: bool,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) audio_delay: u8,
    pub(in crate::cartridge) audio_channel_index: u8,
    pub(in crate::cartridge) audio_outputs: [f32; 8],
    pub(in crate::cartridge) audio_current: f32,
}

impl Namco163 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            chr_banks: [0; 12],
            prg_banks: [0, 1, 2],
            sound_disable: true,
            chr_nt_disabled_low: false,
            chr_nt_disabled_high: false,
            wram_write_enable: false,
            wram_write_protect: 0x0F,
            internal_addr: Cell::new(0),
            internal_auto_increment: false,
            irq_counter: 0,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            audio_delay: 14,
            audio_channel_index: 0,
            audio_outputs: [0.0; 8],
            audio_current: 0.0,
        }
    }

    pub(super) fn chip_ram_addr(&self) -> usize {
        NAMCO163_WRAM_LEN + (self.internal_addr.get() as usize & 0x7F)
    }
}
