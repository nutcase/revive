use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mapper148(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let effective = data & self.bus_conflict_value_switchable_32k(addr);
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.prg_bank = (((effective >> 3) & 0x01) as usize % prg_bank_count) as u8;
            self.chr_bank = ((effective & 0x07) as usize % chr_bank_count) as u8;
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper147(&mut self, addr: u16, data: u8) {
        if (addr & 0x4103) == 0x4102 {
            let effective = if addr >= 0x8000 {
                data & self.bus_conflict_value_switchable_32k(addr)
            } else {
                data
            };
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = ((((effective >> 2) & 0x01) | ((effective >> 6) & 0x02)) as usize
                % prg_bank_count) as u8;
            self.chr_bank = (((effective >> 3) & 0x0F) as usize % chr_bank_count) as u8;
        }
    }
}
