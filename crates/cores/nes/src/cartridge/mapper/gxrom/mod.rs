use super::super::Cartridge;

impl Cartridge {
    /// Mapper 66 (GxROM): bits 4-5 select a 32KB PRG bank and bits 0-1
    /// select an 8KB CHR bank.
    pub(in crate::cartridge) fn write_prg_gxrom(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.prg_bank = (data >> 4) & 0x03;
            self.chr_bank = data & 0x03;
        }
    }

    /// Mapper 38 (Bit Corp. UNL-PCI556): writes through $7000-$7FFF with
    /// bits 0-1 selecting a 32KB PRG bank and bits 2-3 selecting an 8KB
    /// CHR bank.
    pub(in crate::cartridge) fn write_prg_mapper38(&mut self, addr: u16, data: u8) {
        if (0x7000..=0x7FFF).contains(&addr) {
            self.prg_bank = data & 0x03;
            self.chr_bank = (data >> 2) & 0x03;
        }
    }

    /// Mapper 140: GNROM-style banking via $6000-$7FFF, with 2 PRG bits and
    /// 4 CHR bits.
    pub(in crate::cartridge) fn write_prg_mapper140(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            self.prg_bank = (data >> 4) & 0x03;
            self.chr_bank = data & 0x0F;
        }
    }

    /// Mapper 240: GNROM-like banking through a register in the CPU
    /// $4020-$5FFF area.
    pub(in crate::cartridge) fn write_prg_mapper240(&mut self, addr: u16, data: u8) {
        if (addr & 0xE800) == 0x4800 || (addr & 0xE100) == 0x4100 {
            self.prg_bank = data >> 4;
            self.chr_bank = data & 0x0F;
        }
    }

    /// Mapper 86 (Jaleco JF-13): $6000/$E000 bank latch using PRG bits 4-5 and
    /// CHR bits 0-1 plus bit 6. Audio control at $7000/$F000 is ignored.
    pub(in crate::cartridge) fn write_prg_mapper86(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x6FFF).contains(&addr) || (0xE000..=0xEFFF).contains(&addr) {
            self.prg_bank = (data >> 4) & 0x03;
            self.chr_bank = ((data >> 4) & 0x04) | (data & 0x03);
        }
    }

    /// Mapper 107 (Magic Dragon): bits 1-7 select the 32KB PRG bank while the
    /// full byte selects the 8KB CHR bank.
    pub(in crate::cartridge) fn write_prg_mapper107(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            self.prg_bank = data >> 1;
            self.chr_bank = data;
        }
    }
}
