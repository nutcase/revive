use std::sync::OnceLock;

const GB_MASTER_CLOCK_HZ: u32 = 4_194_304;
const AUDIO_SAMPLE_RATE_HZ: u32 = 32_768;
const AUDIO_CYCLES_PER_SAMPLE: u32 = GB_MASTER_CLOCK_HZ / AUDIO_SAMPLE_RATE_HZ;
const AUDIO_SAMPLE_BUFFER_LIMIT: usize = 262_144;

const FRAME_SEQ_HZ: f32 = 512.0;
const NOISE_PHASE_BITS: u32 = 24;
const NOISE_PHASE_ONE: u32 = 1 << NOISE_PHASE_BITS;
const CHANNEL_FULL_SCALE: f32 = 2_048.0;
const DEFAULT_GB_AUDIO_HPF_ALPHA: f32 = 0.996;

const NR10: usize = 0x10;
const NR11: usize = 0x11;
const NR12: usize = 0x12;
const NR13: usize = 0x13;
const NR14: usize = 0x14;
const NR21: usize = 0x16;
const NR22: usize = 0x17;
const NR23: usize = 0x18;
const NR24: usize = 0x19;
const NR30: usize = 0x1A;
const NR31: usize = 0x1B;
const NR32: usize = 0x1C;
const NR33: usize = 0x1D;
const NR34: usize = 0x1E;
const NR41: usize = 0x20;
const NR42: usize = 0x21;
const NR43: usize = 0x22;
const NR44: usize = 0x23;
const NR50: usize = 0x24;
const NR51: usize = 0x25;
const NR52: usize = 0x26;
const WAVE_RAM_START: usize = 0x30;
const WAVE_RAM_END: usize = 0x3F;

static GB_AUDIO_HPF_ENABLED: OnceLock<bool> = OnceLock::new();
static GB_AUDIO_HPF_ALPHA: OnceLock<f32> = OnceLock::new();

#[derive(Debug)]
pub(crate) struct GbApu {
    cycle_accum: u32,
    frame_seq_accum: f32,
    frame_seq_step: u8,
    master_enabled: bool,
    ch1_on: bool,
    ch2_on: bool,
    ch3_on: bool,
    ch4_on: bool,
    ch1_phase: f32,
    ch2_phase: f32,
    ch3_phase: f32,
    ch4_phase: u32,
    ch4_lfsr: u16,
    ch1_length: u16,
    ch2_length: u16,
    ch3_length: u16,
    ch4_length: u16,
    ch1_shadow_freq: u16,
    ch1_sweep_counter: u8,
    ch1_volume: u8,
    ch2_volume: u8,
    ch4_volume: u8,
    ch1_env_period: u8,
    ch2_env_period: u8,
    ch4_env_period: u8,
    ch1_env_counter: u8,
    ch2_env_counter: u8,
    ch4_env_counter: u8,
    hpf_enabled: bool,
    hpf_alpha: f32,
    hpf_prev_in_l: f32,
    hpf_prev_in_r: f32,
    hpf_prev_out_l: f32,
    hpf_prev_out_r: f32,
    audio_samples: Vec<i16>,
}

impl Default for GbApu {
    fn default() -> Self {
        Self {
            cycle_accum: 0,
            frame_seq_accum: 0.0,
            frame_seq_step: 0,
            master_enabled: false,
            ch1_on: false,
            ch2_on: false,
            ch3_on: false,
            ch4_on: false,
            ch1_phase: 0.0,
            ch2_phase: 0.0,
            ch3_phase: 0.0,
            ch4_phase: 0,
            ch4_lfsr: 0x7FFF,
            ch1_length: 0,
            ch2_length: 0,
            ch3_length: 0,
            ch4_length: 0,
            ch1_shadow_freq: 0,
            ch1_sweep_counter: 0,
            ch1_volume: 0,
            ch2_volume: 0,
            ch4_volume: 0,
            ch1_env_period: 0,
            ch2_env_period: 0,
            ch4_env_period: 0,
            ch1_env_counter: 0,
            ch2_env_counter: 0,
            ch4_env_counter: 0,
            hpf_enabled: gb_audio_hpf_enabled(),
            hpf_alpha: gb_audio_hpf_alpha(),
            hpf_prev_in_l: 0.0,
            hpf_prev_in_r: 0.0,
            hpf_prev_out_l: 0.0,
            hpf_prev_out_r: 0.0,
            audio_samples: Vec::new(),
        }
    }
}

