use std::f32::consts::PI;

use super::patches::Patch;

const MULTIPLIER: [f32; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
];

pub(super) const FEEDBACK_INDEX: [f32; 8] = [
    0.0,
    PI / 16.0,
    PI / 8.0,
    PI / 4.0,
    PI / 2.0,
    PI,
    2.0 * PI,
    4.0 * PI,
];

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::cartridge) struct Vrc7Operator {
    pub(super) phase: f32,
    pub(super) envelope: f32,
    pub(super) state: u8,
    pub(super) last_output: f32,
}

#[derive(Clone, Copy)]
pub(super) struct OperatorParams {
    multiplier: f32,
    waveform: bool,
    attack: u8,
    decay: u8,
    sustain_level: f32,
    release: u8,
    attenuation: f32,
    pub(super) modulation_depth: f32,
}

impl OperatorParams {
    pub(super) fn modulator(patch: Patch) -> Self {
        let output_level = patch.bytes[2] & 0x3F;
        Self {
            multiplier: MULTIPLIER[(patch.bytes[0] & 0x0F) as usize],
            waveform: patch.bytes[3] & 0x08 != 0,
            attack: patch.bytes[4] >> 4,
            decay: patch.bytes[4] & 0x0F,
            sustain_level: sustain_level(patch.bytes[6] >> 4),
            release: patch.bytes[6] & 0x0F,
            attenuation: db_to_gain(output_level as f32 * 0.75),
            modulation_depth: FEEDBACK_INDEX[(patch.bytes[3] & 0x07) as usize].max(PI / 16.0),
        }
    }

    pub(super) fn carrier(patch: Patch, volume: u8, sustain_override: bool) -> Self {
        Self {
            multiplier: MULTIPLIER[(patch.bytes[1] & 0x0F) as usize],
            waveform: patch.bytes[3] & 0x10 != 0,
            attack: patch.bytes[5] >> 4,
            decay: patch.bytes[5] & 0x0F,
            sustain_level: sustain_level(patch.bytes[7] >> 4),
            release: if sustain_override {
                5
            } else {
                patch.bytes[7] & 0x0F
            },
            attenuation: db_to_gain(volume as f32 * 3.0),
            modulation_depth: 0.0,
        }
    }
}

impl Vrc7Operator {
    pub(super) fn restart(&mut self) {
        self.phase = 0.0;
        self.envelope = 0.0;
        self.state = EnvelopeState::Attack as u8;
        self.last_output = 0.0;
    }

    pub(super) fn release(&mut self) {
        self.state = EnvelopeState::Release as u8;
    }

    pub(super) fn clock(
        &mut self,
        base_increment: f32,
        params: OperatorParams,
        phase_modulation: f32,
    ) -> f32 {
        self.clock_envelope(params);
        if self.state == EnvelopeState::Off as u8 {
            self.last_output = 0.0;
            return 0.0;
        }

        self.phase = (self.phase + base_increment * params.multiplier).fract();
        let angle = self.phase * 2.0 * PI + phase_modulation;
        let sample = waveform_sample(angle, params.waveform) * self.envelope * params.attenuation;
        self.last_output = sample;
        sample
    }

    fn clock_envelope(&mut self, params: OperatorParams) {
        match EnvelopeState::from_u8(self.state) {
            EnvelopeState::Off => {
                self.envelope = 0.0;
            }
            EnvelopeState::Attack => {
                let step = attack_step(params.attack);
                if step == 0.0 {
                    return;
                }
                self.envelope = (self.envelope + step).min(1.0);
                if self.envelope >= 1.0 {
                    self.state = EnvelopeState::Decay as u8;
                }
            }
            EnvelopeState::Decay => {
                let step = decay_step(params.decay);
                if step == 0.0 {
                    return;
                }
                self.envelope = (self.envelope - step).max(params.sustain_level);
                if self.envelope <= params.sustain_level {
                    self.state = EnvelopeState::Sustain as u8;
                }
            }
            EnvelopeState::Sustain => {}
            EnvelopeState::Release => {
                let step = release_step(params.release);
                if step == 0.0 {
                    return;
                }
                self.envelope = (self.envelope - step).max(0.0);
                if self.envelope <= 0.0 {
                    self.state = EnvelopeState::Off as u8;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeState {
    Off = 0,
    Attack = 1,
    Decay = 2,
    Sustain = 3,
    Release = 4,
}

impl EnvelopeState {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Attack,
            2 => Self::Decay,
            3 => Self::Sustain,
            4 => Self::Release,
            _ => Self::Off,
        }
    }
}

fn waveform_sample(angle: f32, half_sine: bool) -> f32 {
    let sample = angle.sin();
    if half_sine && sample < 0.0 {
        0.0
    } else {
        sample
    }
}

fn attack_step(rate: u8) -> f32 {
    envelope_step(rate, 0.000005)
}

fn decay_step(rate: u8) -> f32 {
    envelope_step(rate, 0.0000008)
}

fn release_step(rate: u8) -> f32 {
    envelope_step(rate, 0.0000009)
}

fn envelope_step(rate: u8, scale: f32) -> f32 {
    if rate == 0 {
        0.0
    } else {
        scale * 2.0f32.powi(i32::from(rate))
    }
}

fn sustain_level(value: u8) -> f32 {
    db_to_gain(value as f32 * 3.0)
}

fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(-db / 20.0)
}
