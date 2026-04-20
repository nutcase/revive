use crate::cartridge::{Cartridge, Mirroring};

impl Cartridge {
    pub(super) fn mask_namco108_bank_data(mapper: u16, reg: usize, data: u8) -> u8 {
        match reg {
            0 | 1 => {
                if mapper == 95 {
                    data & 0x3F
                } else {
                    data & 0x3E
                }
            }
            2..=5 => data & 0x3F,
            6 | 7 => data & 0x0F,
            _ => 0,
        }
    }

    pub(super) fn update_mapper95_mirroring(&mut self, bank_registers: &[u8; 8]) {
        let lower = bank_registers[0] & 0x20 != 0;
        let upper = bank_registers[1] & 0x20 != 0;
        self.mirroring = match (lower, upper) {
            (false, false) => Mirroring::OneScreenLower,
            (true, true) => Mirroring::OneScreenUpper,
            (false, true) => Mirroring::Horizontal,
            (true, false) => Mirroring::HorizontalSwapped,
        };
    }
}
