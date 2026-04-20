use super::super::super::Cartridge;
use super::{Namco163, NAMCO163_INTERNAL_RAM_LEN, NAMCO163_WRAM_LEN};

impl Namco163 {
    fn active_channels(chip_ram: &[u8]) -> u8 {
        (((chip_ram[0x7F] >> 4) & 0x07) + 1).clamp(1, 8)
    }

    fn channel_base(active_channels: u8, channel_index: u8) -> usize {
        0x40 + ((8 - active_channels + channel_index) as usize) * 8
    }

    fn clock_audio_channel(&mut self, chip_ram: &mut [u8]) {
        let active = Self::active_channels(chip_ram);
        let channel_index = self.audio_channel_index.min(active - 1);
        let base = Self::channel_base(active, channel_index);

        let freq = chip_ram[base] as u32
            | ((chip_ram[base + 2] as u32) << 8)
            | (((chip_ram[base + 4] as u32) & 0x03) << 16);
        let mut phase = chip_ram[base + 1] as u32
            | ((chip_ram[base + 3] as u32) << 8)
            | ((chip_ram[base + 5] as u32) << 16);
        let length = 256u32 - (chip_ram[base + 4] as u32 & 0xFC);
        let wave_address = chip_ram[base + 6] as u32;
        let volume = (chip_ram[base + 7] & 0x0F) as i32;

        let sample = if length == 0 || volume == 0 {
            0.0
        } else {
            phase = (phase + freq) % (length << 16);
            let sample_index = (((phase >> 16) + wave_address) & 0xFF) as usize;
            let packed = chip_ram[sample_index >> 1];
            let nibble = if sample_index & 1 == 0 {
                packed & 0x0F
            } else {
                (packed >> 4) & 0x0F
            };

            chip_ram[base + 1] = phase as u8;
            chip_ram[base + 3] = (phase >> 8) as u8;
            chip_ram[base + 5] = (phase >> 16) as u8;

            ((nibble as i32 - 8) * volume) as f32
        };

        self.audio_outputs[channel_index as usize] = sample;
        for index in active as usize..8 {
            self.audio_outputs[index] = 0.0;
        }
        self.audio_current =
            self.audio_outputs[..active as usize].iter().sum::<f32>() / active as f32 / 32.0;
        self.audio_channel_index = if active <= 1 {
            0
        } else {
            (channel_index + 1) % active
        };
    }
}

impl Cartridge {
    pub(in crate::cartridge) fn clock_audio_namco163(&mut self) -> f32 {
        let Some(namco163) = self.mappers.namco163.as_mut() else {
            return 0.0;
        };
        if namco163.sound_disable {
            namco163.audio_current = 0.0;
            return 0.0;
        }

        if namco163.audio_delay == 0 {
            namco163.audio_delay = 14;
            if self.prg_ram.len() >= NAMCO163_WRAM_LEN + NAMCO163_INTERNAL_RAM_LEN {
                let chip_ram = &mut self.prg_ram
                    [NAMCO163_WRAM_LEN..NAMCO163_WRAM_LEN + NAMCO163_INTERNAL_RAM_LEN];
                namco163.clock_audio_channel(chip_ram);
            }
        } else {
            namco163.audio_delay -= 1;
        }

        namco163.audio_current
    }
}
