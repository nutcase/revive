use super::super::super::Cartridge;

const MMC5_LENGTH_TABLE: [u8; 32] = [
    10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14, 12, 16, 24, 18, 48, 20, 96, 22,
    192, 24, 72, 26, 16, 28, 32, 30,
];

const MMC5_DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 0, 0, 0],
    [1, 0, 0, 1, 1, 1, 1, 1],
];

const MMC5_CPU_HZ: u32 = 1_789_773;
const MMC5_FRAME_STEP_HZ: u32 = 240;
const MMC5_PULSE_SCALE: f32 = -0.1494 / 15.0;
const MMC5_PCM_SCALE: f32 = -0.575 / 127.0;

#[derive(Debug, Clone, Default)]
pub(in crate::cartridge) struct Mmc5Pulse {
    pub(in crate::cartridge) duty: u8,
    pub(in crate::cartridge) length_counter: u8,
    pub(in crate::cartridge) envelope_divider: u8,
    pub(in crate::cartridge) envelope_decay: u8,
    pub(in crate::cartridge) envelope_disable: bool,
    pub(in crate::cartridge) envelope_start: bool,
    pub(in crate::cartridge) volume: u8,
    pub(in crate::cartridge) timer: u16,
    pub(in crate::cartridge) timer_reload: u16,
    pub(in crate::cartridge) duty_counter: u8,
    pub(in crate::cartridge) length_enabled: bool,
}

impl Mmc5Pulse {
    pub(super) fn new() -> Self {
        Self {
            length_enabled: true,
            ..Self::default()
        }
    }

    pub(super) fn write_control(&mut self, data: u8) {
        self.duty = (data >> 6) & 0x03;
        self.length_enabled = (data & 0x20) == 0;
        self.envelope_disable = (data & 0x10) != 0;
        self.volume = data & 0x0F;
    }

    pub(super) fn write_timer_low(&mut self, data: u8) {
        self.timer_reload = (self.timer_reload & 0xFF00) | data as u16;
    }

    pub(super) fn write_timer_high(&mut self, data: u8, enabled: bool) {
        self.timer_reload = (self.timer_reload & 0x00FF) | (((data & 0x07) as u16) << 8);
        if enabled {
            self.length_counter = MMC5_LENGTH_TABLE[((data >> 3) & 0x1F) as usize];
        }
        self.timer = self.timer_reload;
        self.duty_counter = 0;
        self.envelope_start = true;
    }

    fn clock_timer(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_reload;
            self.duty_counter = (self.duty_counter + 1) & 0x07;
        } else {
            self.timer -= 1;
        }
    }

    fn clock_envelope(&mut self) {
        if self.envelope_start {
            self.envelope_start = false;
            self.envelope_decay = 15;
            self.envelope_divider = self.volume;
        } else if self.envelope_divider == 0 {
            self.envelope_divider = self.volume;
            if self.envelope_decay > 0 {
                self.envelope_decay -= 1;
            } else if !self.length_enabled {
                self.envelope_decay = 15;
            }
        } else {
            self.envelope_divider -= 1;
        }
    }

    fn clock_length_counter(&mut self) {
        if self.length_enabled && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.length_counter == 0
            || MMC5_DUTY_TABLE[self.duty as usize][self.duty_counter as usize] == 0
        {
            return 0;
        }
        if self.envelope_disable {
            self.volume
        } else {
            self.envelope_decay
        }
    }
}

impl Cartridge {
    pub(super) fn mmc5_clock_pcm_sample(&mut self, value: u8) {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return;
        };
        if !mmc5.pcm_read_mode {
            return;
        }
        if value == 0 {
            mmc5.pcm_irq_pending.set(true);
        } else {
            mmc5.pcm_irq_pending.set(false);
            mmc5.pcm_dac = value;
        }
    }

    pub(in crate::cartridge) fn clock_audio_mmc5(&mut self) -> f32 {
        let Some(mmc5) = self.mappers.mmc5.as_mut() else {
            return 0.0;
        };

        mmc5.audio_even_cycle = !mmc5.audio_even_cycle;
        if mmc5.audio_even_cycle {
            mmc5.pulse1.clock_timer();
            mmc5.pulse2.clock_timer();
        }

        mmc5.audio_frame_accum += MMC5_FRAME_STEP_HZ;
        if mmc5.audio_frame_accum >= MMC5_CPU_HZ {
            mmc5.audio_frame_accum -= MMC5_CPU_HZ;
            mmc5.pulse1.clock_envelope();
            mmc5.pulse2.clock_envelope();
            mmc5.pulse1.clock_length_counter();
            mmc5.pulse2.clock_length_counter();
        }

        let pulse_mix = if mmc5.pulse1_enabled {
            mmc5.pulse1.output() as f32
        } else {
            0.0
        } + if mmc5.pulse2_enabled {
            mmc5.pulse2.output() as f32
        } else {
            0.0
        };

        pulse_mix * MMC5_PULSE_SCALE + (mmc5.pcm_dac as f32 * MMC5_PCM_SCALE)
    }
}
