use super::*;

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            pulse1: PulseChannel::new(true),
            pulse2: PulseChannel::new(false),
            triangle: TriangleChannel::new(),
            noise: NoiseChannel::new(),
            dmc: DmcChannel::new(),

            frame_counter: 0,
            cycle_count: 0,

            frame_mode: false,
            irq_disable: true,
            frame_irq: false,

            pulse1_enabled: false,
            pulse2_enabled: false,
            triangle_enabled: false,
            noise_enabled: false,
            dmc_enabled: false,

            audio_ring: None,
            output_buffer: Vec::new(),
            sample_rate: 44100.0,
            cpu_clock_rate: 1789773.0,

            sample_counter: 0.0,

            sample_accumulator: 0.0,
            sample_accumulator_count: 0,

            aa_filter1: LowPassFilter::new(1789773.0, 18000.0),
            aa_filter2: LowPassFilter::new(1789773.0, 18000.0),

            high_pass_90hz: HighPassFilter::new(44100.0, 90.0),
            high_pass_440hz: HighPassFilter::new(44100.0, 440.0),
            low_pass_14khz: LowPassFilter::new(44100.0, 14000.0),

            expansion_audio: 0.0,
        }
    }
}
