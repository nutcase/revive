use super::runtime_config::FrameRunConfig;
use super::{Emulator, CPU_CLOCK_DIVIDER, PPU_CLOCK_DIVIDER};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static LAST_PCFFFF_GOOD_PC: AtomicU32 = AtomicU32::new(0);

#[derive(Default)]
pub(super) struct FrameRunTimings {
    pub(super) sync: Duration,
    pub(super) main_loop: Duration,
    pub(super) cpu_exec: Duration,
    pub(super) ppu_step: Duration,
    pub(super) apu_inline: Duration,
    pub(super) catchup: Duration,
    pub(super) audio: Duration,
}

pub(super) enum CpuInstructionSlice {
    Continue {
        cpu_cycles: u16,
        extra_master: u64,
        cpu_time: Duration,
    },
    StopFrame,
}

impl Emulator {
    pub(super) fn apply_frame_start_debug_controls(&mut self, frame_count: u32) {
        let nmi_guard_frames: u32 = std::env::var("NMI_GUARD_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if nmi_guard_frames > 0 {
            if frame_count <= nmi_guard_frames {
                self.bus.get_ppu_mut().nmi_enabled = false;
                let _ = self.bus.read_u8(0x4210);
            } else if frame_count == nmi_guard_frames + 1 {
                let nmi_en = (self.bus.nmitimen() & 0x80) != 0;
                self.bus.get_ppu_mut().nmi_enabled = nmi_en;
                let _ = self.bus.read_u8(0x4210);
            }
        }

        if std::env::var_os("SHOW_PC").is_some()
            && (frame_count <= 16 || frame_count.is_multiple_of(30))
        {
            let cnt4800 = self
                .bus
                .spc7110
                .as_mut()
                .map_or(0, |s| s.debug_drain_4800_count());
            println!(
                "[pc] frame={} S-CPU PC=${:02X}:{:04X} P=0x{:02X} I={} r4800={}",
                frame_count,
                self.cpu.pb(),
                self.cpu.pc(),
                self.cpu.p().bits(),
                (self.cpu.p().bits() & 0x04) != 0,
                cnt4800,
            );
        }

        if std::env::var_os("DEBUG_CPU_FLAGS").is_some() && frame_count <= 8 {
            println!(
                "[cpu-flags] frame={} PC={:02X}:{:04X} P=0x{:02X} I={}",
                frame_count,
                self.cpu.pb(),
                self.cpu.pc(),
                self.cpu.p().bits(),
                (self.cpu.p().bits() & 0x04) != 0
            );
        }

        if let Some(n) = std::env::var("FORCE_CLI_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            if frame_count <= n {
                self.cpu
                    .p_mut()
                    .remove(crate::cpu::StatusFlags::IRQ_DISABLE);
                if std::env::var_os("DEBUG_CPU_FLAGS").is_some() {
                    println!(
                        "[cpu-flags] forced CLI at frame={} PC={:02X}:{:04X}",
                        frame_count,
                        self.cpu.pb(),
                        self.cpu.pc()
                    );
                }
            }
        }

