use super::Psg;
use super::channel::PsgChannel;
use super::tables::*;

impl Psg {
    pub(crate) fn render_host_sample(&mut self, psg_cycles: u32) -> i16 {
        // A small internal oversample substantially reduces aliasing on
        // high-frequency tones/noise (notably NF=31 percussion) without
        // changing the external 44.1 kHz sample rate or save-state format.
        const OVERSAMPLE: u32 = 2;

        let mut mixed_sum: i64 = 0;
        let base_cycles = psg_cycles / OVERSAMPLE;
        let remainder = psg_cycles % OVERSAMPLE;

        for phase in 0..OVERSAMPLE {
            let cycles = base_cycles + u32::from(phase < remainder);
            self.clock(cycles);
            mixed_sum += i64::from(self.mix_current_state());
        }

        let mixed = (mixed_sum / i64::from(OVERSAMPLE)) as i32;
        self.finalize_sample(mixed)
    }

    fn mix_current_state(&self) -> i32 {
        let mut mix: i32 = 0;
        for channel_index in 0..PSG_CHANNEL_COUNT {
            let state = self.channels[channel_index];
            mix += self.sample_channel(channel_index, state);
        }
        mix
    }

    fn finalize_sample(&mut self, mix: i32) -> i16 {
        // sample_channel() returns values with 16 fractional bits.
        // Per-channel max = 31 * 65536 = 2,031,616; 6-channel max = 12,189,696.
        // Apply gain and shift: (mix * gain) >> 16.
        let scaled = ((mix as i64 * PSG_OUTPUT_GAIN as i64) >> 16) as i32;
        let input = scaled.clamp(i16::MIN as i32, i16::MAX as i32) as f64;
        // PSG samples are unsigned around the midpoint, so remove the resulting DC bias
        // before converting back to i16.  Keeping this transient preserves old save states.
        const DC_BLOCK_R: f64 = 0.995;
        let output = input - *self.dc_prev_input + DC_BLOCK_R * self.post_filter_state;
        self.dc_prev_input.0 = input;
        self.post_filter_state = output;
        output.clamp(i16::MIN as f64, i16::MAX as f64) as i16
    }

    pub(crate) fn clock(&mut self, psg_cycles: u32) {
        if psg_cycles == 0 {
            return;
        }

        let lfo_active = self.lfo_active();
        let lfo_halted = self.lfo_halted();
        let lfo_mod = self.lfo_modulation(lfo_active);
        for idx in 0..PSG_CHANNEL_COUNT {
            let ch = &mut self.channels[idx];
            if ch.control & PSG_CH_CTRL_KEY_ON == 0 {
                continue;
            }
            if ch.control & PSG_CH_CTRL_DDA != 0 {
                continue;
            }
            if idx == 1 && lfo_halted {
                continue;
            }
            if idx >= 4 && ch.noise_control & PSG_NOISE_ENABLE != 0 {
                let period = noise_period(ch.noise_control);
                ch.noise_phase = ch.noise_phase.wrapping_add(psg_cycles);
                let steps = (ch.noise_phase / period) as usize;
                ch.noise_phase %= period;
                for _ in 0..steps {
                    let lfsr = ch.noise_lfsr;
                    let feedback =
                        ((lfsr >> 0) ^ (lfsr >> 1) ^ (lfsr >> 11) ^ (lfsr >> 12) ^ (lfsr >> 17))
                            & 0x01;
                    ch.noise_lfsr = (lfsr >> 1) | (feedback << 17);
                    if ch.noise_lfsr == 0 {
                        ch.noise_lfsr = 1;
                    }
                }
                continue;
            }

            let period = if idx == 0 && lfo_active {
                let effective_period = ((ch.frequency as i32 + lfo_mod).rem_euclid(0x1000)) as u16;
                tone_divider(effective_period)
            } else if idx == 1 && lfo_active {
                tone_divider(ch.frequency) * lfo_frequency_scale(self.lfo_frequency)
            } else {
                tone_divider(ch.frequency)
            };

            ch.phase = ch.phase.wrapping_add(psg_cycles);
            let steps = (ch.phase / period) as usize;
            ch.phase %= period;
            if steps != 0 {
                let advance = (steps & (PSG_WAVE_SIZE - 1)) as u8;
                ch.wave_pos = ch.wave_pos.wrapping_add(advance) & (PSG_WAVE_SIZE as u8 - 1);
            }
        }
    }

