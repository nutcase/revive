use std::cell::Cell;

mod chr;
mod prg;
mod ram;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mmc2 {
    pub(in crate::cartridge) prg_bank: u8,
    pub(in crate::cartridge) chr_bank_0_fd: u8,
    pub(in crate::cartridge) chr_bank_0_fe: u8,
    pub(in crate::cartridge) chr_bank_1_fd: u8,
    pub(in crate::cartridge) chr_bank_1_fe: u8,
    pub(in crate::cartridge) latch_0: Cell<bool>, // false=FD, true=FE
    pub(in crate::cartridge) latch_1: Cell<bool>,
}

impl Mmc2 {
    pub(in crate::cartridge) fn new() -> Self {
        Mmc2 {
            prg_bank: 0,
            chr_bank_0_fd: 0,
            chr_bank_0_fe: 0,
            chr_bank_1_fd: 0,
            chr_bank_1_fe: 0,
            latch_0: Cell::new(true), // FE selected initially
            latch_1: Cell::new(true), // FE selected initially
        }
    }
}