        if frame_count.is_multiple_of(5) && frame_count > 2 && crate::debug_flags::render_verbose()
        {
            println!("Frame progress: {}/10", frame_count);
        }
    }

    pub(super) fn apply_frame_start_irq_controls(&mut self, frame_count: u32) {
        if crate::debug_flags::sa1_force_irq_each_frame() && self.bus.is_sa1_active() {
            self.bus.sa1_mut().registers.interrupt_pending |=
                crate::cartridge::sa1::Sa1::IRQ_LINE_BIT;
        }
        if let Some(n) = crate::debug_flags::force_scpu_irq_frames() {
            if frame_count <= n {
                self.cpu.trigger_irq(&mut self.bus);
            }
        }
    }

    pub(super) fn maybe_trace_burnin_obj_check_context(&mut self, pc: u32) {
        if !crate::debug_flags::trace_burnin_obj_checks()
            || pc >> 16 != 0x00
            || !matches!(
                pc & 0xFFFF,
                0x9AC4 | 0x9AEC | 0x9B61 | 0x9B8E | 0x9BD0 | 0x9BD8
            )
        {
            return;
        }

        let ppu_frame = self.bus.get_ppu().get_frame();
        let ppu_sl = self.bus.get_ppu().scanline;
        let ppu_cyc = self.bus.get_ppu().get_cycle();
        let ppu_vblank = self.bus.get_ppu().is_vblank() as u8;
        let hvbjoy = self.bus.read_u8(0x4212);
        let stat77 = self.bus.read_u8(0x213E);
        println!(
            "[BURNIN-OBJ-CHECK-CTX] PC=00:{:04X} frame={} sl={} cyc={} vblank={} hvbjoy={:02X} stat77={:02X}",
            pc & 0xFFFF,
            ppu_frame,
            ppu_sl,
            ppu_cyc,
            ppu_vblank,
            hvbjoy,
            stat77
        );
    }

    pub(super) fn maybe_print_watch_addr(&mut self, frame_count: u32) {
        let Some(watch) = crate::debug_flags::watch_addr() else {
            return;
        };

        let wbank = (watch >> 16) as u8;
        let woff = (watch & 0xFFFF) as u16;
        let val = self.bus.read_u8(((wbank as u32) << 16) | woff as u32);
        println!(
            "[watch] frame={} addr={:02X}:{:04X} val={:02X} PC={:02X}:{:04X}",
            frame_count,
            wbank,
            woff,
            val,
            self.cpu.pb(),
            self.cpu.pc()
        );
    }

    pub(super) fn apply_smw_bootstrap_guard(&mut self, enabled: bool) {
        if !enabled {
            return;
        }

        let addr = 0x00BBAAusize;
        for bank in &[0x7E0000u32, 0x7F0000u32] {
            self.bus.write_u8(bank + addr as u32, 0xAA);
            self.bus.write_u8(bank + addr as u32 + 1, 0xBB);
        }
    }

    #[inline]
    pub(super) fn run_cpu_instruction_slice(
        &mut self,
        frame_count: u32,
        loop_iterations: u64,
        cycles_per_frame: u64,
        start_cycles: u64,
        config: &FrameRunConfig,
        stall_trace: &mut super::runtime_config::FrameStallTraceState,
    ) -> CpuInstructionSlice {
        let pc = self.cpu.get_pc();

        self.maybe_trace_pc_ffff(frame_count, pc, config.trace_pc_ffff);
        if self.debugger.check_breakpoint(pc) {
            return CpuInstructionSlice::StopFrame;
        }

        self.maybe_trace_stall(frame_count, pc, config.stall_threshold, stall_trace);
        self.maybe_trace_frame_pc(frame_count, pc, config.trace_pc_frame);
        self.maybe_trace_burnin_obj_check_context(pc);
        self.maybe_print_watch_addr(frame_count);

        if self.debugger.is_paused() && !self.debugger.should_step() {
            return CpuInstructionSlice::StopFrame;
        }

        self.apply_smw_bootstrap_guard(config.smw_force_bbaa);

        let remaining_cycles = cycles_per_frame - (self.master_cycles - start_cycles);
        let mut batch_cycles: u16 = if self.debugger.is_paused() {
            1
        } else {
            (remaining_cycles / CPU_CLOCK_DIVIDER)
                .min(config.batch_max as u64)
                .max(1) as u16
        };
        if batch_cycles == 0 {
            batch_cycles = 1;
        }

        let record_trace = config.trace_exec || self.debugger.is_paused();
        let need_opcode = record_trace || config.trace_loop_cycles || config.trace_pc_ffff_once;
        let opcode = if need_opcode { self.bus.read_u8(pc) } else { 0 };
        if record_trace {
            let operands = self.fetch_operands(pc, opcode);
            self.debugger
                .record_trace(&self.cpu, &self.bus, opcode, &operands);
        }

        let before_pc = pc;
        let allow_batch = config.batch_exec
            && batch_cycles > 8
            && !self.debugger.is_paused()
            && !Self::save_state_exact_capture_env_active();
        let cpu_start = Instant::now();
        let cpu_cycles: u16 = if config.perf_verbose {
            let cycles = if allow_batch {
                self.cpu.step_multiple(&mut self.bus, batch_cycles)
            } else {
                self.cpu.step(&mut self.bus) as u16
            };
            let cpu_time = cpu_start.elapsed();
            self.performance_stats.add_cpu_time(cpu_time);
            cycles
        } else if allow_batch {
            self.cpu.step_multiple(&mut self.bus, batch_cycles)
        } else {
            self.cpu.step(&mut self.bus) as u16
        };
        let cpu_time = cpu_start.elapsed();

        if config.trace_loop_cycles && loop_iterations < 20 {
            println!(
                "[loop] iter={} cpu_cycles={} master_cycles={} pc={:02X}:{:04X}",
                loop_iterations + 1,
                cpu_cycles,
                self.master_cycles,
                before_pc >> 16,
                before_pc & 0xFFFF
            );
        }

        let after_pc = self.cpu.get_pc();
        if config.trace_pc_ffff_once && before_pc != 0x00_FFFF && after_pc == 0x00_FFFF {
            let last_good = LAST_PCFFFF_GOOD_PC.swap(before_pc, Ordering::Relaxed);
            println!(
                "[PCFFFF-TRANS] frame={} from {:02X}:{:04X} opcode={:02X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} DB={:02X} DP={:04X} last_good={:02X}:{:04X}",
                frame_count,
                before_pc >> 16,
                before_pc & 0xFFFF,
                opcode,
                self.cpu.a(),
                self.cpu.x(),
                self.cpu.y(),
                self.cpu.sp(),
                self.cpu.p().bits(),
                self.cpu.db(),
                self.cpu.dp(),
                last_good >> 16,
                last_good & 0xFFFF
            );
        } else if config.trace_pc_ffff_once {
            LAST_PCFFFF_GOOD_PC.store(before_pc, Ordering::Relaxed);
        }

        if self.maybe_save_state_at_frame_anchor() {
            return CpuInstructionSlice::StopFrame;
        }

        let extra_master = self.bus.take_last_instr_extra_master();
        self.step_superfx_for_master_cycles(extra_master);
        if self.maybe_save_state_at_frame_anchor() {
            return CpuInstructionSlice::StopFrame;
        }

        CpuInstructionSlice::Continue {
            cpu_cycles,
            extra_master,
            cpu_time,
        }
    }

    pub(super) fn step_sa1_for_cpu_slice(&mut self, cpu_cycles: u16, config: &FrameRunConfig) {
        if !self.bus.is_sa1_active() {
            return;
        }

        if !self.fast_mode && self.sa1_batch_cpu <= 1 {
            self.run_sa1_cycles_with_dma(cpu_cycles as u32, config.perf_verbose);
            return;
        }

        let mut sa1_batch_cycles: u16 = 0;
        if self.sa1_batch_cpu > 1 {
            self.sa1_cycle_debt = self.sa1_cycle_debt.saturating_add(cpu_cycles);
            if self.sa1_cycle_debt >= self.sa1_batch_cpu {
                sa1_batch_cycles = self.sa1_cycle_debt;
                self.sa1_cycle_debt = 0;
            }
        } else {
            sa1_batch_cycles = cpu_cycles;
        }
        if sa1_batch_cycles > 0 {
            self.run_sa1_cycles_with_dma(sa1_batch_cycles as u32, config.perf_verbose);
        }
    }

    pub(super) fn step_ppu_for_cpu_slice(
        &mut self,
        cpu_cycles: u16,
        extra_master: u64,
        perf_verbose: bool,
    ) -> Duration {
        let master = (cpu_cycles as u64)
            .saturating_mul(CPU_CLOCK_DIVIDER)
            .saturating_add(extra_master)
            .saturating_add(self.ppu_cycle_accum as u64);
        let mut ppu_cycles = (master / PPU_CLOCK_DIVIDER) as u16;
        self.ppu_cycle_accum = (master % PPU_CLOCK_DIVIDER) as u8;
        if ppu_cycles == 0 {
            ppu_cycles = 1;
        }

        let ppu_start = Instant::now();
        self.step_ppu(ppu_cycles, true);
        let ppu_time = ppu_start.elapsed();
        if perf_verbose {
            self.performance_stats.add_ppu_time(ppu_time);
        }
        ppu_time
    }

    pub(super) fn step_apu_for_cpu_slice(
        &mut self,
        cpu_cycles: u16,
        extra_master: u64,
        perf_verbose: bool,
    ) -> Duration {
        let batch = self.apu_step_batch;
        let force = self.apu_step_force;
        let apu_em = extra_master.saturating_add(self.apu_master_cycle_accum as u64);
        let apu_extra_cpu = (apu_em / CPU_CLOCK_DIVIDER) as u32;
        self.apu_master_cycle_accum = (apu_em % CPU_CLOCK_DIVIDER) as u8;
        let inline_apu_cpu = self.bus.last_cpu_instr_apu_synced_bus_cycles as u32;
        let total_cpu = (cpu_cycles as u32)
            .saturating_sub(inline_apu_cpu)
            .saturating_add(apu_extra_cpu);
        let step_fn = |apu: &mut crate::audio::apu::Apu| {
            apu.add_cpu_cycles(total_cpu);
            let pending = apu.pending_cpu_cycles();
            if pending >= batch || pending >= force {
                apu.sync();
            }
        };

        let apu_start = Instant::now();
        self.bus.with_apu_mut(step_fn);
        let apu_time = apu_start.elapsed();
        if perf_verbose {
            self.performance_stats.add_apu_time(apu_time);
        }
        apu_time
    }

    pub(super) fn finish_frame_boundary_catchup(&mut self, start_ppu_frame: u64) -> Duration {
        let catchup_start = Instant::now();
        if self.bus.get_ppu().get_frame() == start_ppu_frame {
            let remaining_master = self.bus.get_ppu().remaining_master_cycles_in_frame();
            if remaining_master > 0 && remaining_master <= 341 * 4 {
                self.advance_time_without_cpu(remaining_master);
            }
        }

        if self.bus.is_superfx_active() {
            let remaining_master = self.bus.get_ppu().remaining_master_cycles_in_frame();
            if remaining_master > 0 && remaining_master <= 341 * 4 {
                self.advance_time_without_cpu(remaining_master);
            }
        }
        catchup_start.elapsed()
    }

    pub(super) fn flush_end_of_frame_sa1(&mut self, config: &FrameRunConfig) {
        if !self.bus.is_sa1_active() || self.sa1_cycle_debt == 0 {
            return;
        }

        let pending = self.sa1_cycle_debt;
        self.sa1_cycle_debt = 0;
        let sa1_start = if config.perf_verbose {
            Some(Instant::now())
        } else {
            None
        };
        let mut remaining = pending as u32;
        while remaining > 0 {
            let chunk = remaining.min(u8::MAX as u32) as u8;
            self.bus.run_sa1_scheduler(chunk);
            self.bus.process_sa1_dma();
            remaining -= chunk as u32;
        }
        if let Some(sa1_start) = sa1_start {
            self.performance_stats.add_sa1_time(sa1_start.elapsed());
        }
    }

    pub(super) fn maybe_run_sa1_frame_catchup(
        &mut self,
        cycles_per_frame: u64,
        config: &FrameRunConfig,
    ) {
        static FLAG: OnceLock<bool> = OnceLock::new();
        let sa1_catchup = *FLAG.get_or_init(|| {
            std::env::var("SA1_CATCHUP")
                .map(|v| !(v == "0" || v.to_lowercase() == "false"))
                .unwrap_or(false)
        });
        if !sa1_catchup || !self.bus.is_sa1_active() {
            return;
        }

        let expected_sa1_cycles = cycles_per_frame / 2;
        let actual_sa1_cycles = self.bus.take_sa1_cycle_accum();
        if actual_sa1_cycles >= expected_sa1_cycles {
            return;
        }

        let remaining = (expected_sa1_cycles - actual_sa1_cycles) as u32;
        if remaining == 0 {
            return;
        }

        let sa1_start = if config.perf_verbose {
            Some(Instant::now())
        } else {
            None
        };
        self.bus.run_sa1_cycles_direct(remaining);
        self.bus.process_sa1_dma();
        if let Some(sa1_start) = sa1_start {
            self.performance_stats.add_sa1_time(sa1_start.elapsed());
        }
    }

    pub(super) fn mix_frame_audio(&mut self) -> Duration {
        let audio_start = Instant::now();
        let emit_output = !self.suppress_next_audio_output;
        self.suppress_next_audio_output = false;
        self.step_apu_debt(true);

        if self.audio_system.is_enabled() {
            let audio_system = &mut self.audio_system;
            self.bus.with_apu_mut(|apu| {
                if emit_output {
                    audio_system.mix_frame_from_apu(apu);
                } else {
                    audio_system.drain_frame_from_apu(apu);
                }
            });
        }
        audio_start.elapsed()
    }

    pub(super) fn maybe_log_starfox_slow_frame(
        &mut self,
        enabled: bool,
        threshold_ms: u128,
        timings: &FrameRunTimings,
        total_time: Duration,
    ) {
        if !enabled || total_time.as_millis() < threshold_ms {
            return;
        }

        let cpu_read_bank_top = self
            .bus
            .top_cpu_read_banks(4)
            .into_iter()
            .map(|(bank, ns, count)| format!("{bank:02X}:{}ms/{}", ns / 1_000_000, count))
            .collect::<Vec<_>>()
            .join(",");
        let (cpu_step_pre_ns, cpu_step_core_ns, cpu_step_post_ns, cpu_step_count) =
            self.cpu.take_step_profile_ns();
        let (
            cpu_bus_read_ns,
            cpu_bus_write_ns,
            cpu_bus_cycle_ns,
            cpu_bus_tick_ns,
            cpu_bus_read_count,
            cpu_bus_write_count,
            cpu_bus_cycle_count,
            cpu_bus_tick_count,
        ) = self.bus.take_cpu_profile();
        eprintln!(
            "[STARFOX-RUNFRAME-SLOW] frame={} cpu_pc={:06X} inidisp={:02X} tm={:02X} mode={} sync_ms={} loop_ms={} cpu_ms={} cpu_pre_ms={} cpu_core_ms={} cpu_post_ms={} cpu_steps={} bus_read_ms={} bus_write_ms={} bus_cycle_ms={} bus_tick_ms={} bus_reads={} bus_writes={} bus_cycles={} bus_ticks={} read_banks=[{}] ppu_ms={} apu_inline_ms={} catchup_ms={} audio_ms={} total_ms={}",
            self.frame_count,
            self.current_cpu_pc(),
            self.current_inidisp(),
            self.current_tm(),
            self.current_bg_mode(),
            timings.sync.as_millis(),
            timings.main_loop.as_millis(),
            timings.cpu_exec.as_millis(),
            cpu_step_pre_ns / 1_000_000,
            cpu_step_core_ns / 1_000_000,
            cpu_step_post_ns / 1_000_000,
            cpu_step_count,
            cpu_bus_read_ns / 1_000_000,
            cpu_bus_write_ns / 1_000_000,
            cpu_bus_cycle_ns / 1_000_000,
            cpu_bus_tick_ns / 1_000_000,
            cpu_bus_read_count,
            cpu_bus_write_count,
            cpu_bus_cycle_count,
            cpu_bus_tick_count,
            cpu_read_bank_top,
            timings.ppu_step.as_millis(),
            timings.apu_inline.as_millis(),
            timings.catchup.as_millis(),
            timings.audio.as_millis(),
            total_time.as_millis(),
        );
    }
}
