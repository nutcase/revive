use super::{Emulator, CPU_CLOCK_DIVIDER, PPU_CLOCK_DIVIDER};
use std::time::Instant;

impl Emulator {
    pub(super) fn run_sa1_cycles_with_dma(&mut self, scpu_cycles: u32, perf_verbose: bool) {
        if scpu_cycles == 0 {
            return;
        }

        let sa1_start = if perf_verbose {
            Some(Instant::now())
        } else {
            None
        };

        let mut remaining = scpu_cycles;
        let mut pending_dma = self.bus.sa1_dma_pending();
        // In fast mode, avoid per-chunk DMA processing unless already pending.
        let process_each_chunk = !self.fast_mode || pending_dma;

        while remaining > 0 {
            let chunk = remaining.min(u8::MAX as u32) as u8;
            self.bus.run_sa1_scheduler(chunk);
            if process_each_chunk {
                if self.bus.sa1_dma_pending() {
                    self.bus.process_sa1_dma();
                }
            } else if !pending_dma && self.bus.sa1_dma_pending() {
                pending_dma = true;
            }
            remaining -= chunk as u32;
        }

        if !process_each_chunk && pending_dma {
            self.bus.process_sa1_dma();
        }

        if let Some(start) = sa1_start {
            self.performance_stats.add_sa1_time(start.elapsed());
        }
    }

    pub(super) fn step_superfx_for_master_cycles(&mut self, master_cycles: u64) {
        if master_cycles == 0 || !self.bus.is_superfx_active() {
            return;
        }

        let total = master_cycles.saturating_add(self.superfx_master_cycle_accum as u64);
        let mut superfx_cpu_cycles = total / CPU_CLOCK_DIVIDER;
        self.superfx_master_cycle_accum = (total % CPU_CLOCK_DIVIDER) as u8;

        while superfx_cpu_cycles > 0 {
            if self.superfx_save_state_hit_pending() {
                break;
            }
            let chunk = superfx_cpu_cycles.min(u8::MAX as u64) as u8;
            self.bus.tick_superfx_cpu_cycles(chunk);
            if self.superfx_save_state_hit_pending() {
                break;
            }
            superfx_cpu_cycles -= chunk as u64;
        }
    }

    /// Advance emulated time without executing any S-CPU instructions.
    ///
    /// Used to model stalls such as general DMA (MDMA), where the S-CPU is halted while
    /// the PPU/APU (and SA-1) continue to run.
    pub(super) fn advance_time_without_cpu(&mut self, master_cycles: u64) {
        if master_cycles == 0 {
            return;
        }

        // Step SA-1 scheduler (if present) during the stall.
        // Use S-CPU cycle equivalents as a rough proxy for elapsed time.
        let total = master_cycles.saturating_add(self.sa1_master_cycle_accum as u64);
        let sa1_scpu_equiv = total / CPU_CLOCK_DIVIDER;
        self.sa1_master_cycle_accum = (total % CPU_CLOCK_DIVIDER) as u8;
        if self.bus.is_sa1_active() {
            self.run_sa1_cycles_with_dma(sa1_scpu_equiv as u32, false);
        }

        // SuperFX is also an independent coprocessor and must continue to run while
        // master time advances during CPU stalls/catchup windows.
        self.step_superfx_for_master_cycles(master_cycles);
        if self.superfx_save_state_hit_pending() {
            return;
        }

        // Step PPU: PPU clock is master/4.
        //
        // IMPORTANT: Preserve master->PPU fractional remainder across CPU and stall paths.
        // Otherwise, repeated small stalls (e.g., slow memory extra master cycles) will
        // systematically drop remainder and cause video timing drift (tearing in dumps).
        let master_with_rem = master_cycles.saturating_add(self.ppu_cycle_accum as u64);
        let mut ppu_cycles = master_with_rem / PPU_CLOCK_DIVIDER;
        self.ppu_cycle_accum = (master_with_rem % PPU_CLOCK_DIVIDER) as u8;
        while ppu_cycles > 0 {
            let chunk = ppu_cycles.min(u16::MAX as u64) as u16;
            self.step_ppu(chunk, false);
            ppu_cycles -= chunk as u64;
        }

        // Step APU for the same elapsed master time.
        {
            let total = master_cycles.saturating_add(self.apu_master_cycle_accum as u64);
            let apu_cpu_cycles = (total / CPU_CLOCK_DIVIDER) as u32;
            self.apu_master_cycle_accum = (total % CPU_CLOCK_DIVIDER) as u8;
            // During stalls (DMA etc.), accumulate via add_cpu_cycles and flush immediately.
            let batch = self.apu_step_batch;
            self.bus.with_apu_mut(|apu| {
                apu.add_cpu_cycles(apu_cpu_cycles);
                if apu.pending_cpu_cycles() >= batch {
                    apu.sync();
                }
            });
        }

        self.master_cycles = self.master_cycles.saturating_add(master_cycles);
    }

    pub(super) fn step_apu_debt(&mut self, _force: bool) {
        // Legacy path: flush any remaining apu_cycle_debt (used by advance_time_without_cpu).
        let debt = self.apu_cycle_debt;
        if debt == 0 {
            // Also flush pending cycles in the APU itself.
            self.bus.with_apu_mut(|apu| apu.sync());
            return;
        }

        let step_fn = |apu: &mut crate::audio::apu::Apu| {
            // First flush any cycles accumulated via add_cpu_cycles().
            apu.sync();
            // Then step for the debt from advance_time_without_cpu path.
            let mut remaining = debt;
            while remaining > 0 {
                let chunk = remaining.min(u8::MAX as u32) as u8;
                apu.step(chunk);
                remaining -= chunk as u32;
            }
        };

        self.bus.with_apu_mut(step_fn);
        self.apu_cycle_debt = 0;
    }
}
