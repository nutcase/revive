mod chr;
mod prg;

use super::super::super::super::Cartridge;

impl Cartridge {
    fn mapper191_outer_bank_writable(&self) -> bool {
        self.chr_rom.len() > 0x20000
    }

    fn mapper191_effective_outer_bank(&self) -> usize {
        if self.mapper191_outer_bank_writable() {
            (self.mappers.mmc3_variant.mapper191_outer_bank & 0x03) as usize
        } else {
            3
        }
    }

    fn mapper195_mode_for_bank(raw_bank: usize) -> Option<u8> {
        match raw_bank {
            0x00..=0x03 => Some(0x82),
            0x0A..=0x0B => Some(0xC8),
            0x28..=0x2B => Some(0x80),
            0x46..=0x47 => Some(0xC0),
            0x4C..=0x4F => Some(0x88),
            0x64..=0x67 => Some(0x8A),
            0x7C..=0x7D => Some(0xC2),
            0xCA => Some(0xCA),
            _ => None,
        }
    }

    fn resolve_mapper195_chr_bank(&self, raw_bank: usize) -> (bool, usize) {
        match self.mappers.mmc3_variant.mapper195_mode {
            0x80 => {
                if (0x28..=0x2B).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0x82 => {
                if (0x00..=0x03).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0x88 => {
                if (0x4C..=0x4F).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0x8A => {
                if (0x64..=0x67).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0xC0 => {
                if (0x46..=0x47).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0xC2 => {
                if (0x7C..=0x7D).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0xC8 => {
                if (0x0A..=0x0B).contains(&raw_bank) {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank)
                }
            }
            0xCA => (false, raw_bank),
            _ => (false, raw_bank),
        }
    }

    fn read_chr_mixed_bank(bank_data: &[u8], bank: usize, local_offset: usize) -> u8 {
        if bank_data.is_empty() {
            return 0;
        }

        let bank_count = (bank_data.len() / 0x0400).max(1);
        let chr_addr = ((bank % bank_count) * 0x0400 + local_offset) % bank_data.len();
        bank_data[chr_addr]
    }

    fn write_chr_mixed_bank(bank_data: &mut [u8], bank: usize, local_offset: usize, data: u8) {
        if bank_data.is_empty() {
            return;
        }

        let bank_count = (bank_data.len() / 0x0400).max(1);
        let chr_addr = ((bank % bank_count) * 0x0400 + local_offset) % bank_data.len();
        bank_data[chr_addr] = data;
    }

    fn resolve_mixed_chr_bank(&self, raw_bank: usize) -> (bool, usize) {
        match self.mapper {
            74 => match raw_bank {
                8 | 9 => (true, raw_bank - 8),
                _ => (false, raw_bank),
            },
            119 => {
                if raw_bank & 0x40 != 0 {
                    (true, raw_bank & 0x07)
                } else {
                    (false, raw_bank & 0x3F)
                }
            }
            191 => {
                if self.mapper191_effective_outer_bank() != 3 {
                    (false, raw_bank | 0x80)
                } else if raw_bank & 0x80 != 0 {
                    (true, raw_bank & 0x01)
                } else {
                    (false, raw_bank & 0x7F)
                }
            }
            192 => match raw_bank {
                8..=11 => (true, raw_bank - 8),
                _ => (false, raw_bank),
            },
            194 => match raw_bank {
                0 | 1 => (true, raw_bank),
                _ => (false, raw_bank),
            },
            195 => self.resolve_mapper195_chr_bank(raw_bank),
            _ => (false, raw_bank),
        }
    }

    fn read_chr_mixed_mmc3(&self, addr: u16) -> u8 {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let (raw_bank, local_offset) = self.resolve_chr_bank_raw_mmc3(addr, mmc3);
            let (is_ram, bank) = self.resolve_mixed_chr_bank(raw_bank);
            if is_ram {
                return Self::read_chr_mixed_bank(&self.chr_ram, bank, local_offset);
            }

            Self::read_chr_mixed_bank(&self.chr_rom, bank, local_offset)
        } else {
            0
        }
    }

    fn write_chr_mixed_mmc3(&mut self, addr: u16, data: u8) {
        if let Some(ref mmc3) = self.mappers.mmc3 {
            let (raw_bank, local_offset) = self.resolve_chr_bank_raw_mmc3(addr, mmc3);
            let (is_ram, bank) = self.resolve_mixed_chr_bank(raw_bank);
            if is_ram {
                Self::write_chr_mixed_bank(&mut self.chr_ram, bank, local_offset, data);
            } else if self.mapper == 195 {
                if let Some(mode) = Self::mapper195_mode_for_bank(raw_bank) {
                    self.mappers.mmc3_variant.mapper195_mode = mode;
                }
            }
        }
    }
}
