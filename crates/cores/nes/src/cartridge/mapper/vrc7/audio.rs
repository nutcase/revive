use std::f32::consts::PI;

use crate::cartridge::state::{Vrc7AudioState, Vrc7ChannelState, Vrc7OperatorState};

use super::super::super::Cartridge;
use super::Vrc7;

const CHANNEL_COUNT: usize = 6;
const REGISTER_COUNT: usize = 0x40;
const CPU_CLOCK_HZ: f32 = 1_789_773.0;
const VRC7_UPDATE_HZ: f32 = 49_716.0;
const OUTPUT_SCALE: f32 = 0.45;

const MULTIPLIER: [f32; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
];

const FEEDBACK_INDEX: [f32; 8] = [
    0.0,
    PI / 16.0,
    PI / 8.0,
    PI / 4.0,
    PI / 2.0,
    PI,
    2.0 * PI,
    4.0 * PI,
];

const VRC7_PATCHES: [[u8; 8]; 16] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x03, 0x21, 0x05, 0x06, 0xE8, 0x81, 0x42, 0x27],
    [0x13, 0x41, 0x14, 0x0D, 0xD8, 0xF6, 0x23, 0x12],
    [0x11, 0x11, 0x08, 0x08, 0xFA, 0xB2, 0x20, 0x12],
    [0x31, 0x61, 0x0C, 0x07, 0xA8, 0x64, 0x61, 0x27],
    [0x32, 0x21, 0x1E, 0x06, 0xE1, 0x76, 0x01, 0x28],
    [0x02, 0x01, 0x06, 0x00, 0xA3, 0xE2, 0xF4, 0xF4],
    [0x21, 0x61, 0x1D, 0x07, 0x82, 0x81, 0x11, 0x07],
    [0x23, 0x21, 0x22, 0x17, 0xA2, 0x72, 0x01, 0x17],
    [0x35, 0x11, 0x25, 0x00, 0x40, 0x73, 0x72, 0x01],
    [0xB5, 0x01, 0x0F, 0x0F, 0xA8, 0xA5, 0x51, 0x02],
    [0x17, 0xC1, 0x24, 0x07, 0xF8, 0xF8, 0x22, 0x12],
    [0x71, 0x23, 0x11, 0x06, 0x65, 0x74, 0x18, 0x16],
    [0x01, 0x02, 0xD3, 0x05, 0xC9, 0x95, 0x03, 0x02],
    [0x61, 0x63, 0x0C, 0x00, 0x94, 0xC0, 0x33, 0xF6],
    [0x21, 0x72, 0x0D, 0x00, 0xC1, 0xD5, 0x56, 0x06],
];

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::cartridge) struct Vrc7Operator {
    pub(in crate::cartridge) phase: f32,
    pub(in crate::cartridge) envelope: f32,
    pub(in crate::cartridge) state: u8,
    pub(in crate::cartridge) last_output: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::cartridge) struct Vrc7Channel {
    pub(in crate::cartridge) modulator: Vrc7Operator,
    pub(in crate::cartridge) carrier: Vrc7Operator,
    pub(in crate::cartridge) key_on: bool,
}

#[derive(Debug, Clone)]
pub(in crate::cartridge) struct Vrc7Audio {
    pub(in crate::cartridge) register_select: u8,
    pub(in crate::cartridge) registers: [u8; REGISTER_COUNT],
    pub(in crate::cartridge) channels: [Vrc7Channel; CHANNEL_COUNT],
    pub(in crate::cartridge) update_accumulator: f32,
    pub(in crate::cartridge) last_output: f32,
}

#[derive(Clone, Copy)]
struct Patch {
    bytes: [u8; 8],
}

impl Vrc7Audio {
    pub(in crate::cartridge) fn new() -> Self {
        Self {
            register_select: 0,
            registers: [0; REGISTER_COUNT],
            channels: [Vrc7Channel::default(); CHANNEL_COUNT],
            update_accumulator: 0.0,
            last_output: 0.0,
        }
    }

    pub(in crate::cartridge) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(in crate::cartridge) fn write_select(&mut self, data: u8) {
        self.register_select = data & 0x3F;
    }

    pub(in crate::cartridge) fn write_data(&mut self, data: u8) {
        let reg = self.register_select as usize;
        if reg >= REGISTER_COUNT {
            return;
        }

        if !(0x00..=0x07).contains(&reg)
            && reg != 0x0F
            && !(0x10..=0x15).contains(&reg)
            && !(0x20..=0x25).contains(&reg)
            && !(0x30..=0x35).contains(&reg)
        {
            return;
        }

        let old_key = (0..CHANNEL_COUNT).contains(&(reg.saturating_sub(0x20)))
            && (self.registers[reg] & 0x10 != 0);
        self.registers[reg] = data;

        if (0x20..=0x25).contains(&reg) {
            let channel = reg - 0x20;
            let new_key = data & 0x10 != 0;
            if !old_key && new_key {
                self.key_on(channel);
            } else if old_key && !new_key {
                self.key_off(channel);
            }
        }
    }

    pub(in crate::cartridge) fn clock(&mut self) -> f32 {
        self.update_accumulator += VRC7_UPDATE_HZ / CPU_CLOCK_HZ;
        while self.update_accumulator >= 1.0 {
            self.update_accumulator -= 1.0;
            self.last_output = self.compute_sample();
        }
        self.last_output
    }

