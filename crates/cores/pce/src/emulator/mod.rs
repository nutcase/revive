mod bios_font;
mod hucard;

#[cfg(test)]
mod tests;

use crate::bus::{Bus, CompatBusStateV1, IRQ_REQUEST_TIMER};
use crate::cpu::Cpu;
use crate::debugger::{DebugBreak, DebugTick, Debugger};
use hucard::{ParsedHuCard, RESET_VECTOR_LEGACY, RESET_VECTOR_PRIMARY, large_hucard_mapper_size};
use std::error::Error;

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub struct Emulator {
    pub cpu: Cpu,
    pub bus: Bus,
    cycles: u64,
    audio_buffer: Vec<i16>,
    audio_batch_size: usize,
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
struct CompatEmulatorStateV1 {
    cpu: Cpu,
    bus: CompatBusStateV1,
    cycles: u64,
    audio_buffer: Vec<i16>,
    audio_batch_size: usize,
}

impl Emulator {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(),
            cycles: 0,
            audio_buffer: Vec::new(),
            audio_batch_size: 1024,
        }
    }

    /// Load a program into memory and wire the reset vector to it.
    pub fn load_program(&mut self, start: u16, data: &[u8]) {
        self.bus.load(start, data);
        let lo = (start & 0x00FF) as u8;
        let hi = (start >> 8) as u8;
        // Prefer HuC6280 reset vector ($FFFE) while keeping the legacy slot
        // populated for older tests and tooling that still read $FFFC.
        self.bus.write(RESET_VECTOR_PRIMARY, lo);
        self.bus.write(RESET_VECTOR_PRIMARY + 1, hi);
        self.bus.write(RESET_VECTOR_LEGACY, lo);
        self.bus.write(RESET_VECTOR_LEGACY + 1, hi);
    }

    /// Load a HuCard `.pce` image, handling optional 512-byte headers and
    /// mapping the upper MPR banks so the reset vector points into ROM.
    pub fn load_hucard(&mut self, image: &[u8]) -> Result<(), Box<dyn Error>> {
        let parsed = ParsedHuCard::from_bytes(image)?;
        let ParsedHuCard { mut rom, header } = parsed;
        self.bus = Bus::new();
        self.audio_buffer.clear();
        let backup_bytes = header
            .as_ref()
            .map(|descriptor| descriptor.backup_ram_bytes())
            .unwrap_or(0);
        debug_assert!(
            header.is_none() || backup_bytes == header.as_ref().unwrap().backup_ram_bytes()
        );
        self.bus.configure_cart_ram(backup_bytes);
        let large_hucard_mapper = large_hucard_mapper_size(&rom);
        if let Some(mapped_size) = large_hucard_mapper {
            rom.resize(mapped_size, 0xFF);
        }
        self.bus.load_rom_image(rom);
        if large_hucard_mapper.is_some() {
            self.bus.enable_large_hucard_mapper();
        }

        let pages = self.bus.rom_page_count();
        if pages == 0 {
            return Err("HuCard contains no ROM banks".into());
        }

        let mut mapped = false;
        if let Some(ref descriptor) = header {
            if let Some(layout) = descriptor.recommended_layout(pages) {
                mapped = self.apply_header_layout(&layout, descriptor);
            }
        }

        if !mapped {
            self.map_boot_window(pages);
        }

        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.bus);
        self.seed_cpu_stack();
        self.load_bios_font();
        self.cycles = 0;
    }

    pub fn tick(&mut self) -> u32 {
        let cycles = self.cpu.step(&mut self.bus);
        #[cfg(feature = "trace_hw_writes")]
        self.bus.set_last_pc_for_trace(self.cpu.pc);
        let mut bus_cycles = cycles;
        if cycles == 0 && self.cpu.is_waiting() {
            bus_cycles = 1;
        }
        if cycles > 0 {
            self.cycles += cycles as u64;
        } else if self.cpu.is_waiting() {
            self.cycles += 1;
        }
        self.bus.tick(bus_cycles, self.cpu.clock_high_speed);
        self.bus.drain_audio_samples_into(&mut self.audio_buffer);
        cycles
    }

    pub fn tick_debugger(&mut self, debugger: &mut Debugger) -> DebugTick {
        if debugger.paused {
            return DebugTick::Paused;
        }

        let pc = self.cpu.pc;
        if debugger.breakpoints.contains(&pc) {
            debugger.paused = true;
            let br = DebugBreak::Breakpoint(pc);
            debugger.last_break = Some(br);
            return DebugTick::Break(br);
        }

        let cycles = self.tick();
        if debugger.step_pending() {
            debugger.clear_step_pending();
            debugger.paused = true;
            let br = DebugBreak::Step(self.cpu.pc);
            debugger.last_break = Some(br);
            DebugTick::Break(br)
        } else {
            DebugTick::Ran(cycles)
        }
    }

    pub fn request_irq(&mut self) {
        self.bus.raise_irq(IRQ_REQUEST_TIMER);
    }

    pub fn request_nmi(&mut self) {
        self.cpu.request_nmi();
    }

    /// Run until BRK is encountered or until the optional cycle limit is hit.
    pub fn run_until_halt(&mut self, cycle_budget: Option<u64>) {
        while !self.cpu.halted {
            let cycles = self.tick() as u64;
            if let Some(budget) = cycle_budget {
                if self.cycles >= budget {
                    break;
                }
                if cycles == 0 && !self.cpu.is_waiting() {
                    break;
                }
            }
        }
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    pub fn set_audio_batch_size(&mut self, samples: usize) {
        self.audio_batch_size = samples.max(1);
    }

    pub fn set_video_output_enabled(&mut self, enabled: bool) {
        self.bus.set_video_output_enabled(enabled);
    }

    pub fn save_state_to_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let bytes = bincode::encode_to_vec(self, bincode::config::standard())?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load_state_from_file<P: AsRef<std::path::Path>>(
        &mut self,
        path: P,
    ) -> Result<(), Box<dyn Error>> {
        let bytes = std::fs::read(path)?;
        let config = bincode::config::standard();
        if let Ok((state, used)) = bincode::decode_from_slice::<Emulator, _>(&bytes, config) {
            if used == bytes.len() {
                self.adopt_loaded_state(state);
                return Ok(());
            }
        }

        if let Ok((state, used)) =
            bincode::decode_from_slice::<CompatEmulatorStateV1, _>(&bytes, config)
        {
            if used == bytes.len() {
                self.adopt_loaded_state(state.into());
                return Ok(());
            }
        }

        let (state, _): (Emulator, usize) = match bincode::decode_from_slice(&bytes, config) {
            Ok(decoded) => decoded,
            Err(bincode::error::DecodeError::UnexpectedEnd { additional }) => {
                // Backward-compatibility path for older state files that are
                // short by a few bytes after struct layout changes.
                let mut padded = bytes.clone();
                let extra = additional.saturating_add(64);
                padded.resize(padded.len().saturating_add(extra), 0);
                bincode::decode_from_slice(&padded, config)?
            }
            Err(err) => return Err(Box::new(err)),
        };
        self.adopt_loaded_state(state);
        Ok(())
    }

    pub fn take_audio_samples(&mut self) -> Option<Vec<i16>> {
        if self.audio_buffer.len() < self.audio_batch_size {
            return None;
        }
        let tail = self.audio_buffer.split_off(self.audio_batch_size);
        Some(std::mem::replace(&mut self.audio_buffer, tail))
    }

    pub fn drain_audio_samples(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio_buffer)
    }

    pub fn drain_audio_samples_into(&mut self, out: &mut Vec<i16>) {
        out.clear();
        out.extend_from_slice(&self.audio_buffer);
        self.audio_buffer.clear();
    }

    pub fn pending_audio_samples(&self) -> usize {
        self.audio_buffer.len()
    }

    /// Copy the current frame into `buf`, reusing its allocation.
    /// Returns `true` if a frame was ready.
    pub fn take_frame_into(&mut self, buf: &mut Vec<u32>) -> bool {
        self.bus.take_frame_into(buf)
    }

    pub fn take_frame(&mut self) -> Option<Vec<u32>> {
        self.bus.take_frame()
    }

    pub fn framebuffer(&self) -> &[u32] {
        self.bus.framebuffer()
    }

    pub fn display_width(&self) -> usize {
        self.bus.display_width()
    }

    pub fn display_height(&self) -> usize {
        self.bus.display_height()
    }

    pub fn display_y_offset(&self) -> usize {
        self.bus.display_y_offset()
    }

    pub fn backup_ram(&self) -> Option<&[u8]> {
        self.bus.cart_ram()
    }

    pub fn backup_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.bus.cart_ram_mut()
    }

    pub fn load_backup_ram(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        self.bus
            .load_cart_ram(data)
            .map_err(|err| Box::<dyn Error>::from(err.to_string()))?;
        Ok(())
    }

    pub fn save_backup_ram(&self) -> Option<Vec<u8>> {
        self.bus.cart_ram().map(|ram| ram.to_vec())
    }

    pub fn bram(&self) -> &[u8] {
        self.bus.bram()
    }

    pub fn bram_mut(&mut self) -> &mut [u8] {
        self.bus.bram_mut()
    }

    pub fn load_bram(&mut self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        self.bus
            .load_bram(data)
            .map_err(|err| Box::<dyn Error>::from(err.to_string()))?;
        Ok(())
    }

    pub fn save_bram(&self) -> Vec<u8> {
        self.bus.bram().to_vec()
    }

    pub fn work_ram(&self) -> &[u8] {
        self.bus.work_ram()
    }

    pub fn work_ram_mut(&mut self) -> &mut [u8] {
        self.bus.work_ram_mut()
    }

    fn adopt_loaded_state(&mut self, mut state: Emulator) {
        state.bus.rebuild_mpr_mappings();
        state.bus.post_load_fixup();
        state.audio_batch_size = self.audio_batch_size;
        state.audio_buffer.clear();
        let _ = state.bus.take_audio_samples();
        *self = state;
    }
}

impl From<CompatEmulatorStateV1> for Emulator {
    fn from(value: CompatEmulatorStateV1) -> Self {
        Self {
            cpu: value.cpu,
            bus: value.bus.into(),
            cycles: value.cycles,
            audio_buffer: value.audio_buffer,
            audio_batch_size: value.audio_batch_size,
        }
    }
}
