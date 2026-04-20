mod chr;
mod prg;
mod ram;
mod registers;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Mmc1 {
    pub(in crate::cartridge) shift_register: u8,
    pub(in crate::cartridge) shift_count: u8,
    pub(in crate::cartridge) control: u8,
    pub(in crate::cartridge) chr_bank_0: u8,
    pub(in crate::cartridge) chr_bank_1: u8,
    pub(in crate::cartridge) prg_bank: u8,
    pub(in crate::cartridge) prg_ram_disable: bool,
}

impl Mmc1 {
    pub(in crate::cartridge) fn new() -> Self {
        Mmc1 {
            shift_register: 0x10,
            shift_count: 0,
            control: 0x0C, // Default: 16KB PRG mode, last bank fixed
            chr_bank_0: 0,
            chr_bank_1: 0,
            prg_bank: 0,
            prg_ram_disable: false,
        }
    }
}
