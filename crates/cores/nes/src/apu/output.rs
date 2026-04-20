use std::sync::Arc;

use super::*;

impl Apu {
    /// Attach a ring buffer for direct sample delivery (bypasses output_buffer).
    pub fn set_audio_ring(&mut self, ring: Arc<crate::SpscRingBuffer>) {
        self.audio_ring = Some(ring);
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
