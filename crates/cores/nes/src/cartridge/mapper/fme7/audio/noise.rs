use super::Sunsoft5BAudio;

impl Sunsoft5BAudio {
    pub(super) fn write_noise_period(&mut self, data: u8) {
        self.noise_period = data & 0x1F;
    }

    pub(super) fn clock_noise_generator(&mut self) {
        if self.noise_counter > 0 {
            self.noise_counter -= 1;
        }
        if self.noise_counter == 0 {
            self.noise_counter = self.noise_period.max(1);
            let feedback = (self.noise_lfsr ^ (self.noise_lfsr >> 3)) & 1;
            self.noise_lfsr = (self.noise_lfsr >> 1) | (feedback << 16);
            self.noise_output = (self.noise_lfsr & 1) != 0;
        }
    }
}
