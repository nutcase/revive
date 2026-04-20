use super::*;

impl Cartridge {
    pub(in crate::cartridge) fn read_nametable_namco163(
        &self,
        logical_nt: usize,
        offset: usize,
        internal: &[[u8; 1024]; 2],
    ) -> u8 {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return internal[logical_nt & 1][offset];
        };
        let bank = namco163.chr_banks[8 + (logical_nt & 3)];
        if bank >= 0xE0 {
            let ciram_addr = Self::namco163_ciram_addr(bank, offset);
            self.chr_ram.get(ciram_addr).copied().unwrap_or(0)
        } else {
            let bank_count = self.namco163_chr_rom_bank_count_1k();
            let chr_addr = (bank as usize % bank_count) * 0x0400 + offset;
            self.chr_rom.get(chr_addr).copied().unwrap_or(0)
        }
    }

    pub(in crate::cartridge) fn write_nametable_namco163(
        &mut self,
        logical_nt: usize,
        offset: usize,
        _internal: &mut [[u8; 1024]; 2],
        data: u8,
    ) {
        let Some(namco163) = self.mappers.namco163.as_ref() else {
            return;
        };
        let bank = namco163.chr_banks[8 + (logical_nt & 3)];
        if bank >= 0xE0 {
            let ciram_addr = Self::namco163_ciram_addr(bank, offset);
            if let Some(cell) = self.chr_ram.get_mut(ciram_addr) {
                *cell = data;
            }
        }
    }
}
