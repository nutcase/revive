use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_bandai(&self, addr: u16) -> u8 {
        if self.mapper == 153 {
            let chr_addr = (addr & 0x1FFF) as usize;
            return self.chr_ram.get(chr_addr).copied().unwrap_or(0);
        }

        if let Some(ref bandai) = self.mappers.bandai_fcg {
            let slot = ((addr >> 10) & 7) as usize;
            let bank = bandai.chr_banks[slot] as usize;
            let offset = (addr & 0x03FF) as usize;

            let chr_addr = bank * 0x0400 + offset;

            if chr_addr < self.chr_rom.len() {
                self.chr_rom[chr_addr]
            } else if !self.chr_rom.is_empty() {
                self.chr_rom[chr_addr % self.chr_rom.len()]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_bandai(&mut self, addr: u16, data: u8) {
        if self.mapper == 153 {
            let chr_addr = (addr & 0x1FFF) as usize;
            if chr_addr < self.chr_ram.len() {
                self.chr_ram[chr_addr] = data;
            }
        }
    }
}
