use super::super::Cartridge;

impl Cartridge {
    /// Mapper 94 (UN1ROM): fixed CHR-RAM and a 16KB PRG bank at $8000
    /// selected by bits 2-4 of the data bus, with the last 16KB fixed.
    pub(in crate::cartridge) fn write_prg_mapper94(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let rom_offset = if addr < 0xC000 {
                (self.prg_bank as usize) * 0x4000 + ((addr - 0x8000) as usize)
            } else {
                self.prg_rom.len().saturating_sub(0x4000) + ((addr - 0xC000) as usize)
            };

            let rom_value = if rom_offset < self.prg_rom.len() {
                self.prg_rom[rom_offset]
            } else {
                0xFF
            };

            let effective = data & rom_value;
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);
            self.prg_bank = (((effective >> 2) & 0x07) as usize % bank_count) as u8;
        }
    }
}
