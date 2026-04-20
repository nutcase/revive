use super::super::{Cartridge, Mirroring};

impl Cartridge {
    fn mapper103_bank_window(&self) -> usize {
        self.prg_rom.len().min(0x18000)
    }

    fn mapper103_read_rom(&self, base: usize, offset: usize) -> u8 {
        if self.prg_rom.is_empty() {
            return 0;
        }
        self.prg_rom[(base + offset) % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn read_prg_mapper103(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xB7FF => self.mapper103_read_rom(0x18000, (addr - 0x8000) as usize),
            0xB800..=0xD7FF => {
                if !self.mappers.simple.mapper103_prg_ram_disabled && !self.prg_ram.is_empty() {
                    self.prg_ram[(addr - 0xB800) as usize % self.prg_ram.len()]
                } else {
                    self.mapper103_read_rom(0x1B800, (addr - 0xB800) as usize)
                }
            }
            0xD800..=0xFFFF => self.mapper103_read_rom(0x1D800, (addr - 0xD800) as usize),
            _ => 0,
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper103(&mut self, addr: u16, data: u8) {
        match addr {
            0x8000..=0x8FFF => {
                let bank_count = (self.mapper103_bank_window() / 0x2000).max(1);
                self.prg_bank = (data as usize % bank_count) as u8;
            }
            0xB800..=0xD7FF => {
                if !self.prg_ram.is_empty() {
                    let ram_addr = (addr - 0xB800) as usize % self.prg_ram.len();
                    self.prg_ram[ram_addr] = data;
                }
            }
            0xE000..=0xEFFF => {
                self.mirroring = if data & 0x08 != 0 {
                    Mirroring::Horizontal
                } else {
                    Mirroring::Vertical
                };
            }
            0xF000..=0xFFFF => {
                self.mappers.simple.mapper103_prg_ram_disabled = data & 0x10 != 0;
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_ram_mapper103(&self, addr: u16) -> u8 {
        if !self.mappers.simple.mapper103_prg_ram_disabled && !self.prg_ram.is_empty() {
            return self.prg_ram[(addr - 0x6000) as usize % self.prg_ram.len()];
        }

        let bank_window = self.mapper103_bank_window();
        let bank_count = (bank_window / 0x2000).max(1);
        let bank = self.prg_bank as usize % bank_count;
        let base = bank * 0x2000;
        self.mapper103_read_rom(base, (addr - 0x6000) as usize)
    }

    pub(in crate::cartridge) fn write_prg_ram_mapper103(&mut self, addr: u16, data: u8) {
        if self.prg_ram.is_empty() {
            return;
        }
        let ram_addr = (addr - 0x6000) as usize % self.prg_ram.len();
        self.prg_ram[ram_addr] = data;
    }
}
