use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_sunsoft4_nametable_chr(
        &self,
        physical_nt: usize,
        offset: usize,
    ) -> u8 {
        if self.chr_rom.is_empty() || offset >= 1024 {
            return 0;
        }

        let Some(sunsoft4) = self.mappers.sunsoft4.as_ref() else {
            return 0;
        };
        let bank_count = (self.chr_rom.len() / 0x400).max(1);
        let bank = (sunsoft4.nametable_banks[physical_nt & 1] as usize) % bank_count;
        let chr_addr = bank * 0x400 + offset;
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }
}
