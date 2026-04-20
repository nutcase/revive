use crate::cartridge::{Cartridge, Mirroring};

mod read;
mod write;

impl Cartridge {
    fn mapper227_outer_bank(&self) -> usize {
        (((self.mappers.multicart.mapper227_latch >> 8) as usize & 0x01) << 2)
            | ((self.mappers.multicart.mapper227_latch >> 5) as usize & 0x03)
    }

    fn mapper227_inner_bank(&self) -> usize {
        (self.mappers.multicart.mapper227_latch as usize >> 2) & 0x07
    }
    /// Mapper 227: address-latched multicart with UNROM-like and NROM-like
    /// modes over a 1 MiB PRG ROM and fixed 8 KiB CHR-RAM.
    pub(in crate::cartridge) fn sync_mapper234_state(&mut self) {
        let prg_bank_count = (self.prg_rom.len() / 0x8000).max(1);
        let chr_bank_count = (self.chr_rom.len() / 0x2000).max(1);

        self.mirroring = if self.mappers.multicart.mapper234_reg0 & 0x80 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };

        let (prg_bank, chr_bank) = if self.mappers.multicart.mapper234_reg0 & 0x40 != 0 {
            let outer = ((self.mappers.multicart.mapper234_reg0 >> 1) as usize) & 0x07;
            let prg = (outer << 1) | ((self.mappers.multicart.mapper234_reg1 as usize) & 0x01);
            let chr =
                (outer << 3) | (((self.mappers.multicart.mapper234_reg1 >> 4) as usize) & 0x07);
            (prg, chr)
        } else {
            let outer = (self.mappers.multicart.mapper234_reg0 as usize) & 0x0F;
            let prg = outer;
            let chr =
                (outer << 2) | (((self.mappers.multicart.mapper234_reg1 >> 4) as usize) & 0x03);
            (prg, chr)
        };

        self.prg_bank = (prg_bank % prg_bank_count) as u8;
        self.chr_bank = (chr_bank % chr_bank_count) as u8;
    }

    pub(in crate::cartridge) fn apply_mapper234_value(&mut self, addr: u16, value: u8) {
        match addr {
            0xFF80..=0xFF9F => {
                if self.mappers.multicart.mapper234_reg0 & 0x3F == 0 {
                    self.mappers.multicart.mapper234_reg0 = value;
                    self.sync_mapper234_state();
                }
            }
            0xFFE8..=0xFFF7 => {
                self.mappers.multicart.mapper234_reg1 = value;
                self.sync_mapper234_state();
            }
            _ => {}
        }
    }
    /// Mapper 200: switchable 16KB PRG bank mirrored into both CPU halves.
    fn mapper228_chip_base(&self) -> Option<usize> {
        const CHIP_SIZE: usize = 0x80000;

        let chip = self.mappers.multicart.mapper228_chip_select as usize;
        let chip_count = self.prg_rom.len() / CHIP_SIZE;
        if chip_count == 3 {
            match chip {
                0 | 1 => Some(chip * CHIP_SIZE),
                2 => None,
                3 => Some(2 * CHIP_SIZE),
                _ => None,
            }
        } else if chip < chip_count {
            Some(chip * CHIP_SIZE)
        } else {
            None
        }
    }
}
