use super::super::super::Cartridge;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpansionAudioChip {
    Mmc5,
    Fme7,
    Vrc6,
    Vrc7,
    Namco163,
}

impl Cartridge {
    /// Clock mapper expansion audio one CPU cycle and return output sample.
    pub fn clock_expansion_audio(&mut self) -> f32 {
        match self.active_expansion_audio_chip() {
            Some(ExpansionAudioChip::Mmc5) => self.clock_audio_mmc5(),
            Some(ExpansionAudioChip::Fme7) => self.clock_audio_fme7(),
            Some(ExpansionAudioChip::Vrc6) => self.clock_audio_vrc6(),
            Some(ExpansionAudioChip::Vrc7) => self.clock_audio_vrc7(),
            Some(ExpansionAudioChip::Namco163) => self.clock_audio_namco163(),
            None => 0.0,
        }
    }

    fn active_expansion_audio_chip(&self) -> Option<ExpansionAudioChip> {
        if self.uses_mmc5() {
            Some(ExpansionAudioChip::Mmc5)
        } else if self.mappers.fme7.is_some() {
            Some(ExpansionAudioChip::Fme7)
        } else if self.uses_vrc6() {
            Some(ExpansionAudioChip::Vrc6)
        } else if self.uses_vrc7() {
            Some(ExpansionAudioChip::Vrc7)
        } else if self.uses_namco163() {
            Some(ExpansionAudioChip::Namco163)
        } else {
            None
        }
    }

    fn clock_audio_fme7(&mut self) -> f32 {
        self.mappers
            .fme7
            .as_mut()
            .map_or(0.0, |fme7| fme7.audio.clock())
    }

    fn clock_audio_vrc7(&mut self) -> f32 {
        self.mappers
            .vrc7
            .as_mut()
            .map_or(0.0, |vrc7| vrc7.clock_audio())
    }
}
