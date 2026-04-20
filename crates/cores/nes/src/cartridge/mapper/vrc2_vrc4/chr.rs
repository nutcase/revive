use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_mapper21(&self, addr: u16) -> u8 {
        self.read_chr_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_chr_mapper22(&self, addr: u16) -> u8 {
        self.read_chr_mapper23(addr)
    }

    pub(in crate::cartridge) fn read_chr_mapper23(&self, addr: u16) -> u8 {
        if let Some(vrc) = self.mappers.vrc2_vrc4.as_ref() {
            let chr_data = self.vrc2_vrc4_chr_data();
            if chr_data.is_empty() {
                return 0;
            }

            let bank_count = (chr_data.len() / 0x0400).max(1);
            let slot = ((addr >> 10) & 0x07) as usize;
            let bank =
                Self::vrc2_vrc4_effective_chr_bank(self.mapper, vrc.chr_banks[slot], bank_count);
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            chr_data[chr_addr % chr_data.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper25(&self, addr: u16) -> u8 {
        self.read_chr_mapper23(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper21(&mut self, addr: u16, data: u8) {
        self.write_chr_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper22(&mut self, addr: u16, data: u8) {
        self.write_chr_mapper23(addr, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper23(&mut self, addr: u16, data: u8) {
        let (bank, chr_len, use_ram) = if let Some(vrc) = self.mappers.vrc2_vrc4.as_ref() {
            let chr_data = self.vrc2_vrc4_chr_data();
            if chr_data.is_empty() {
                return;
            }

            let bank_count = (chr_data.len() / 0x0400).max(1);
            let slot = ((addr >> 10) & 0x07) as usize;
            (
                Self::vrc2_vrc4_effective_chr_bank(self.mapper, vrc.chr_banks[slot], bank_count),
                chr_data.len(),
                !self.chr_ram.is_empty(),
            )
        } else {
            return;
        };

        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        if use_ram {
            self.chr_ram[chr_addr % chr_len] = data;
        } else if !self.chr_rom.is_empty() {
            self.chr_rom[chr_addr % chr_len] = data;
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper25(&mut self, addr: u16, data: u8) {
        self.write_chr_mapper23(addr, data);
    }
}
