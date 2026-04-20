#![allow(unreachable_patterns)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::audio::AudioSystem;
use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;
use crate::debugger::Debugger;
use crate::shutdown;

mod debug_capture;
mod debug_controls;
mod diagnostics;
mod display_compat;
mod frame_output;
mod frame_step;
mod headless;
mod input_flow;
mod perf;
mod render_output;
mod runner_api;
mod runtime_config;
mod save_state;
mod starfox_diag;
mod superfx_view;
mod time_advance;

pub(in crate::emulator) use frame_output::write_framebuffer_image;
#[cfg(test)]
pub(crate) use frame_output::write_framebuffer_png;
use frame_step::{CpuInstructionSlice, FrameRunTimings};
use headless::{HeadlessDiagnosticsConfig, HeadlessDumpConfig};
pub use perf::PerformanceStats;
use runtime_config::{EmulatorRuntimeConfig, FrameRunConfig, FrameStallTraceState, RunLoopConfig};

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 224;
#[allow(dead_code)]
const MASTER_CLOCK_NTSC: f64 = 21_477_272.0;
// 実機は CPU:PPU=6:4（=3:2）。
// ここでは「master clock からの分周」を使って CPUサイクル→PPUドット数へ変換する。
const CPU_CLOCK_DIVIDER: u64 = 6;
const PPU_CLOCK_DIVIDER: u64 = 4;

pub struct Emulator {
    cpu: Cpu,
    bus: Bus,
    frame_buffer: Vec<u32>,
    master_cycles: u64,
    // Pending "stall" time in master cycles (e.g., MDMA); CPU is halted while PPU/APU advance.
    pending_stall_master_cycles: u64,
    // CPU->PPU 変換時の端数（master cycles を PPU_CLOCK_DIVIDER で割った余り; 0..PPU_CLOCK_DIVIDER-1）
    ppu_cycle_accum: u8,
    // APU step batching: pending CPU cycles to apply to the APU.
    apu_cycle_debt: u32,
    // master->CPU conversion remainder for APU batching (0..CPU_CLOCK_DIVIDER-1)
    apu_master_cycle_accum: u8,
    // master->CPU conversion remainder for SuperFX batching (0..CPU_CLOCK_DIVIDER-1)
    superfx_master_cycle_accum: u8,
    // master->CPU conversion remainder for SA-1 during stall processing (0..CPU_CLOCK_DIVIDER-1)
    sa1_master_cycle_accum: u8,
    apu_step_batch: u32,
    apu_step_force: u32,
    // SA-1 batching: accumulate S-CPU cycles before stepping SA-1 to reduce overhead.
    sa1_cycle_debt: u16,
    sa1_batch_cpu: u16,
    fast_mode: bool,
    rom_checksum: u32,
    frame_count: u64,
    // Performance optimization fields
    frame_skip_count: u8,
    max_frame_skip: u8,
    adaptive_timing: bool,
    frame_skip_threshold: f64,
    performance_stats: PerformanceStats,
    audio_system: AudioSystem,
    suppress_next_audio_output: bool,
    present_every_auto: u64,
    present_auto_good_streak: u8,
    present_auto_bad_streak: u8,
    present_auto_cooldown: u8,
    // NMI handling
    #[allow(dead_code)]
    nmi_triggered_this_flag: bool,
    debugger: Debugger,
    #[allow(dead_code)]
    rom_title: String,
    black_screen_streak: u32,
    black_screen_reported: bool,
    headless: bool,
    headless_max_frames: u64,
    srm_path: Option<PathBuf>,
    srm_autosave_every: Option<u64>,
    srm_last_autosave_frame: u64,
    boot_fallback_applied: bool,
    #[allow(dead_code)]
    palette_fallback_applied: bool,
    save_state_capture_stop_requested: bool,
}

