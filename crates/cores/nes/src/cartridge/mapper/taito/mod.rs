use std::cell::Cell;

mod irq;
mod shared;
mod tc0190;
mod x1005;
mod x1017;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct TaitoTc0190 {
    pub(in crate::cartridge) prg_banks: [u8; 2],
    pub(in crate::cartridge) chr_banks: [u8; 6],
    pub(in crate::cartridge) irq_latch: u8,
    pub(in crate::cartridge) irq_counter: u8,
    pub(in crate::cartridge) irq_reload: bool,
    pub(in crate::cartridge) irq_enabled: bool,
    pub(in crate::cartridge) irq_pending: Cell<bool>,
    pub(in crate::cartridge) irq_delay: u8,
}

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct TaitoX1005 {
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) chr_banks: [u8; 6],
    pub(in crate::cartridge) ram_enabled: bool,
}

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct TaitoX1017 {
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) chr_banks: [u8; 6],
    pub(in crate::cartridge) ram_enabled: [bool; 3],
    pub(in crate::cartridge) chr_invert: bool,
}

impl TaitoTc0190 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1],
            chr_banks: [0, 1, 2, 3, 4, 5],
            irq_latch: 0,
            irq_counter: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            irq_delay: 0,
        }
    }
}

impl TaitoX1005 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1, 2],
            chr_banks: [0, 1, 2, 3, 4, 5],
            ram_enabled: false,
        }
    }
}

impl TaitoX1017 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1, 2],
            chr_banks: [0, 1, 2, 3, 4, 5],
            ram_enabled: [false; 3],
            chr_invert: false,
        }
    }
}
