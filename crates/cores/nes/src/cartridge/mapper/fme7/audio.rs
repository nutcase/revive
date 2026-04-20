mod envelope;
mod mixer;
mod noise;
mod registers;
mod tone;

/// YM2149F-compatible (Sunsoft 5B) expansion audio DAC volume table.
/// Pre-scaled for NES mixing: ~0.12 max per channel, 3 channels total ~0.36.
/// Logarithmic curve (~3 dB per step) matching the hardware DAC.
const AY_VOLUME: [f32; 16] = [
    0.0, 0.00095, 0.00134, 0.00190, 0.00268, 0.00379, 0.00535, 0.00755, 0.01067, 0.01506, 0.02128,
    0.03006, 0.04247, 0.05999, 0.08474, 0.11973,
];

/// Sunsoft 5B expansion audio (YM2149F / AY-3-8910 compatible).
/// 3 square wave channels + noise generator + envelope generator.
#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Sunsoft5BAudio {
    register_select: u8,

    // Tone generators (3 channels)
    tone_period: [u16; 3], // 12-bit period
    tone_counter: [u16; 3],
    tone_output: [bool; 3], // Current square wave state

    // Noise generator
    noise_period: u8, // 5-bit period
    noise_counter: u8,
    noise_lfsr: u32, // 17-bit LFSR
    noise_output: bool,

    // Mixer (register 7): bits 0-2 = tone disable A/B/C, bits 3-5 = noise disable A/B/C
    mixer: u8,

    // Per-channel volume (bits 0-3 = volume, bit 4 = envelope mode)
    volume: [u8; 3],

    // Envelope generator
    envelope_period: u16,
    envelope_counter: u16,
    envelope_shape: u8,
    envelope_volume: u8, // 0-15
    envelope_holding: bool,
    envelope_up: bool, // true = attack (counting up)

    // CPU clock prescaler (divides by 16)
    prescaler: u8,

    // Cached output (only changes at prescaler tick)
    last_output: f32,
}

impl Sunsoft5BAudio {
    pub(in crate::cartridge) fn new() -> Self {
        Sunsoft5BAudio {
            register_select: 0,
            tone_period: [0; 3],
            tone_counter: [1; 3],
            tone_output: [false; 3],
            noise_period: 0,
            noise_counter: 1,
            noise_lfsr: 1,
            noise_output: false,
            mixer: 0xFF, // All outputs disabled by default
            volume: [0; 3],
            envelope_period: 0,
            envelope_counter: 1,
            envelope_shape: 0,
            envelope_volume: 0,
            envelope_holding: true,
            envelope_up: false,
            prescaler: 0,
            last_output: 0.0,
        }
    }

    /// Clock one CPU cycle. Returns expansion audio output.
    pub(in crate::cartridge) fn clock(&mut self) -> f32 {
        self.prescaler += 1;
        if self.prescaler >= 16 {
            self.prescaler = 0;
            self.clock_internal();
            self.last_output = self.compute_output();
        }
        self.last_output
    }

    fn clock_internal(&mut self) {
        self.clock_tone_generators();
        self.clock_noise_generator();
        self.clock_envelope();
    }
}
