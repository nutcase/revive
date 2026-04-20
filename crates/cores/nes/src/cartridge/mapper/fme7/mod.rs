use std::cell::Cell;

mod audio;
mod banking;
mod irq;

pub(in crate::cartridge) use audio::Sunsoft5BAudio;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Fme7 {
    pub(in crate::cartridge) command: u8,
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) prg_bank_6000: u8,
    pub(in crate::cartridge) prg_ram_enabled: bool,
    pub(in crate::cartridge) prg_ram_select: bool,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_counter_enabled: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) audio: Sunsoft5BAudio,
}

impl Fme7 {
    pub(in crate::cartridge) fn new() -> Self {
        Fme7 {
            command: 0,
            chr_banks: [0; 8],
            prg_banks: [0; 3],
            prg_bank_6000: 0,
            prg_ram_enabled: false,
            prg_ram_select: false,
            irq_counter: 0,
            irq_counter_enabled: false,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            audio: Sunsoft5BAudio::new(),
        }
    }
}