impl Emulator {
    pub fn new(
        cartridge: Cartridge,
        display_title: String,
        srm_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let quiet = crate::debug_flags::quiet();
        let rom = cartridge.rom.clone();
        let mut bus = Bus::new_with_mapper(
            cartridge.rom,
            cartridge.header.mapper_type,
            cartridge.header.ram_size,
        );
        // CPUテストROM用の補助（通常ROMでは無効）
        // - 65C816 TEST: cputest-full.sfc 等
        // - 明示的に有効化したい場合は CPU_TEST_MODE=1
        let title_up = display_title.to_ascii_uppercase();
        let starfox_title_suppress_bg1 = title_up.contains("STAR FOX")
            && std::env::var("STARFOX_TITLE_SUPPRESS_BG1")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
        bus.get_ppu_mut()
            .set_starfox_title_bg1_suppression(starfox_title_suppress_bg1);
        let cpu_test_env = std::env::var_os("CPU_TEST_MODE").is_some();
        if cpu_test_env
            || title_up.contains("CPU TEST")
            || title_up.contains("CPUTEST")
            || title_up.contains("65C816 TEST")
        {
            bus.enable_cpu_test_mode();
        }
        if (crate::debug_flags::mapper() || crate::debug_flags::boot_verbose()) && !quiet {
            println!("Mapper: {:?}", cartridge.header.mapper_type);
        }
        let mut cpu = Cpu::new();

        // SNESのリセットベクターは0x00FFFCにある
        let reset_vector_lo = bus.read_u8(0x00FFFC) as u16;
        let reset_vector_hi = bus.read_u8(0x00FFFD) as u16;
        let reset_vector = (reset_vector_hi << 8) | reset_vector_lo;
        if crate::debug_flags::boot_verbose() && !quiet {
            println!(
                "Reset vector: 0x{:04X} (lo=0x{:02X}, hi=0x{:02X})",
                reset_vector, reset_vector_lo, reset_vector_hi
            );
        }

        // リセットベクターが無効な場合、デバッグ情報を表示
        if (reset_vector == 0x0000 || reset_vector == 0xFFFF)
            && crate::debug_flags::boot_verbose()
            && !quiet
        {
            println!("WARNING: Invalid reset vector detected!");
            println!(
                "ROM info: title='{}', mapper={:?}, size={}KB",
                cartridge.header.title,
                cartridge.header.mapper_type,
                cartridge.header.rom_size / 1024
            );
            println!("Memory around reset vector (0xFFFC-0xFFFF):");
            for addr in 0xFFFC..=0xFFFF {
                let val = bus.read_u8(addr);
                println!("  0x{:04X}: 0x{:02X}", addr, val);
            }
        }

        cpu.reset(reset_vector);

        // Initialize stack area to prevent 0xFFFF values
        cpu.init_stack(&mut bus);

        // --- Optional override via env: FORCE_MAPPER=lorom|hirom|exhirom ---
        if let Ok(val) = std::env::var("FORCE_MAPPER") {
            use crate::cartridge::MapperType;
            let forced = match val.to_lowercase().as_str() {
                "lorom" => Some(MapperType::LoRom),
                "hirom" => Some(MapperType::HiRom),
                "exhirom" => Some(MapperType::ExHiRom),
                _ => None,
            };
            if let Some(m) = forced {
                if !quiet {
                    println!("FORCE_MAPPER applied: {:?}", m);
                }
                bus.set_mapper_type(m);
                let lo = bus.read_u8(0x00FFFC) as u16;
                let hi = bus.read_u8(0x00FFFD) as u16;
                let rv = (hi << 8) | lo;
                cpu.reset(rv);
                cpu.init_stack(&mut bus);
            }
        }

        // --- Runtime mapper self-check: probe candidates and pick the healthiest ---
        let disable_autocorrect = std::env::var("DISABLE_MAPPER_AUTOCORRECT")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        let force_best = std::env::var("FORCE_MAPPER_BEST")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        let header_checksum_valid =
            (cartridge.header.checksum ^ cartridge.header.checksum_complement) == 0xFFFF;
        let reset_vector_valid = reset_vector != 0x0000 && reset_vector != 0xFFFF;
        use crate::cartridge::MapperType;
        fn sample_non_ff(bus: &mut Bus, addr: u32, n: usize) -> usize {
            let mut cnt = 0usize;
            for off in 0..(n as u32) {
                if bus.read_u8(addr.wrapping_add(off)) != 0xFF {
                    cnt += 1;
                }
            }
            cnt
        }
        fn score_mapper(bus: &mut Bus, mapper: MapperType) -> (usize, u16) {
            let mut score = 0usize;
            bus.set_mapper_type(mapper);
            // reset vector
            let rv_lo = bus.read_u8(0x00FFFC) as u16;
            let rv_hi = bus.read_u8(0x00FFFD) as u16;
            let rv = (rv_hi << 8) | rv_lo;
            // sample around reset in bank 00
            score += sample_non_ff(bus, rv as u32, 32);
            // sample high regions in common code banks
            for &bank in &[0x00u8, 0x80u8, 0x85u8, 0xC0u8] {
                let base = ((bank as u32) << 16) | 0xFF80;
                score += sample_non_ff(bus, base, 0x80);
            }
            (score, rv)
        }
        if !disable_autocorrect {
            if header_checksum_valid && reset_vector_valid && !force_best {
                if !quiet {
                    println!("Mapper auto-correct skipped (valid header/reset)");
                }
            } else {
                let current_mapper = bus.get_mapper_type();
                // Build candidate set
                let mut candidates = vec![current_mapper];
                if !candidates.contains(&MapperType::LoRom) {
                    candidates.push(MapperType::LoRom);
                }
                if !candidates.contains(&MapperType::HiRom) {
                    candidates.push(MapperType::HiRom);
                }
                if !candidates.contains(&MapperType::ExHiRom) {
                    candidates.push(MapperType::ExHiRom);
                }

                // Skip auto-correct for special mappers (non Lo/Hi/ExHiROM)
                if !matches!(
                    current_mapper,
                    MapperType::LoRom | MapperType::HiRom | MapperType::ExHiRom
                ) {
                    if !quiet {
                        println!(
                            "Mapper auto-correct skipped for special mapper: {:?}",
                            current_mapper
                        );
                    }
                } else {
                    let mut best = current_mapper;
                    let mut best_score = 0usize;
                    let mut cur_score = 0usize;
                    let mut best_rv: u16 = reset_vector;
                    for cand in candidates.into_iter() {
                        let (s, rv) = score_mapper(&mut bus, cand);
                        if crate::debug_flags::mapper() {
                            println!("Mapper score {:?}: {} (reset=0x{:04X})", cand, s, rv);
                        }
                        if cand == current_mapper {
                            cur_score = s;
                        }
                        if s > best_score {
                            best_score = s;
                            best = cand;
                            best_rv = rv;
                        }
                    }
                    // Adopt best only if it clearly beats current (margin to avoid mis-picks)
                    if best != current_mapper
                        && (force_best || best_score >= cur_score.saturating_add(100))
                    {
                        if !quiet {
                            println!(
                            "Mapper auto-correct: {:?} -> {:?} (best score={}, cur score={}), reset=0x{:04X}",
                            current_mapper, best, best_score, cur_score, best_rv
                        );
                        }
                        bus.set_mapper_type(best);
                        cpu.reset(best_rv);
                        cpu.init_stack(&mut bus);
                    } else {
                        // Keep current mapper
                        bus.set_mapper_type(current_mapper);
                    }
                }
            }
        } else if !quiet {
            println!("Mapper auto-correct disabled by env");
        }

        // Optional ROM byte dump for boot diagnosis
        if std::env::var("DUMP_BOOT_BYTES")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
        {
            fn dump_range(bus: &mut Bus, base: u32, len: usize) {
                print!(
                    "DUMP {:02X}:{:04X}-{:04X}: ",
                    (base >> 16) & 0xFF,
                    base as u16,
                    (base as u16).wrapping_add(len as u16)
                );
                for i in 0..len as u32 {
                    print!("{:02X} ", bus.read_u8(base + i));
                }
                println!();
            }
            let pc_reset = cpu.pc() as u32;
            if !quiet {
                println!("Boot PC after mapper: {:02X}:{:04X}", cpu.pb(), cpu.pc());
            }
            dump_range(&mut bus, pc_reset.wrapping_sub(8) & 0x00FFFF, 32);
            dump_range(&mut bus, 0x00FFFC, 4);
        }

        let runtime_config = EmulatorRuntimeConfig::from_env(bus.is_sa1_active());

        // Use caller-provided display title (already normalized/fallback applied)
        let rom_title = if display_title.trim().is_empty() {
            String::from("(Unknown Title)")
        } else {
            display_title
        };

        let frame_buffer = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];

