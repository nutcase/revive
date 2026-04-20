use super::super::super::Cartridge;

impl Cartridge {
    /// Clock mapper expansion audio one CPU cycle and return output sample.
    pub fn clock_expansion_audio(&mut self) -> f32 {
        if self.uses_mmc5() {
            self.clock_audio_mmc5()
        } else if let Some(ref mut fme7) = self.mappers.fme7 {
            fme7.audio.clock()
        } else if self.uses_vrc6() {
            self.clock_audio_vrc6()
        } else if self.uses_namco163() {
            self.clock_audio_namco163()
        } else {
            0.0
        }
    }
}
