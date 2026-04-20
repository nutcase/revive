use super::{Sunsoft5BAudio, AY_VOLUME};

impl Sunsoft5BAudio {
    pub(super) fn compute_output(&self) -> f32 {
        let mut total = 0.0f32;

        for ch in 0..3 {
            let tone_disable = (self.mixer >> ch) & 1 != 0;
            let noise_disable = (self.mixer >> (ch + 3)) & 1 != 0;
            let gate =
                (self.tone_output[ch] || tone_disable) && (self.noise_output || noise_disable);

            if gate {
                total += AY_VOLUME[self.channel_volume(ch) as usize];
            }
        }

        total
    }

    fn channel_volume(&self, channel: usize) -> u8 {
        let vol_reg = self.volume[channel];
        if vol_reg & 0x10 != 0 {
            self.envelope_volume
        } else {
            vol_reg & 0x0F
        }
    }
}
