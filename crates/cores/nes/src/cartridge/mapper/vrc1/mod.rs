use super::super::{Cartridge, Mirroring};

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc1 {
    pub(in crate::cartridge) prg_banks: [u8; 3],
    pub(in crate::cartridge) chr_bank_0: u8,
    pub(in crate::cartridge) chr_bank_1: u8,
}

impl Vrc1 {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            prg_banks: [0, 1, 2],
            chr_bank_0: 0,
            chr_bank_1: 1,
        }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_vrc1(&self, addr: u16) -> u8 {
        if let Some(ref vrc1) = self.mappers.vrc1 {
            let bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let fixed_last = (bank_count - 1) as u8;
            let bank = match addr {
                0x8000..=0x9FFF => vrc1.prg_banks[0],
                0xA000..=0xBFFF => vrc1.prg_banks[1],
                0xC000..=0xDFFF => vrc1.prg_banks[2],
                0xE000..=0xFFFF => fixed_last,
                _ => return 0,
            } as usize
                % bank_count;

            let base = bank * 0x2000;
            let offset = (addr as usize) & 0x1FFF;
            self.prg_rom.get(base + offset).copied().unwrap_or(0)
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_prg_vrc1(&mut self, addr: u16, data: u8) {
        if let Some(ref mut vrc1) = self.mappers.vrc1 {
            let prg_bank_count = (self.prg_rom.len() / 0x2000).max(1);
            let chr_bank_count = (self.chr_rom.len() / 0x1000).max(1);

            match addr {
                0x8000..=0x8FFF => {
                    vrc1.prg_banks[0] = ((data & 0x0F) as usize % prg_bank_count) as u8;
                }
                0x9000..=0x9FFF => {
                    self.mirroring = if data & 0x01 != 0 {
                        Mirroring::Horizontal
                    } else {
                        Mirroring::Vertical
                    };
                    vrc1.chr_bank_0 =
                        ((vrc1.chr_bank_0 & 0x0F) | ((data & 0x02) << 3)) % chr_bank_count as u8;
                    vrc1.chr_bank_1 =
                        ((vrc1.chr_bank_1 & 0x0F) | ((data & 0x04) << 2)) % chr_bank_count as u8;
                }
                0xA000..=0xAFFF => {
                    vrc1.prg_banks[1] = ((data & 0x0F) as usize % prg_bank_count) as u8;
                }
                0xC000..=0xCFFF => {
                    vrc1.prg_banks[2] = ((data & 0x0F) as usize % prg_bank_count) as u8;
                }
                0xE000..=0xEFFF => {
                    vrc1.chr_bank_0 = (((vrc1.chr_bank_0 & 0x10) | (data & 0x0F)) as usize
                        % chr_bank_count) as u8;
                }
                0xF000..=0xFFFF => {
                    vrc1.chr_bank_1 = (((vrc1.chr_bank_1 & 0x10) | (data & 0x0F)) as usize
                        % chr_bank_count) as u8;
                }
                _ => {}
            }
        }
    }

    pub(in crate::cartridge) fn read_chr_vrc1(&self, addr: u16) -> u8 {
        if let Some(ref vrc1) = self.mappers.vrc1 {
            if self.chr_rom.is_empty() {
                return 0;
            }
            let bank_count = (self.chr_rom.len() / 0x1000).max(1);
            let (bank, offset) = if addr < 0x1000 {
                ((vrc1.chr_bank_0 as usize) % bank_count, addr as usize)
            } else {
                (
                    (vrc1.chr_bank_1 as usize) % bank_count,
                    (addr as usize) - 0x1000,
                )
            };
            self.chr_rom[(bank * 0x1000 + offset) % self.chr_rom.len()]
        } else {
            0
        }
    }

    pub(in crate::cartridge) fn write_chr_vrc1(&mut self, addr: u16, data: u8) {
        let (bank, offset, chr_len) = if let Some(ref vrc1) = self.mappers.vrc1 {
            if self.chr_rom.is_empty() {
                return;
            }
            let bank_count = (self.chr_rom.len() / 0x1000).max(1);
            let (bank, offset) = if addr < 0x1000 {
                ((vrc1.chr_bank_0 as usize) % bank_count, addr as usize)
            } else {
                (
                    (vrc1.chr_bank_1 as usize) % bank_count,
                    (addr as usize) - 0x1000,
                )
            };
            (bank, offset, self.chr_rom.len())
        } else {
            return;
        };

        self.chr_rom[(bank * 0x1000 + offset) % chr_len] = data;
    }
}
