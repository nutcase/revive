use super::Bus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusTimingState {
    pub dma_cycles: u32,
    pub dma_in_progress: bool,
    pub dmc_stall_cycles: u32,
    pub ppu_frame_complete: bool,
}

impl Bus {
    fn service_dmc_sample(&mut self) {
        if let Some((addr, stall_cycles)) = self.apu.pull_dmc_sample_request() {
            let data = self.read_dmc_sample(addr);
            self.apu.push_dmc_sample(data);
            self.dmc_stall_cycles += stall_cycles as u32;
        }
    }

    #[inline]
    pub fn step_ppu(&mut self) -> bool {
        let nmi = self.ppu.step(self.cartridge.as_ref());
        if self.ppu.mapper_irq_clock {
            self.ppu.mapper_irq_clock = false;
            if let Some(ref mut cartridge) = self.cartridge {
                cartridge.clock_irq_counter();
            }
        }
        nmi
    }

    #[inline]
    pub fn step_cpu_cycle(&mut self) -> bool {
        let mut nmi_triggered = false;

        for _ in 0..3 {
            if self.step_ppu() {
                nmi_triggered = true;
            }
        }
        self.clock_mapper_irq_cycles(1);
        self.step_apu();

        nmi_triggered
    }

    pub fn step_apu(&mut self) {
        let exp = if let Some(ref mut cartridge) = self.cartridge {
            cartridge.clock_expansion_audio()
        } else {
            0.0
        };
        self.apu.set_expansion_audio(exp);
        self.service_dmc_sample();
        self.apu.step();
    }

    pub fn ppu_frame_complete(&mut self) -> bool {
        let complete = self.ppu.frame_complete;
        if complete {
            self.ppu.frame_complete = false;
        }
        complete
    }

    // Check if APU frame IRQ is pending
    pub fn apu_irq_pending(&self) -> bool {
        self.apu.irq_pending()
    }

    pub fn mapper_irq_pending(&self) -> bool {
        if let Some(ref cartridge) = self.cartridge {
            cartridge.irq_pending()
        } else {
            false
        }
    }

    pub fn clock_mapper_irq_cycles(&mut self, cycles: u32) {
        if let Some(ref mut cartridge) = self.cartridge {
            cartridge.clock_irq_counter_cycles(cycles);
        }
    }

    pub fn is_dma_in_progress(&self) -> bool {
        self.dma_in_progress
    }

    pub fn step_dma(&mut self) -> bool {
        if self.dma_in_progress && self.dma_cycles > 0 {
            self.dma_cycles -= 1;
            if self.dma_cycles == 0 {
                self.dma_in_progress = false;
                return true; // DMA completed this cycle
            }
        }
        false
    }

    pub fn take_dmc_stall_cycles(&mut self) -> u32 {
        std::mem::take(&mut self.dmc_stall_cycles)
    }

    pub fn timing_state(&self) -> BusTimingState {
        BusTimingState {
            dma_cycles: self.dma_cycles,
            dma_in_progress: self.dma_in_progress,
            dmc_stall_cycles: self.dmc_stall_cycles,
            ppu_frame_complete: self.ppu.frame_complete,
        }
    }

    pub fn restore_timing_state(&mut self, state: BusTimingState) {
        self.dma_cycles = state.dma_cycles;
        self.dma_in_progress = state.dma_in_progress;
        self.dmc_stall_cycles = state.dmc_stall_cycles;
        self.ppu.frame_complete = state.ppu_frame_complete;
    }

    fn read_dmc_sample(&self, addr: u16) -> u8 {
        match addr {
            0x8000..=0xFFFF => {
                if let Some(ref cartridge) = self.cartridge {
                    cartridge.read_prg(addr)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }
}
