use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn sync_mapper59_latch(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

        self.prg_bank =
            (((self.mappers.multicart.mapper59_latch as usize >> 4) & 0x07) % prg_bank_count) as u8;
        self.chr_bank =
            ((self.mappers.multicart.mapper59_latch as usize & 0x07) % chr_bank_count) as u8;
        self.mirroring = if self.mappers.multicart.mapper59_latch & 0x0008 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
    }

    /// Mapper 59: address-latched 16KB/32KB PRG selector with an optional
    /// "jumper read" mode that replaces ROM data with open bus / strap bits.
    pub(in crate::cartridge) fn read_prg_mapper59(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0;
        }

        if self.mappers.multicart.mapper59_latch & 0x0100 != 0 {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank16 = (self.prg_bank as usize) % bank_count;
        let bank = if self.mappers.multicart.mapper59_latch & 0x0080 != 0 {
            bank16
        } else {
            (bank16 & !1) | usize::from(addr >= 0xC000)
        };
        let rom_addr = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_mapper59(&mut self, addr: u16) {
        if addr < 0x8000 || self.mappers.multicart.mapper59_locked {
            return;
        }

        self.mappers.multicart.mapper59_latch = addr & 0x03FF;
        self.mappers.multicart.mapper59_locked = (addr & 0x0200) != 0;
        self.sync_mapper59_latch();
    }
}