impl GbApu {
    pub fn reset(&mut self, io: &mut [u8; 128]) {
        self.cycle_accum = 0;
        self.frame_seq_accum = 0.0;
        self.frame_seq_step = 0;
        self.master_enabled = (io[NR52] & 0x80) != 0;
        self.ch1_on = false;
        self.ch2_on = false;
        self.ch3_on = false;
        self.ch4_on = false;
        self.ch1_phase = 0.0;
        self.ch2_phase = 0.0;
        self.ch3_phase = 0.0;
        self.ch4_phase = 0;
        self.ch4_lfsr = 0x7FFF;
        self.ch1_length = 0;
        self.ch2_length = 0;
        self.ch3_length = 0;
        self.ch4_length = 0;
        self.ch1_shadow_freq = 0;
        self.ch1_sweep_counter = 0;
        self.ch1_volume = 0;
        self.ch2_volume = 0;
        self.ch4_volume = 0;
        self.ch1_env_period = 0;
        self.ch2_env_period = 0;
        self.ch4_env_period = 0;
        self.ch1_env_counter = 0;
        self.ch2_env_counter = 0;
        self.ch4_env_counter = 0;
        self.hpf_prev_in_l = 0.0;
        self.hpf_prev_in_r = 0.0;
        self.hpf_prev_out_l = 0.0;
        self.hpf_prev_out_r = 0.0;
        self.audio_samples.clear();
        self.refresh_status(io);
    }

    pub fn is_apu_register(index: usize) -> bool {
        (NR10..=NR52).contains(&index) || (WAVE_RAM_START..=WAVE_RAM_END).contains(&index)
    }

    pub fn read_reg(&self, index: usize, io: &[u8; 128]) -> u8 {
        if index == NR52 {
            return io[NR52];
        }
        if (WAVE_RAM_START..=WAVE_RAM_END).contains(&index) {
            return io[index];
        }

        let read_mask = match index {
            NR10 => 0x80,
            NR11 => 0x3F,
            NR12 => 0x00,
            NR13 => 0xFF,
            NR14 => 0xBF,
            NR21 => 0x3F,
            NR22 => 0x00,
            NR23 => 0xFF,
            NR24 => 0xBF,
            NR30 => 0x7F,
            NR31 => 0xFF,
            NR32 => 0x9F,
            NR33 => 0xFF,
            NR34 => 0xBF,
            NR41 => 0xFF,
            NR42 => 0x00,
            NR43 => 0x00,
            NR44 => 0xBF,
            NR50 => 0x00,
            NR51 => 0x00,
            _ => 0x00,
        };
        io[index] | read_mask
    }

    pub fn write_reg(&mut self, index: usize, value: u8, io: &mut [u8; 128]) {
        if !Self::is_apu_register(index) {
            return;
        }

        if index == NR52 {
            self.write_nr52(value, io);
            self.refresh_status(io);
            return;
        }

        if !self.master_enabled && !(WAVE_RAM_START..=WAVE_RAM_END).contains(&index) {
            return;
        }

        io[index] = value;

        match index {
            NR12 => {
                if (value & 0xF8) == 0 {
                    self.ch1_on = false;
                }
            }
            NR22 => {
                if (value & 0xF8) == 0 {
                    self.ch2_on = false;
                }
            }
            NR30 => {
                if (value & 0x80) == 0 {
                    self.ch3_on = false;
                    self.ch3_phase = 0.0;
                }
            }
            NR42 => {
                if (value & 0xF8) == 0 {
                    self.ch4_on = false;
                }
            }
            NR14 => {
                if (value & 0x80) != 0 {
                    self.trigger_ch1(io);
                }
            }
            NR24 => {
                if (value & 0x80) != 0 {
                    self.trigger_ch2(io);
                }
            }
            NR34 => {
                if (value & 0x80) != 0 {
                    self.trigger_ch3(io);
                }
            }
            NR44 => {
                if (value & 0x80) != 0 {
                    self.trigger_ch4(io);
                }
            }
            _ => {}
        }

        self.refresh_status(io);
    }

