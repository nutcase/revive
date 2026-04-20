use super::super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_fme7(&mut self, addr: u16, data: u8) {
        if let Some(ref mut fme7) = self.mappers.fme7 {
            match addr {
                0x8000..=0x9FFF => {
                    fme7.command = data & 0x0F;
                }
                0xA000..=0xBFFF => match fme7.command {
                    0..=7 => {
                        fme7.chr_banks[fme7.command as usize] = data;
                    }
                    8 => {
                        fme7.prg_ram_enabled = (data & 0x80) != 0;
                        fme7.prg_ram_select = (data & 0x40) != 0;
                        fme7.prg_bank_6000 = data & 0x3F;
                    }
                    9 => {
                        fme7.prg_banks[0] = data & 0x3F;
                    }
                    0xA => {
                        fme7.prg_banks[1] = data & 0x3F;
                    }
                    0xB => {
                        fme7.prg_banks[2] = data & 0x3F;
                    }
                    0xC => {
                        self.mirroring = match data & 0x03 {
                            0 => Mirroring::Vertical,
                            1 => Mirroring::Horizontal,
                            2 => Mirroring::OneScreenLower,
                            3 => Mirroring::OneScreenUpper,
                            _ => unreachable!(),
                        };
                    }
                    0xD => {
                        fme7.irq_counter_enabled = (data & 0x80) != 0;
                        fme7.irq_enabled = (data & 0x01) != 0;
                        fme7.irq_pending.set(false);
                    }
                    0xE => {
                        let high = fme7.irq_counter & 0xFF00;
                        fme7.irq_counter = high | (data as u16);
                    }
                    0xF => {
                        let low = fme7.irq_counter & 0x00FF;
                        fme7.irq_counter = ((data as u16) << 8) | low;
                    }
                    _ => {}
                },
                0xC000..=0xDFFF => {
                    fme7.audio.write_select(data);
                }
                0xE000..=0xFFFF => {
                    fme7.audio.write_data(data);
                }
                _ => {}
            }
        }
    }
}
