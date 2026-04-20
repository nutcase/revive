use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    /// MMC1 PRG write - shift register + register decode
    pub(in crate::cartridge) fn write_prg_mmc1(&mut self, addr: u16, data: u8) {
        if let Some(ref mut mmc1) = self.mappers.mmc1 {
            // Check for reset (bit 7 set)
            if data & 0x80 != 0 {
                mmc1.shift_register = 0x10;
                mmc1.shift_count = 0;
                mmc1.control |= 0x0C;
                return;
            }

            // Shift in bit 0 (LSB first)
            mmc1.shift_register >>= 1;
            if data & 0x01 != 0 {
                mmc1.shift_register |= 0x10;
            }
            mmc1.shift_count = mmc1.shift_count.saturating_add(1);

            // After 5 writes, update the target register
            if mmc1.shift_count >= 5 {
                let register_data = mmc1.shift_register & 0x1F;

                let register_select = match addr {
                    0x8000..=0x9FFF => 0, // Control
                    0xA000..=0xBFFF => 1, // CHR bank 0
                    0xC000..=0xDFFF => 2, // CHR bank 1
                    0xE000..=0xFFFF => 3, // PRG bank
                    _ => return,
                };

                match register_select {
                    0 => {
                        mmc1.control = register_data;
                        self.mirroring = match register_data & 0x03 {
                            0 => Mirroring::OneScreenLower,
                            1 => Mirroring::OneScreenUpper,
                            2 => Mirroring::Vertical,
                            3 => Mirroring::Horizontal,
                            _ => self.mirroring,
                        };
                    }
                    1 => {
                        mmc1.chr_bank_0 = register_data;
                    }
                    2 => {
                        mmc1.chr_bank_1 = register_data;
                    }
                    3 => {
                        mmc1.prg_bank = register_data & 0x0F;
                        mmc1.prg_ram_disable = (register_data & 0x10) != 0;
                    }
                    _ => {}
                }

                mmc1.shift_register = 0x10;
                mmc1.shift_count = 0;
            }
        }
    }
}
