mod chr;
mod nametable;
mod prg;
mod ram;

use super::super::super::Cartridge;

impl Cartridge {
    fn namco163_chr_rom_bank_count_1k(&self) -> usize {
        (self.chr_rom.len() / 0x0400).max(1)
    }

    fn namco163_prg_rom_bank_count_8k(&self) -> usize {
        (self.prg_rom.len() / 0x2000).max(1)
    }

    fn namco163_ciram_addr(bank: u8, offset: usize) -> usize {
        ((bank as usize) & 1) * 0x0400 + offset
    }

    fn namco163_chr_bank_uses_ciram(&self, slot: usize, bank: u8) -> bool {
        if bank < 0xE0 {
            return false;
        }
        match slot {
            0..=3 => self
                .mappers
                .namco163
                .as_ref()
                .map(|n| !n.chr_nt_disabled_low)
                .unwrap_or(false),
            4..=7 => self
                .mappers
                .namco163
                .as_ref()
                .map(|n| !n.chr_nt_disabled_high)
                .unwrap_or(false),
            _ => true,
        }
    }

    fn read_namco163_chr_bank(&self, bank: u8, offset: usize, slot: usize) -> u8 {
        if self.namco163_chr_bank_uses_ciram(slot, bank) {
            let ciram_addr = Self::namco163_ciram_addr(bank, offset);
            return self.chr_ram.get(ciram_addr).copied().unwrap_or(0);
        }

        let bank_count = self.namco163_chr_rom_bank_count_1k();
        let chr_addr = ((bank as usize % bank_count) * 0x0400) + offset;
        self.chr_rom.get(chr_addr).copied().unwrap_or(0)
    }

    fn write_namco163_chr_bank(&mut self, bank: u8, offset: usize, slot: usize, data: u8) {
        if self.namco163_chr_bank_uses_ciram(slot, bank) {
            let ciram_addr = Self::namco163_ciram_addr(bank, offset);
            if let Some(cell) = self.chr_ram.get_mut(ciram_addr) {
                *cell = data;
            }
        }
    }
}
