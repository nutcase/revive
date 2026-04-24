pub mod audio;
mod bus;
pub mod input;
pub mod vdp;
mod z80;

use std::path::Path;

use bus::Bus;
pub use input::Button;
use sega8_common::emulator;
pub use sega8_common::emulator::StepResult;
pub use vdp::{FRAME_HEIGHT, FRAME_WIDTH};
use z80::Z80;

const STATE_MAGIC: [u8; 4] = *b"SMS1";

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Emulator {
    cpu: Z80,
    bus: Bus,
}

impl Emulator {
    pub fn new(rom: Vec<u8>) -> Result<Self, String> {
        if rom.is_empty() {
            return Err("Master System ROM is empty".to_string());
        }
        Ok(Self {
            cpu: emulator::initialized_cpu(),
            bus: Bus::new(rom),
        })
    }

    pub fn step(&mut self) -> StepResult {
        emulator::step_frame(&mut self.cpu, &mut self.bus)
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.bus.frame_buffer()
    }

    pub fn frame_width(&self) -> usize {
        FRAME_WIDTH
    }

    pub fn frame_height(&self) -> usize {
        FRAME_HEIGHT
    }

    pub fn work_ram(&self) -> &[u8] {
        self.bus.work_ram()
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        self.bus.work_ram_mut()
    }

    pub fn cart_ram(&self) -> &[u8] {
        self.bus.cart_ram()
    }

    pub fn cart_ram_mut(&mut self) -> &mut [u8] {
        self.bus.cart_ram_mut()
    }

    pub fn vram(&self) -> &[u8] {
        self.bus.vram()
    }

    pub fn vram_mut(&mut self) -> &mut [u8] {
        self.bus.vram_mut()
    }

    pub fn set_button_pressed(&mut self, player: u8, button: Button, pressed: bool) {
        self.bus.set_button_pressed(player, button, pressed);
    }

    pub fn pending_audio_samples(&self) -> usize {
        self.bus.pending_audio_samples()
    }

    pub fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        self.bus.drain_audio_samples(max_samples)
    }

    pub fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        self.bus.set_audio_output_sample_rate_hz(hz);
    }

    pub fn audio_output_channels(&self) -> u8 {
        self.bus.audio_output_channels()
    }

    pub fn save_state_bytes(&self) -> Result<Vec<u8>, String> {
        emulator::save_state_bytes(&STATE_MAGIC, self)
    }

    pub fn save_state_to_file(&self, path: &Path) -> Result<(), String> {
        emulator::save_state_to_file(&STATE_MAGIC, self, path)
    }

    pub fn load_state_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        *self = emulator::load_state_bytes(&STATE_MAGIC, "Master System", bytes)?;
        Ok(())
    }

    pub fn load_state_from_file(&mut self, path: &Path) -> Result<(), String> {
        *self = emulator::load_state_from_file(&STATE_MAGIC, "Master System", path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_can_write_work_ram() {
        let rom = vec![0x3E, 0x42, 0x32, 0x00, 0xC0, 0x76];
        let mut emulator = Emulator::new(rom).expect("valid rom");

        for _ in 0..16 {
            emulator.step();
        }

        assert_eq!(emulator.work_ram()[0], 0x42);
    }

    #[test]
    fn save_state_bytes_include_magic_header() {
        let emulator = Emulator::new(vec![0x00]).expect("valid rom");
        let bytes = emulator.save_state_bytes().expect("serialize");
        assert!(bytes.starts_with(b"SMS1"));
    }

    #[test]
    fn frame_buffer_is_rgb24_active_area() {
        let emulator = Emulator::new(vec![0x00]).expect("valid rom");
        assert_eq!(emulator.frame_width(), 256);
        assert_eq!(emulator.frame_height(), 192);
        assert_eq!(emulator.frame_buffer().len(), 256 * 192 * 3);
    }
}