    fn sample_channel(&self, channel: usize, state: PsgChannel) -> i32 {
        if state.control & PSG_CH_CTRL_KEY_ON == 0 {
            return 0;
        }
        let raw = if state.control & PSG_CH_CTRL_DDA != 0 {
            sample_to_signed(state.dda_sample)
        } else if channel >= 4 && state.noise_control & PSG_NOISE_ENABLE != 0 {
            if state.noise_lfsr & 0x01 != 0 {
                sample_to_signed(0x1F)
            } else {
                sample_to_signed(0x00)
            }
        } else {
            let base = channel * PSG_WAVE_SIZE;
            let offset = (state.wave_pos as usize) & (PSG_WAVE_SIZE - 1);
            let wave_index = base + offset;
            sample_to_signed(self.waveform_ram[wave_index])
        };
        if raw == 0 {
            return 0;
        }

        // Logarithmic volume mixing (Mednafen-compatible).
        // Combine channel volume, channel balance, and main balance as
        // attenuation indices (additive in dB domain).
        let db_table = psg_db_table();
        let scale_tab = psg_balance_scale_tab();

        // Channel volume: 5-bit, 31=max(0 attenuation), 0=min(31 attenuation)
        let al = 0x1F_u8.wrapping_sub(state.control & PSG_CH_CTRL_VOLUME_MASK);

        // Channel balance: 4-bit per side, scaled to 5-bit range
        let bal_l = 0x1F - scale_tab[((state.balance >> 4) & 0x0F) as usize];
        let bal_r = 0x1F - scale_tab[(state.balance & 0x0F) as usize];

        // Main balance: 4-bit per side, scaled to 5-bit range
        let gbal_l = 0x1F - scale_tab[((self.main_balance >> 4) & 0x0F) as usize];
        let gbal_r = 0x1F - scale_tab[(self.main_balance & 0x0F) as usize];

        // Sum attenuations (clamped to 31 = silence)
        let vol_l = ((al as u16 + bal_l as u16 + gbal_l as u16).min(0x1F)) as usize;
        let vol_r = ((al as u16 + bal_r as u16 + gbal_r as u16).min(0x1F)) as usize;

        // Apply logarithmic volume (fixed-point 16.16).
        // Return with 16 fractional bits intact; the final output stage shifts
        // after accumulating all channels and applying the output gain.
        let left = raw as i64 * db_table[vol_l] as i64;
        let right = raw as i64 * db_table[vol_r] as i64;
        ((left + right) / 2) as i32
    }

    fn lfo_depth(&self) -> u8 {
        self.lfo_control & 0x03
    }

    fn lfo_halted(&self) -> bool {
        self.lfo_control & 0x80 != 0
    }

    fn lfo_active(&self) -> bool {
        self.lfo_depth() != 0
            && !self.lfo_halted()
            && self.channels[1].control & PSG_CH_CTRL_KEY_ON != 0
    }

    fn lfo_modulation(&self, lfo_active: bool) -> i32 {
        if !lfo_active {
            return 0;
        }
        let depth_shift = ((self.lfo_depth() - 1) << 1) as i32;
        let raw = self.current_lfo_sample();
        raw << depth_shift
    }

    fn current_lfo_sample(&self) -> i32 {
        let state = self.channels[1];
        if state.control & PSG_CH_CTRL_DDA != 0 {
            (state.dda_sample & 0x1F) as i32 - 0x10
        } else {
            let base = PSG_WAVE_SIZE;
            let offset = (state.wave_pos as usize) & (PSG_WAVE_SIZE - 1);
            self.waveform_ram[base + offset] as i32 - 0x10
        }
    }
}

#[inline]
fn tone_divider(period: u16) -> u32 {
    let period = (period & 0x0FFF) as u32;
    if period == 0 { 0x1000 } else { period }
}

#[inline]
fn noise_period(noise_control: u8) -> u32 {
    let nf = (noise_control & PSG_NOISE_FREQ_MASK) as u32;
    let raw = 31u32.saturating_sub(nf);
    if raw == 0 { 64 } else { raw * 128 }
}

#[inline]
fn lfo_frequency_scale(value: u8) -> u32 {
    if value == 0 { 256 } else { value as u32 }
}

#[inline]
fn sample_to_signed(sample: u8) -> i32 {
    ((sample & 0x1F) as i32 * 2) - 0x1F
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_divider_uses_raw_12bit_frequency_value() {
        assert_eq!(tone_divider(0x0000), 0x1000);
        assert_eq!(tone_divider(0x0001), 0x0001);
        assert_eq!(tone_divider(0x0010), 0x0010);
        assert_eq!(tone_divider(0x0FFF), 0x0FFF);
    }

    #[test]
    fn tone_channel_advances_after_exact_divider_cycles() {
        let mut psg = Psg::new();
        let channel = &mut psg.channels[0];
        channel.frequency = 8;
        channel.control = PSG_CH_CTRL_KEY_ON | 0x1F;
        channel.wave_pos = 0;

        psg.clock(7);
        assert_eq!(psg.channels[0].wave_pos, 0);

        psg.clock(1);
        assert_eq!(psg.channels[0].wave_pos, 1);
    }
}
