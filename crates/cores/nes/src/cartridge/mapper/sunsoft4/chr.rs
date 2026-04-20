use crate::cartridge::Cartridge;

impl Cartridge {
    fn read_sunsoft4_chr_bank_2k(&self, bank: u8, offset: usize) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.chr_rom.len() / 0x800).max(1);
        let bank = (bank as usize) % bank_count;
        let chr_addr = bank * 0x800 + offset;
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }

    fn write_sunsoft4_chr_bank_2k(&mut self, bank: u8, offset: usize, data: u8) {
        if self.chr_rom.is_empty() {
            return;
        }

        let bank_count = (self.chr_rom.len() / 0x800).max(1);
        let bank = (bank as usize) % bank_count;
        let chr_len = self.chr_rom.len();
        let chr_addr = bank * 0x800 + offset;
        self.chr_rom[chr_addr % chr_len] = data;
    }

    pub(in crate::cartridge) fn read_chr_sunsoft4(&self, addr: u16) -> u8 {
        let Some(sunsoft4) = self.mappers.sunsoft4.as_ref() else {
            return 0;
        };
        let slot = ((addr as usize) >> 11) & 0x03;
        let offset = (addr as usize) & 0x07FF;
        self.read_sunsoft4_chr_bank_2k(sunsoft4.chr_banks[slot], offset)
    }

    pub(in crate::cartridge) fn write_chr_sunsoft4(&mut self, addr: u16, data: u8) {
        let Some(bank) = self
            .mappers
            .sunsoft4
            .as_ref()
            .map(|sunsoft4| sunsoft4.chr_banks[((addr as usize) >> 11) & 0x03])
        else {
            return;
        };
        let offset = (addr as usize) & 0x07FF;
        self.write_sunsoft4_chr_bank_2k(bank, offset, data);
    }
}
