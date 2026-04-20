use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn sync_mapper61_latch(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
        let prg_bank = ((((self.mappers.multicart.mapper61_latch as usize) & 0x000F) << 1)
            | (((self.mappers.multicart.mapper61_latch as usize) >> 5) & 0x01))
            % prg_bank_count;
        let chr_bank = if self.chr_rom.len() > 0x20000 {
            ((((self.mappers.multicart.mapper61_latch as usize) >> 8) & 0x0F) << 1)
                | (((self.mappers.multicart.mapper61_latch as usize) >> 6) & 0x01)
        } else {
            ((self.mappers.multicart.mapper61_latch as usize) >> 8) & 0x0F
        } % chr_bank_count;

        self.prg_bank = prg_bank as u8;
        self.chr_bank = chr_bank as u8;
        self.mirroring = if self.mappers.multicart.mapper61_latch & 0x0080 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
    }

    /// Mapper 61: address-latched NROM multicart with a 5-bit 16KB PRG bank,
    /// 4/5-bit 8KB CHR bank, and a 16KB/32KB PRG mode bit.
    pub(in crate::cartridge) fn read_prg_mapper61(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank16 = (self.prg_bank as usize) % bank_count;
        let bank = if self.mappers.multicart.mapper61_latch & 0x0010 != 0 {
            bank16
        } else {
            (bank16 & !1) | usize::from(addr >= 0xC000)
        };
        let rom_addr = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_mapper61(&mut self, addr: u16) {
        if addr < 0x8000 {
            return;
        }

        self.mappers.multicart.mapper61_latch = addr & 0x0FFF;
        self.sync_mapper61_latch();
    }
}
