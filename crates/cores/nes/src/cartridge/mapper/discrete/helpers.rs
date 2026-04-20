use crate::cartridge::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn bus_conflict_value_fixed_last_16k(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0xFF;
        }

        let offset = if addr < 0xC000 {
            (self.prg_bank as usize) * 0x4000 + (addr.saturating_sub(0x8000) as usize)
        } else {
            self.prg_rom.len().saturating_sub(0x4000) + (addr.saturating_sub(0xC000) as usize)
        };

        self.prg_rom[offset % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn bus_conflict_value_switchable_32k(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() {
            return 0xFF;
        }

        let offset = (self.prg_bank as usize) * 0x8000 + (addr.saturating_sub(0x8000) as usize);
        self.prg_rom[offset % self.prg_rom.len()]
    }
}
