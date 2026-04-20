use crate::cartridge::Cartridge;

impl Cartridge {
    fn mapper63_chr_write_protected(&self) -> bool {
        self.mappers.multicart.mapper63_latch & 0x0400 != 0
    }
    /// NROM CHR read - 8KB CHR ROM direct mapping
    pub(in crate::cartridge) fn read_chr_nrom(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_rom.len() {
            self.chr_rom[chr_addr]
        } else {
            0
        }
    }
    pub(in crate::cartridge) fn read_chr_mapper99(&self, addr: u16) -> u8 {
        let offset = (self.chr_bank as usize) * 0x2000 + (addr as usize & 0x1FFF);
        self.chr_rom.get(offset).copied().unwrap_or(0xFF)
    }
    /// NROM CHR write
    pub(in crate::cartridge) fn write_chr_nrom(&mut self, addr: u16, data: u8) {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_rom.len() {
            self.chr_rom[chr_addr] = data;
        }
    }
    pub(in crate::cartridge) fn read_chr_mapper63(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        self.chr_ram.get(chr_addr).copied().unwrap_or(0)
    }
    pub(in crate::cartridge) fn write_chr_mapper63(&mut self, addr: u16, data: u8) {
        if self.mapper63_chr_write_protected() {
            return;
        }

        let chr_addr = (addr & 0x1FFF) as usize;
        if let Some(slot) = self.chr_ram.get_mut(chr_addr) {
            *slot = data;
        }
    }
    pub(in crate::cartridge) fn read_chr_mapper221(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr]
        } else {
            0
        }
    }
    pub(in crate::cartridge) fn write_chr_mapper221(&mut self, addr: u16, data: u8) {
        if self.mappers.multicart.mapper221_chr_write_protect {
            return;
        }

        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr] = data;
        }
    }
    pub(in crate::cartridge) fn read_chr_mapper231(&self, addr: u16) -> u8 {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr]
        } else {
            0
        }
    }
    pub(in crate::cartridge) fn write_chr_mapper231(&mut self, addr: u16, data: u8) {
        let chr_addr = (addr & 0x1FFF) as usize;
        if chr_addr < self.chr_ram.len() {
            self.chr_ram[chr_addr] = data;
        }
    }
    /// Mapper 13 (CPROM): fixed 4KB CHR-RAM at $0000 and a switchable
    /// 4KB CHR-RAM page at $1000 selected by bits 0-1.
    pub(in crate::cartridge) fn write_prg_cprom(&mut self, addr: u16, data: u8) {
        if addr >= 0x8000 {
            let rom_value = if self.prg_rom.is_empty() {
                0xFF
            } else {
                self.prg_rom[(addr - 0x8000) as usize % self.prg_rom.len()]
            };
            self.chr_bank = (data & rom_value) & 0x03;
        }
    }
    pub(in crate::cartridge) fn read_chr_cprom(&self, addr: u16) -> u8 {
        if self.chr_rom.is_empty() {
            return 0;
        }

        let bank = if addr < 0x1000 {
            0usize
        } else {
            (self.chr_bank as usize) % (self.chr_rom.len() / 0x1000).max(1)
        };
        let offset = if addr < 0x1000 {
            addr as usize
        } else {
            (addr as usize) - 0x1000
        };
        let chr_addr = bank * 0x1000 + offset;
        self.chr_rom[chr_addr % self.chr_rom.len()]
    }
    pub(in crate::cartridge) fn write_chr_cprom(&mut self, addr: u16, data: u8) {
        if self.chr_rom.is_empty() {
            return;
        }

        let bank_count = (self.chr_rom.len() / 0x1000).max(1);
        let bank = if addr < 0x1000 {
            0usize
        } else {
            (self.chr_bank as usize) % bank_count
        };
        let offset = if addr < 0x1000 {
            addr as usize
        } else {
            (addr as usize) - 0x1000
        };
        let chr_len = self.chr_rom.len();
        let chr_addr = bank * 0x1000 + offset;
        self.chr_rom[chr_addr % chr_len] = data;
    }
}
