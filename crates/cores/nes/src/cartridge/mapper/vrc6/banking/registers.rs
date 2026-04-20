use super::super::super::super::{Cartridge, Mirroring};

impl Cartridge {
    pub(in crate::cartridge) fn vrc6_apply_banking_control(&mut self, data: u8) {
        self.mirroring = match data & 0x0C {
            0x00 => Mirroring::Vertical,
            0x04 => Mirroring::Horizontal,
            0x08 => Mirroring::OneScreenLower,
            _ => Mirroring::OneScreenUpper,
        };
    }

    pub(in crate::cartridge) fn write_prg_vrc6(&mut self, addr: u16, data: u8) {
        let normalized = self.vrc6_normalize_addr(addr);
        let Some(vrc6) = self.mappers.vrc6.as_mut() else {
            return;
        };
        let mut prg_bank = None;
        let mut chr_bank = None;
        let mut apply_banking_control = None;

        match normalized & 0xF003 {
            0x8000..=0x8003 => {
                vrc6.prg_bank_16k = data & 0x0F;
                prg_bank = Some(vrc6.prg_bank_16k);
            }
            0x9000 => vrc6.pulse1.write_control(data),
            0x9001 => vrc6.pulse1.write_period_low(data),
            0x9002 => vrc6.pulse1.write_period_high(data),
            0x9003 => {
                vrc6.audio_halt = data & 0x01 != 0;
                vrc6.audio_freq_shift = if data & 0x04 != 0 {
                    8
                } else if data & 0x02 != 0 {
                    4
                } else {
                    0
                };
            }
            0xA000 => vrc6.pulse2.write_control(data),
            0xA001 => vrc6.pulse2.write_period_low(data),
            0xA002 => vrc6.pulse2.write_period_high(data),
            0xB000 => vrc6.saw.write_rate(data),
            0xB001 => vrc6.saw.write_period_low(data),
            0xB002 => vrc6.saw.write_period_high(data),
            0xB003 => {
                vrc6.banking_control = data;
                apply_banking_control = Some(data);
            }
            0xC000..=0xC003 => vrc6.prg_bank_8k = data & 0x1F,
            0xD000..=0xD003 => {
                let index = (normalized & 0x0003) as usize;
                vrc6.chr_banks[index] = data;
                if index == 0 {
                    chr_bank = Some(data);
                }
            }
            0xE000..=0xE003 => {
                let index = 4 + (normalized & 0x0003) as usize;
                vrc6.chr_banks[index] = data;
            }
            0xF000 => vrc6.irq_latch = data,
            0xF001 => {
                vrc6.irq_enable_after_ack = data & 0x01 != 0;
                vrc6.irq_enabled = data & 0x02 != 0;
                vrc6.irq_cycle_mode = data & 0x04 != 0;
                vrc6.irq_pending.set(false);
                vrc6.irq_prescaler = 341;
                if vrc6.irq_enabled {
                    vrc6.irq_counter = vrc6.irq_latch;
                }
            }
            0xF002 => {
                vrc6.irq_pending.set(false);
                vrc6.irq_enabled = vrc6.irq_enable_after_ack;
            }
            _ => {}
        }

        if let Some(bank) = prg_bank {
            self.prg_bank = bank;
        }
        if let Some(bank) = chr_bank {
            self.chr_bank = bank;
        }
        if let Some(control) = apply_banking_control {
            self.vrc6_apply_banking_control(control);
        }
    }
}
