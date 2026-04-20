use super::Sunsoft5BAudio;

impl Sunsoft5BAudio {
    pub(super) fn write_tone_period_low(&mut self, channel: usize, data: u8) {
        self.tone_period[channel] = (self.tone_period[channel] & 0xF00) | data as u16;
    }

    pub(super) fn write_tone_period_high(&mut self, channel: usize, data: u8) {
        self.tone_period[channel] =
            (self.tone_period[channel] & 0x0FF) | ((data as u16 & 0x0F) << 8);
    }

    pub(super) fn clock_tone_generators(&mut self) {
        for ch in 0..3 {
            if self.tone_counter[ch] > 0 {
                self.tone_counter[ch] -= 1;
            }
            if self.tone_counter[ch] == 0 {
                self.tone_counter[ch] = self.tone_period[ch].max(1);
                self.tone_output[ch] = !self.tone_output[ch];
            }
        }
    }
}
