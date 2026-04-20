use super::super::Cartridge;

impl Cartridge {
    /// Mapper 11 (Color Dreams): bits 0-1 select a 32KB PRG bank and
    /// bits 4-7 select an 8KB CHR bank.
    pub(in crate::cartridge) fn write_prg_color_dreams(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.prg_bank = data & 0x03;
            self.chr_bank = (data >> 4) & 0x0F;
        }
    }

    /// Mapper 46 (Rumble Station): outer bank register at $6000-$7FFF,
    /// with four PRG high bits and four CHR high bits.
    pub(in crate::cartridge) fn write_prg_mapper46_outer(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            let prg_high = ((data & 0x0F) as usize) << 1;
            let chr_high = (((data >> 4) & 0x0F) as usize) << 3;

            self.prg_bank = ((prg_high | (self.prg_bank as usize & 0x01)) % prg_bank_count) as u8;
            self.chr_bank = ((chr_high | (self.chr_bank as usize & 0x07)) % chr_bank_count) as u8;
        }
    }

    /// Mapper 46 inner register at $8000-$FFFF, with one PRG low bit and
    /// three CHR low bits.
    pub(in crate::cartridge) fn write_prg_mapper46_inner(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);
            let prg_bank = (self.prg_bank as usize & !0x01) | (data as usize & 0x01);
            let chr_bank = (self.chr_bank as usize & !0x07) | (((data >> 4) & 0x07) as usize);

            self.prg_bank = (prg_bank % prg_bank_count) as u8;
            self.chr_bank = (chr_bank % chr_bank_count) as u8;
        }
    }

    /// Mapper 144: Death Race board where CPU D0 is effectively forced by the
    /// ROM during bus conflicts before the usual Color Dreams decode.
    pub(in crate::cartridge) fn write_prg_mapper144(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let rom_value = if self.prg_rom.is_empty() {
                0xFF
            } else {
                let offset = (self.prg_bank as usize) * 0x8000 + (addr - 0x8000) as usize;
                self.prg_rom[offset % self.prg_rom.len()]
            };
            let effective = rom_value & (data | 0x01);
            self.prg_bank = effective & 0x03;
            self.chr_bank = (effective >> 4) & 0x0F;
        }
    }
}
