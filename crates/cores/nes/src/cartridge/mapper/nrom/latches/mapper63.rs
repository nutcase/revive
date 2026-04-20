use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    /// Mapper 63: writes use the complemented CPU address as a latch. Bits
    /// 9-2 choose a 16KB PRG page, bit 1 selects NROM-128 vs NROM-256 mode,
    /// bit 0 selects mirroring, and bit 10 write-protects the 8KB CHR-RAM.
    /// This matches the common submapper-0 wiring.
    pub(in crate::cartridge) fn read_prg_mapper63(&self, addr: u16) -> u8 {
        if self.prg_rom.is_empty() || addr < 0x8000 {
            return 0xFF;
        }

        let bank16 = ((self.mappers.multicart.mapper63_latch as usize) >> 2) & 0x00FF;
        let bank_count = (self.prg_rom.len() / 0x4000).max(1);
        let bank = if self.mappers.multicart.mapper63_latch & 0x0002 != 0 {
            (bank16 & !1) | usize::from(addr >= 0xC000)
        } else {
            bank16
        };

        if bank >= bank_count {
            return 0xFF;
        }

        let offset = bank * 0x4000 + ((addr - 0x8000) as usize & 0x3FFF);
        self.prg_rom[offset]
    }

    pub(in crate::cartridge) fn write_prg_mapper63(&mut self, addr: u16) {
        if addr < 0x8000 {
            return;
        }

        self.mappers.multicart.mapper63_latch = !addr;
        self.prg_bank = (((self.mappers.multicart.mapper63_latch as usize) >> 2)
            % (self.prg_rom.len() / 0x4000).max(1)) as u8;
        self.mirroring = if self.mappers.multicart.mapper63_latch & 0x0001 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
    }
}