    pub(in crate::cartridge) fn snapshot_state(&self) -> Vrc7AudioState {
        Vrc7AudioState {
            register_select: self.register_select,
            registers: self.registers.to_vec(),
            update_accumulator: self.update_accumulator,
            last_output: self.last_output,
            channels: self.channels.map(|channel| Vrc7ChannelState {
                modulator: operator_state(channel.modulator),
                carrier: operator_state(channel.carrier),
                key_on: channel.key_on,
            }),
        }
    }

    pub(in crate::cartridge) fn restore_state(&mut self, state: &Vrc7AudioState) {
        self.register_select = state.register_select;
        self.registers = [0; REGISTER_COUNT];
        let len = state.registers.len().min(REGISTER_COUNT);
        self.registers[..len].copy_from_slice(&state.registers[..len]);
        self.update_accumulator = state.update_accumulator;
        self.last_output = state.last_output;
        self.channels = state.channels.map(|channel| Vrc7Channel {
            modulator: operator_from_state(channel.modulator),
            carrier: operator_from_state(channel.carrier),
            key_on: channel.key_on,
        });
    }

    fn key_on(&mut self, channel: usize) {
        let channel = &mut self.channels[channel];
        channel.key_on = true;
        channel.modulator.phase = 0.0;
        channel.modulator.envelope = 0.0;
        channel.modulator.state = EnvelopeState::Attack as u8;
        channel.modulator.last_output = 0.0;
        channel.carrier.phase = 0.0;
        channel.carrier.envelope = 0.0;
        channel.carrier.state = EnvelopeState::Attack as u8;
        channel.carrier.last_output = 0.0;
    }

    fn key_off(&mut self, channel: usize) {
        let channel = &mut self.channels[channel];
        channel.key_on = false;
        channel.modulator.state = EnvelopeState::Release as u8;
        channel.carrier.state = EnvelopeState::Release as u8;
    }

    fn compute_sample(&mut self) -> f32 {
        let mut total = 0.0;
        for channel in 0..CHANNEL_COUNT {
            total += self.compute_channel(channel);
        }
        (total / CHANNEL_COUNT as f32 * OUTPUT_SCALE).clamp(-0.35, 0.35)
    }

    fn compute_channel(&mut self, channel: usize) -> f32 {
        let reg10 = self.registers[0x10 + channel];
        let reg20 = self.registers[0x20 + channel];
        let reg30 = self.registers[0x30 + channel];
        let freq = u16::from(reg10) | (u16::from(reg20 & 0x01) << 8);
        if freq == 0 {
            return 0.0;
        }

        let octave = (reg20 >> 1) & 0x07;
        let instrument = (reg30 >> 4) as usize;
        let volume = reg30 & 0x0F;
        let patch = self.patch(instrument);
        let base_increment = (freq as f32) / 2.0f32.powi(19 - i32::from(octave));

        let mod_params = OperatorParams::modulator(patch);
        let car_params = OperatorParams::carrier(patch, volume, reg20 & 0x20 != 0);
        let feedback = FEEDBACK_INDEX[(patch.bytes[3] & 0x07) as usize];
        let mod_index = if feedback == 0.0 {
            0.0
        } else {
            feedback * self.channels[channel].modulator.last_output
        };

        let channel_state = &mut self.channels[channel];
        let mod_sample = channel_state
            .modulator
            .clock(base_increment, mod_params, mod_index);
        let modulation = mod_sample * mod_params.modulation_depth;
        channel_state
            .carrier
            .clock(base_increment, car_params, modulation)
    }

    fn patch(&self, instrument: usize) -> Patch {
        if instrument == 0 {
            let mut bytes = [0; 8];
            bytes.copy_from_slice(&self.registers[..8]);
            Patch { bytes }
        } else {
            Patch {
                bytes: VRC7_PATCHES[instrument],
            }
        }
    }
}

impl Vrc7 {
    pub(in crate::cartridge) fn clock_audio(&mut self) -> f32 {
        if self.audio_silenced {
            0.0
        } else {
            self.audio.clock()
        }
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn clock_audio_vrc7(&mut self) -> f32 {
        if let Some(vrc7) = self.mappers.vrc7.as_mut() {
            vrc7.clock_audio()
        } else {
            0.0
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

#[derive(Clone, Copy)]
struct OperatorParams {
    multiplier: f32,
    waveform: bool,
    attack: u8,
    decay: u8,
    sustain_level: f32,
    release: u8,
    attenuation: f32,
    modulation_depth: f32,
}

impl OperatorParams {
    fn modulator(patch: Patch) -> Self {
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

    fn carrier(patch: Patch, volume: u8, sustain_override: bool) -> Self {
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
    fn clock(&mut self, base_increment: f32, params: OperatorParams, phase_modulation: f32) -> f32 {
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

fn operator_state(operator: Vrc7Operator) -> Vrc7OperatorState {
    Vrc7OperatorState {
        phase: operator.phase,
        envelope: operator.envelope,
        state: operator.state,
        last_output: operator.last_output,
    }
}

fn operator_from_state(state: Vrc7OperatorState) -> Vrc7Operator {
    Vrc7Operator {
        phase: state.phase,
        envelope: state.envelope,
        state: state.state,
        last_output: state.last_output,
    }
}
