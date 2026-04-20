use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper225(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }

        if self.mappers.multicart.mapper225_nrom128 {
            self.read_multicart_prg_16k(addr, self.prg_bank as usize, 0)
        } else {
            self.read_multicart_prg_32k(addr, (self.prg_bank as usize) >> 1, 0)
        }
    }

    pub(in crate::cartridge) fn read_prg_low_mapper225(&self, addr: u16) -> u8 {
        if (0x5800..=0x5FFF).contains(&addr) && !self.prg_ram.is_empty() {
            self.prg_ram[(addr as usize) & 0x03] & 0x0F
        } else {
            0
        }
    }
}
