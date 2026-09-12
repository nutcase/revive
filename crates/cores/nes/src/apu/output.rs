use std::sync::Arc;

use super::*;

impl Apu {
    /// Controls host sample delivery without stopping audio hardware clocks.
    pub fn set_audio_output_enabled(&mut self, enabled: bool) {
        self.output_enabled = enabled;
        if !enabled {
            self.output_buffer.clear();
        }
    }

    /// Attach a ring buffer for direct sample delivery (bypasses output_buffer).
    pub fn set_audio_ring(&mut self, ring: Arc<crate::SpscRingBuffer>) {
        self.audio_ring = Some(ring);
    }

    pub fn get_audio_buffer_into(&mut self, out: &mut Vec<f32>) {
        out.clear();
        out.extend(self.output_buffer.drain(..));
    }

    pub fn get_audio_buffer(&mut self) -> Vec<f32> {
        self.output_buffer.drain(..).collect()
    }

    /// Push accumulated samples directly into the ring buffer, avoiding
    /// an intermediate Vec allocation.
    pub fn drain_to_ring(&mut self, ring: &crate::SpscRingBuffer) {
        if !self.output_buffer.is_empty() {
            ring.push_slice(&self.output_buffer);
            self.output_buffer.clear();
        }
    }
}

#[cfg(test)]
mod buffer_tests {
    use super::*;

    #[test]
    fn disabled_output_preserves_filters_channels_and_frame_irq() {
        let mut audible = Apu::new();
        let mut muted = Apu::new();
        for apu in [&mut audible, &mut muted] {
            for (addr, value) in [
                (0x4015, 0x0F),
                (0x4000, 0xBF),
                (0x4002, 0x80),
                (0x4003, 0x08),
            ] {
                apu.write_register(addr, value);
            }
        }
        muted.set_audio_output_enabled(false);
        for _ in 0..60_000 {
            audible.step();
            muted.step();
        }
        assert!(muted.get_audio_buffer().is_empty());
        assert!(audible.get_audio_buffer().iter().any(|&s| s != 0.0));
        assert_eq!(
            bincode::serialize(&audible.snapshot_state()).unwrap(),
            bincode::serialize(&muted.snapshot_state()).unwrap()
        );
        muted.set_audio_output_enabled(true);
        for _ in 0..10_000 {
            audible.step();
            muted.step();
        }
        assert_eq!(audible.get_audio_buffer(), muted.get_audio_buffer());
    }

    #[test]
    fn float_audio_drain_reuses_output_and_clears_empty_frames() {
        let mut apu = Apu::new();
        apu.output_buffer.extend([0.25, -0.5, 1.0]);
        let mut out = Vec::with_capacity(16);
        let allocation = out.as_ptr();
        apu.get_audio_buffer_into(&mut out);
        assert_eq!(out, [0.25, -0.5, 1.0]);
        apu.get_audio_buffer_into(&mut out);
        assert!(out.is_empty());
        assert_eq!(out.as_ptr(), allocation);
    }
}
