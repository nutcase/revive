use super::super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_fme7(&self, addr: u16) -> u8 {
        if let Some(ref fme7) = self.mappers.fme7 {
            let slot = ((addr >> 10) & 7) as usize;
            let bank = fme7.chr_banks[slot] as usize;
            let offset = (addr & 0x03FF) as usize;

            let chr_addr = bank * 0x0400 + offset;

            if !self.chr_ram.is_empty() {
                if chr_addr < self.chr_ram.len() {
                    self.chr_ram[chr_addr]
                } else {
                    self.chr_ram[chr_addr % self.chr_ram.len()]
                }
            } else if chr_addr < self.chr_rom.len() {
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

    pub(in crate::cartridge) fn write_chr_fme7(&mut self, addr: u16, data: u8) {
        if !self.chr_ram.is_empty() {
            if let Some(ref fme7) = self.mappers.fme7 {
                let slot = ((addr >> 10) & 7) as usize;
                let bank = fme7.chr_banks[slot] as usize;
                let offset = (addr & 0x03FF) as usize;
                let chr_addr = bank * 0x0400 + offset;
                if chr_addr < self.chr_ram.len() {
                    self.chr_ram[chr_addr] = data;
                }
            }
        }
    }
}
