use crate::z80::Z80_CLOCK_HZ;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Psg {
    last_data: u8,
    writes: u64,
    latched_channel: usize,
    latched_is_volume: bool,
    tone_period: [u16; 3],
    tone_output: [bool; 3],
    tone_counter: [u16; 3],
    attenuation: [u8; 4],
    noise_control: u8,
    noise_lfsr: u16,
    noise_counter: u16,
    sample_counter: u32,
}

const PSG_VOLUME: [i16; 16] = [
    8000, 6355, 5048, 4009, 3184, 2529, 2009, 1596, 1268, 1007, 800, 635, 505, 401, 318, 0,
];

impl Default for Psg {
    fn default() -> Self {
        Self {
            last_data: 0,
            writes: 0,
            latched_channel: 0,
            latched_is_volume: false,
            tone_period: [1, 1, 1],
            tone_output: [true, true, true],
            tone_counter: [1, 1, 1],
            attenuation: [0x0F; 4],
            noise_control: 0,
            noise_lfsr: 0x4000,
            noise_counter: 0x10,
            sample_counter: 0,
        }
    }
}

impl Psg {
    const PSG_CLOCK_HZ: u32 = (Z80_CLOCK_HZ as u32) / 16;

    pub fn write_data(&mut self, value: u8) {
        self.last_data = value;
        self.writes += 1;
        if (value & 0x80) != 0 {
            self.latched_channel = ((value >> 5) & 0x03) as usize;
            self.latched_is_volume = (value & 0x10) != 0;
            self.apply_latched_data(value & 0x0F);
            return;
        }

        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = value & 0x0F;
        } else if self.latched_channel < 3 {
            let lo = self.tone_period[self.latched_channel] & 0x000F;
            let hi = ((value & 0x3F) as u16) << 4;
            self.tone_period[self.latched_channel] = lo | hi;
        }
    }

    pub fn last_data(&self) -> u8 {
        self.last_data
    }

    fn apply_latched_data(&mut self, data: u8) {
        if self.latched_is_volume {
            self.attenuation[self.latched_channel] = data & 0x0F;
            return;
        }

        if self.latched_channel < 3 {
            let hi = self.tone_period[self.latched_channel] & 0x03F0;
            self.tone_period[self.latched_channel] = hi | data as u16;
        } else {
            self.noise_control = data & 0x07;
            self.noise_lfsr = 0x4000;
            self.noise_counter = Self::noise_period(data & 0x07, self.tone_period[2]);
        }
    }

    fn noise_period(noise_control: u8, tone3_period: u16) -> u16 {
        match noise_control & 0x03 {
            0x00 => 0x10,
            0x01 => 0x20,
            0x02 => 0x40,
            _ => tone3_period.max(1),
        }
    }

    fn clock_noise_lfsr(&mut self) {
        let bit0 = self.noise_lfsr & 1;
        let feedback = if (self.noise_control & 0x04) != 0 {
            bit0 ^ ((self.noise_lfsr >> 3) & 1)
        } else {
            bit0
        };
        self.noise_lfsr = ((self.noise_lfsr >> 1) | (feedback << 14)) & 0x7FFF;
    }

    fn clock_tick(&mut self) {
        let noise_uses_tone3 = (self.noise_control & 0x03) == 0x03;
        for ch in 0..3 {
            self.tone_counter[ch] = self.tone_counter[ch].saturating_sub(1);
            if self.tone_counter[ch] == 0 {
                let period = (self.tone_period[ch] & 0x03FF).max(1);
                self.tone_counter[ch] = period;
                let was_high = self.tone_output[ch];
                self.tone_output[ch] = !self.tone_output[ch];
                if noise_uses_tone3 && ch == 2 && was_high && !self.tone_output[ch] {
                    self.clock_noise_lfsr();
                }
            }
        }

        if !noise_uses_tone3 {
            self.noise_counter = self.noise_counter.saturating_sub(1);
            if self.noise_counter == 0 {
                self.noise_counter = Self::noise_period(self.noise_control, self.tone_period[2]);
                self.clock_noise_lfsr();
            }
        }
    }

    fn next_sample(&mut self, sample_rate_hz: u32) -> i16 {
        self.sample_counter = self.sample_counter.saturating_add(Self::PSG_CLOCK_HZ);
        while self.sample_counter >= sample_rate_hz {
            self.sample_counter -= sample_rate_hz;
            self.clock_tick();
        }

        let mut mix = 0i32;
        for ch in 0..3 {
            let vol = PSG_VOLUME[self.attenuation[ch].min(15) as usize] as i32;
            mix += if self.tone_output[ch] { vol } else { -vol };
        }
        let noise_vol = PSG_VOLUME[self.attenuation[3].min(15) as usize] as i32;
        mix += if (self.noise_lfsr & 1) != 0 {
            noise_vol
        } else {
            -noise_vol
        };
        ((mix * 9) / 40).clamp(i16::MIN as i32, i16::MAX as i32) as i16
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Audio {
    psg: Psg,
    output_sample_rate_hz: u64,
    sample_accumulator: u64,
    sample_buffer: Vec<i16>,
}

impl Audio {
    const DEFAULT_OUTPUT_SAMPLE_RATE_HZ: u64 = 44_100;
    const OUTPUT_CHANNELS: u8 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_channels(&self) -> u8 {
        Self::OUTPUT_CHANNELS
    }

    pub fn set_output_sample_rate_hz(&mut self, hz: u32) {
        self.output_sample_rate_hz = (hz as u64).clamp(8_000, 192_000);
    }

    pub fn write_psg(&mut self, value: u8) {
        self.psg.write_data(value);
    }

    pub fn step(&mut self, z80_cycles: u32) {
        let sample_rate_hz = self.output_sample_rate_hz.max(1);
        self.sample_accumulator += z80_cycles as u64 * sample_rate_hz;
        let produced = (self.sample_accumulator / Z80_CLOCK_HZ) as usize;
        self.sample_accumulator %= Z80_CLOCK_HZ;
        for _ in 0..produced {
            let sample = self.psg.next_sample(sample_rate_hz as u32);
            self.sample_buffer.push(sample);
            self.sample_buffer.push(sample);
        }
    }

    pub fn pending_samples(&self) -> usize {
        self.sample_buffer.len()
    }

    pub fn drain_samples(&mut self, max_samples: usize) -> Vec<i16> {
        let count = max_samples.min(self.sample_buffer.len());
        self.sample_buffer.drain(0..count).collect()
    }

    pub fn psg(&self) -> &Psg {
        &self.psg
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            psg: Psg::default(),
            output_sample_rate_hz: Self::DEFAULT_OUTPUT_SAMPLE_RATE_HZ,
            sample_accumulator: 0,
            sample_buffer: Vec::new(),
        }
    }
}
