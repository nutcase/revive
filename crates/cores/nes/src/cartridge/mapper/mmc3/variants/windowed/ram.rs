use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_ram_mapper37(&mut self, _addr: u16, data: u8) {
        if self.mmc3_prg_ram_writable() {
            self.mappers.mmc3_variant.mapper37_outer_bank = data & 0x07;
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper47(&mut self, _addr: u16, data: u8) {
        if self.mmc3_prg_ram_writable() {
            self.mappers.mmc3_variant.mapper47_outer_bank = data & 0x01;
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper114(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000 => self.mappers.mmc3_variant.mapper114_override = data,
            0x6001 => self.mappers.mmc3_variant.mapper114_chr_outer_bank = data & 0x01,
            _ => {}
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper115(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000 => self.mappers.mmc3_variant.mapper115_override = data,
            0x6001 => self.mappers.mmc3_variant.mapper115_chr_outer_bank = data & 0x01,
            0x6002 => {}
            _ => self.write_prg_ram_mmc3(addr, data),
        }
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper205(&mut self, _addr: u16, data: u8) {
        self.mappers.mmc3_variant.mapper205_block = data & 0x03;
    }
}
