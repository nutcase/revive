use super::{HighPassFilterState, LowPassFilterState};

// High-quality audio filters
pub(super) struct HighPassFilter {
    prev_input: f32,
    prev_output: f32,
    alpha: f32,
}

pub(super) struct LowPassFilter {
    prev_output: f32,
    alpha: f32,
}

impl HighPassFilter {
    pub(super) fn new(sample_rate: f32, cutoff: f32) -> Self {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        let dt = 1.0 / sample_rate;
        let alpha = rc / (rc + dt);

        HighPassFilter {
            prev_input: 0.0,
            prev_output: 0.0,
            alpha,
        }
    }

    pub(super) fn process(&mut self, input: f32) -> f32 {
        let output = self.alpha * (self.prev_output + input - self.prev_input);
        self.prev_input = input;
        self.prev_output = output;
        output
    }

    pub(super) fn snapshot_state(&self) -> HighPassFilterState {
        HighPassFilterState {
            prev_input: self.prev_input,
            prev_output: self.prev_output,
        }
    }

    pub(super) fn restore_state(&mut self, state: &HighPassFilterState) {
        self.prev_input = state.prev_input;
        self.prev_output = state.prev_output;
    }
}

impl LowPassFilter {
    pub(super) fn new(sample_rate: f32, cutoff: f32) -> Self {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
        let dt = 1.0 / sample_rate;
        let alpha = dt / (rc + dt);

        LowPassFilter {
            prev_output: 0.0,
            alpha,
        }
    }

    pub(super) fn process(&mut self, input: f32) -> f32 {
        let output = self.prev_output + self.alpha * (input - self.prev_output);
        self.prev_output = output;
        output
    }

    pub(super) fn snapshot_state(&self) -> LowPassFilterState {
        LowPassFilterState {
            prev_output: self.prev_output,
        }
    }

    pub(super) fn restore_state(&mut self, state: &LowPassFilterState) {
        self.prev_output = state.prev_output;
    }
}
