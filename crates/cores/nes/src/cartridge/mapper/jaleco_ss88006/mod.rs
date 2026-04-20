use std::cell::Cell;

mod chr;
mod irq;
mod prg;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct JalecoSs88006 {
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_ram_enabled: bool,
    pub(in crate::cartridge) prg_ram_write_enabled: bool,
    pub(in crate::cartridge) irq_reload: u16,
    pub(in crate::cartridge) irq_counter: u16,
    pub(in crate::cartridge) irq_control: u8,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
}

impl JalecoSs88006 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1, 2],
            chr_banks: [0; 8],
            prg_ram_enabled: false,
            prg_ram_write_enabled: false,
            irq_reload: 0,
            irq_counter: 0,
            irq_control: 0,
            irq_pending: Cell::new(false),
        }
    }
}
