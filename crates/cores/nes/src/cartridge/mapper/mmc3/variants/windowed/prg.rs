use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper37(&self, addr: u16) -> u8 {
        let (base, bank_mask) = self.mapper37_prg_window();
        self.read_prg_windowed_mmc3(addr, base, bank_mask)
    }

    pub(in crate::cartridge) fn read_prg_mapper47(&self, addr: u16) -> u8 {
        let base = ((self.mappers.mmc3_variant.mapper47_outer_bank & 0x01) as usize) << 4;
        self.read_prg_windowed_mmc3(addr, base, 0x0F)
    }

    pub(in crate::cartridge) fn read_prg_mapper44(&self, addr: u16) -> u8 {
        let (base, bank_mask) = self.mapper44_prg_window();
        self.read_prg_windowed_mmc3(addr, base, bank_mask)
    }

    pub(in crate::cartridge) fn read_prg_mapper114(&self, addr: u16) -> u8 {
        if self.mappers.mmc3_variant.mapper114_override & 0x80 != 0 {
            return self.read_prg_nrom_override(
                addr,
                self.mapper114_selected_16k_bank(),
                self.mappers.mmc3_variant.mapper114_override & 0x40 != 0,
            );
        }
        self.read_prg_mmc3(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper123(&self, addr: u16) -> u8 {
        if self.mappers.mmc3_variant.mapper123_override & 0x40 != 0 {
            return self.read_prg_nrom_override(
                addr,
                self.mapper123_selected_16k_bank(),
                self.mappers.mmc3_variant.mapper123_override & 0x02 != 0,
            );
        }
        self.read_prg_mmc3(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper115(&self, addr: u16) -> u8 {
        if self.mappers.mmc3_variant.mapper115_override & 0x80 != 0 {
            return self.read_prg_nrom_override(
                addr,
                self.mapper115_selected_16k_bank(),
                self.mappers.mmc3_variant.mapper115_override & 0x20 != 0,
            );
        }
        self.read_prg_mmc3(addr)
    }

    pub(in crate::cartridge) fn read_prg_mapper205(&self, addr: u16) -> u8 {
        let (base, bank_mask) = self.mapper205_prg_window();
        self.read_prg_windowed_mmc3(addr, base, bank_mask)
    }

    pub(in crate::cartridge) fn write_prg_mapper114(&mut self, addr: u16, data: u8) {
        if let Some(synthetic_addr) = Self::translate_mapper114_addr(addr) {
            let synthetic_data = if synthetic_addr == 0x8000 {
                Self::mapper114_scramble_index(data)
            } else {
                data
            };
            self.write_prg_mmc3(synthetic_addr, synthetic_data);
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper123(&mut self, addr: u16, data: u8) {
        if (addr & 0xF800) == 0x5800 {
            self.mappers.mmc3_variant.mapper123_override = data;
            return;
        }

        if (0x8000..=0xFFFF).contains(&addr) {
            let synthetic_data = if (addr & 0xE001) == 0x8000 {
                Self::mapper114_scramble_index(data)
            } else {
                data
            };
            self.write_prg_mmc3(addr, synthetic_data);
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper12(&mut self, addr: u16, data: u8) {
        if (addr & 0xE001) == 0xA001 {
            self.mappers.mmc3_variant.mapper12_chr_outer = data & 0x11;
            return;
        }

        self.write_prg_mmc3(addr, data);
    }

    pub(in crate::cartridge) fn write_prg_mapper44(&mut self, addr: u16, data: u8) {
        if (addr & 0xE001) == 0xA001 {
            self.mappers.mmc3_variant.mapper44_outer_bank = data & 0x07;
            return;
        }

        self.write_prg_mmc3(addr, data);
    }
}
