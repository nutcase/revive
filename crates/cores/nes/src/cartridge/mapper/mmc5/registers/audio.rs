use super::super::super::super::Cartridge;

impl Cartridge {
    pub(super) fn write_mmc5_audio_register(&mut self, addr: u16, data: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };

        match addr {
            0x5000 => mmc5.pulse1.write_control(data),
            0x5001 => {}
            0x5002 => mmc5.pulse1.write_timer_low(data),
            0x5003 => mmc5.pulse1.write_timer_high(data, mmc5.pulse1_enabled),
            0x5004 => mmc5.pulse2.write_control(data),
            0x5005 => {}
            0x5006 => mmc5.pulse2.write_timer_low(data),
            0x5007 => mmc5.pulse2.write_timer_high(data, mmc5.pulse2_enabled),
            0x5010 => {
                mmc5.pcm_irq_enabled = data & 0x80 != 0;
                mmc5.pcm_read_mode = data & 0x01 != 0;
            }
            0x5011 => {
                if !mmc5.pcm_read_mode {
                    if data == 0 {
                        mmc5.pcm_irq_pending.set(true);
                    } else {
                        mmc5.pcm_irq_pending.set(false);
                        mmc5.pcm_dac = data;
                    }
                }
            }
            0x5015 => {
                mmc5.pulse1_enabled = data & 0x01 != 0;
                mmc5.pulse2_enabled = data & 0x02 != 0;
                if !mmc5.pulse1_enabled {
                    mmc5.pulse1.length_counter = 0;
                }
                if !mmc5.pulse2_enabled {
                    mmc5.pulse2.length_counter = 0;
                }
            }
            _ => {}
        }
    }

    pub(super) fn read_mmc5_audio_register(&self, addr: u16) -> u8 {
        let Some(mmc5) = self.mappers.mmc5.as_ref() else {
            return 0;
        };

        match addr {
            0x5010 => {
                let value = ((mmc5.pcm_irq_enabled && mmc5.pcm_irq_pending.get()) as u8) << 7
                    | (mmc5.pcm_read_mode as u8);
                mmc5.pcm_irq_pending.set(false);
                value
            }
            0x5015 => {
                ((mmc5.pulse1.length_counter > 0) as u8)
                    | (((mmc5.pulse2.length_counter > 0) as u8) << 1)
            }
            _ => 0,
        }
    }
}
