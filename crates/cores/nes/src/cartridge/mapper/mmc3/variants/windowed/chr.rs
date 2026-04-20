use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_chr_mapper37(&self, addr: u16) -> u8 {
        let chr_base = if self.mappers.mmc3_variant.mapper37_outer_bank & 0x04 != 0 {
            0x80
        } else {
            0
        };
        self.read_chr_windowed_mmc3(addr, chr_base, 0x7F)
    }

    pub(in crate::cartridge) fn write_chr_mapper37(&mut self, addr: u16, data: u8) {
        let chr_base = if self.mappers.mmc3_variant.mapper37_outer_bank & 0x04 != 0 {
            0x80
        } else {
            0
        };
        self.write_chr_windowed_mmc3(addr, chr_base, 0x7F, data);
    }

    pub(in crate::cartridge) fn read_chr_mapper47(&self, addr: u16) -> u8 {
        let chr_base = ((self.mappers.mmc3_variant.mapper47_outer_bank & 0x01) as usize) << 7;
        self.read_chr_windowed_mmc3(addr, chr_base, 0x7F)
    }

    pub(in crate::cartridge) fn read_chr_mapper12(&self, addr: u16) -> u8 {
        let chr_base = if addr < 0x1000 {
            ((self.mappers.mmc3_variant.mapper12_chr_outer & 0x01) as usize) << 8
        } else {
            (((self.mappers.mmc3_variant.mapper12_chr_outer >> 4) & 0x01) as usize) << 8
        };
        self.read_chr_windowed_mmc3(addr, chr_base, 0xFF)
    }

    pub(in crate::cartridge) fn read_chr_mapper44(&self, addr: u16) -> u8 {
        let (base, bank_mask) = self.mapper44_chr_window();
        self.read_chr_windowed_mmc3(addr, base, bank_mask)
    }

    pub(in crate::cartridge) fn write_chr_mapper47(&mut self, addr: u16, data: u8) {
        let chr_base = ((self.mappers.mmc3_variant.mapper47_outer_bank & 0x01) as usize) << 7;
        self.write_chr_windowed_mmc3(addr, chr_base, 0x7F, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper12(&mut self, addr: u16, data: u8) {
        let chr_base = if addr < 0x1000 {
            ((self.mappers.mmc3_variant.mapper12_chr_outer & 0x01) as usize) << 8
        } else {
            (((self.mappers.mmc3_variant.mapper12_chr_outer >> 4) & 0x01) as usize) << 8
        };
        self.write_chr_windowed_mmc3(addr, chr_base, 0xFF, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper44(&mut self, addr: u16, data: u8) {
        let (base, bank_mask) = self.mapper44_chr_window();
        self.write_chr_windowed_mmc3(addr, base, bank_mask, data);
    }

    pub(in crate::cartridge) fn read_chr_mapper114(&self, addr: u16) -> u8 {
        let chr_base = ((self.mappers.mmc3_variant.mapper114_chr_outer_bank & 0x01) as usize) << 8;
        self.read_chr_windowed_mmc3(addr, chr_base, 0xFF)
    }

    pub(in crate::cartridge) fn read_chr_mapper115(&self, addr: u16) -> u8 {
        let chr_base = ((self.mappers.mmc3_variant.mapper115_chr_outer_bank & 0x01) as usize) << 8;
        self.read_chr_windowed_mmc3(addr, chr_base, 0xFF)
    }

    pub(in crate::cartridge) fn read_chr_mapper205(&self, addr: u16) -> u8 {
        let (base, bank_mask) = self.mapper205_chr_window();
        self.read_chr_windowed_mmc3(addr, base, bank_mask)
    }

    pub(in crate::cartridge) fn write_chr_mapper114(&mut self, addr: u16, data: u8) {
        let chr_base = ((self.mappers.mmc3_variant.mapper114_chr_outer_bank & 0x01) as usize) << 8;
        self.write_chr_windowed_mmc3(addr, chr_base, 0xFF, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper115(&mut self, addr: u16, data: u8) {
        let chr_base = ((self.mappers.mmc3_variant.mapper115_chr_outer_bank & 0x01) as usize) << 8;
        self.write_chr_windowed_mmc3(addr, chr_base, 0xFF, data);
    }

    pub(in crate::cartridge) fn write_chr_mapper205(&mut self, addr: u16, data: u8) {
        let (base, bank_mask) = self.mapper205_chr_window();
        self.write_chr_windowed_mmc3(addr, base, bank_mask, data);
    }
}