    pub fn mix_audio_for_cycles(&mut self, cycles: u32, io: &mut [u8; 128]) {
        self.cycle_accum = self.cycle_accum.wrapping_add(cycles);
        while self.cycle_accum >= AUDIO_CYCLES_PER_SAMPLE {
            self.cycle_accum -= AUDIO_CYCLES_PER_SAMPLE;
            let (left, right) = self.mix_sample(io);
            let (left, right) = self.apply_output_filter(left, right);
            if self.audio_samples.len() + 2 >= AUDIO_SAMPLE_BUFFER_LIMIT {
                self.audio_samples.clear();
            }
            self.audio_samples.push(left);
            self.audio_samples.push(right);
        }
    }

    pub fn take_audio_samples_i16_into(&mut self, out: &mut Vec<i16>) {
        out.clear();
        out.extend_from_slice(&self.audio_samples);
        self.audio_samples.clear();
    }

    pub fn audio_sample_rate_hz(&self) -> u32 {
        AUDIO_SAMPLE_RATE_HZ
    }

    fn write_nr52(&mut self, value: u8, io: &mut [u8; 128]) {
        let enable = (value & 0x80) != 0;
        if !enable {
            self.master_enabled = false;
            self.ch1_on = false;
            self.ch2_on = false;
            self.ch3_on = false;
            self.ch4_on = false;
            self.ch1_phase = 0.0;
            self.ch2_phase = 0.0;
            self.ch3_phase = 0.0;
            self.ch4_phase = 0;
            self.ch4_lfsr = 0x7FFF;
            self.ch1_length = 0;
            self.ch2_length = 0;
            self.ch3_length = 0;
            self.ch4_length = 0;
            self.ch1_shadow_freq = 0;
            self.ch1_sweep_counter = 0;
            self.ch1_volume = 0;
            self.ch2_volume = 0;
            self.ch4_volume = 0;
            self.ch1_env_period = 0;
            self.ch2_env_period = 0;
            self.ch4_env_period = 0;
            self.ch1_env_counter = 0;
            self.ch2_env_counter = 0;
            self.ch4_env_counter = 0;
            self.frame_seq_accum = 0.0;
            self.frame_seq_step = 0;
            self.cycle_accum = 0;
            for slot in io.iter_mut().take(NR52).skip(NR10) {
                *slot = 0;
            }
            io[NR52] = 0x70;
            return;
        }

        if !self.master_enabled {
            self.master_enabled = true;
            self.frame_seq_accum = 0.0;
            self.frame_seq_step = 0;
            self.cycle_accum = 0;
        }
    }

    fn refresh_status(&mut self, io: &mut [u8; 128]) {
        let mut status = 0x70;
        if self.master_enabled {
            status |= 0x80;
        }
        if self.ch1_on {
            status |= 1 << 0;
        }
        if self.ch2_on {
            status |= 1 << 1;
        }
        if self.ch3_on {
            status |= 1 << 2;
        }
        if self.ch4_on {
            status |= 1 << 3;
        }
        io[NR52] = status;
    }