        // Attempt to load existing SRAM from disk (if provided), unless disabled
        if !runtime_config.ignore_sram {
            if let Some(ref path) = srm_path {
                if let Ok(bytes) = std::fs::read(path) {
                    let load_len = bytes.len().min(bus.sram_size());
                    if load_len > 0 {
                        bus.sram_mut()[..load_len].copy_from_slice(&bytes[..load_len]);
                        bus.clear_sram_dirty();
                        if !quiet {
                            println!("SRAM loaded: {} bytes from {}", load_len, path.display());
                        }
                    }
                }
            }
        } else if !quiet {
            println!("SRAM load skipped (IGNORE_SRAM=1)");
        }

        // Calculate ROM checksum for save state validation
        let rom_checksum = calculate_checksum(&rom);

        // Initialize audio system (silent when headless/no-audio to avoid device errors)
        let audio_system = if runtime_config.headless_requested || runtime_config.audio_off {
            if !quiet {
                println!("HEADLESS: using silent audio backend (no device init)");
            }
            AudioSystem::new_silent()
        } else if runtime_config.external_audio_output {
            if !quiet {
                println!("Audio: using external audio output backend");
            }
            let mut asys = AudioSystem::new_external_output();
            let apu_handle = bus.get_apu_shared();
            asys.set_apu(apu_handle);
            asys.start();
            asys
        } else {
            match AudioSystem::new() {
                Ok(mut asys) => {
                    let apu_handle = bus.get_apu_shared();
                    asys.set_apu(apu_handle);
                    asys.start();
                    asys
                }
                Err(e) => {
                    eprintln!(
                        "WARNING: audio init failed, falling back to silent backend: {}",
                        e
                    );
                    AudioSystem::new_silent()
                }
            }
        };

