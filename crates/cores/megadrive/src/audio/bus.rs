use super::*;

impl AudioBus {
    const M68K_CLOCK_HZ: u64 = 7_670_454;
    const DEFAULT_OUTPUT_SAMPLE_RATE_HZ: u64 = 44_100;
    const OUTPUT_CHANNELS: u8 = 2;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn output_sample_rate_hz(&self) -> u32 {
        self.output_sample_rate_hz as u32
    }

    pub fn output_channels(&self) -> u8 {
        Self::OUTPUT_CHANNELS
    }

    pub fn set_output_sample_rate_hz(&mut self, hz: u32) {
        self.output_sample_rate_hz = (hz as u64).clamp(8_000, 192_000);
    }

    pub fn read_ym2612(&self, port: u8) -> u8 {
        if (port & 0x01) == 0 {
            self.ym2612.read_status()
        } else {
            0xFF
        }
    }

    pub fn write_ym2612(&mut self, port: u8, value: u8) {
        self.ym_writes_from_68k += 1;
        self.ym2612.write_port(port, value);
    }

    pub fn write_ym2612_from_z80(&mut self, port: u8, value: u8) {
        self.ym_writes_from_z80 += 1;
        self.ym2612.write_port_from_z80(port, value);
    }

    pub fn write_psg(&mut self, value: u8) {
        self.psg_writes_from_68k += 1;
        self.psg.write_data(value);
    }

    pub fn write_psg_from_z80(&mut self, value: u8) {
        self.psg_writes_from_z80 += 1;
        self.psg.write_data(value);
    }

    pub fn step_z80_cycles(&mut self, z80_cycles: u32) {
        self.ym2612.step_z80_cycles(z80_cycles);
    }

    pub fn step(&mut self, m68k_cycles: u32) {
        self.cycles += m68k_cycles as u64;
        let sample_rate_hz = self.output_sample_rate_hz.max(1);
        self.sample_accumulator += m68k_cycles as u64 * sample_rate_hz;
        let produced = (self.sample_accumulator / Self::M68K_CLOCK_HZ) as usize;
        self.sample_accumulator %= Self::M68K_CLOCK_HZ;
        for _ in 0..produced {
            let psg_oversample_u64 = 4u64;
            let psg_oversample_i32 = 4i32;
            let mut psg_acc = 0i32;
            for _ in 0..psg_oversample_u64 {
                psg_acc += self
                    .psg
                    .next_sample((sample_rate_hz * psg_oversample_u64) as u32)
                    as i32;
            }
            let psg_sample = (psg_acc / psg_oversample_i32) * 2 / 5;
            let (ym_left, ym_right) = self.ym2612.next_sample_stereo(sample_rate_hz as u32);
            let left = (psg_sample + ym_left as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            let right =
                (psg_sample + ym_right as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            self.sample_buffer.push(left);
            self.sample_buffer.push(right);
        }
    }

    pub fn ym2612(&self) -> &Ym2612 {
        &self.ym2612
    }

    pub fn psg(&self) -> &Psg {
        &self.psg
    }

    pub fn ym_write_count(&self) -> u64 {
        self.ym2612.writes()
    }

    pub fn ym_dac_write_count(&self) -> u64 {
        self.ym2612.dac_data_writes()
    }

    pub fn psg_write_count(&self) -> u64 {
        self.psg.writes()
    }

    pub fn ym_writes_from_68k(&self) -> u64 {
        self.ym_writes_from_68k
    }

    pub fn ym_writes_from_z80(&self) -> u64 {
        self.ym_writes_from_z80
    }

    pub fn psg_writes_from_68k(&self) -> u64 {
        self.psg_writes_from_68k
    }

    pub fn psg_writes_from_z80(&self) -> u64 {
        self.psg_writes_from_z80
    }

    pub fn pending_samples(&self) -> usize {
        self.sample_buffer.len()
    }

    pub fn drain_samples(&mut self, max_samples: usize) -> Vec<i16> {
        let count = max_samples.min(self.sample_buffer.len());
        self.sample_buffer.drain(0..count).collect()
    }
}

impl Default for AudioBus {
    fn default() -> Self {
        Self {
            ym2612: Ym2612::default(),
            psg: Psg::default(),
            ym_writes_from_68k: 0,
            ym_writes_from_z80: 0,
            psg_writes_from_68k: 0,
            psg_writes_from_z80: 0,
            cycles: 0,
            output_sample_rate_hz: Self::DEFAULT_OUTPUT_SAMPLE_RATE_HZ,
            sample_accumulator: 0,
            sample_buffer: Vec::new(),
        }
    }
}