    fn trigger_ch1(&mut self, io: &mut [u8; 128]) {
        if !self.master_enabled || (io[NR12] & 0xF8) == 0 {
            self.ch1_on = false;
            return;
        }

        self.ch1_on = true;
        self.ch1_phase = 0.0;
        self.ch1_length = (64 - u16::from(io[NR11] & 0x3F)).max(1);
        self.ch1_volume = (io[NR12] >> 4) & 0x0F;
        self.ch1_env_period = io[NR12] & 0x07;
        self.ch1_env_counter = self.ch1_env_period;
        self.ch1_shadow_freq = ((u16::from(io[NR14] & 0x07)) << 8) | u16::from(io[NR13]);
        let sweep_period = (io[NR10] >> 4) & 0x07;
        self.ch1_sweep_counter = if sweep_period == 0 { 8 } else { sweep_period };

        let sweep_shift = io[NR10] & 0x07;
        let sweep_negate = (io[NR10] & 0x08) != 0;
        if sweep_shift != 0
            && self
                .compute_ch1_sweep_frequency(sweep_negate, sweep_shift)
                .is_none()
        {
            self.ch1_on = false;
        }
    }

    fn trigger_ch2(&mut self, io: &mut [u8; 128]) {
        if !self.master_enabled || (io[NR22] & 0xF8) == 0 {
            self.ch2_on = false;
            return;
        }

        self.ch2_on = true;
        self.ch2_phase = 0.0;
        self.ch2_length = (64 - u16::from(io[NR21] & 0x3F)).max(1);
        self.ch2_volume = (io[NR22] >> 4) & 0x0F;
        self.ch2_env_period = io[NR22] & 0x07;
        self.ch2_env_counter = self.ch2_env_period;
    }

    fn trigger_ch3(&mut self, io: &mut [u8; 128]) {
        if !self.master_enabled || (io[NR30] & 0x80) == 0 {
            self.ch3_on = false;
            return;
        }

        self.ch3_on = true;
        self.ch3_phase = 0.0;
        self.ch3_length = (256 - u16::from(io[NR31])).max(1);
    }

    fn trigger_ch4(&mut self, io: &mut [u8; 128]) {
        if !self.master_enabled || (io[NR42] & 0xF8) == 0 {
            self.ch4_on = false;
            return;
        }

        self.ch4_on = true;
        self.ch4_phase = 0;
        self.ch4_lfsr = 0x7FFF;
        self.ch4_length = (64 - u16::from(io[NR41] & 0x3F)).max(1);
        self.ch4_volume = (io[NR42] >> 4) & 0x0F;
        self.ch4_env_period = io[NR42] & 0x07;
        self.ch4_env_counter = self.ch4_env_period;
    }

    fn advance_timing(&mut self, io: &mut [u8; 128]) {
        if !self.master_enabled {
            return;
        }
        self.frame_seq_accum += FRAME_SEQ_HZ / AUDIO_SAMPLE_RATE_HZ as f32;
        while self.frame_seq_accum >= 1.0 {
            self.frame_seq_accum -= 1.0;
            self.clock_frame_sequencer(io);
        }
    }

