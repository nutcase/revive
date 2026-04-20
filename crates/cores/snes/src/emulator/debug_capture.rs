use super::{write_framebuffer_image, Emulator};
use std::sync::OnceLock;

impl Emulator {
    pub(super) fn maybe_dump_framebuffer_at(&mut self) {
        #[derive(Clone)]
        struct DumpCfg {
            frame: u64,
            quit: bool,
        }

        static CFG: OnceLock<Option<DumpCfg>> = OnceLock::new();
        let Some(cfg) = CFG
            .get_or_init(|| {
                let frame = std::env::var("DUMP_FRAME_AT")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())?;
                let quit = std::env::var("DUMP_FRAME_QUIT")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false);
                Some(DumpCfg { frame, quit })
            })
            .clone()
        else {
            return;
        };

        if self.frame_count != cfg.frame {
            return;
        }

        let out_path = std::env::var("DUMP_FRAME_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("logs/frame_{:05}.png", self.frame_count));
        let use_gui_frame = std::env::var("DUMP_FRAME_USE_GUI")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let fb = if use_gui_frame {
            self.framebuffer()
        } else {
            self.bus.get_ppu().get_framebuffer()
        };
        let _ = std::fs::create_dir_all("logs");
        if let Err(e) = write_framebuffer_image(std::path::Path::new(&out_path), fb, 256, 224) {
            eprintln!("DUMP_FRAME_AT: failed to write {}: {}", out_path, e);
        } else if !crate::debug_flags::quiet() {
            println!("DUMP_FRAME_AT: wrote {}", out_path);
        }

        if cfg.quit {
            crate::shutdown::request_quit();
        }
    }

    pub(super) fn maybe_dump_framebuffer_on_scanline(&mut self, scanline: u16) {
        #[derive(Clone)]
        struct DumpCfg {
            frame: u64,
            scanline: u16,
            quit: bool,
        }

        static CFG: OnceLock<Option<DumpCfg>> = OnceLock::new();
        let Some(cfg) = CFG
            .get_or_init(|| {
                let frame = std::env::var("DUMP_SCANLINE_FRAME")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())?;
                let scanline = std::env::var("DUMP_SCANLINE")
                    .ok()
                    .and_then(|v| v.parse::<u16>().ok())?;
                let quit = std::env::var("DUMP_FRAME_QUIT")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false);
                Some(DumpCfg {
                    frame,
                    scanline,
                    quit,
                })
            })
            .clone()
        else {
            return;
        };

        if self.frame_count != cfg.frame || scanline != cfg.scanline {
            return;
        }

        let out_path = std::env::var("DUMP_FRAME_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                format!(
                    "logs/frame_{:05}_sl{:03}.png",
                    self.frame_count, cfg.scanline
                )
            });
        let fb = self.bus.get_ppu().get_framebuffer();
        let _ = std::fs::create_dir_all("logs");
        if let Err(e) = write_framebuffer_image(std::path::Path::new(&out_path), fb, 256, 224) {
            eprintln!(
                "DUMP_SCANLINE: failed to write {} at frame {} scanline {}: {}",
                out_path, self.frame_count, cfg.scanline, e
            );
        } else if !crate::debug_flags::quiet() {
            println!(
                "DUMP_SCANLINE: wrote {} at frame {} scanline {}",
                out_path, self.frame_count, cfg.scanline
            );
        }

        if cfg.quit {
            crate::shutdown::request_quit();
        }
    }

    pub(super) fn maybe_dump_mem_at(&mut self) {
        #[derive(Clone)]
        struct DumpCfg {
            frame: u64,
            prefix: String,
            quit: bool,
            ppu_state: bool,
        }

        static CFG: OnceLock<Option<DumpCfg>> = OnceLock::new();
        let Some(cfg) = CFG
            .get_or_init(|| {
                let frame = std::env::var("DUMP_MEM_AT")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())?;
                let prefix = std::env::var("DUMP_MEM_PREFIX")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| format!("logs/mem_{:05}", frame));
                let quit = std::env::var("DUMP_MEM_QUIT")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false);
                let ppu_state = std::env::var("DUMP_MEM_PPU_STATE")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false);
                Some(DumpCfg {
                    frame,
                    prefix,
                    quit,
                    ppu_state,
                })
            })
            .clone()
        else {
            return;
        };

        if self.frame_count != cfg.frame {
            return;
        }

        let _ = std::fs::create_dir_all("logs");

        let ppu = self.bus.get_ppu();
        let bwram = self.bus.sa1_bwram_slice();
        let iram = self.bus.sa1_iram_slice();
        let write_bin = |suffix: &str, data: &[u8]| -> Result<(), String> {
            let path = format!("{}_{}.bin", cfg.prefix, suffix);
            std::fs::write(path, data).map_err(|e| e.to_string())
        };

        let mut ok = true;
        if let Err(e) = write_bin("wram", self.bus.wram()) {
            eprintln!("DUMP_MEM_AT: failed to write WRAM: {}", e);
            ok = false;
        }
        if let Err(e) = write_bin("vram", ppu.get_vram()) {
            eprintln!("DUMP_MEM_AT: failed to write VRAM: {}", e);
            ok = false;
        }
        if let Err(e) = write_bin("cgram", ppu.get_cgram()) {
            eprintln!("DUMP_MEM_AT: failed to write CGRAM: {}", e);
            ok = false;
        }
        if let Err(e) = write_bin("oam", ppu.get_oam()) {
            eprintln!("DUMP_MEM_AT: failed to write OAM: {}", e);
            ok = false;
        }
        if !bwram.is_empty() {
            if let Err(e) = write_bin("bwram", bwram) {
                eprintln!("DUMP_MEM_AT: failed to write BW-RAM: {}", e);
                ok = false;
            }
        }
        if !iram.is_empty() {
            if let Err(e) = write_bin("iram", iram) {
                eprintln!("DUMP_MEM_AT: failed to write SA-1 IRAM: {}", e);
                ok = false;
            }
        }
        // Dump SPC RAM (64KB) when DUMP_MEM_SPC_RAM is set
        if std::env::var_os("DUMP_MEM_SPC_RAM").is_some() {
            let mut spc_ram_buf = vec![0u8; 65536];
            self.bus.with_apu_mut(|apu| {
                let state = apu.inner.get_state();
                spc_ram_buf.copy_from_slice(&state.ram);
            });
            let spc_path = format!("{}_spcram.bin", cfg.prefix);
            if let Err(e) = std::fs::write(&spc_path, &spc_ram_buf) {
                eprintln!("DUMP_MEM_AT: failed to write SPC RAM: {}", e);
                ok = false;
            }
        }
        if std::env::var_os("DUMP_MEM_APU_STATE").is_some() {
            let mut apu_dump = String::new();
            self.bus.with_apu_mut(|apu| {
                let (smp_pc, smp_psw, smp_a, smp_x, smp_y, smp_sp, smp_stopped) =
                    if let Some(smp) = apu.inner.smp.as_ref() {
                        (
                            smp.reg_pc,
                            smp.get_psw(),
                            smp.reg_a,
                            smp.reg_x,
                            smp.reg_y,
                            smp.reg_sp,
                            smp.is_stopped() as u8,
                        )
                    } else {
                        (0, 0, 0, 0, 0, 0, 1)
                    };
                let mut code = [0u8; 16];
                if smp_stopped == 0 {
                    for (i, b) in code.iter_mut().enumerate() {
                        *b = apu
                            .inner
                            .read_u8(smp_pc.wrapping_add(i as u16) as u32);
                    }
                }
                let _ = std::fmt::Write::write_fmt(
                    &mut apu_dump,
                    format_args!(
                        "apu_cycles={}\nboot_state={}\nport_latch=[{:02X} {:02X} {:02X} {:02X}]\ninner_ports=[{:02X} {:02X} {:02X} {:02X}]\nsmp_pc={:04X} psw={:02X} A={:02X} X={:02X} Y={:02X} SP={:02X} stopped={}\ncode={:02X?}\n",
                        apu.total_smp_cycles,
                        apu.handshake_state_str(),
                        apu.port_latch[0],
                        apu.port_latch[1],
                        apu.port_latch[2],
                        apu.port_latch[3],
                        apu.inner.cpu_read_port(0),
                        apu.inner.cpu_read_port(1),
                        apu.inner.cpu_read_port(2),
                        apu.inner.cpu_read_port(3),
                        smp_pc,
                        smp_psw,
                        smp_a,
                        smp_x,
                        smp_y,
                        smp_sp,
                        smp_stopped,
                        code
                    ),
                );
            });
            let apu_path = format!("{}_apu.txt", cfg.prefix);
            if let Err(e) = std::fs::write(&apu_path, apu_dump) {
                eprintln!("DUMP_MEM_AT: failed to write APU state: {}", e);
                ok = false;
            }
        }
        // Always dump S-CPU core state when DUMP_MEM_AT is active (debug aid).
        let cpu_path = format!("{}_cpu.txt", cfg.prefix);
        let cpu_dump = format!(
            "pc={:02X}:{:04X}\nA={:04X} X={:04X} Y={:04X} SP={:04X}\nDB={:02X} DP={:04X}\nP={:02X} emu={} M8={} X8={}\n",
            self.cpu.pb(),
            self.cpu.pc(),
            self.cpu.a(),
            self.cpu.x(),
            self.cpu.y(),
            self.cpu.sp(),
            self.cpu.db(),
            self.cpu.dp(),
            self.cpu.p().bits(),
            self.cpu.emulation_mode(),
            self.cpu.p().contains(crate::cpu::StatusFlags::MEMORY_8BIT),
            self.cpu.p().contains(crate::cpu::StatusFlags::INDEX_8BIT),
        );
        if let Err(e) = std::fs::write(&cpu_path, cpu_dump) {
            eprintln!("DUMP_MEM_AT: failed to write CPU state: {}", e);
            ok = false;
        }
        if ok && !crate::debug_flags::quiet() {
            println!("DUMP_MEM_AT: wrote {}_*", cfg.prefix);
        }
        if cfg.ppu_state {
            self.bus.get_ppu().debug_ppu_state();
        }

        if cfg.quit {
            crate::shutdown::request_quit();
        }
    }

    pub(super) fn maybe_save_state_at(&mut self) {
        #[derive(Clone)]
        struct SaveCfg {
            frame: u64,
            path: String,
            quit: bool,
        }

        let read_cfg = || {
            let frame = std::env::var("SAVE_STATE_AT")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())?;
            let path = std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("logs/state_{:05}.json", frame));
            let quit = std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Some(SaveCfg { frame, path, quit })
        };
        let cfg = if cfg!(test) {
            read_cfg()
        } else {
            static CFG: OnceLock<Option<SaveCfg>> = OnceLock::new();
            CFG.get_or_init(read_cfg).clone()
        };
        let Some(cfg) = cfg else {
            return;
        };

        if self.frame_count != cfg.frame {
            return;
        }

        if let Some(parent) = std::path::Path::new(&cfg.path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match self.save_state_to_file(std::path::Path::new(&cfg.path)) {
            Ok(()) => {
                if !crate::debug_flags::quiet() {
                    println!("SAVE_STATE_AT: wrote {}", cfg.path);
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
            }
            Err(e) => eprintln!("SAVE_STATE_AT: failed to write {}: {}", cfg.path, e),
        }
    }

    pub(super) fn maybe_write_save_state(&mut self, path: &str) -> Result<(), String> {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        self.save_state_to_file(std::path::Path::new(path))
    }

    pub(super) fn request_save_state_capture_stop(&mut self) {
        self.save_state_capture_stop_requested = true;
    }

    pub(super) fn take_save_state_capture_stop_requested(&mut self) -> bool {
        std::mem::take(&mut self.save_state_capture_stop_requested)
    }

    pub(super) fn maybe_save_state_at_cpu_exec_pc(&mut self) -> bool {
        #[derive(Clone)]
        struct SavePcCfg {
            pc: u32,
            path: String,
            quit: bool,
        }

        let read_cfg = || {
            let raw_pc = std::env::var("SAVE_STATE_AT_CPU_EXEC_PC")
                .ok()
                .filter(|s| !s.trim().is_empty())?;
            let pc = Self::parse_hex_pc_token(&raw_pc)?;
            let path = std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("logs/state_pc_{pc:06X}.json"));
            let quit = std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Some(SavePcCfg { pc, path, quit })
        };
        let cfg = if cfg!(test) {
            read_cfg()
        } else {
            static CFG: OnceLock<Option<SavePcCfg>> = OnceLock::new();
            CFG.get_or_init(read_cfg).clone()
        };
        let Some(cfg) = cfg else {
            return false;
        };

        if self.bus.last_cpu_exec_pc != cfg.pc {
            return false;
        }

        match self.maybe_write_save_state(&cfg.path) {
            Ok(()) => {
                self.request_save_state_capture_stop();
                if !crate::debug_flags::quiet() {
                    println!(
                        "SAVE_STATE_AT_CPU_EXEC_PC: wrote {} at {:06X}",
                        cfg.path, cfg.pc
                    );
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "SAVE_STATE_AT_CPU_EXEC_PC: failed to write {}: {}",
                    cfg.path, e
                );
                false
            }
        }
    }

    pub(super) fn maybe_save_state_at_frame_anchor(&mut self) -> bool {
        if !Self::save_state_exact_capture_env_active() {
            return false;
        }

        if cfg!(test) {
            let has_value = |name: &str| {
                std::env::var(name)
                    .ok()
                    .is_some_and(|value| !value.trim().is_empty())
            };

            return (has_value("SAVE_STATE_AT_CPU_EXEC_PC")
                && self.maybe_save_state_at_cpu_exec_pc())
                || (has_value("SAVE_STATE_AT_GSU_PC") && self.maybe_save_state_at_gsu_pc())
                || (has_value("SAVE_STATE_AT_GSU_PC_RANGE")
                    && self.maybe_save_state_at_gsu_pc_range())
                || (has_value("SAVE_STATE_AT_GSU_REG_WRITE")
                    && self.maybe_save_state_at_gsu_reg_write())
                || (Self::save_state_at_superfx_ram_addr_env_active()
                    && self.maybe_save_state_at_superfx_ram_addr());
        }

        self.maybe_save_state_at_cpu_exec_pc()
            || self.maybe_save_state_at_gsu_pc()
            || self.maybe_save_state_at_gsu_pc_range()
            || self.maybe_save_state_at_gsu_reg_write()
            || self.maybe_save_state_at_superfx_ram_addr()
    }

    pub(super) fn maybe_save_state_at_gsu_pc(&mut self) -> bool {
        #[derive(Clone)]
        struct SaveGsuPcCfg {
            pc: u32,
            path: String,
            quit: bool,
        }

        let read_cfg = || {
            let raw_pc = std::env::var("SAVE_STATE_AT_GSU_PC")
                .ok()
                .filter(|s| !s.trim().is_empty())?;
            let pc = Self::parse_hex_pc_token(&raw_pc)?;
            let path = std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("logs/state_gsu_pc_{pc:06X}.json"));
            let quit = std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Some(SaveGsuPcCfg { pc, path, quit })
        };
        let cfg = if cfg!(test) {
            read_cfg()
        } else {
            static CFG: OnceLock<Option<SaveGsuPcCfg>> = OnceLock::new();
            CFG.get_or_init(read_cfg).clone()
        };
        let Some(cfg) = cfg else {
            return false;
        };

        let Some(gsu) = self.bus.superfx.as_ref() else {
            return false;
        };
        let gsu_pc = if gsu.debug_current_exec_pc() != 0 {
            ((gsu.debug_current_exec_pbr() as u32) << 16) | u32::from(gsu.debug_current_exec_pc())
        } else {
            ((gsu.debug_pbr() as u32) << 16) | u32::from(gsu.debug_reg(15))
        };
        if gsu_pc != cfg.pc {
            return false;
        }

        match self.maybe_write_save_state(&cfg.path) {
            Ok(()) => {
                self.request_save_state_capture_stop();
                if !crate::debug_flags::quiet() {
                    println!("SAVE_STATE_AT_GSU_PC: wrote {} at {:06X}", cfg.path, cfg.pc);
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
                true
            }
            Err(e) => {
                eprintln!("SAVE_STATE_AT_GSU_PC: failed to write {}: {}", cfg.path, e);
                false
            }
        }
    }

    pub(super) fn maybe_save_state_at_gsu_pc_range(&mut self) -> bool {
        #[derive(Clone)]
        struct SaveGsuPcRangeCfg {
            start: u32,
            end: u32,
            path: String,
            quit: bool,
            require_latched_hit: bool,
        }

        let read_cfg = || {
            let raw_range = std::env::var("SAVE_STATE_AT_GSU_PC_RANGE")
                .ok()
                .filter(|s| !s.trim().is_empty())?;
            let (start, end) = Self::parse_hex_pc_range(&raw_range)?;
            let path = std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("logs/state_gsu_pc_{start:06X}_{end:06X}.json"));
            let quit = std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let require_latched_hit = Self::save_state_at_gsu_pc_requires_latched_hit_env();
            Some(SaveGsuPcRangeCfg {
                start,
                end,
                path,
                quit,
                require_latched_hit,
            })
        };
        let cfg = if cfg!(test) {
            read_cfg()
        } else {
            static CFG: OnceLock<Option<SaveGsuPcRangeCfg>> = OnceLock::new();
            CFG.get_or_init(read_cfg).clone()
        };
        let Some(cfg) = cfg else {
            return false;
        };

        let Some(gsu) = self.bus.superfx.as_mut() else {
            return false;
        };
        let pc_hit = gsu.debug_take_save_state_pc_hit();
        if cfg.require_latched_hit && pc_hit.is_none() {
            return false;
        }
        let gsu_pc = if let Some((pbr, pc)) = pc_hit {
            ((pbr as u32) << 16) | u32::from(pc)
        } else if gsu.debug_current_exec_pc() != 0 {
            ((gsu.debug_current_exec_pbr() as u32) << 16) | u32::from(gsu.debug_current_exec_pc())
        } else {
            ((gsu.debug_pbr() as u32) << 16) | u32::from(gsu.debug_reg(15))
        };
        if gsu_pc < cfg.start || gsu_pc > cfg.end {
            return false;
        }

        match self.maybe_write_save_state(&cfg.path) {
            Ok(()) => {
                self.request_save_state_capture_stop();
                if !crate::debug_flags::quiet() {
                    println!(
                        "SAVE_STATE_AT_GSU_PC_RANGE: wrote {} at {:06X} in {:06X}-{:06X}",
                        cfg.path, gsu_pc, cfg.start, cfg.end
                    );
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "SAVE_STATE_AT_GSU_PC_RANGE: failed to write {}: {}",
                    cfg.path, e
                );
                false
            }
        }
    }

    pub(super) fn maybe_save_state_at_gsu_reg_write(&mut self) -> bool {
        #[derive(Clone)]
        struct SaveGsuRegWriteCfg {
            path: String,
            quit: bool,
        }

        let read_cfg = || {
            std::env::var("SAVE_STATE_AT_GSU_REG_WRITE")
                .ok()
                .filter(|s| !s.trim().is_empty())?;
            let path = std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "logs/state_gsu_reg_write.json".to_string());
            let quit = std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            Some(SaveGsuRegWriteCfg { path, quit })
        };
        let cfg = if cfg!(test) {
            read_cfg()
        } else {
            static CFG: OnceLock<Option<SaveGsuRegWriteCfg>> = OnceLock::new();
            CFG.get_or_init(read_cfg).clone()
        };
        let Some(cfg) = cfg else {
            return false;
        };

        let Some(gsu) = self.bus.superfx.as_mut() else {
            return false;
        };
        let Some((pbr, pc)) = gsu.debug_take_save_state_pc_hit() else {
            return false;
        };

        match self.maybe_write_save_state(&cfg.path) {
            Ok(()) => {
                self.request_save_state_capture_stop();
                if !crate::debug_flags::quiet() {
                    println!(
                        "SAVE_STATE_AT_GSU_REG_WRITE: wrote {} at {:06X}",
                        cfg.path,
                        ((pbr as u32) << 16) | u32::from(pc)
                    );
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "SAVE_STATE_AT_GSU_REG_WRITE: failed to write {}: {}",
                    cfg.path, e
                );
                false
            }
        }
    }

    pub(super) fn save_state_at_gsu_pc_requires_latched_hit_env() -> bool {
        let has_value = |name: &str| {
            std::env::var(name)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        };
        let read = || {
            has_value("TRACE_SUPERFX_EXEC_AT_FRAME")
                || has_value("SAVE_STATE_AT_GSU_PC_HIT_INDEX")
                || has_value("SAVE_STATE_AT_GSU_REG_EQ")
                || has_value("SAVE_STATE_AT_GSU_REG_WRITE")
                || has_value("SAVE_STATE_AT_GSU_RECENT_EXEC_TAIL")
        };
        if cfg!(test) {
            read()
        } else {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(read)
        }
    }

    #[cfg(test)]
    pub(super) fn gsu_only_save_requires_latched_hit_env(
        save_state_at_gsu_pc_range: bool,
        save_state_at_superfx_ram_addr: bool,
    ) -> bool {
        save_state_at_gsu_pc_range
            || save_state_at_superfx_ram_addr
            || Self::save_state_at_gsu_pc_requires_latched_hit_env()
    }

    #[cfg(test)]
    pub(super) fn gsu_only_should_save_state(
        save_state_at_gsu_pc_range: bool,
        save_state_at_superfx_ram_addr: bool,
        save_state_pc_hit: Option<(u8, u16)>,
        save_state_ram_addr_hit: Option<(u8, u16, u16)>,
    ) -> bool {
        if Self::gsu_only_save_requires_latched_hit_env(
            save_state_at_gsu_pc_range,
            save_state_at_superfx_ram_addr,
        ) {
            save_state_pc_hit.is_some() || save_state_ram_addr_hit.is_some()
        } else {
            true
        }
    }

    pub(super) fn save_state_exact_capture_env_active() -> bool {
        let has_value = |name: &str| {
            std::env::var(name)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        };
        let read = || {
            has_value("SAVE_STATE_AT_CPU_EXEC_PC")
                || has_value("SAVE_STATE_AT_GSU_PC")
                || has_value("SAVE_STATE_AT_GSU_PC_RANGE")
                || has_value("SAVE_STATE_AT_GSU_REG_WRITE")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_ADDRS")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ")
        };
        if cfg!(test) {
            read()
        } else {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(read)
        }
    }

    pub(super) fn save_state_at_superfx_ram_addr_env_active() -> bool {
        let has_value = |name: &str| {
            std::env::var(name)
                .ok()
                .is_some_and(|value| !value.trim().is_empty())
        };
        let read = || {
            has_value("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_ADDRS")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ")
                || has_value("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ")
        };
        if cfg!(test) {
            read()
        } else {
            static VALUE: OnceLock<bool> = OnceLock::new();
            *VALUE.get_or_init(read)
        }
    }

    pub(super) fn superfx_save_state_hit_pending(&self) -> bool {
        self.bus
            .superfx
            .as_ref()
            .is_some_and(|gsu| gsu.debug_has_pending_save_state_hit())
    }

    pub(super) fn maybe_save_state_at_superfx_ram_addr(&mut self) -> bool {
        #[derive(Clone)]
        struct SaveSuperfxRamAddrCfg {
            path: String,
            quit: bool,
        }

        if !Self::save_state_at_superfx_ram_addr_env_active() {
            return false;
        }
        let cfg = SaveSuperfxRamAddrCfg {
            path: std::env::var("SAVE_STATE_PATH")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "logs/state_superfx_ram_addr.json".to_string()),
            quit: std::env::var("SAVE_STATE_QUIT")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        };

        let Some(gsu) = self.bus.superfx.as_mut() else {
            return false;
        };
        let Some((pbr, pc, addr)) = gsu.debug_take_save_state_ram_addr_hit() else {
            return false;
        };

        if std::env::var_os("TRACE_SUPERFX_LAST_WRITERS").is_some() {
            eprintln!(
                "[SAVE-RAM-HIT-GSU] pbr={:02X} pc={:04X} addr={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r9={:04X} r10={:04X} r11={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} sfr={:04X}",
                gsu.debug_pbr(),
                gsu.debug_reg(15).wrapping_sub(1),
                addr,
                gsu.debug_reg(1),
                gsu.debug_reg(2),
                gsu.debug_reg(3),
                gsu.debug_reg(4),
                gsu.debug_reg(5),
                gsu.debug_reg(6),
                gsu.debug_reg(7),
                gsu.debug_reg(9),
                gsu.debug_reg(10),
                gsu.debug_reg(11),
                gsu.debug_reg(12),
                gsu.debug_reg(13),
                gsu.debug_reg(14),
                gsu.debug_reg(15),
                gsu.read_register(0x3030, 0) as u16 | ((gsu.read_register(0x3031, 0) as u16) << 8),
            );
            eprintln!(
                "[SAVE-RAM-HIT-LAST] r0={:?} r1={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r10={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                gsu.debug_last_nontrivial_reg_write(0),
                gsu.debug_last_nontrivial_reg_write(1),
                gsu.debug_last_nontrivial_reg_write(4),
                gsu.debug_last_nontrivial_reg_write(5),
                gsu.debug_last_nontrivial_reg_write(6),
                gsu.debug_last_nontrivial_reg_write(7),
                gsu.debug_last_nontrivial_reg_write(8),
                gsu.debug_last_nontrivial_reg_write(9),
                gsu.debug_last_nontrivial_reg_write(10),
                gsu.debug_last_nontrivial_reg_write(11),
                gsu.debug_last_nontrivial_reg_write(12),
                gsu.debug_last_nontrivial_reg_write(13),
                gsu.debug_last_nontrivial_reg_write(14),
                gsu.debug_last_nontrivial_reg_write(15),
            );
            let reg_filters = std::env::var("TRACE_SUPERFX_RECENT_REGS")
                .ok()
                .map(|raw| {
                    raw.split(',')
                        .filter_map(|token| {
                            let token = token.trim();
                            if token.is_empty() {
                                return None;
                            }
                            let token = token.strip_prefix('r').unwrap_or(token);
                            let token = token.strip_prefix('R').unwrap_or(token);
                            let parsed = token.parse::<u8>().ok().or_else(|| {
                                let token = token.trim_start_matches("0x").trim_start_matches("0X");
                                u8::from_str_radix(token, 16).ok()
                            })?;
                            Some(parsed & 0x0F)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if !reg_filters.is_empty() {
                let limit = std::env::var("TRACE_SUPERFX_RECENT_REGS_LIMIT")
                    .ok()
                    .and_then(|raw| raw.trim().parse::<usize>().ok())
                    .filter(|&n| n > 0)
                    .unwrap_or(64);
                for reg in reg_filters {
                    eprintln!(
                        "[SAVE-RAM-HIT-RECENT] r{} last_any={:?} recent={:?}",
                        reg,
                        gsu.debug_last_reg_write(reg),
                        gsu.debug_recent_reg_writes_for_reg(reg, limit),
                    );
                }
            }
        }

        match self.maybe_write_save_state(&cfg.path) {
            Ok(()) => {
                self.request_save_state_capture_stop();
                if !crate::debug_flags::quiet() {
                    println!(
                        "SAVE_STATE_AT_SUPERFX_RAM_ADDR: wrote {} at {:02X}:{:04X} addr={:04X}",
                        cfg.path, pbr, pc, addr
                    );
                }
                if cfg.quit {
                    crate::shutdown::request_quit();
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "SAVE_STATE_AT_SUPERFX_RAM_ADDR: failed to write {}: {}",
                    cfg.path, e
                );
                false
            }
        }
    }

    pub(super) fn parse_hex_pc_token(raw: &str) -> Option<u32> {
        let token = raw
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X")
            .replace(':', "");
        (!token.is_empty())
            .then_some(token)
            .and_then(|token| u32::from_str_radix(&token, 16).ok())
    }

    pub(super) fn parse_hex_pc_range(raw: &str) -> Option<(u32, u32)> {
        let (start_raw, end_raw) = raw.trim().split_once('-')?;
        let start_raw = start_raw.trim();
        let end_raw = end_raw.trim();
        let start = Self::parse_hex_pc_token(start_raw)?;
        let end = if end_raw.contains(':') {
            Self::parse_hex_pc_token(end_raw)?
        } else if let Some((bank, _)) = start_raw.split_once(':') {
            Self::parse_hex_pc_token(&format!("{bank}:{end_raw}"))?
        } else {
            Self::parse_hex_pc_token(end_raw)?
        };
        Some((start.min(end), start.max(end)))
    }
}
