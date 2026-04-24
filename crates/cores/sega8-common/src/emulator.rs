use std::path::Path;

use crate::z80::{BusIo, Z80};

pub const STEP_CYCLES: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct StepResult {
    pub cpu_cycles: u32,
    pub frame_ready: bool,
    pub pc: u16,
    pub total_cycles: u64,
    pub frame_count: u64,
}

pub trait FrameBus: BusIo {
    fn step_components(&mut self, cycles: u32) -> bool;
    fn vdp_interrupt_enabled(&self) -> bool;
    fn frame_count(&self) -> u64;
}

pub fn initialized_cpu() -> Z80 {
    let mut cpu = Z80::new();
    cpu.write_reset_byte(0x01);
    cpu
}

pub fn step_frame<B: FrameBus>(cpu: &mut Z80, bus: &mut B) -> StepResult {
    cpu.step(STEP_CYCLES, bus);
    let frame_ready = bus.step_components(STEP_CYCLES);
    if frame_ready && bus.vdp_interrupt_enabled() {
        cpu.request_interrupt();
    }
    StepResult {
        cpu_cycles: STEP_CYCLES,
        frame_ready,
        pc: cpu.pc(),
        total_cycles: cpu.cycles(),
        frame_count: bus.frame_count(),
    }
}

pub fn save_state_bytes<T: bincode::Encode>(magic: &[u8; 4], state: &T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(16 * 1024);
    bytes.extend_from_slice(magic);
    let encoded =
        bincode::encode_to_vec(state, bincode::config::standard()).map_err(|e| e.to_string())?;
    bytes.extend_from_slice(&encoded);
    Ok(bytes)
}

pub fn save_state_to_file<T: bincode::Encode>(
    magic: &[u8; 4],
    state: &T,
    path: &Path,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = save_state_bytes(magic, state)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

pub fn load_state_bytes<T: bincode::Decode<()>>(
    magic: &[u8; 4],
    system_label: &str,
    bytes: &[u8],
) -> Result<T, String> {
    if bytes.len() < magic.len() || bytes[..magic.len()] != magic[..] {
        return Err(format!("invalid {system_label} state file header"));
    }
    let (state, _): (T, usize) =
        bincode::decode_from_slice(&bytes[magic.len()..], bincode::config::standard())
            .map_err(|e| e.to_string())?;
    Ok(state)
}

pub fn load_state_from_file<T: bincode::Decode<()>>(
    magic: &[u8; 4],
    system_label: &str,
    path: &Path,
) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    load_state_bytes(magic, system_label, &bytes)
}
