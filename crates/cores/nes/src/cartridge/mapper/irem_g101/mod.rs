use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct IremG101 {
    pub(in crate::cartridge) prg_banks: [u8; 2],
    pub(in crate::cartridge) chr_banks: [u8; 8],
    pub(in crate::cartridge) prg_mode: bool,
}

impl IremG101 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1],
            chr_banks: [0, 1, 2, 3, 4, 5, 6, 7],
            prg_mode: false,
        }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_mapper32(&self, addr: u16) -> u8 {
        if let Some(g101) = self.mappers.irem_g101.as_ref() {
            if self.prg_rom.is_empty() {
                return 0;
            }

            let bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let second_last = bank_count.saturating_sub(2);
            let last = bank_count.saturating_sub(1);
            let bank = match addr {
                0x8000..=0x9FFF => {
                    if g101.prg_mode {
                        second_last
                    } else {
                        g101.prg_banks[0] as usize
                    }
                }
                0xA000..=0xBFFF => g101.prg_banks[1] as usize,
                0xC000..=0xDFFF => {
                    if g101.prg_mode {
                        g101.prg_banks[0] as usize
                    } else {
                        second_last
                    }
                }
                0xE000..=0xFFFF => last,
                _ => return 0,
            } % bank_count;

            let prg_addr = bank * 0x2000 + (addr as usize & 0x1FFF);
            self.prg_rom[prg_addr % self.prg_rom.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_mapper32(&mut self, addr: u16, data: u8) {
        if let Some(g101) = self.mappers.irem_g101.as_mut() {
            let prg_bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x0400).max(1);

            match addr {
                0x8000..=0x8FFF => {
                    g101.prg_banks[0] = (data as usize % prg_bank_count) as u8;
                    self.prg_bank = g101.prg_banks[0];
                }
                0x9000..=0x9FFF => {
                    g101.prg_mode = data & 0x02 != 0;
                    self.mirroring = if data & 0x01 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                }
                0xA000..=0xAFFF => {
                    g101.prg_banks[1] = (data as usize % prg_bank_count) as u8;
                }
                0xB000..=0xBFFF => {
                    let index = (addr as usize) & 0x07;
                    g101.chr_banks[index] = (data as usize % chr_bank_count) as u8;
                    if index == 0 {
                        self.chr_bank = g101.chr_banks[0];
                    }
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn read_chr_mapper32(&self, addr: u16) -> u8 {
        if let Some(g101) = self.mappers.irem_g101.as_ref() {
            if self.chr_rom.is_empty() {
                return 0;
            }

            let bank_count = (self.chr_rom.len() / 0x0400).max(1);
            let bank = g101.chr_banks[((addr >> 10) & 0x07) as usize] as usize % bank_count;
            let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
            self.chr_rom[chr_addr % self.chr_rom.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_mapper32(&mut self, addr: u16, data: u8) {
        let (bank, chr_len) = if let Some(g101) = self.mappers.irem_g101.as_ref() {
            if self.chr_rom.is_empty() {
                return;
            }

            let bank_count = (self.chr_rom.len() / 0x0400).max(1);
            (
                g101.chr_banks[((addr >> 10) & 0x07) as usize] as usize % bank_count,
                self.chr_rom.len(),
            )
        } else {
            return;
        };

        let chr_addr = bank * 0x0400 + (addr as usize & 0x03FF);
        self.chr_rom[chr_addr % chr_len] = data;
    }
}
