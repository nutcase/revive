use std::cell::Cell;

mod chr;
mod irq;
mod prg;
mod ram;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc7 {
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) control: u8,
    pub(in crate::cartridge) wram_enabled: bool,
    pub(in crate::cartridge) audio_silenced: bool,
    pub(in crate::cartridge) irq_latch: u8,
    pub(in crate::cartridge) irq_counter: u8,
    pub(in crate::cartridge) irq_enable_after_ack: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_cycle_mode: bool,
    pub(in crate::cartridge) irq_prescaler: i16,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl Vrc7 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1, 2],
            chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
            control: 0,
            wram_enabled: false,
            audio_silenced: false,
            irq_latch: 0,
            irq_counter: 0,
            irq_enable_after_ack: false,
            irq_enabled: false,
            irq_cycle_mode: false,
            irq_prescaler: 341,
            irq_pending: Cell::new(false),
        }
    }
}
