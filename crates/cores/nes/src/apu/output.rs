use std::sync::Arc;

use super::*;

impl Apu {
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