    fn clock_frame_sequencer(&mut self, io: &mut [u8; 128]) {
        match self.frame_seq_step {
            0 | 4 => self.clock_length_counters(io),
            2 | 6 => {
                self.clock_length_counters(io);
                self.clock_ch1_sweep(io);
            }
            7 => self.clock_envelopes(io),
            _ => {}
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 0x07;
    }

    fn compute_ch1_sweep_frequency(&self, negate: bool, shift: u8) -> Option<u16> {
        if shift == 0 {
            return Some(self.ch1_shadow_freq);
        }
        let delta = self.ch1_shadow_freq >> shift;
        let next = if negate {
            i32::from(self.ch1_shadow_freq) - i32::from(delta)
        } else {
            i32::from(self.ch1_shadow_freq) + i32::from(delta)
        };
        if (0..=2047).contains(&next) {
            Some(next as u16)
        } else {
            None
        }
    }

    fn clock_ch1_sweep(&mut self, io: &mut [u8; 128]) {
        if !self.ch1_on {
            return;
        }

        let sweep_period = (io[NR10] >> 4) & 0x07;
        let sweep_shift = io[NR10] & 0x07;
        let sweep_negate = (io[NR10] & 0x08) != 0;

        if self.ch1_sweep_counter > 0 {
            self.ch1_sweep_counter -= 1;
        }
        if self.ch1_sweep_counter != 0 {
            return;
        }
        self.ch1_sweep_counter = if sweep_period == 0 { 8 } else { sweep_period };

        if sweep_shift == 0 {
            return;
        }

        let Some(next_frequency) = self.compute_ch1_sweep_frequency(sweep_negate, sweep_shift)
        else {
            self.ch1_on = false;
            self.refresh_status(io);
            return;
        };

        self.ch1_shadow_freq = next_frequency;
        io[NR13] = (next_frequency & 0x00FF) as u8;
        io[NR14] = (io[NR14] & !0x07) | ((next_frequency >> 8) as u8 & 0x07);

        if self
            .compute_ch1_sweep_frequency(sweep_negate, sweep_shift)
            .is_none()
        {
            self.ch1_on = false;
        }

        self.refresh_status(io);
    }

    fn clock_length_counters(&mut self, io: &mut [u8; 128]) {
        let mut changed = false;

        if self.ch1_on && (io[NR14] & 0x40) != 0 {
            self.ch1_length = self.ch1_length.saturating_sub(1);
            if self.ch1_length == 0 {
                self.ch1_on = false;
                changed = true;
            }
        }

        if self.ch2_on && (io[NR24] & 0x40) != 0 {
            self.ch2_length = self.ch2_length.saturating_sub(1);
            if self.ch2_length == 0 {
                self.ch2_on = false;
                changed = true;
            }
        }

        if self.ch3_on && (io[NR34] & 0x40) != 0 {
            self.ch3_length = self.ch3_length.saturating_sub(1);
            if self.ch3_length == 0 {
                self.ch3_on = false;
                self.ch3_phase = 0.0;
                changed = true;
            }
        }

        if self.ch4_on && (io[NR44] & 0x40) != 0 {
            self.ch4_length = self.ch4_length.saturating_sub(1);
            if self.ch4_length == 0 {
                self.ch4_on = false;
                changed = true;
            }
        }

        if changed {
            self.refresh_status(io);
        }
    }

    fn envelope_tick(period: u8, counter: &mut u8) -> bool {
        if period == 0 {
            return false;
        }
        if *counter == 0 {
            *counter = period;
        }
        if *counter > 1 {
            *counter -= 1;
            return false;
        }
        *counter = period;
        true
    }

    fn clock_envelopes(&mut self, io: &mut [u8; 128]) {
        if self.ch1_on && Self::envelope_tick(self.ch1_env_period, &mut self.ch1_env_counter) {
            if (io[NR12] & 0x08) != 0 {
                self.ch1_volume = (self.ch1_volume.saturating_add(1)).min(15);
            } else {
                self.ch1_volume = self.ch1_volume.saturating_sub(1);
            }
        }

        if self.ch2_on && Self::envelope_tick(self.ch2_env_period, &mut self.ch2_env_counter) {
            if (io[NR22] & 0x08) != 0 {
                self.ch2_volume = (self.ch2_volume.saturating_add(1)).min(15);
            } else {
                self.ch2_volume = self.ch2_volume.saturating_sub(1);
            }
        }

        if self.ch4_on && Self::envelope_tick(self.ch4_env_period, &mut self.ch4_env_counter) {
            if (io[NR42] & 0x08) != 0 {
                self.ch4_volume = (self.ch4_volume.saturating_add(1)).min(15);
            } else {
                self.ch4_volume = self.ch4_volume.saturating_sub(1);
            }
        }
    }

    fn mix_sample(&mut self, io: &mut [u8; 128]) -> (i16, i16) {
        self.advance_timing(io);

        if !self.master_enabled {
            return (0, 0);
        }

        let ch1 = self.next_square_sample(1, io);
        let ch2 = self.next_square_sample(2, io);
        let ch3 = self.next_wave_sample(io);
        let ch4 = self.next_noise_sample(io);

        let nr51 = io[NR51];
        let mut left = 0i32;
        let mut right = 0i32;

        if (nr51 & (1 << 4)) != 0 {
            left += i32::from(ch1);
        }
        if (nr51 & (1 << 0)) != 0 {
            right += i32::from(ch1);
        }
        if (nr51 & (1 << 5)) != 0 {
            left += i32::from(ch2);
        }
        if (nr51 & (1 << 1)) != 0 {
            right += i32::from(ch2);
        }
        if (nr51 & (1 << 6)) != 0 {
            left += i32::from(ch3);
        }
        if (nr51 & (1 << 2)) != 0 {
            right += i32::from(ch3);
        }
        if (nr51 & (1 << 7)) != 0 {
            left += i32::from(ch4);
        }
        if (nr51 & (1 << 3)) != 0 {
            right += i32::from(ch4);
        }

        let nr50 = io[NR50];
        let left_master = i32::from((nr50 >> 4) & 0x07) + 1;
        let right_master = i32::from(nr50 & 0x07) + 1;
        left = (left * left_master) / 8;
        right = (right * right_master) / 8;

        (
            left.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            right.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        )
    }

    fn next_square_sample(&mut self, channel: u8, io: &[u8; 128]) -> i16 {
        let (enabled, phase, duty, volume, freq_reg) = if channel == 1 {
            (
                self.ch1_on,
                self.ch1_phase,
                (io[NR11] >> 6) & 0x03,
                self.ch1_volume,
                ((u16::from(io[NR14] & 0x07)) << 8) | u16::from(io[NR13]),
            )
        } else {
            (
                self.ch2_on,
                self.ch2_phase,
                (io[NR21] >> 6) & 0x03,
                self.ch2_volume,
                ((u16::from(io[NR24] & 0x07)) << 8) | u16::from(io[NR23]),
            )
        };

        if !enabled || volume == 0 || freq_reg >= 2048 {
            return 0;
        }

        let duty_cycle = match duty {
            0 => 0.125_f32,
            1 => 0.25_f32,
            2 => 0.5_f32,
            _ => 0.75_f32,
        };

        let frequency_hz = 131_072.0_f32 / (2048 - freq_reg) as f32;
        let mut phase_next = phase + (frequency_hz / AUDIO_SAMPLE_RATE_HZ as f32);
        while phase_next >= 1.0_f32 {
            phase_next -= 1.0_f32;
        }
        if channel == 1 {
            self.ch1_phase = phase_next;
        } else {
            self.ch2_phase = phase_next;
        }

        let polarity = if phase_next < duty_cycle {
            1.0_f32
        } else {
            -1.0_f32
        };
        let amp = (f32::from(volume) / 15.0_f32) * CHANNEL_FULL_SCALE;
        (polarity * amp).round() as i16
    }

    fn next_wave_sample(&mut self, io: &mut [u8; 128]) -> i16 {
        if !self.ch3_on {
            return 0;
        }

        if (io[NR30] & 0x80) == 0 {
            self.ch3_on = false;
            self.ch3_phase = 0.0;
            self.refresh_status(io);
            return 0;
        }

        let volume_code = (io[NR32] >> 5) & 0x03;
        let freq_reg = ((u16::from(io[NR34] & 0x07)) << 8) | u16::from(io[NR33]);
        if volume_code == 0 || freq_reg >= 2048 {
            return 0;
        }

        let frequency_hz = 65_536.0_f32 / (2048 - freq_reg) as f32;
        let mut phase_next = self.ch3_phase + (frequency_hz / AUDIO_SAMPLE_RATE_HZ as f32);
        while phase_next >= 1.0_f32 {
            phase_next -= 1.0_f32;
        }
        self.ch3_phase = phase_next;

        let sample_index = ((phase_next * 32.0_f32) as usize).min(31);
        let wave_byte = io[WAVE_RAM_START + (sample_index / 2)];
        let nibble = if (sample_index & 1) == 0 {
            wave_byte >> 4
        } else {
            wave_byte & 0x0F
        };
        let centered = i16::from(nibble) - 8;
        let base = centered * (CHANNEL_FULL_SCALE as i16 / 8);
        match volume_code {
            1 => base,
            2 => base / 2,
            3 => base / 4,
            _ => 0,
        }
    }

    fn next_noise_sample(&mut self, io: &[u8; 128]) -> i16 {
        if !self.ch4_on {
            return 0;
        }
        if self.ch4_volume == 0 {
            return 0;
        }

        let nr43 = io[NR43];
        let divisor_code = u32::from(nr43 & 0x07);
        let width7 = (nr43 & 0x08) != 0;
        let shift = u32::from((nr43 >> 4) & 0x0F);
        let divisor = if divisor_code == 0 {
            8
        } else {
            divisor_code * 16
        };
        let denominator = divisor << (shift + 1);
        let frequency_hz = 524_288.0_f32 / denominator as f32;
        let step = (((frequency_hz / AUDIO_SAMPLE_RATE_HZ as f32) * NOISE_PHASE_ONE as f32).round()
            as u32)
            .max(1);

        let mut phase = self.ch4_phase.saturating_add(step);
        while phase >= NOISE_PHASE_ONE {
            phase -= NOISE_PHASE_ONE;
            let bit = (self.ch4_lfsr ^ (self.ch4_lfsr >> 1)) & 1;
            self.ch4_lfsr = (self.ch4_lfsr >> 1) | (bit << 14);
            if width7 {
                self.ch4_lfsr = (self.ch4_lfsr & !(1 << 6)) | (bit << 6);
            }
        }
        self.ch4_phase = phase;

        let polarity = if (self.ch4_lfsr & 1) == 0 {
            1.0_f32
        } else {
            -1.0_f32
        };
        let amp = (f32::from(self.ch4_volume) / 15.0_f32) * CHANNEL_FULL_SCALE;
        (polarity * amp).round() as i16
    }

    fn apply_output_filter(&mut self, left: i16, right: i16) -> (i16, i16) {
        if !self.hpf_enabled {
            return (left, right);
        }

        let left_in = f32::from(left);
        let right_in = f32::from(right);
        let left_out = self.hpf_alpha * (self.hpf_prev_out_l + left_in - self.hpf_prev_in_l);
        let right_out = self.hpf_alpha * (self.hpf_prev_out_r + right_in - self.hpf_prev_in_r);

        self.hpf_prev_in_l = left_in;
        self.hpf_prev_in_r = right_in;
        self.hpf_prev_out_l = left_out;
        self.hpf_prev_out_r = right_out;

        (
            left_out.clamp(i16::MIN as f32, i16::MAX as f32).round() as i16,
            right_out.clamp(i16::MIN as f32, i16::MAX as f32).round() as i16,
        )
    }
}

fn gb_audio_hpf_enabled() -> bool {
    *GB_AUDIO_HPF_ENABLED.get_or_init(|| {
        let raw = match std::env::var("GB_AUDIO_HPF") {
            Ok(value) => value,
            Err(_) => return true,
        };
        let lowered = raw.trim().to_ascii_lowercase();
        !(lowered.is_empty()
            || lowered == "0"
            || lowered == "false"
            || lowered == "off"
            || lowered == "no")
    })
}

fn gb_audio_hpf_alpha() -> f32 {
    *GB_AUDIO_HPF_ALPHA.get_or_init(|| {
        let raw = match std::env::var("GB_AUDIO_HPF_ALPHA") {
            Ok(value) => value,
            Err(_) => return DEFAULT_GB_AUDIO_HPF_ALPHA,
        };
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return DEFAULT_GB_AUDIO_HPF_ALPHA;
        }
        trimmed
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.90, 0.9999))
            .unwrap_or(DEFAULT_GB_AUDIO_HPF_ALPHA)
    })
}
