mod audio;
mod banking;
mod control;
mod exram;

use super::super::super::Cartridge;

impl Cartridge {
    pub(in crate::cartridge) fn write_prg_mmc5(&mut self, addr: u16, data: u8) {
        match addr {
            0x5000..=0x5015 => self.write_mmc5_audio_register(addr, data),
            0x5100..=0x5130 => self.write_mmc5_banking_register(addr, data),
            0x5200..=0x5206 => self.write_mmc5_control_register(addr, data),
            0x5C00..=0x5FFF => self.write_mmc5_exram(addr, data),
            0x6000..=0x7FFF => self.write_prg_ram_mmc5(addr, data),
            0x8000..=0xFFFF => self.write_mmc5_prg_window(addr, data),
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_prg_low_mmc5(&self, addr: u16) -> u8 {
        match addr {
            0x5010 | 0x5015 => self.read_mmc5_audio_register(addr),
            0x5204..=0x5206 => self.read_mmc5_control_register(addr),
            0x5C00..=0x5FFF => self.read_mmc5_exram(addr),
            _ => 0,
        }
    }

    pub(in crate::cartridge) fn mmc5_cpu_read_side_effects(&mut self, addr: u16, value: u8) {
        if (0x8000..=0xBFFF).contains(&addr) {
            self.mmc5_clock_pcm_sample(value);
        }
    }
}
