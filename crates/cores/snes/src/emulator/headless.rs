use super::{
    write_framebuffer_image, Emulator, EmulatorRuntimeConfig, SCREEN_HEIGHT, SCREEN_WIDTH,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
pub(super) struct HeadlessDiagnosticsConfig {
    pc_dump: bool,
    visibility_check: bool,
    dump_vram_head: bool,
    log_cpu_pc: bool,
}

impl HeadlessDiagnosticsConfig {
    pub(super) fn from_env(quiet: bool) -> Self {
        Self {
            pc_dump: std::env::var_os("HEADLESS_PC_DUMP").is_some(),
            visibility_check: EmulatorRuntimeConfig::read_strict_bool_env(
                "HEADLESS_VIS_CHECK",
                !quiet,
            ),
            dump_vram_head: std::env::var_os("DUMP_VRAM_HEAD").is_some(),
            log_cpu_pc: EmulatorRuntimeConfig::read_strict_bool_env("HEADLESS_LOG_CPUPC", false),
        }
    }

    pub(super) fn is_summary_frame(self, frame_count: u64, max_frames: u64) -> bool {
        frame_count == 60
            || frame_count == 120
            || frame_count == 180
            || frame_count == 370
            || frame_count == max_frames
    }
}

#[derive(Clone, Debug)]
pub(super) struct HeadlessDumpConfig {
    dump_frame: bool,
    dump_frame_use_gui: bool,
    dump_frame_path: PathBuf,
    dump_mem: bool,
    dump_ppu_state: bool,
    dump_audio_wav: bool,
    dump_final_ppu_state: bool,
    dump_wram_path: Option<PathBuf>,
}

impl HeadlessDumpConfig {
    pub(super) fn from_env() -> Self {
        Self {
            dump_frame: EmulatorRuntimeConfig::read_strict_bool_env("HEADLESS_DUMP_FRAME", false),
            dump_frame_use_gui: EmulatorRuntimeConfig::read_strict_bool_env(
                "HEADLESS_DUMP_FRAME_USE_GUI",
                false,
            ),
            dump_frame_path: std::env::var("HEADLESS_DUMP_FRAME_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("logs/headless_fb.png")),
            dump_mem: EmulatorRuntimeConfig::read_strict_bool_env("HEADLESS_DUMP_MEM", false),
            dump_ppu_state: EmulatorRuntimeConfig::read_strict_bool_env(
                "HEADLESS_DUMP_PPU_STATE",
                false,
            ),
            dump_audio_wav: std::env::var_os("DUMP_AUDIO_WAV").is_some(),
            dump_final_ppu_state: std::env::var_os("DUMP_PPU_STATE").is_some(),
            dump_wram_path: std::env::var_os("DUMP_WRAM").map(PathBuf::from),
        }
    }
}

impl Emulator {
    pub(super) fn print_headless_init_summary(&mut self) {
        let (nmi_w, mdma_w, hdma_w, dma_cfg) = self.bus.get_init_counters();
        let (imp_w, vwl, vwh, cg, oam) = self.bus.get_ppu().get_init_counters();
        println!(
            "INIT summary: $4200 writes={} MDMAEN!=0={} HDMAEN!=0={} DMAreg={} PPU important={} VRAM L/H={}/{} CGRAM={} OAM={}",
            nmi_w, mdma_w, hdma_w, dma_cfg, imp_w, vwl, vwh, cg, oam
        );
        println!("{}", self.bus.get_dma_config_summary());
        // OBJ (sprite) timing summary
        let obj_sum = { self.bus.get_ppu_mut().take_obj_summary() };
        println!("{}", obj_sum);
    }

    pub(super) fn dump_binary_file(label: &str, error_label: &str, path: &str, data: &[u8]) {
        if let Err(e) = std::fs::write(path, data) {
            eprintln!("Failed to dump {}: {}", error_label, e);
        } else {
            println!("{} dumped to {} ({} bytes)", label, path, data.len());
        }
    }

    pub(super) fn dump_headless_frame(&mut self, config: &HeadlessDumpConfig) {
        if !config.dump_frame {
            return;
        }

        let fb = if config.dump_frame_use_gui {
            self.framebuffer()
        } else {
            self.bus.get_ppu().get_framebuffer()
        };
        let _ = std::fs::create_dir_all("logs");
        if let Err(e) =
            write_framebuffer_image(&config.dump_frame_path, fb, SCREEN_WIDTH, SCREEN_HEIGHT)
        {
            eprintln!("Failed to dump framebuffer: {}", e);
        } else {
            println!("Framebuffer dumped to {}", config.dump_frame_path.display());
        }
    }

    pub(super) fn dump_headless_memory(&self, config: &HeadlessDumpConfig) {
        if !config.dump_mem {
            return;
        }

        let _ = std::fs::create_dir_all("logs");
        Self::dump_binary_file("WRAM", "WRAM", "logs/wram.bin", self.bus.wram());

        {
            let ppu = self.bus.get_ppu();
            Self::dump_binary_file("VRAM", "VRAM", "logs/vram.bin", ppu.get_vram());
            Self::dump_binary_file("CGRAM", "CGRAM", "logs/cgram.bin", ppu.get_cgram());
            Self::dump_binary_file("OAM", "OAM", "logs/oam.bin", ppu.get_oam());
        }

        let bwram = self.bus.sa1_bwram_slice();
        if !bwram.is_empty() {
            Self::dump_binary_file("BWRAM", "BW-RAM", "logs/bwram.bin", bwram);
        }

        // Dump SuperFX Game RAM if available
        if let Some(gram) = self.bus.superfx_game_ram_slice() {
            Self::dump_binary_file("Game RAM", "Game RAM", "logs/game_ram.bin", gram);
        }
    }

    pub(super) fn dump_headless_audio_wav(&self, config: &HeadlessDumpConfig) {
        if !config.dump_audio_wav {
            return;
        }

        let apu = self.bus.apu.lock().unwrap();
        if let Some(dsp) = apu.inner.dsp.as_ref() {
            dsp.dump_audio_wav();
        }
    }

    pub(super) fn maybe_dump_spc_trace_state(&self) {
        if !crate::debug_flags::trace_ipl_xfer() {
            return;
        }

        let mut apu = self.bus.apu.lock().unwrap();
        let spc_state = apu.inner.debug_spc_state();
        eprintln!("[SPC-STATE] {}", spc_state);
        // Extended: dump SPC RAM around PC and key areas
        let pc = apu.inner.smp.as_ref().map(|s| s.reg_pc).unwrap_or(0);
        let start = pc.saturating_sub(32);
        eprint!("[SPC-CODE] @{:04X}:", start);
        for i in 0..64u16 {
            eprint!(" {:02X}", apu.inner.peek_u8(start.wrapping_add(i)));
        }
        eprintln!();
        // Dump zero page $00-$FF (full direct page)
        eprint!("[SPC-ZP] @0000:");
        for i in 0..256u16 {
            eprint!(" {:02X}", apu.inner.peek_u8(i));
        }
        eprintln!();
        // Dump code at key SPC addresses
        for &addr in &[0x0900u16, 0x131D, 0x1350, 0x1650, 0x09C0] {
            eprint!("[SPC-FUNC] @{:04X}:", addr);
            for i in 0..64u16 {
                eprint!(" {:02X}", apu.inner.peek_u8(addr.wrapping_add(i)));
            }
            eprintln!();
        }
        // Total SMP cycles
        eprintln!("[SPC-DIAG] total_smp_cycles={}", apu.total_smp_cycles);
        // Dump sample directory at $FF00 (DIR=$FF)
        let dir_base = if let Some(dsp) = &mut apu.inner.dsp {
            (dsp.get_register(0x5D) as u16) * 0x100
        } else {
            0xFF00
        };
        eprint!("[SPC-DIR] @{:04X}:", dir_base);
        for i in 0..64u16 {
            eprint!(" {:02X}", apu.inner.peek_u8(dir_base.wrapping_add(i)));
        }
        eprintln!();
        // Dump first 36 BRR bytes at each sample start from directory
        for entry in 0..8u16 {
            let addr = dir_base.wrapping_add(entry * 4);
            let start =
                apu.inner.peek_u8(addr) as u16 | ((apu.inner.peek_u8(addr + 1) as u16) << 8);
            let loopaddr =
                apu.inner.peek_u8(addr + 2) as u16 | ((apu.inner.peek_u8(addr + 3) as u16) << 8);
            eprint!(
                "[SPC-SAMPLE] #{:02X} start={:04X} loop={:04X} brr:",
                entry, start, loopaddr
            );
            for i in 0..18u16 {
                eprint!(" {:02X}", apu.inner.peek_u8(start.wrapping_add(i)));
            }
            eprintln!();
        }
        eprint!("[SPC-IO] @00F0:");
        for i in 0xF0u16..=0xFF {
            eprint!(" {:02X}", apu.inner.peek_u8(i));
        }
        eprintln!();
        // Dump DSP registers
        eprint!("[SPC-DSP] regs:");
        let dsp_addr = apu.inner.peek_u8(0xF2);
        eprint!(" addr={:02X}", dsp_addr);
        if let Some(dsp) = &mut apu.inner.dsp {
            for r in [0x0Cu8, 0x1C, 0x4C, 0x5C, 0x6C, 0x6D, 0x7D] {
                eprint!(" ${:02X}={:02X}", r, dsp.get_register(r));
            }
        }
        eprintln!();
    }

    pub(super) fn dump_wram_if_requested(&self, config: &HeadlessDumpConfig, quiet: bool) {
        let Some(path) = config.dump_wram_path.as_ref() else {
            return;
        };

        match std::fs::write(path, self.bus.wram()) {
            Ok(_) => {
                if !quiet {
                    println!(
                        "[dump_wram] wrote WRAM ({} bytes) to {}",
                        self.bus.wram().len(),
                        path.display()
                    );
                }
            }
            Err(e) => eprintln!("[dump_wram] failed to write {}: {}", path.display(), e),
        }
    }

    pub(super) fn dump_headless_outputs(
        &mut self,
        config: &HeadlessDumpConfig,
        print_stats: bool,
        quiet: bool,
    ) {
        self.dump_headless_frame(config);
        self.dump_headless_memory(config);
        if config.dump_ppu_state {
            self.bus.get_ppu().debug_ppu_state();
        }
        if print_stats {
            self.print_performance_stats();
        }
        self.dump_headless_audio_wav(config);
        println!(
            "HEADLESS mode finished ({} / {} frames)",
            self.frame_count, self.headless_max_frames
        );
        if config.dump_final_ppu_state {
            self.bus.get_ppu().debug_ppu_state();
        }
        self.maybe_dump_spc_trace_state();
        self.dump_wram_if_requested(config, quiet);
    }

    pub(super) fn maybe_print_headless_pc_dump(&self, config: &HeadlessDiagnosticsConfig) {
        if !config.pc_dump || !self.frame_count.is_multiple_of(120) {
            return;
        }

        let cpu = self.cpu.core.state();
        let ppu = self.bus.get_ppu();
        let wram_flag = self.bus.wram().get(0x0122).copied().unwrap_or(0);
        println!(
            "[HEADLESS-PC] frame={} PB={:02X} PC={:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} E={} DB={:02X} DP={:04X} NMI(en={},latched={},vblank={}) WRAM[0122]={:02X}",
            self.frame_count,
            cpu.pb,
            cpu.pc,
            cpu.a,
            cpu.x,
            cpu.y,
            cpu.sp,
            cpu.p.bits(),
            cpu.emulation_mode,
            cpu.db,
            cpu.dp,
            ppu.nmi_enabled,
            ppu.nmi_latched,
            ppu.is_vblank(),
            wram_flag
        );
    }

    pub(super) fn print_headless_checkpoint_diagnostics(
        &mut self,
        config: &HeadlessDiagnosticsConfig,
        quiet: bool,
    ) {
        {
            let ppu = self.bus.get_ppu();
            if !quiet {
                println!(
                    "PPU usage @frame {}: VRAM {}/{} CGRAM {}/{} OAM {}/{}",
                    self.frame_count,
                    ppu.vram_usage(),
                    0x10000,
                    ppu.cgram_usage(),
                    0x200,
                    ppu.oam_usage(),
                    0x220
                );
            }
            // CGRAM head dump (first few colors)
            let head = ppu.dump_cgram_head(8);
            if !head.is_empty() && !quiet {
                let hex: Vec<String> = head.iter().map(|c| format!("{:04X}", c)).collect();
                println!("CGRAM head: [{}]", hex.join(", "));
            }
        }

        // Print VRAM FG summary and reset its counters (separate mutable borrow)
        let summary = { self.bus.get_ppu_mut().take_vram_write_summary() };
        if !quiet {
            println!("VRAM summary: {}", summary);
        }
        // DMA dest summary (consumes internal counters)
        let dma_sum = { self.bus.take_dma_dest_summary() };
        if !quiet {
            println!("{}", dma_sum);
        }
        // HDMA activity summary (consumes counters)
        let hdma_sum = { self.bus.take_hdma_summary() };
        if !quiet {
            println!("{}", hdma_sum);
        }
        // Render metrics (consumes counters)
        let rm = { self.bus.get_ppu_mut().take_render_metrics_summary() };
        if !quiet {
            println!("{}", rm);
        }

        self.print_headless_visibility_diagnostics(config, quiet);
        self.print_headless_cpu_pc_diagnostics(config);
    }

    pub(super) fn print_headless_visibility_diagnostics(
        &self,
        config: &HeadlessDiagnosticsConfig,
        quiet: bool,
    ) {
        if !config.visibility_check {
            return;
        }

        let (non_black, first_non_black, sample0, sample128, sample256) = {
            let fb = self.bus.get_ppu().get_framebuffer();
            let nb = fb.iter().filter(|&&px| px != 0xFF000000).count();
            let first = fb
                .iter()
                .position(|&px| px != 0xFF000000)
                .unwrap_or(usize::MAX);
            let s0 = if !fb.is_empty() { fb[0] } else { 0 };
            let s128 = if fb.len() > 128 { fb[128] } else { 0 };
            let s256 = if fb.len() > 256 { fb[256] } else { 0 };
            (nb, first, s0, s128, s256)
        };
        let ppu = self.bus.get_ppu();
        let brightness = ppu.brightness;
        let screen_display = ppu.screen_display;
        let tm = ppu.get_main_screen_designation();
        let bg_mode = ppu.get_bg_mode();
        println!(
            "VISIBILITY: frame={} non_black_pixels={} first_non_black_idx={} brightness={} forced_blank={} INIDISP=0x{:02X} TM=0x{:02X} mode={}",
            self.frame_count,
            non_black,
            first_non_black,
            brightness,
            (screen_display & 0x80) != 0,
            screen_display,
            tm,
            bg_mode
        );
        // Optional: dump small VRAM/OAM/CGRAM slices for early frames (debug)
        if config.dump_vram_head && self.frame_count <= 4 {
            let vram = ppu.dump_vram_head(64);
            let cgram = ppu.dump_cgram_head(16);
            let oam = ppu.dump_oam_head(32);
            println!("VRAM[0..64]: {:02X?}", vram);
            println!("CGRAM[0..16]: {:04X?}", cgram);
            println!("OAM[0..32]: {:02X?}", oam);
        }
        // Debug TM bits for frames with graphics
        if non_black > 0 {
            let bg1_en = (tm & 0x01) != 0;
            let bg2_en = (tm & 0x02) != 0;
            let bg3_en = (tm & 0x04) != 0;
            let bg4_en = (tm & 0x08) != 0;
            let obj_en = (tm & 0x10) != 0;
            println!(
                "  TM bits: BG1={} BG2={} BG3={} BG4={} OBJ={}",
                bg1_en, bg2_en, bg3_en, bg4_en, obj_en
            );
        }
        if !quiet {
            println!(
                "FB SAMPLE: [0]=0x{:08X} [128]=0x{:08X} [256]=0x{:08X}",
                sample0, sample128, sample256
            );
        }
        // Small top-left region inspection (16x16)
        let (tl_nonblack, tl_total) = {
            let fb = self.bus.get_ppu().get_framebuffer();
            let mut cnt = 0usize;
            let w = SCREEN_WIDTH;
            let h = SCREEN_HEIGHT;
            let rw = 16usize;
            let rh = 16usize;
            for y in 0..rh.min(h) {
                for x in 0..rw.min(w) {
                    let idx = y * w + x;
                    if idx < fb.len() && fb[idx] != 0xFF000000 {
                        cnt += 1;
                    }
                }
            }
            (cnt, rw.min(w) * rh.min(h))
        };
        if !quiet {
            println!("FB TOPLEFT: non_black={}/{}", tl_nonblack, tl_total);
        }
        // Sample first 10 non-black pixels
        if non_black > 0 && !quiet {
            let fb = self.bus.get_ppu().get_framebuffer();
            let samples: Vec<_> = fb
                .iter()
                .enumerate()
                .filter(|(_, &px)| px != 0xFF000000)
                .take(10)
                .map(|(idx, &px)| {
                    let x = idx % SCREEN_WIDTH;
                    let y = idx / SCREEN_WIDTH;
                    format!("({},{})=0x{:08X}", x, y, px)
                })
                .collect();
            println!("NON-BLACK SAMPLES: {}", samples.join(", "));
        }
    }

    pub(super) fn print_headless_cpu_pc_diagnostics(&self, config: &HeadlessDiagnosticsConfig) {
        if !config.log_cpu_pc {
            return;
        }

        println!(
            "CPU PC: {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} icount={}",
            self.cpu.pb(),
            self.cpu.pc(),
            self.cpu.a(),
            self.cpu.x(),
            self.cpu.y(),
            self.cpu.sp(),
            self.cpu.p().bits(),
            self.cpu.debug_instruction_count
        );
        let sa1 = self.bus.sa1();
        println!(
            "SA1 PC: {:02X}:{:04X} A={:04X} X={:04X} Y={:04X} SP={:04X} P={:02X} icount={}",
            sa1.cpu.pb(),
            sa1.cpu.pc(),
            sa1.cpu.a(),
            sa1.cpu.x(),
            sa1.cpu.y(),
            sa1.cpu.sp(),
            sa1.cpu.p().bits(),
            sa1.cpu.debug_instruction_count
        );
    }
}
