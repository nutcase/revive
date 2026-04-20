use super::Apu;

impl Apu {
    /// Non-linear mixer (nesdev wiki) without filters. Called every CPU cycle
    /// for oversampling accumulation.
    #[inline]
    pub(super) fn raw_mix(&self) -> f32 {
        let pulse1_out = if self.pulse1_enabled && self.pulse1.length_counter > 0 {
            self.pulse1.output()
        } else {
            0.0
        };
        let pulse2_out = if self.pulse2_enabled && self.pulse2.length_counter > 0 {
            self.pulse2.output()
        } else {
            0.0
        };
        let triangle_out = if self.triangle_enabled && self.triangle.length_counter > 0 {
            self.triangle.output()
        } else {
            0.0
        };
        let noise_out = if self.noise_enabled && self.noise.length_counter > 0 {
            self.noise.output()
        } else {
            0.0
        };
        let dmc_out = self.dmc.output();

        // Non-linear mixer (nesdev wiki) - models the NES resistor DAC
        // Channel outputs are 0.0-15.0. Mixer naturally outputs 0.0-~1.0.
        let pulse_sum = pulse1_out + pulse2_out;
        let pulse_out = if pulse_sum > 0.0 {
            95.88 / (8128.0 / pulse_sum + 100.0)
        } else {
            0.0
        };

        let tnd_sum = triangle_out / 8227.0 + noise_out / 12241.0 + dmc_out / 22638.0;
        let tnd_out = if tnd_sum > 0.0 {
            159.79 / (1.0 / tnd_sum + 100.0)
        } else {
            0.0
        };

        pulse_out + tnd_out
    }

    /// Average accumulated raw mix, apply hardware filters, produce final sample.
    pub(super) fn produce_sample(&mut self) -> f32 {
        let averaged = if self.sample_accumulator_count > 0 {
            self.sample_accumulator / self.sample_accumulator_count as f32
        } else {
            0.0
        };
        self.sample_accumulator = 0.0;
        self.sample_accumulator_count = 0;

        // Apply NES hardware filter chain (nesdev wiki)
        let filtered = self.high_pass_90hz.process(averaged);
        let filtered = self.high_pass_440hz.process(filtered);
        let filtered = self.low_pass_14khz.process(filtered);

        // Scale to fill audio output range (HP filters center the signal around 0)
        (filtered * 1.8).clamp(-1.0, 1.0)
    }
}
