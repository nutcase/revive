use std::cell::Cell;

mod irq;
mod prg;
mod rambo1;
mod variants;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mmc3 {
    pub(in crate::cartridge) bank_select: u8,
    pub(in crate::cartridge) bank_registers: [u8; 8],
    pub(in crate::cartridge) extra_bank_registers: [u8; 8],
    pub(in crate::cartridge) irq_latch: u8,
    pub(in crate::cartridge) irq_counter: u8,
    pub(in crate::cartridge) irq_reload: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) prg_ram_enabled: bool,
    pub(in crate::cartridge) prg_ram_write_protect: bool,
    pub(in crate::cartridge) irq_cycle_mode: bool,
    pub(in crate::cartridge) irq_prescaler: u8,
    pub(in crate::cartridge) irq_delay: u8,
}

impl Mmc3 {
    pub(in crate::cartridge) fn new() -> Self {
        Mmc3 {
            bank_select: 0,
            bank_registers: [0; 8],
            extra_bank_registers: [0; 8],
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            prg_ram_enabled: true,
            prg_ram_write_protect: false,
            irq_cycle_mode: false,
            irq_prescaler: 4,
            irq_delay: 0,
        }
    }
}
