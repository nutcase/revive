use super::super::super::Cartridge;
use super::Vrc6;

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc6Pulse {
    pub(in crate::cartridge) volume: u8,
    pub(in crate::cartridge) duty: u8,
    pub(in crate::cartridge) ignore_duty: bool,
    pub(in crate::cartridge) period: u16,
    pub(in crate::cartridge) enabled: bool,
    pub(in crate::cartridge) step: u8,
    pub(in crate::cartridge) divider: u16,
}

impl Vrc6Pulse {
    pub(super) fn new() -> Self {
        Self {
            volume: 0,
            duty: 0,
            ignore_duty: false,
            period: 0,
            enabled: false,
            step: 15,
            divider: 0,
        }
    }

    pub(super) fn write_control(&mut self, data: u8) {
        self.volume = data & 0x0F;
        self.duty = (data >> 4) & 0x07;
        self.ignore_duty = data & 0x80 != 0;
    }

    pub(super) fn write_period_low(&mut self, data: u8) {
        self.period = (self.period & 0x0F00) | data as u16;
    }

    pub(super) fn write_period_high(&mut self, data: u8) {
        self.period = (self.period & 0x00FF) | (((data & 0x0F) as u16) << 8);
        self.enabled = data & 0x80 != 0;
        if !self.enabled {
            self.step = 15;
        }
    }

    fn effective_period(&self, shift: u8) -> u16 {
        (self.period >> shift).max(1)
    }

    fn clock(&mut self, shift: u8, halt: bool) {
        if halt || !self.enabled {
            return;
        }

        if self.divider == 0 {
            self.divider = self.effective_period(shift);
            self.step = self.step.wrapping_sub(1) & 0x0F;
        } else {
            self.divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        if self.ignore_duty || self.step <= self.duty {
            self.volume
        } else {
            0
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc6Saw {
    pub(in crate::cartridge) rate: u8,
    pub(in crate::cartridge) period: u16,
    pub(in crate::cartridge) enabled: bool,
    pub(in crate::cartridge) step: u8,
    pub(in crate::cartridge) divider: u16,
    pub(in crate::cartridge) accumulator: u8,
}

impl Vrc6Saw {
    pub(super) fn new() -> Self {
        Self {
            rate: 0,
            period: 0,
            enabled: false,
            step: 0,
            divider: 0,
            accumulator: 0,
        }
    }

    pub(super) fn write_rate(&mut self, data: u8) {
        self.rate = data & 0x3F;
    }

    pub(super) fn write_period_low(&mut self, data: u8) {
        self.period = (self.period & 0x0F00) | data as u16;
    }

    pub(super) fn write_period_high(&mut self, data: u8) {
        self.period = (self.period & 0x00FF) | (((data & 0x0F) as u16) << 8);
        self.enabled = data & 0x80 != 0;
        if !self.enabled {
            self.step = 0;
            self.accumulator = 0;
        }
    }

    fn effective_period(&self, shift: u8) -> u16 {
        (self.period >> shift).max(1)
    }

    fn clock(&mut self, shift: u8, halt: bool) {
        if halt || !self.enabled {
            return;
        }

        if self.divider == 0 {
            self.divider = self.effective_period(shift);
            self.step = (self.step + 1) % 14;
            if self.step == 0 {
                self.accumulator = 0;
            } else if self.step & 1 == 0 {
                self.accumulator = self.accumulator.wrapping_add(self.rate);
            }
        } else {
            self.divider -= 1;
        }
    }

    fn output(&self) -> u8 {
        if self.enabled {
            self.accumulator >> 3
        } else {
            0
        }
    }
}

const VRC6_AUDIO_SCALE: f32 = 0.35 / 63.0;

impl Vrc6 {
    pub(in crate::cartridge) fn clock_audio(&mut self) -> f32 {
        self.pulse1.clock(self.audio_freq_shift, self.audio_halt);
        self.pulse2.clock(self.audio_freq_shift, self.audio_halt);
        self.saw.clock(self.audio_freq_shift, self.audio_halt);

        let mix =
            self.pulse1.output() as f32 + self.pulse2.output() as f32 + self.saw.output() as f32;
        mix * VRC6_AUDIO_SCALE
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn clock_audio_vrc6(&mut self) -> f32 {
        if let Some(vrc6) = self.mappers.vrc6.as_mut() {
            vrc6.clock_audio()
        } else {
            0.0
        }
    }
}
