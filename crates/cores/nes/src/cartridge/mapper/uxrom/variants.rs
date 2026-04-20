use crate::cartridge::Cartridge;

impl Cartridge {
    /// Mapper 58: multicart board that can behave as either NROM-256
    /// (32KB switchable) or NROM-128 (16KB mirrored) depending on the
    /// latched address mode bit.
    pub(in crate::cartridge) fn read_prg_mapper58(&self, addr: u16) -> u8 {
        if self.mappers.multicart.mapper58_nrom128 {
            let bank_count = (self.prg_rom.len() / 0x4000).max(1);
            let bank = (self.prg_bank as usize) % bank_count;
            let offset = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
            self.prg_rom[offset % self.prg_rom.len()]
        } else {
            let bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let bank = ((self.prg_bank as usize) >> 1) % bank_count;
            let offset = bank * 0x8000 + (addr - 0x8000) as usize;
            self.prg_rom[offset % self.prg_rom.len()]
        }
    }

    /// Mapper 81: the four low address bits on writes to $8000-$FFFF latch
    /// both the 16KB PRG bank at $8000 and the 8KB CHR bank.
    pub(in crate::cartridge) fn write_prg_mapper81(&mut self, addr: u16) {
        if addr >= 0x8000 {
            self.prg_bank = ((addr >> 2) & 0x03) as u8;
            self.chr_bank = (addr & 0x03) as u8;
        }
    }

    /// Mapper 58 latches bank bits directly from the CPU address.
    pub(in crate::cartridge) fn write_prg_mapper58(&mut self, addr: u16) {
        if addr >= 0x8000 {
            let prg_bank_count_16k = (self.prg_rom.len() / 0x4000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

            self.prg_bank = ((addr as usize & 0x07) % prg_bank_count_16k) as u8;
            self.chr_bank = (((addr as usize >> 3) & 0x07) % chr_bank_count) as u8;
            self.mappers.multicart.mapper58_nrom128 = addr & 0x40 != 0;
            self.mirroring = if addr & 0x80 != 0 {
                crate::cartridge::Mirroring::Horizontal
            } else {
                crate::cartridge::Mirroring::Vertical
            };
        }
    }
}
