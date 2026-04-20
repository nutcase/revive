use super::super::Cartridge;

impl Cartridge {
    /// Mapper 3 (CNROM) PRG write - CHR bank switching with bus conflicts
    pub(in crate::cartridge) fn write_prg_cnrom(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            // Read ROM value at the write address to handle bus conflicts
            let rom_value = if (addr as usize) < self.prg_rom.len() {
                self.prg_rom[addr as usize]
            } else {
                let mirrored_addr = (addr - 0x8000) % (self.prg_rom.len() as u16);
                self.prg_rom[mirrored_addr as usize]
            };

            // Bus conflict: use AND of written value and ROM value
            let effective_value = data & rom_value;
            self.chr_bank = effective_value & 0x03;
        }
    }

    /// Mapper 87 PRG write - CHR bank switching at $6000-$7FFF
    pub(in crate::cartridge) fn write_prg_mapper87(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            // Swap bits 0 and 1
            self.chr_bank = ((data & 0x01) << 1) | ((data & 0x02) >> 1);
        }
    }

    /// Mapper 101 - JF-10 bad dump variant with normal CHR bit ordering.
    pub(in crate::cartridge) fn write_prg_mapper101(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.chr_bank = (data as usize % bank_count) as u8;
        }
    }

    /// Mapper 145: low-address latch variant of CNROM using bit 7 as the
    /// 8KB CHR bank select.
    pub(in crate::cartridge) fn write_prg_mapper145(&mut self, addr: u16, data: u8) {
        if (addr & 0xE100) == 0x4100 {
            let bank_count = (self.chr_rom.len() / 0x2000).max(1);
            self.chr_bank = (((data >> 7) as usize) % bank_count) as u8;
        }
    }

    /// Mapper 185: CNROM variant with CHR output disabled during startup
    /// probes when the exact chip-select wiring is not available.
    pub(in crate::cartridge) fn read_chr_mapper185(&self, addr: u16) -> u8 {
        let remaining = self.mappers.simple.mapper185_disabled_reads.get();
        if remaining > 0 {
            self.mappers
                .simple
                .mapper185_disabled_reads
                .set(remaining - 1);
            return 0;
        }

        self.read_chr_cnrom(addr)
    }

    pub(in crate::cartridge) fn write_chr_mapper185(&mut self, addr: u16, data: u8) {
        self.write_chr_cnrom(addr, data);
    }

    /// Mapper 184 (Sunsoft-1): bits 0-2 select the lower 4KB CHR bank and
    /// bits 4-5 select the upper bank, which always maps into banks 4-7.
    pub(in crate::cartridge) fn write_prg_mapper184(&mut self, addr: u16, data: u8) {
        if (0x6000..=0x7FFF).contains(&addr) {
            let bank_count = (self.chr_rom.len() / 0x1000).max(1);
            self.chr_bank = ((data & 0x07) as usize % bank_count) as u8;
            let upper_bank = 4 | ((data >> 4) & 0x03);
            self.chr_bank_1 = (upper_bank as usize % bank_count) as u8;
        }
    }

    /// CNROM/Mapper 87 CHR read - 8KB CHR bank switching
    pub(in crate::cartridge) fn read_chr_cnrom(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.chr_rom.len() / 0x2000).max(1);
        let bank = (self.chr_bank as usize) % bank_count;
        let bank_addr = bank * 0x2000 + (addr as usize);
        self.chr_rom[bank_addr % self.chr_rom.len()]
    }

    /// CNROM/Mapper 87 CHR write
    pub(in crate::cartridge) fn write_chr_cnrom(&mut self, addr: u16, data: u8) {
        if !self.chr_rom.is_empty() {
            let bank_count = (self.chr_rom.len() / 0x2000).max(1);
            let bank = (self.chr_bank as usize) % bank_count;
            let bank_addr = bank * 0x2000 + (addr as usize);
            let chr_len = self.chr_rom.len();
            self.chr_rom[bank_addr % chr_len] = data;
        }
    }

    pub(in crate::cartridge) fn read_chr_split_4k(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.chr_rom.len() / 0x1000).max(1);
        let (bank, offset) = if addr < 0x1000 {
            ((self.chr_bank as usize) % bank_count, addr as usize)
        } else {
            (
                (self.chr_bank_1 as usize) % bank_count,
                (addr as usize) - 0x1000,
            )
        };
        let chr_addr = bank * 0x1000 + offset;
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }

    pub(in crate::cartridge) fn write_chr_split_4k(&mut self, addr: u16, data: u8) {
        if self.chr_rom.is_empty() {
            return;
        }

        let bank_count = (self.chr_rom.len() / 0x1000).max(1);
        let (bank, offset) = if addr < 0x1000 {
            ((self.chr_bank as usize) % bank_count, addr as usize)
        } else {
            (
                (self.chr_bank_1 as usize) % bank_count,
                (addr as usize) - 0x1000,
            )
        };
        let chr_len = self.chr_rom.len();
        let chr_addr = bank * 0x1000 + offset;
        self.chr_rom[chr_addr % chr_len] = data;
    }
}
