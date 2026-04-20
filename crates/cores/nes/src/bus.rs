mod controller;
mod cpu_map;
mod state;
mod timing;

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::memory::Memory;
use crate::ppu::Ppu;

pub use state::BusFlatState;
pub use timing::BusTimingState;

pub struct Bus {
    memory: Memory,
    ppu: Ppu,
    apu: Apu,
    cartridge: Option<Cartridge>,
    pub controller: u8,
    pub controller2: u8,
    controller_state: [u16; 2],
    strobe: bool,          // Controller strobe mode
    dma_cycles: u32,       // Cycles to add due to DMA operations
    dma_in_progress: bool, // Flag to indicate DMA is in progress
    dmc_stall_cycles: u32,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    pub fn new() -> Self {
        Bus {
            memory: Memory::new(),
            ppu: Ppu::new(),
            apu: Apu::new(),
            cartridge: None,
            controller: 0,
            controller2: 0,
            controller_state: [0; 2],
            strobe: false,
            dma_cycles: 0,
            dma_in_progress: false,
            dmc_stall_cycles: 0,
        }
    }

    pub fn load_cartridge(&mut self, cartridge: Cartridge) {
        self.cartridge = Some(cartridge);
    }

    pub fn get_ppu_buffer(&self) -> &[u8] {
        self.ppu.get_buffer()
    }

    pub fn set_audio_ring(&mut self, ring: std::sync::Arc<crate::SpscRingBuffer>) {
        self.apu.set_audio_ring(ring);
    }

    pub fn get_audio_buffer(&mut self) -> Vec<f32> {
        self.apu.get_audio_buffer()
    }

    pub fn drain_audio_to_ring(&mut self, ring: &crate::SpscRingBuffer) {
        self.apu.drain_to_ring(ring);
    }

    pub fn audio_diag_full(&self) -> crate::AudioDiagFull {
        self.apu.audio_diag_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dmc_sample_fetch_schedules_cpu_stall_cycles() {
        let mut bus = Bus::new();
        bus.apu.write_register(0x4010, 0x0F);
        bus.apu.write_register(0x4012, 0x00);
        bus.apu.write_register(0x4013, 0x00);
        bus.apu.write_register(0x4015, 0x10);

        bus.step_apu();
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
        bus.step_apu();
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
        bus.step_apu();

        assert_eq!(bus.take_dmc_stall_cycles(), 3);
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
    }

    #[test]
    fn step_cpu_cycle_services_dmc_sample_requests() {
        let mut bus = Bus::new();
        bus.apu.write_register(0x4010, 0x0F);
        bus.apu.write_register(0x4012, 0x00);
        bus.apu.write_register(0x4013, 0x00);
        bus.apu.write_register(0x4015, 0x10);

        bus.step_cpu_cycle();
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
        bus.step_cpu_cycle();
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
        bus.step_cpu_cycle();

        assert_eq!(bus.take_dmc_stall_cycles(), 3);
        assert_eq!(bus.take_dmc_stall_cycles(), 0);
    }

    #[test]
    fn restore_timing_state_restores_dma_and_frame_flags() {
        let mut bus = Bus::new();
        bus.restore_timing_state(BusTimingState {
            dma_cycles: 7,
            dma_in_progress: true,
            dmc_stall_cycles: 2,
            ppu_frame_complete: true,
        });

        assert!(bus.is_dma_in_progress());
        assert_eq!(bus.take_dmc_stall_cycles(), 2);
        assert!(bus.ppu_frame_complete());
        assert!(!bus.ppu_frame_complete());

        assert!(!bus.step_dma());
        let timing = bus.timing_state();
        assert_eq!(timing.dma_cycles, 6);
        assert!(timing.dma_in_progress);
        assert_eq!(timing.dmc_stall_cycles, 0);
        assert!(!timing.ppu_frame_complete);
    }
}
