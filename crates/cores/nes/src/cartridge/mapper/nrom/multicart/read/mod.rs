mod address_latch;
mod bank_modes;
mod outer_bank;

use crate::cartridge::Cartridge;

impl Cartridge {
    fn read_multicart_prg_16k(&self, addr: u16, bank: usize, empty_value: u8) -> u8 {
        if self.prg_rom.is_empty() {
            return empty_value;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let offset = (bank % bank_count) * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    fn read_multicart_prg_32k(&self, addr: u16, bank: usize, empty_value: u8) -> u8 {
        if self.prg_rom.is_empty() {
            return empty_value;
        }

        let bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let offset = (bank % bank_count) * 0x8000 + (addr - 0x8000) as usize;
        self.prg_rom[offset % self.prg_rom.len()]
    }
}
