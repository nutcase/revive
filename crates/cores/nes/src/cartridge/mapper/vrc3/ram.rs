use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_ram_mapper142(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || !(0x6000..=0x7FFF).contains(&addr) {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let bank = (self.mappers.simple.mapper142_prg_banks[3] as usize) % bank_count;
        let offset = bank * 0x2000 + ((addr - 0x6000) as usize & 0x1FFF);
        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn read_prg_ram_vrc3(&self, addr: u16) -> u8 {
        let offset = (addr - 0x6000) as usize;
        self.prg_ram.get(offset).copied().unwrap_or(0)
    }

    pub(in crate::cartridge) fn write_prg_ram_vrc3(&mut self, addr: u16, data: u8) {
        let offset = (addr - 0x6000) as usize;
        if let Some(slot) = self.prg_ram.get_mut(offset) {
            *slot = data;
        }
    }
}
