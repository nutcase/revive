mod operator;
mod patches;
mod state;

use operator::{OperatorParams, Vrc7Operator, FEEDBACK_INDEX};

use super::Vrc7;

pub(super) const CHANNEL_COUNT: usize = 6;
pub(super) const REGISTER_COUNT: usize = 0x40;
const CPU_CLOCK_HZ: f32 = 1_789_773.0;
const VRC7_UPDATE_HZ: f32 = 49_716.0;
const OUTPUT_SCALE: f32 = 0.45;

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

        if !is_writable_register(reg) {
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

    fn key_on(&mut self, channel: usize) {
        let channel = &mut self.channels[channel];
        channel.key_on = true;
        channel.modulator.restart();
        channel.carrier.restart();
    }

    fn key_off(&mut self, channel: usize) {
        let channel = &mut self.channels[channel];
        channel.key_on = false;
        channel.modulator.release();
        channel.carrier.release();
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
        let patch = patches::patch(instrument, &self.registers);
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

fn is_writable_register(reg: usize) -> bool {
    (0x00..=0x07).contains(&reg)
        || reg == 0x0F
        || (0x10..=0x15).contains(&reg)
        || (0x20..=0x25).contains(&reg)
        || (0x30..=0x35).contains(&reg)
}
