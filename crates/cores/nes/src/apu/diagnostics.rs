use super::*;

/// Snapshot of APU channel activity and expansion-audio output.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioDiagFull {
    /// Whether pulse channel 1 is enabled via $4015.
    pub pulse1_enabled: bool,
    /// Current pulse channel 1 length counter.
    pub pulse1_length: u8,
    /// Whether pulse channel 2 is enabled via $4015.
    pub pulse2_enabled: bool,
    /// Current pulse channel 2 length counter.
    pub pulse2_length: u8,
    /// Whether triangle channel is enabled via $4015.
    pub triangle_enabled: bool,
    /// Current triangle channel length counter.
    pub triangle_length: u8,
    /// Whether noise channel is enabled via $4015.
    pub noise_enabled: bool,
    /// Current noise channel length counter.
    pub noise_length: u8,
    /// Current effective noise envelope volume.
    pub noise_vol: u8,
    /// Current noise timer period.
    pub noise_period: u16,
    /// Whether the noise envelope is in constant-volume mode.
    pub noise_envelope_disable: bool,
    /// Most recent cartridge expansion-audio sample.
    pub expansion: f32,
}

impl Apu {
    pub fn set_expansion_audio(&mut self, value: f32) {
        self.expansion_audio = value;
    }

    pub fn audio_diag_full(&self) -> AudioDiagFull {
        AudioDiagFull {
            pulse1_enabled: self.pulse1_enabled,
            pulse1_length: self.pulse1.length_counter,
            pulse2_enabled: self.pulse2_enabled,
            pulse2_length: self.pulse2.length_counter,
            triangle_enabled: self.triangle_enabled,
            triangle_length: self.triangle.length_counter,
            noise_enabled: self.noise_enabled,
            noise_length: self.noise.length_counter,
            noise_vol: if self.noise.envelope_disable {
                self.noise.volume
            } else {
                self.noise.envelope_decay
            },
            noise_period: self.noise.timer_reload,
            noise_envelope_disable: self.noise.envelope_disable,
            expansion: self.expansion_audio,
        }
    }
}
