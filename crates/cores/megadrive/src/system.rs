use crate::cartridge::Cartridge;
use crate::cpu::M68k;
use crate::input::{Button, ControllerType};
use crate::memory::MemoryMap;
use crate::vdp::{FRAME_HEIGHT, FRAME_WIDTH};
use std::path::Path;

const STATE_MAGIC: [u8; 4] = *b"MDS1";
const STATE_DECODE_STACK_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct Emulator {
    cpu: M68k,
    memory: MemoryMap,
}

impl Emulator {
    pub fn new(cartridge: Cartridge) -> Self {
        let mut emulator = Self {
            cpu: M68k::new(),
            memory: MemoryMap::new(cartridge),
        };
        emulator.reset();
        emulator
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.memory);
    }

    pub fn from_parts(cpu: M68k, memory: MemoryMap) -> Self {
        Self { cpu, memory }
    }

    pub fn into_parts(self) -> (M68k, MemoryMap) {
        (self.cpu, self.memory)
    }

    pub fn step(&mut self) -> StepResult {
        let cpu_cycles = self.cpu.step(&mut self.memory);
        self.memory.step_subsystems(cpu_cycles);
        let frame_ready = self.memory.step_vdp(cpu_cycles);
        if frame_ready {
            self.memory.request_z80_interrupt();
        }

        StepResult {
            cpu_cycles,
            frame_ready,
            pc: self.cpu.pc(),
            total_cycles: self.cpu.cycles(),
            frame_count: self.memory.frame_count(),
        }
    }

    pub fn header(&self) -> &crate::cartridge::RomHeader {
        self.memory.cartridge().header()
    }

    pub fn frame_buffer(&self) -> &[u8] {
        self.memory.frame_buffer()
    }

    pub fn frame_width(&self) -> usize {
        FRAME_WIDTH
    }

    pub fn frame_height(&self) -> usize {
        FRAME_HEIGHT
    }

    pub fn work_ram(&self) -> &[u8] {
        self.memory.work_ram()
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        self.memory.work_ram_mut()
    }

    pub fn set_button_pressed(&mut self, button: Button, pressed: bool) {
        self.memory.set_button_pressed(button, pressed);
    }

    pub fn set_button2_pressed(&mut self, button: Button, pressed: bool) {
        self.memory.set_button2_pressed(button, pressed);
    }

    pub fn set_controller_type(&mut self, player: u8, controller_type: ControllerType) {
        self.memory.set_controller_type(player, controller_type);
    }

    pub fn pending_audio_samples(&self) -> usize {
        self.memory.pending_audio_samples()
    }

    pub fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        self.memory.drain_audio_samples(max_samples)
    }

    pub fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        self.memory.set_audio_output_sample_rate_hz(hz);
    }

    pub fn audio_output_channels(&self) -> u8 {
        self.memory.audio_output_channels()
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
            return Err("invalid state file header".to_string());
        }
        let payload = bytes[STATE_MAGIC.len()..].to_vec();
        let (tx, rx) = std::sync::mpsc::channel::<Result<Box<Emulator>, String>>();
        let handle = std::thread::Builder::new()
            .name("md-state-decode".to_string())
            .stack_size(STATE_DECODE_STACK_BYTES)
            .spawn(move || {
                let (state, _): (Emulator, usize) =
                    bincode::decode_from_slice(&payload, bincode::config::standard())
                        .map_err(|e| e.to_string())?;
                tx.send(Ok(Box::new(state)))
                    .map_err(|e| format!("state decode channel send failed: {e}"))?;
                Ok::<(), String>(())
            })
            .map_err(|e| e.to_string())?;
        match handle.join() {
            Ok(result) => result?,
            Err(_) => return Err("state decode thread panicked".to_string()),
        }
        let state = rx
            .recv()
            .map_err(|e| format!("state decode channel receive failed: {e}"))??;
        *self = *state;
        self.memory.refresh_runtime_after_state_load();
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
    pub pc: u32,
    pub total_cycles: u64,
    pub frame_count: u64,
}

#[cfg(test)]
mod tests {
    use crate::{Cartridge, Emulator};

    #[test]
    fn advances_program_counter() {
        let mut rom = vec![0; 0x200];
        rom[4..8].copy_from_slice(&0x00000100u32.to_be_bytes());
        rom[0x100..0x102].copy_from_slice(&0x4E71u16.to_be_bytes()); // NOP

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let mut emulator = Emulator::new(cart);

        let step = emulator.step();
        assert_eq!(step.pc, 0x00000102);
    }

    #[test]
    fn drains_audio_samples_through_emulator_api() {
        let mut rom = vec![0; 0x200];
        rom[4..8].copy_from_slice(&0x00000100u32.to_be_bytes());
        rom[0x100..0x102].copy_from_slice(&0x4E71u16.to_be_bytes()); // NOP loop

        let cart = Cartridge::from_bytes(rom).expect("valid rom");
        let mut emulator = Emulator::new(cart);

        for _ in 0..64 {
            emulator.step();
        }

        assert!(emulator.pending_audio_samples() > 0);
        let drained = emulator.drain_audio_samples(64);
        assert!(!drained.is_empty());
    }

    #[test]
    fn exposes_work_ram_access() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid rom");
        let mut emulator = Emulator::new(cart);

        assert_eq!(emulator.work_ram().len(), 0x10000);
        emulator.work_ram_mut()[0x1234] = 0xAB;
        assert_eq!(emulator.work_ram()[0x1234], 0xAB);
    }

    #[test]
    fn save_state_bytes_include_magic_header() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid rom");
        let emulator = Emulator::new(cart);
        let bytes = emulator.save_state_bytes().expect("serialize");
        assert!(bytes.starts_with(b"MDS1"));
    }

    #[test]
    fn save_state_to_file_writes_magic_header() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid rom");
        let emulator = Emulator::new(cart);
        let path =
            std::env::temp_dir().join(format!("md_state_test_{}_slot0.mdst", std::process::id()));

        emulator.save_state_to_file(&path).expect("save file");
        let bytes = std::fs::read(&path).expect("read file");
        assert!(bytes.starts_with(b"MDS1"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_state_bytes_round_trip_restores_ram() {
        let cart = Cartridge::from_bytes(vec![0; 0x200]).expect("valid rom");
        let mut emulator = Emulator::new(cart);
        emulator.work_ram_mut()[0x3456] = 0x5A;
        let bytes = emulator.save_state_bytes().expect("serialize");
        emulator.work_ram_mut()[0x3456] = 0x00;
        emulator.load_state_bytes(&bytes).expect("deserialize");
        assert_eq!(emulator.work_ram()[0x3456], 0x5A);
    }
}
