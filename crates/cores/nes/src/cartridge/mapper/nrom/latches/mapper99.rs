use crate::cartridge::Cartridge;

impl Cartridge {
    /// Mapper 99 (Vs. System): fixed 24KB PRG at $A000-$FFFF plus a single
    /// selectable 8KB page at $8000-$9FFF. Missing pages float open bus.
    pub(in crate::cartridge) fn read_prg_mapper99(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0xFF;
        }

        let bank = match addr {
            0x8000..=0x9FFF => self.prg_bank as usize,
            0xA000..=0xBFFF => 1,
            0xC000..=0xDFFF => 2,
            _ => 3,
        };
        let offset = bank * 0x2000 + ((addr - 0x8000) as usize & 0x1FFF);
        self.prg_rom.get(offset).copied().unwrap_or(0xFF)
    }

    pub(in crate::cartridge) fn write_prg_low_mapper99(&mut self, addr: u16, data: u8) {
        if addr != 0x4016 {
            return;
        }

        let high_bank = data & 0x04 != 0;
        self.prg_bank = if high_bank { 4 } else { 0 };
        self.chr_bank = if high_bank { 1 } else { 0 };
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper99(&self, addr: u16) -> u8 {
        if self.prg_ram.is_empty() || !(0x6000..=0x7FFF).contains(&addr) {
            return 0;
        }

        let offset = (addr as usize - 0x6000) & 0x07FF;
        self.prg_ram[offset]
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper99(&mut self, addr: u16, data: u8) {
        if self.prg_ram.is_empty() || !(0x6000..=0x7FFF).contains(&addr) {
            return;
        }

        let offset = (addr as usize - 0x6000) & 0x07FF;
        self.prg_ram[offset] = data;
    }
}
