pub mod audio;
mod bus;
pub mod input;
pub mod vdp;
mod z80;

use std::path::Path;

use bus::Bus;
pub use input::Button;
pub use vdp::{FRAME_HEIGHT, FRAME_WIDTH};
use z80::Z80;

const STATE_MAGIC: [u8; 4] = *b"SMS1";
const STEP_CYCLES: u32 = 64;

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
        let mut cpu = Z80::new();
        cpu.write_reset_byte(0x01);
        Ok(Self {
            cpu,
            bus: Bus::new(rom),
        })
    }

    pub fn step(&mut self) -> StepResult {
        self.cpu.step(STEP_CYCLES, &mut self.bus);
        let frame_ready = self.bus.step(STEP_CYCLES);
        if frame_ready && self.bus.vdp_interrupt_enabled() {
            self.cpu.request_interrupt();
        }
        StepResult {
            cpu_cycles: STEP_CYCLES,
            frame_ready,
            pc: self.cpu.pc(),
            total_cycles: self.cpu.cycles(),
            frame_count: self.bus.frame_count(),
        }
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
        let mut bytes = Vec::with_capacity(16 * 1024);
        bytes.extend_from_slice(&STATE_MAGIC);
        let encoded =
            bincode::encode_to_vec(self, bincode::config::standard()).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&encoded);
        Ok(bytes)
    }

    pub fn save_state_to_file(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = self.save_state_bytes()?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    }

    pub fn load_state_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < STATE_MAGIC.len() || bytes[..STATE_MAGIC.len()] != STATE_MAGIC {
            return Err("invalid Master System state file header".to_string());
        }
        let (state, _): (Emulator, usize) =
            bincode::decode_from_slice(&bytes[STATE_MAGIC.len()..], bincode::config::standard())
                .map_err(|e| e.to_string())?;
        *self = state;
        Ok(())
    }

    pub fn load_state_from_file(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        self.load_state_bytes(&bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct StepResult {
    pub cpu_cycles: u32,
    pub frame_ready: bool,
    pub pc: u16,
    pub total_cycles: u64,
    pub frame_count: u64,
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
