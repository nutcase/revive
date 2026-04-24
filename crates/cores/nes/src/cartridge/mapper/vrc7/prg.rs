use super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn read_prg_vrc7(&self, addr: u16) -> u8 {
        let Some(vrc7) = self.mappers.vrc7.as_ref() else {
            return 0;
        };
        if self.prg_rom.is_empty() {
            return 0;
        }

        let bank_count = (self.prg_rom.len() / 0x2000).max(1);
        let bank = match addr {
            0x8000..=0x9FFF => vrc7.prg_banks[0] as usize,
            0xA000..=0xBFFF => vrc7.prg_banks[1] as usize,
            0xC000..=0xDFFF => vrc7.prg_banks[2] as usize,
            0xE000..=0xFFFF => bank_count - 1,
            _ => return 0,
        } % bank_count;
        let rom_addr = bank * 0x2000 + (addr as usize & 0x1FFF);
        self.prg_rom[rom_addr % self.prg_rom.len()]
    }

    pub(in crate::cartridge) fn write_prg_vrc7(&mut self, addr: u16, data: u8) {
        let select = Self::vrc7_register_select(addr);
        let mut mirroring = None;
        let Some(vrc7) = self.mappers.vrc7.as_mut() else {
            return;
        };

        match (addr & 0xF000, select) {
            (0x8000, 0) => {
                vrc7.prg_banks[0] = data & 0x3F;
                self.prg_bank = vrc7.prg_banks[0];
            }
            (0x8000, 1) => vrc7.prg_banks[1] = data & 0x3F,
            (0x9000, 0) => vrc7.prg_banks[2] = data & 0x3F,
            (0x9000, _) => {
                if !vrc7.audio_silenced {
                    if addr & 0x0020 == 0 {
                        vrc7.audio.write_select(data);
                    } else {
                        vrc7.audio.write_data(data);
                    }
                }
            }
            (0xA000..=0xD000, 0 | 1) => {
                let base = match addr & 0xF000 {
                    0xA000 => 0,
                    0xB000 => 2,
                    0xC000 => 4,
                    0xD000 => 6,
                    _ => unreachable!(),
                };
                let index = base + usize::from(select);
                vrc7.chr_banks[index] = data;
                if index == 0 {
                    self.chr_bank = data;
                }
            }
            (0xE000, 0) => {
                vrc7.control = data;
                vrc7.wram_enabled = data & 0x80 != 0;
                vrc7.audio_silenced = data & 0x40 != 0;
                if vrc7.audio_silenced {
                    vrc7.audio.reset();
                }
                mirroring = Some(match data & 0x03 {
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Horizontal,
                    2 => Mirroring::OneScreenLower,
                    _ => Mirroring::OneScreenUpper,
                });
            }
            (0xE000, 1) => vrc7.irq_latch = data,
            (0xF000, 0) => {
                vrc7.irq_enable_after_ack = data & 0x01 != 0;
                vrc7.irq_enabled = data & 0x02 != 0;
                vrc7.irq_cycle_mode = data & 0x04 != 0;
                vrc7.irq_pending.set(false);
                vrc7.irq_prescaler = 341;
                if vrc7.irq_enabled {
                    vrc7.irq_counter = vrc7.irq_latch;
                }
            }
            (0xF000, 1) => {
                vrc7.irq_pending.set(false);
                vrc7.irq_enabled = vrc7.irq_enable_after_ack;
            }
            _ => {}
        }

        if let Some(mirroring) = mirroring {
            self.mirroring = mirroring;
        }
    }

    fn vrc7_register_select(addr: u16) -> u8 {
        if addr & 0x0018 != 0 {
            1
        } else {
            0
        }
    }
}
