use std::sync::Arc;

mod channels;
mod diagnostics;
mod filter;
mod lifecycle;
mod mixer;
mod output;
mod registers;
mod state;
mod tables;
mod timing;

#[cfg(test)]
mod tests;

pub use diagnostics::AudioDiagFull;
pub use state::*;

use channels::{DmcChannel, NoiseChannel, PulseChannel, TriangleChannel};
use filter::{HighPassFilter, LowPassFilter};
use tables::{DMC_RATE_TABLE, LENGTH_TABLE, NOISE_PERIOD_TABLE};

pub struct Apu {
    pulse1: PulseChannel,
    pulse2: PulseChannel,
    triangle: TriangleChannel,
    noise: NoiseChannel,
    dmc: DmcChannel,

    frame_counter: u16,
    cycle_count: u64,

    // Frame counter control
    frame_mode: bool,  // false = 4-step, true = 5-step
    irq_disable: bool, // IRQ inhibit flag
    frame_irq: bool,   // Frame IRQ flag

    // Status register
    pulse1_enabled: bool,
    pulse2_enabled: bool,
    triangle_enabled: bool,
    noise_enabled: bool,
    dmc_enabled: bool,

    // Audio output — samples are pushed directly to ring buffer when available,
    // or fall back to the Vec buffer.
    audio_ring: Option<Arc<crate::audio_ring::SpscRingBuffer>>,
    output_buffer: Vec<f32>,
    sample_rate: f32,
    cpu_clock_rate: f32,

    // Fractional sample accumulator
    sample_counter: f32,

    // Oversampling anti-aliasing: accumulate raw mixer output every CPU cycle,
    // then average when producing an output sample (~40x oversampling).
    sample_accumulator: f32,
    sample_accumulator_count: u32,

    // Anti-aliasing pre-filters running at CPU rate (~1.79 MHz).
    // Two cascaded first-order IIR low-pass at 18 kHz.
    aa_filter1: LowPassFilter,
    aa_filter2: LowPassFilter,

    // NES hardware audio filters (nesdev wiki)
    high_pass_90hz: HighPassFilter,  // AC coupling capacitor (~90 Hz)
    high_pass_440hz: HighPassFilter, // Amplifier feedback (~440 Hz)
    low_pass_14khz: LowPassFilter,   // Amplifier bandwidth (~14 kHz)

    // Expansion audio (e.g. Sunsoft 5B) — set by bus each CPU cycle
    expansion_audio: f32,
}