        // Enable multitap via env (MULTITAP=1)
        if runtime_config.multitap {
            bus.get_input_system_mut().set_multitap_enabled(true);
            if !quiet {
                println!("Input: Multitap enabled (controllers 3/4 active)");
            }
        }

        Ok(Emulator {
            cpu,
            bus,
            frame_buffer,
            master_cycles: 0,
            pending_stall_master_cycles: 0,
            ppu_cycle_accum: 0,
            apu_cycle_debt: 0,
            apu_master_cycle_accum: 0,
            superfx_master_cycle_accum: 0,
            sa1_master_cycle_accum: 0,
            apu_step_batch: runtime_config.apu_step_batch,
            apu_step_force: runtime_config.apu_step_force,
            sa1_cycle_debt: 0,
            sa1_batch_cpu: runtime_config.sa1_batch_cpu,
            fast_mode: runtime_config.fast_mode,
            rom_checksum,
            frame_count: 0,
            frame_skip_count: 0,
            max_frame_skip: runtime_config.max_frame_skip, // Allow skipping up to N frames for performance
            adaptive_timing: runtime_config.adaptive_timing && !runtime_config.disable_frame_skip,
            frame_skip_threshold: runtime_config.frame_skip_threshold,
            performance_stats: PerformanceStats::new(),
            audio_system,
            suppress_next_audio_output: false,
            present_every_auto: 1,
            present_auto_good_streak: 0,
            present_auto_bad_streak: 0,
            present_auto_cooldown: 0,
            nmi_triggered_this_flag: false,
            debugger: Debugger::new(),
            rom_title,
            black_screen_streak: 0,
            black_screen_reported: false,
            headless: runtime_config.headless_requested,
            headless_max_frames: runtime_config.headless_max_frames,
            srm_path,
            srm_autosave_every: runtime_config.srm_autosave_every,
            srm_last_autosave_frame: 0,
            boot_fallback_applied: false,
            palette_fallback_applied: false,
            save_state_capture_stop_requested: false,
        })
    }

    pub fn run(&mut self) {
        let quiet = crate::debug_flags::quiet();
        let run_config = RunLoopConfig::from_env();
        let headless_diagnostics = HeadlessDiagnosticsConfig::from_env(quiet);
        if !self.headless {
            eprintln!("Emulator::run is headless-only; use the SDL runner for interactive mode");
            self.save_sram_if_dirty();
            return;
        }

        // 起動直後にテストパターンを約2秒間（120フレーム）表示（Dragon Quest III修正のため有効化）
        self.maybe_show_startup_test_pattern(run_config.force_test_pattern);
        // Optional: auto-load save state at startup (debug aid)
        let loaded_state = self.try_auto_load_state();
        if self.headless && loaded_state && self.frame_count >= self.headless_max_frames {
            self.headless_max_frames += self.frame_count;
        }
        // SuperFX workaround: bypass BG1 window masking because the viewport
        // metadata computation in the SuperFX 3D pipeline is not yet accurate enough.
        self.apply_superfx_workarounds();

        let mut stats_timer = Instant::now();

        let headless_fast_render_from =
            run_config.headless_fast_render_from(self.headless_max_frames);
        if !quiet {
            println!(
                "HEADLESS mode: running {} frames without window",
                self.headless_max_frames
            );
        }
        while self.frame_count < self.headless_max_frames && !shutdown::should_quit() {
            let frame_start = Instant::now();
            self.apply_scripted_input_for_headless();
            if run_config.headless_fast_render {
                let enable = self.frame_count >= headless_fast_render_from;
                self.bus
                    .get_ppu_mut()
                    .set_framebuffer_rendering_enabled(enable);
            }
            if run_config.mode7_test {
                self.run_mode7_diag_frame();
            } else {
                self.run_frame();
            }
            if self.take_save_state_capture_stop_requested() {
                break;
            }
            // Headlessでもレンダーパイプを通し、フォールバック描画/テストパターンを反映させる。
            // ただし HEADLESS_FAST_RENDER=1 の場合は、最後の数フレームのみ描画して高速化する。
            if !run_config.headless_fast_render || self.frame_count >= headless_fast_render_from {
                self.render();
            }
            // CPUテストROM: 終了状態（PASS/FAIL）に到達したら早期終了する
            self.maybe_quit_on_cpu_test_result();
            if shutdown::should_quit() {
                break;
            }
            // Periodic minimal palette injection to ensure visibility until game loads CGRAM
            self.maybe_inject_min_palette_periodic();
            // Debug: periodically dump CPU PC/PB to identify headless stalls (HEADLESS_PC_DUMP=1)
            self.maybe_print_headless_pc_dump(&headless_diagnostics);
            let frame_time = frame_start.elapsed();
            let _ = self.performance_stats.update(frame_time);
            self.frame_count += 1;
            // Apply compatibility display fixes before diagnostics/dumps so
            // captured state matches what a frontend would present.
            self.maybe_auto_unblank();
            self.maybe_force_unblank();
            self.maybe_inject_min_palette_periodic();
            self.maybe_dump_framebuffer_at();
            self.maybe_dump_mem_at();
            self.maybe_save_state_at();
            if shutdown::should_quit() {
                break;
            }
            self.maybe_dump_starfox_diag_at();

            // Periodic SRAM autosave (optional)
            self.maybe_autosave_sram();
            if run_config.headless_stats && stats_timer.elapsed() >= Duration::from_secs(2) {
                self.print_performance_stats();
                stats_timer = Instant::now();
            }
            if headless_diagnostics.is_summary_frame(self.frame_count, self.headless_max_frames) {
                self.print_headless_checkpoint_diagnostics(&headless_diagnostics, quiet);
            }
        }
        // Final init summary (used by tools/smoke.sh; can be disabled for CPU test runs)
        if run_config.headless_summary {
            self.print_headless_init_summary();
        }

        let dump_config = HeadlessDumpConfig::from_env();
        self.dump_headless_outputs(&dump_config, run_config.headless_stats, quiet);
        self.save_sram_if_dirty();
    }

    fn enforce_frame_loop_guard(
        &mut self,
        frame_count: u32,
        loop_iterations: u64,
        start_cycles: u64,
        cycles_per_frame: u64,
        max_iterations: u64,
    ) {
        if loop_iterations <= max_iterations {
            return;
        }

        eprintln!(
            "FATAL: Frame {} exceeded {} loop iterations! Possible infinite loop.",
            frame_count, max_iterations
        );
        eprintln!(
            "  master_cycles={}, start_cycles={}, target={}",
            self.master_cycles, start_cycles, cycles_per_frame
        );
        eprintln!(
            "  CPU PC={:02X}:{:04X}",
            self.cpu.get_pc() >> 16,
            self.cpu.get_pc() & 0xFFFF
        );
        eprintln!(
            "  CPU waiting_for_irq={}, stopped={}",
            self.cpu.core.state().waiting_for_irq,
            self.cpu.core.state().stopped
        );

        // Print last 10 loop iteration details.
        if frame_count >= 997 {
            eprintln!("\n  Collecting final diagnostics...");
            for i in 0..10 {
                let pc = self.cpu.get_pc();
                let opcode = self.bus.read_u8(pc);
                let cpu_cycles = self.cpu.step(&mut self.bus);
                eprintln!(
                    "    Loop {}: PC={:02X}:{:04X} opcode=0x{:02X} cycles={}",
                    loop_iterations + i + 1,
                    pc >> 16,
                    pc & 0xFFFF,
                    opcode,
                    cpu_cycles
                );
                if cpu_cycles == 0 {
                    eprintln!("    WARNING: CPU returned 0 cycles!");
                    break;
                }
            }
        }
        std::process::exit(1);
    }

    fn maybe_trace_pc_ffff(&self, frame_count: u32, pc: u32, enabled: bool) {
        if !enabled || pc != 0x00FFFF {
            return;
        }

        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT_FFFF: AtomicU32 = AtomicU32::new(0);
        let n = COUNT_FFFF.fetch_add(1, Ordering::Relaxed);
        if n < 8 {
            let st = self.cpu.core.state();
            println!(
                "[PCFFFF] frame={} count={} wait_irq={} stopped={} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} DB={:02X} DP={:04X}",
                frame_count,
                n,
                st.waiting_for_irq,
                st.stopped,
                st.a,
                st.x,
                st.y,
                st.sp,
                st.p.bits(),
                st.db,
                st.dp
            );
        }
    }

    fn maybe_trace_stall(
        &self,
        frame_count: u32,
        pc: u32,
        threshold: u32,
        state: &mut FrameStallTraceState,
    ) {
        if threshold == 0 {
            return;
        }

        state.ring[state.ring_pos] = pc;
        state.ring_pos = (state.ring_pos + 1) & 0x0F;
        if pc == state.pc {
            state.count = state.count.saturating_add(1);
            if state.count == threshold {
                let mut recent = Vec::new();
                for i in 0..state.ring.len() {
                    let idx = (state.ring_pos + i) & 0x0F;
                    recent.push(format!(
                        "{:02X}:{:04X}",
                        state.ring[idx] >> 16,
                        state.ring[idx] & 0xFFFF
                    ));
                }
                println!(
                    "[STALL] frame={} PC={:02X}:{:04X} last_diff={:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} DB={:02X} DP={:04X} recent=[{}]",
                    frame_count,
                    self.cpu.pb(),
                    self.cpu.pc(),
                    state.last_diff >> 16,
                    state.last_diff & 0xFFFF,
                    self.cpu.a(),
                    self.cpu.x(),
                    self.cpu.y(),
                    self.cpu.sp(),
                    self.cpu.p().bits(),
                    self.cpu.db(),
                    self.cpu.dp(),
                    recent.join(", ")
                );
                state.count = 0;
            }
        } else {
            state.last_diff = state.pc;
            state.pc = pc;
            state.count = 0;
        }
    }

    fn maybe_trace_frame_pc(&self, frame_count: u32, pc: u32, enabled: bool) {
        if !enabled {
            return;
        }

        use std::sync::atomic::{AtomicU32, Ordering};
        static LAST_LOGGED_FRAME: AtomicU32 = AtomicU32::new(0);

        // Avoid spamming: log once per frame at loop top.
        if LAST_LOGGED_FRAME.swap(frame_count, Ordering::Relaxed) != frame_count {
            println!(
                "[frame_pc] frame={} PC={:02X}:{:04X} A=0x{:04X} X=0x{:04X} Y=0x{:04X} P=0x{:02X} JOYBUSY={}",
                frame_count,
                pc >> 16,
                pc & 0xFFFF,
                self.cpu.a(),
                self.cpu.x(),
                self.cpu.y(),
                self.cpu.p().bits(),
                self.bus.joy_busy_counter()
            );
        }
    }

    fn run_frame(&mut self) {
        let trace_slow_ms = std::env::var("TRACE_STARFOX_GUI_SLOW_MS")
            .ok()
            .and_then(|v| v.parse::<u128>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(0);
        let trace_starfox_run = self.rom_title().to_ascii_uppercase().contains("STAR FOX");
        let trace_starfox_slow = trace_starfox_run && trace_slow_ms > 0;
        let run_frame_wall_start = Instant::now();
        if trace_starfox_slow {
            self.cpu.reset_step_profile();
            self.bus.reset_cpu_profile();
        }
        let mut timings = FrameRunTimings::default();
        // Run exactly one current PPU frame worth of master cycles.
        // NTSC frame length varies with the field:
        // - non-interlace field=1 shortens V=240 by 1 dot
        // - interlace field=0 adds one extra scanline
        crate::cartridge::superfx::set_trace_superfx_exec_frame(self.frame_count.wrapping_add(1));
        let sync_start = Instant::now();
        self.sync_superfx_direct_buffer();
        timings.sync = sync_start.elapsed();
        let cycles_per_frame = self.bus.get_ppu().remaining_master_cycles_in_frame();
        let start_cycles = self.master_cycles;
        let start_ppu_frame = self.bus.get_ppu().get_frame();

        if self.bus.is_sa1_active() {
            self.bus.reset_sa1_cycle_accum();
        }

        let frame_count = (self.frame_count.wrapping_add(1)) as u32;

        self.apply_frame_start_debug_controls(frame_count);

        // Debug: Track loop iterations to detect infinite loops
        let mut loop_iterations = 0u64;
        // ループ検出の許容回数を環境変数で調整できるようにする。
        // ・初期フレーム(<=3): 5,000,000（VBlank待ちなどで多少重くても落とさない）
        // ・重いトレース有効時: 50,000,000 まで許容（WATCH_PC/TRACE_4210/TRACE_BRANCH など）
        // ・通常: 1,000,000
        // LOOP_GUARD_MAX を指定するとその値を上書きする（デバッグ用）。
        let frame_config = FrameRunConfig::from_env(frame_count, self.fast_mode);
        let mut stall_trace = FrameStallTraceState::default();

        self.apply_frame_start_irq_controls(frame_count);

        let main_loop_start = Instant::now();
        while self.master_cycles - start_cycles < cycles_per_frame {
            loop_iterations += 1;
            self.enforce_frame_loop_guard(
                frame_count,
                loop_iterations,
                start_cycles,
                cycles_per_frame,
                frame_config.max_iterations,
            );

            // If a previous instruction triggered a DMA stall, the CPU is halted while time
            // continues to advance. Consume that stall budget here before running more CPU.
            if self.pending_stall_master_cycles > 0 {
                let remaining = cycles_per_frame.saturating_sub(self.master_cycles - start_cycles);
                let consume = self.pending_stall_master_cycles.min(remaining);
                self.advance_time_without_cpu(consume);
                if self.maybe_save_state_at_frame_anchor() {
                    return;
                }
                self.pending_stall_master_cycles -= consume;
                self.pending_stall_master_cycles = self
                    .pending_stall_master_cycles
                    .saturating_add(self.bus.take_pending_stall_master_cycles());
                continue;
            }

            let CpuInstructionSlice::Continue {
                cpu_cycles,
                extra_master,
                cpu_time,
            } = self.run_cpu_instruction_slice(
                frame_count,
                loop_iterations,
                cycles_per_frame,
                start_cycles,
                &frame_config,
                &mut stall_trace,
            )
            else {
                return;
            };

            timings.cpu_exec = timings.cpu_exec.saturating_add(cpu_time);
            self.step_sa1_for_cpu_slice(cpu_cycles, &frame_config);

            timings.ppu_step = timings.ppu_step.saturating_add(self.step_ppu_for_cpu_slice(
                cpu_cycles,
                extra_master,
                frame_config.perf_verbose,
            ));
            timings.apu_inline = timings
                .apu_inline
                .saturating_add(self.step_apu_for_cpu_slice(
                    cpu_cycles,
                    extra_master,
                    frame_config.perf_verbose,
                ));

            // NMI/IRQ は CPU 側の poll_nmi/service_nmi/service_irq で処理する。
            // ここで手動トリガ/クリアすると IRQ のレベル維持が崩れるため触らない。

            self.master_cycles += (cpu_cycles as u64) * CPU_CLOCK_DIVIDER + extra_master;

            // Drain any time consumed by DMA stalls that occurred during this instruction slice.
            // The CPU should remain halted for that duration, but PPU/APU continue to advance.
            self.pending_stall_master_cycles = self
                .pending_stall_master_cycles
                .saturating_add(self.bus.take_pending_stall_master_cycles());

            if self.maybe_save_state_at_frame_anchor() {
                return;
            }
        }
        timings.main_loop = main_loop_start.elapsed();
        timings.catchup = self.finish_frame_boundary_catchup(start_ppu_frame);

        if self.maybe_save_state_at_frame_anchor() {
            return;
        }

        self.flush_end_of_frame_sa1(&frame_config);
        self.maybe_run_sa1_frame_catchup(cycles_per_frame, &frame_config);

        // Frame完了時に主要レジスタのサマリを出力（デバッグ用）
        self.maybe_dump_register_summary(frame_count);

        timings.audio = self.mix_frame_audio();
        let total_time = run_frame_wall_start.elapsed();
        self.maybe_log_starfox_slow_frame(trace_starfox_slow, trace_slow_ms, &timings, total_time);
    }

    fn step_ppu(&mut self, cycles: u16, apply_dram_refresh: bool) {
        // Step in bounded slices so we don't miss per-scanline events when a single call
        // advances across HBlank and/or multiple scanlines (e.g., during MDMA stalls).
        //
        // In particular, the official burn-in tests rely on accurate HV-timer behavior.
        // If we step across a scanline boundary in one lump, we must:
        // - Run HDMA exactly at HBlank entry for that scanline (visible lines only)
        // - Tick scanline-based timers on every scanline advance
        // - Attribute HV-timer H-match to the correct scanline (before wrap)
        let mut remaining = cycles;
        const FIRST_HBLANK_DOT: u16 = 22 + 256; // visible starts at 22, width=256
        const DRAM_REFRESH_START_DOT: u16 = 134;
        const DRAM_REFRESH_MASTER_CYCLES: u64 = 40;

        while remaining > 0 {
            let old_scanline = self.bus.get_ppu().scanline;
            let old_cycle = self.bus.get_ppu().get_cycle();
            let old_line_dots = self.bus.get_ppu().dots_this_scanline(old_scanline);
            let was_hblank = self.bus.get_ppu().is_hblank();
            let was_vblank = self.bus.get_ppu().is_vblank();

            // Compute a slice that won't cross HBlank entry or scanline wrap.
            //
            // IMPORTANT:
            // PPU's HBlank flag flips while *processing* the first HBlank dot (FIRST_HBLANK_DOT).
            // If we slice exactly up to FIRST_HBLANK_DOT, the PPU will not have executed that dot
            // yet, so `is_hblank()` stays false and we can miss the HDMA start-of-HBlank event.
            //
            // To reliably catch the transition, ensure we step at least 1 dot into HBlank
            // (i.e., end at >= FIRST_HBLANK_DOT+1) before checking `is_hblank()`.
            let mut slice = remaining.min(old_line_dots.saturating_sub(old_cycle).max(1));
            if !was_hblank {
                if old_cycle < FIRST_HBLANK_DOT {
                    slice = slice.min((FIRST_HBLANK_DOT + 1).saturating_sub(old_cycle).max(1));
                } else if old_cycle == FIRST_HBLANK_DOT {
                    slice = slice.min(1);
                }
            }
            if apply_dram_refresh && old_cycle < DRAM_REFRESH_START_DOT {
                slice = slice.min(
                    (DRAM_REFRESH_START_DOT + 1)
                        .saturating_sub(old_cycle)
                        .max(1),
                );
            } else if apply_dram_refresh && old_cycle == DRAM_REFRESH_START_DOT {
                slice = slice.min(1);
            }

            self.bus.get_ppu_mut().step(slice);
            remaining -= slice;

            let new_scanline = self.bus.get_ppu().scanline;
            let new_cycle = self.bus.get_ppu().get_cycle();
            let is_hblank = self.bus.get_ppu().is_hblank();
            let is_vblank = self.bus.get_ppu().is_vblank();
            let entered_refresh = apply_dram_refresh
                && old_scanline == new_scanline
                && !was_vblank
                && old_cycle <= DRAM_REFRESH_START_DOT
                && new_cycle > DRAM_REFRESH_START_DOT;

            // Update H/V timer progress for the segment we just stepped.
            // If we wrapped to the next scanline, attribute the segment to the old scanline.
            if old_scanline == new_scanline {
                self.bus.tick_timers_hv(old_cycle, new_cycle, old_scanline);
            } else {
                self.bus
                    .tick_timers_hv(old_cycle, old_line_dots, old_scanline);
            }

            // H-Blank入りでHDMA実行
            //
            // SNES HDMA transfers occur only during active display (scanlines 0 through
            // the last visible line).  During V-blank (scanlines 225+ for non-overscan,
            // 240+ for overscan) no HDMA transfers occur on real hardware.
            // Running HDMA during V-blank would corrupt the Mode 7 shared latch when
            // the NMI handler writes Mode 7 registers between H-blank boundaries.
            if !was_hblank && is_hblank && !self.bus.get_ppu().is_vblank() {
                // Guard a few dots at HBlank head for HDMA operations
                self.bus.get_ppu_mut().on_hblank_start_guard();
                self.bus.hdma_hblank();
            }
            if entered_refresh {
                self.pending_stall_master_cycles = self
                    .pending_stall_master_cycles
                    .saturating_add(DRAM_REFRESH_MASTER_CYCLES);
            }

            // スキャンライン変更時はタイマを進める
            if old_scanline != new_scanline {
                self.bus.tick_timers();
                // JOYBUSYの更新
                self.bus.on_scanline_advance();
                self.maybe_dump_framebuffer_on_scanline(new_scanline);
                // Frame start (scanline counter wrapped to 0)
                if new_scanline < old_scanline {
                    self.bus.on_frame_start();
                }
                // Keep auto-joy timing aligned with the PPU's own v_blank transition.
                if !was_vblank && is_vblank {
                    self.bus.on_vblank_start();
                    if std::env::var_os("TRACE_VBLANK_PC").is_some() {
                        let pc = self.cpu.get_pc();
                        println!(
                            "[TRACE_VBLANK_PC] frame={} PC={:02X}:{:04X} sl={} cyc={}",
                            self.bus.get_ppu().get_frame(),
                            (pc >> 16) as u8,
                            (pc & 0xFFFF) as u16,
                            new_scanline,
                            self.bus.get_ppu().get_cycle()
                        );
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn handle_nmi(&mut self) {
        self.cpu.trigger_nmi(&mut self.bus);
    }
}

fn calculate_checksum(data: &[u8]) -> u32 {
    data.iter()
        .fold(0u32, |acc, &byte| acc.wrapping_add(byte as u32))
}

#[cfg(test)]
mod tests;
