use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn vrc2_vrc4_decode_index(&self, addr: u16) -> Option<u8> {
        match self.mapper {
            21 => match addr & 0x00C6 {
                0x000 => Some(0),
                0x002 | 0x040 => Some(1),
                0x004 | 0x080 => Some(2),
                0x006 | 0x0C0 => Some(3),
                _ => None,
            },
            _ => match (self.mapper, addr & 0x000F) {
                (22, 0x0) => Some(0),
                (22, 0x2 | 0x8) => Some(1),
                (22, 0x1 | 0x4) => Some(2),
                (22, 0x3 | 0xC) => Some(3),
                (23, 0x0) | (25, 0x0) => Some(0),
                (23, 0x1 | 0x4) | (25, 0x2 | 0x8) => Some(1),
                (23, 0x2 | 0x8) | (25, 0x1 | 0x4) => Some(2),
                (23, 0x3 | 0xC) | (25, 0x3 | 0xC) => Some(3),
                _ => None,
            },
        }
    }

    pub(in crate::cartridge) fn vrc2_vrc4_uses_alt_vrc4_decode(&self, addr: u16) -> bool {
        self.mapper != 22 && matches!(addr & 0x000F, 0x4 | 0x8 | 0xC)
    }

    pub(in crate::cartridge) fn vrc2_vrc4_decode_chr_index(
        addr: u16,
        reg: u8,
    ) -> Option<(usize, bool)> {
        let base = match addr & 0xF000 {
            0xB000 => 0,
            0xC000 => 2,
            0xD000 => 4,
            0xE000 => 6,
            _ => return None,
        };
        Some((base + usize::from(reg / 2), reg & 1 != 0))
    }

    pub(in crate::cartridge) fn vrc2_vrc4_chr_data(&self) -> &[u8] {
        if !self.chr_ram.is_empty() {
            &self.chr_ram
        } else {
            &self.chr_rom
        }
    }

    pub(in crate::cartridge) fn vrc2_vrc4_effective_chr_bank(
        mapper: u16,
        raw_bank: u16,
        bank_count: usize,
    ) -> usize {
        let bank = if mapper == 22 {
            raw_bank >> 1
        } else {
            raw_bank
        };
        bank as usize % bank_count
    }

    pub(in crate::cartridge) fn vrc2_vrc4_decode_mirroring(vrc4_mode: bool, data: u8) -> Mirroring {
        if vrc4_mode {
            match data & 0x03 {
                0 => Mirroring::Vertical,
                1 => Mirroring::Horizontal,
                2 => Mirroring::OneScreenLower,
                _ => Mirroring::OneScreenUpper,
            }
        } else if data & 0x01 != 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        }
    }
}
