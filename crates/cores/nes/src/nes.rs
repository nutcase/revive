use std::sync::Arc;

use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;
use crate::{sram, AudioDiagFull, Result, SpscRingBuffer};

/// NES runtime facade used by CLI, UI, and tests.
///
/// This type owns the CPU, bus, PPU, APU, cartridge, and save-state context.
/// Front-ends advance emulation with [`Nes::step`] and exchange video, audio,
/// controller, SRAM, and save-state data through this facade.
pub struct Nes {
    pub(crate) cpu: Cpu,
    pub(crate) bus: Bus,
    pub(crate) current_rom_path: Option<String>,
}

impl Default for Nes {
    fn default() -> Self {
        Self::new()
    }
}

impl Nes {
    /// Create an empty emulator instance.
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            bus: Bus::new(),
            current_rom_path: None,
        }
    }

    /// Load an iNES/NES 2.0 ROM and reset the CPU.
    pub fn load_rom(&mut self, path: &str) -> Result<()> {
        let mut cartridge = Cartridge::load(path)?;

        if cartridge.has_battery_save() {
            if let Ok(Some(sram_data)) = sram::load_sram(path) {
                cartridge.set_sram_data(sram_data);
            }
        }

        self.bus.load_cartridge(cartridge);
        self.cpu.reset(&mut self.bus);
        self.current_rom_path = Some(path.to_string());
        Ok(())
    }

    /// Persist battery-backed SRAM next to the loaded ROM when available.
    pub fn save_sram(&self) -> Result<()> {
        if let Some(ref rom_path) = self.current_rom_path {
            if let Some(sram_data) = self.bus.get_sram_data() {
                sram::save_sram(rom_path, &sram_data)?;
                log::info!("SRAM saved successfully");
            }
        }
        Ok(())
    }

    fn run_single_cpu_cycle(&mut self) -> bool {
        self.bus.step_cpu_cycle()
    }

    fn run_cpu_time(&mut self, cycles: u32) -> bool {
        let mut nmi_triggered = false;

        for _ in 0..cycles {
            if self.run_single_cpu_cycle() {
                nmi_triggered = true;
            }

            let mut stall_cycles = self.bus.take_dmc_stall_cycles();
            while stall_cycles > 0 {
                if self.run_single_cpu_cycle() {
                    nmi_triggered = true;
                }
                stall_cycles -= 1;
                stall_cycles += self.bus.take_dmc_stall_cycles();
            }
        }

        nmi_triggered
    }

    /// Execute until the CPU has advanced one instruction or DMA cycle.
    ///
    /// Returns `true` when a video frame has completed during the step.
    pub fn step(&mut self) -> bool {
        let cpu_cycles = if self.bus.is_dma_in_progress() {
            self.bus.step_dma();
            1
        } else {
            let cycles = self.cpu.step(&mut self.bus);

            if cycles == 0 {
                return false;
            }

            cycles as u32
        };

        let nmi_triggered = self.run_cpu_time(cpu_cycles);

        if nmi_triggered {
            let nmi_cycles = self.cpu.nmi(&mut self.bus) as u32;
            self.run_cpu_time(nmi_cycles);
        }

        // cpu.irq() is silently ignored if I flag is set (returns 0 cycles).
        if self.bus.apu_irq_pending() {
            let irq_cycles = self.cpu.irq(&mut self.bus) as u32;
            if irq_cycles > 0 {
                self.run_cpu_time(irq_cycles);
            }
        }

        if self.bus.mapper_irq_pending() {
            let irq_cycles = self.cpu.irq(&mut self.bus) as u32;
            if irq_cycles > 0 {
                self.run_cpu_time(irq_cycles);
            }
        }

        self.bus.ppu_frame_complete()
    }

    /// Return the current RGB frame buffer, 256x240x3 bytes.
    pub fn get_frame_buffer(&self) -> &[u8] {
        self.bus.get_ppu_buffer()
    }

    /// Attach a ring buffer so the APU pushes samples directly as they
    /// are generated (no batching, no intermediate Vec).
    pub fn set_audio_ring(&mut self, ring: Arc<SpscRingBuffer>) {
        self.bus.set_audio_ring(ring);
    }

    /// Drain generated audio samples from the fallback Vec buffer.
    pub fn get_audio_buffer(&mut self) -> Vec<f32> {
        self.bus.get_audio_buffer()
    }

    /// Return detailed APU diagnostics for debugging.
    pub fn audio_diag_full(&self) -> AudioDiagFull {
        self.bus.audio_diag_full()
    }

    /// Push accumulated audio samples directly into the ring buffer,
    /// avoiding intermediate Vec allocation.
    pub fn drain_audio_to_ring(&mut self, ring: &SpscRingBuffer) {
        self.bus.drain_audio_to_ring(ring);
    }

    /// Set controller 1 state as an NES button bitmask.
    pub fn set_controller(&mut self, controller: u8) {
        self.bus.set_controller(controller);
    }

    /// Set controller 2 state as an NES button bitmask.
    pub fn set_controller2(&mut self, controller: u8) {
        self.bus.set_controller2(controller);
    }

    /// Return the last latched controller 1 state.
    pub fn get_controller(&self) -> u8 {
        self.bus.controller
    }

    /// Return the last latched controller 2 state.
    pub fn get_controller2(&self) -> u8 {
        self.bus.controller2
    }

    /// Direct reference to CPU RAM (2KB).
    pub fn ram(&self) -> &[u8] {
        self.bus.ram_ref()
    }

    /// Mutable reference to CPU RAM (2KB).
    pub fn ram_mut(&mut self) -> &mut [u8] {
        self.bus.ram_mut()
    }

    /// Direct reference to PRG-RAM / SRAM (mapper-dependent, may be None).
    pub fn prg_ram(&self) -> Option<&[u8]> {
        self.bus.prg_ram_ref()
    }

    /// Mutable reference to PRG-RAM / SRAM.
    pub fn prg_ram_mut(&mut self) -> Option<&mut [u8]> {
        self.bus.prg_ram_mut()
    }
}
