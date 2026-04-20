use super::Sunsoft5BAudio;

impl Sunsoft5BAudio {
    pub(in crate::cartridge) fn write_select(&mut self, data: u8) {
        self.register_select = data & 0x0F;
    }

    pub(in crate::cartridge) fn write_data(&mut self, data: u8) {
        match self.register_select {
            0 => self.write_tone_period_low(0, data),
            1 => self.write_tone_period_high(0, data),
            2 => self.write_tone_period_low(1, data),
            3 => self.write_tone_period_high(1, data),
            4 => self.write_tone_period_low(2, data),
            5 => self.write_tone_period_high(2, data),
            6 => self.write_noise_period(data),
            7 => self.mixer = data,
            8 => self.volume[0] = data & 0x1F,
            9 => self.volume[1] = data & 0x1F,
            10 => self.volume[2] = data & 0x1F,
            11 => self.envelope_period = (self.envelope_period & 0xFF00) | data as u16,
            12 => self.envelope_period = (self.envelope_period & 0x00FF) | ((data as u16) << 8),
            13 => self.write_envelope_shape(data),
            _ => {}
        }
    }
}
