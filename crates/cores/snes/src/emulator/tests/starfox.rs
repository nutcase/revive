use super::*;

#[test]
fn star_fox_bootstrap_does_not_leave_superfx_go_visible() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "24");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let pc = ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32;
    let (sfr_low, _) = emulator.read_superfx_sfr_bytes_direct();
    eprintln!("[STARFOX] frame24 pc={pc:06X} sfr_low={sfr_low:02X}");

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert_eq!(sfr_low & 0x20, 0, "pc={pc:06X} sfr_low={sfr_low:02X}");
    assert_ne!(pc, 0x7E4EFD, "stuck in Star Fox SuperFX wait loop");
}
#[test]
#[ignore] // GSU 3D rendering makes this too slow for CI until cycle-accurate scheduling
fn star_fox_enters_visible_phase_within_300_frames() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "300");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let inidisp = emulator.bus.get_ppu().screen_display;
    let tm = emulator.bus.get_ppu().get_main_screen_designation();
    let bg_mode = emulator.bus.get_ppu().get_bg_mode();
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    // Star Fox toggles INIDISP within a frame, so the final snapshot may still be blank.
    // What matters for regression is that it entered a visible phase and rendered pixels.
    assert_ne!(tm, 0, "TM={tm:02X} INIDISP={inidisp:02X}");
    assert_ne!(bg_mode, 0, "mode={bg_mode} INIDISP={inidisp:02X}");
    assert!(
        non_black > 0,
        "expected visible output by 300 frames (INIDISP={inidisp:02X})"
    );
}
#[test]
fn star_fox_diagnostic_pc_after_120_frames() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let frames = std::env::var("STARFOX_DIAG_FRAMES")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "120".to_string());
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", &frames);
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let pc = ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32;
    let a = emulator.cpu.a();
    let x = emulator.cpu.x();
    let y = emulator.cpu.y();
    let sp = emulator.cpu.sp();
    let d = emulator.cpu.dp();
    let db = emulator.cpu.db();
    let p = emulator.cpu.core.state().p.bits();
    let emu = emulator.cpu.core.state().emulation_mode;
    let mut bytes = [0u8; 8];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = emulator.bus.read_u8(pc.wrapping_add(i as u32));
    }
    let (sfr_low, sfr_high) = emulator.read_superfx_sfr_bytes_direct();
    let inidisp = emulator.bus.get_ppu().screen_display;
    let brightness = emulator.bus.get_ppu().brightness;
    let tm = emulator.bus.get_ppu().get_main_screen_designation();
    let bg_mode = emulator.bus.get_ppu().get_bg_mode();
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();
    let _ = emulator.bus.read_u8(0x00_2137);
    let ophct_lo = emulator.bus.read_u8(0x00_213C);
    let ophct_hi = emulator.bus.read_u8(0x00_213C) & 0x01;
    eprintln!(
        "[STARFOX{}] pc={pc:06X} a={:04X} x={:04X} y={:04X} sp={:04X} d={:04X} db={:02X} p={:02X} e={} sfr={:02X}{:02X} inidisp={:02X} bright={} tm={:02X} mode={} non_black={} ophct={:01X}{:02X} code={:02X?}",
        frames, a, x, y, sp, d, db, p, emu as u8, sfr_high, sfr_low, inidisp, brightness, tm, bg_mode, non_black, ophct_hi, ophct_lo, bytes
    );
    if let Ok(path) = std::env::var("STARFOX_SAVE_STATE_PATH") {
        if !path.is_empty() {
            emulator
                .save_state_to_file(Path::new(&path))
                .expect("failed to save Star Fox diagnostic state");
            eprintln!("[STARFOX{}-SAVED] {}", frames, path);
        }
    }
    if std::env::var_os("STARFOX_DIAG_PPU").is_some() {
        let ppu = emulator.bus.get_ppu();
        ppu.debug_ppu_state();
        let cgram = ppu.dump_cgram_head(16);
        eprintln!("[STARFOX{}-PPU] CGRAM_HEAD={:04X?}", frames, cgram);
        if let Some((min_x, min_y, max_x, max_y, count)) = ppu.dbg_superfx_direct_pixel_bounds() {
            eprintln!(
                "[STARFOX{}-PPU-SFX-BOUNDS] min=({}, {}) max=({}, {}) nonzero_pixels={}",
                frames, min_x, min_y, max_x, max_y, count
            );
        } else {
            eprintln!("[STARFOX{}-PPU-SFX-BOUNDS] <none>", frames);
        }
        if let Some((sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color)) =
            ppu.dbg_superfx_direct_sample(128, 96)
        {
            eprintln!(
                "[STARFOX{}-PPU-SFX] x=128 y=96 sxy=({}, {}) tile_base=0x{:04X} row={} p0={:02X} p1={:02X} p2={:02X} p3={:02X} msb_color=0x{:02X} lsb_color=0x{:02X}",
                frames,
                sx,
                sy,
                tile_base,
                row_in_tile,
                p0,
                p1,
                p2,
                p3,
                msb_color,
                lsb_color
            );
        } else {
            eprintln!("[STARFOX{}-PPU-SFX] x=128 y=96 <none>", frames);
        }
        for bg in 0..=1usize {
            let (map_base, tile_base) = ppu.dbg_bg_bases(bg);
            let (map_nonzero, map_samples) = ppu.analyze_vram_region(map_base, 128);
            let (tile_nonzero, tile_samples) = ppu.analyze_vram_region(tile_base, 256);
            let map_entry = if map_samples.len() >= 2 {
                u16::from(map_samples[0]) | (u16::from(map_samples[1]) << 8)
            } else {
                0
            };
            let tile_id = map_entry & 0x03FF;
            let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(16)) & 0x7FFF;
            let (ref_nonzero, ref_samples) = ppu.analyze_vram_region(tile_addr, 32);
            eprintln!(
                "[STARFOX{}-PPU-BG{}] map_base=0x{:04X} map_nonzero={} map_head={:02X?} tile_base=0x{:04X} tile_nonzero={} tile_head={:02X?} entry=0x{:04X} tile_id=0x{:03X} tile_addr=0x{:04X} tile_ref_nonzero={} tile_ref_head={:02X?}",
                frames,
                bg + 1,
                map_base,
                map_nonzero,
                &map_samples[..map_samples.len().min(16)],
                tile_base,
                tile_nonzero,
                &tile_samples[..tile_samples.len().min(16)],
                map_entry,
                tile_id,
                tile_addr,
                ref_nonzero,
                &ref_samples[..ref_samples.len().min(16)]
            );
        }
    }
    if let Some(gsu) = emulator.bus.superfx.as_ref() {
        let nonzero_range = gsu.debug_nonzero_game_ram_range();
        eprintln!(
            "[STARFOX{}-GSU] pbr={:02X} r0={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r8={:04X} r9={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} rombr={:02X} rambr={:02X} scbr={:02X} scmr={:02X} cbr={:04X} cfgr={:02X} colr={:02X} por={:02X} src=r{} dst=r{} game_ram_nonzero={} screen_nonzero={} range={:?}",
            frames,
            gsu.debug_pbr(),
            gsu.debug_reg(0),
            gsu.debug_reg(1),
            gsu.debug_reg(2),
            gsu.debug_reg(3),
            gsu.debug_reg(4),
            gsu.debug_reg(5),
            gsu.debug_reg(6),
            gsu.debug_reg(7),
            gsu.debug_reg(8),
            gsu.debug_reg(9),
            gsu.debug_reg(12),
            gsu.debug_reg(13),
            gsu.debug_reg(14),
            gsu.debug_reg(15),
            gsu.debug_rombr(),
            gsu.debug_rambr(),
            gsu.debug_scbr(),
            gsu.debug_scmr(),
            gsu.debug_cbr(),
            gsu.debug_cfgr(),
            gsu.debug_colr(),
            gsu.debug_por(),
            gsu.debug_src_reg(),
            gsu.debug_dst_reg(),
            gsu.debug_nonzero_game_ram(),
            gsu.debug_nonzero_screen_region(),
            nonzero_range
        );
        if let Some((buffer, height, bpp, mode)) = emulator.bus.superfx_screen_buffer_snapshot() {
            let first_nonzero = buffer.iter().position(|&byte| byte != 0);
            let head_len = buffer.len().min(64);
            let around = first_nonzero.map(|idx| {
                let start = idx.saturating_sub(16);
                let end = (idx + 48).min(buffer.len());
                (idx, buffer[start..end].to_vec())
            });
            eprintln!(
                "[STARFOX{}-GSU-SCREEN] len={} height={} bpp={} mode={} first_nonzero={:?} head={:02X?} around={:?}",
                frames,
                buffer.len(),
                height,
                bpp,
                mode,
                first_nonzero,
                &buffer[..head_len],
                around
            );
        }
        if let Some((buffer, height, bpp, mode)) = emulator.bus.superfx_tile_buffer_snapshot() {
            let first_nonzero = buffer.iter().position(|&byte| byte != 0);
            let head_len = buffer.len().min(64);
            let around = first_nonzero.map(|idx| {
                let start = idx.saturating_sub(16);
                let end = (idx + 48).min(buffer.len());
                (idx, buffer[start..end].to_vec())
            });
            eprintln!(
                "[STARFOX{}-GSU-TILE] len={} height={} bpp={} mode={} first_nonzero={:?} head={:02X?} around={:?}",
                frames,
                buffer.len(),
                height,
                bpp,
                mode,
                first_nonzero,
                &buffer[..head_len],
                around
            );
        }
        eprintln!(
            "[STARFOX{}-GSU-STOPS] recent={:?}",
            frames,
            gsu.debug_recent_stop_snapshot_metas(16)
        );
        if std::env::var_os("TRACE_SUPERFX_PROFILE").is_some() {
            eprintln!(
                "[STARFOX{}-GSU-PROFILE] top={:?} top_alt={:?}",
                frames,
                gsu.debug_top_profile(24),
                gsu.debug_top_profile_by_alt(24)
            );
        }
        if std::env::var_os("TRACE_SUPERFX_PC_TRACE").is_some()
            || std::env::var_os("TRACE_SUPERFX_LAST_TRANSFERS").is_some()
        {
            eprintln!(
                "[STARFOX{}-GSU-PC] recent={:?}",
                frames,
                gsu.debug_recent_pc_transfers()
            );
        }
        if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some()
            || std::env::var_os("TRACE_SUPERFX_LAST_WRITERS").is_some()
        {
            if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some() {
                eprintln!(
                    "[STARFOX{}-GSU-REG] recent={:?}",
                    frames,
                    gsu.debug_recent_reg_writes()
                );
            }
            eprintln!(
                "[STARFOX{}-GSU-LAST] r0={:?} r1={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r10={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                frames,
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
                gsu.debug_last_nontrivial_reg_write(15)
            );
            eprintln!(
                "[STARFOX{}-GSU-LAST-HIST] r0={:?} r1={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r10={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                frames,
                gsu.debug_recent_nontrivial_reg_writes(0),
                gsu.debug_recent_nontrivial_reg_writes(1),
                gsu.debug_recent_nontrivial_reg_writes(4),
                gsu.debug_recent_nontrivial_reg_writes(5),
                gsu.debug_recent_nontrivial_reg_writes(6),
                gsu.debug_recent_nontrivial_reg_writes(7),
                gsu.debug_recent_nontrivial_reg_writes(8),
                gsu.debug_recent_nontrivial_reg_writes(9),
                gsu.debug_recent_nontrivial_reg_writes(10),
                gsu.debug_recent_nontrivial_reg_writes(11),
                gsu.debug_recent_nontrivial_reg_writes(12),
                gsu.debug_recent_nontrivial_reg_writes(13),
                gsu.debug_recent_nontrivial_reg_writes(14),
                gsu.debug_recent_nontrivial_reg_writes(15)
            );
            eprintln!(
                "[STARFOX{}-GSU-LOWRAM] 0020={:?} 0021={:?} 0022={:?} 0023={:?} 0024={:?} 0025={:?} 0026={:?} 0027={:?} 0030={:?} 0031={:?} 0032={:?} 0033={:?}",
                frames,
                gsu.debug_last_low_ram_write(0x0020),
                gsu.debug_last_low_ram_write(0x0021),
                gsu.debug_last_low_ram_write(0x0022),
                gsu.debug_last_low_ram_write(0x0023),
                gsu.debug_last_low_ram_write(0x0024),
                gsu.debug_last_low_ram_write(0x0025),
                gsu.debug_last_low_ram_write(0x0026),
                gsu.debug_last_low_ram_write(0x0027),
                gsu.debug_last_low_ram_write(0x0030),
                gsu.debug_last_low_ram_write(0x0031),
                gsu.debug_last_low_ram_write(0x0032),
                gsu.debug_last_low_ram_write(0x0033)
            );
            eprintln!(
                "[STARFOX{}-GSU-SEED] r0={:?} r1={:?} r4={:?}",
                frames,
                gsu.debug_last_reg_write_excluding(
                    0,
                    &[
                        0x03, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA,
                        0xCB, 0xCC, 0xCD, 0xCE, 0xCF
                    ],
                ),
                gsu.debug_last_reg_write_excluding(1, &[0xD1, 0xE1]),
                gsu.debug_last_reg_write_excluding(4, &[0x04, 0xE4])
            );
            if let Some(raw) = std::env::var_os("TRACE_SUPERFX_LOW_RAM_WORDS") {
                let raw = raw.to_string_lossy();
                for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    let token = token.trim_start_matches("0x").trim_start_matches("0X");
                    let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                        continue;
                    };
                    let value = gsu.debug_read_ram_word_short(addr);
                    let lo = gsu.debug_last_low_ram_write(addr);
                    let hi = gsu.debug_last_low_ram_write(addr.wrapping_add(1));
                    eprintln!(
                        "[STARFOX{}-GSU-RAM] addr={:04X} value={:04X} lo={:?} hi={:?}",
                        frames, addr, value, lo, hi
                    );
                }
            }
            if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some() {
                let recent_exec = gsu.debug_recent_exec_trace();
                eprintln!("[STARFOX{}-GSU-EXEC-LEN] {}", frames, recent_exec.len());
                for entry in recent_exec.iter().rev().take(64) {
                    eprintln!("[STARFOX{}-GSU-EXEC] {:?}", frames, entry);
                }
            }
        }
    }
    if let Ok(addrs) = std::env::var("STARFOX_DIAG_DUMP_ADDRS") {
        for part in addrs.split(',') {
            let raw = part.trim();
            if raw.is_empty() {
                continue;
            }
            let addr = if let Some((bank, off)) = raw.split_once(':') {
                match (
                    u8::from_str_radix(bank.trim_start_matches("0x"), 16),
                    u16::from_str_radix(off.trim_start_matches("0x"), 16),
                ) {
                    (Ok(bank), Ok(off)) => ((bank as u32) << 16) | off as u32,
                    _ => continue,
                }
            } else {
                match u32::from_str_radix(raw.trim_start_matches("0x"), 16) {
                    Ok(addr) => addr,
                    Err(_) => continue,
                }
            };
            let mut bytes = [0u8; 16];
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = emulator.bus.read_u8(addr.wrapping_add(i as u32));
            }
            eprintln!("[STARFOX{}-DUMP] {:06X}: {:02X?}", frames, addr, bytes);
        }
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_step_one_frame_probe_25_frames() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");

    for _ in 0..25 {
        let frame_before = emulator.frame_count();
        emulator.step_one_frame();
        eprintln!(
            "[STARFOX-STEP-PROBE] frame={} cpu_pc={:06X} inidisp={:02X} tm={:02X} mode={}",
            frame_before,
            emulator.current_cpu_pc(),
            emulator.current_inidisp(),
            emulator.current_tm(),
            emulator.current_bg_mode()
        );
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_unblank_frame_scan() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");

    let max_frames = std::env::var("STARFOX_UNBLANK_MAX_FRAMES")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(600);
    let unblanked = emulator.warmup_until_unblanked(max_frames);
    let ppu = emulator.bus.get_ppu();
    eprintln!(
        "[STARFOX-UNBLANK] unblanked={} frame={} inidisp={:02X} bright={} tm={:02X} mode={} non_black={}",
        unblanked,
        emulator.frame_count(),
        ppu.screen_display,
        ppu.current_brightness(),
        ppu.get_main_screen_designation(),
        ppu.get_bg_mode(),
        ppu.get_framebuffer()
            .iter()
            .filter(|&&p| (p & 0x00FF_FFFF) != 0)
            .count()
    );
    if unblanked {
        if let Ok(path) = std::env::var("STARFOX_SAVE_STATE_PATH") {
            if !path.is_empty() {
                emulator
                    .save_state_to_file(Path::new(&path))
                    .expect("failed to save Star Fox unblank state");
                eprintln!("[STARFOX-UNBLANK-SAVED] {}", path);
            }
        }
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_dump_frame_184_png() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    while emulator.frame_count() < 184 {
        emulator.step_one_frame();
    }
    let path = Path::new("/tmp/starfox_ppu_184_test.png");
    write_framebuffer_png(path, emulator.bus.get_ppu().get_framebuffer(), 256, 224)
        .expect("failed to write png");
    eprintln!("STARFOX_DUMP {}", path.display());

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_dump_frame_184_no_color_math_png() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    while emulator.frame_count() < 184 {
        emulator.step_one_frame();
    }
    {
        let ppu = emulator.bus.get_ppu_mut();
        ppu.cgwsel = 0x00;
        ppu.cgadsub = 0x00;
    }
    let path = Path::new("/tmp/starfox_ppu_184_nomath.png");
    write_framebuffer_png(path, emulator.bus.get_ppu().get_framebuffer(), 256, 224)
        .expect("failed to write png");
    eprintln!("STARFOX_DUMP {}", path.display());

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_startup_similarity_scan_post_unblank() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    let ref_path = Path::new("/tmp/starfox_startup_ref.png");
    if !rom_path.exists() || !ref_path.exists() {
        return;
    }

    let sample_step = std::env::var("STARFOX_POST_UNBLANK_STEP")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(4);
    let max_frames = std::env::var("STARFOX_POST_UNBLANK_MAX")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(120);

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let (ref_w, ref_h, ref_rgba_raw) = read_png_rgba(ref_path).expect("failed to read startup ref");
    let ref_rgba = if ref_w == 256 && ref_h == 224 {
        ref_rgba_raw
    } else {
        resize_rgba_nearest(&ref_rgba_raw, ref_w as usize, ref_h as usize, 256, 224)
    };

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    assert!(emulator.warmup_until_unblanked(600));

    let start_frame = emulator.frame_count();
    let end_frame = start_frame.saturating_add(max_frames);
    let mut best_frame = start_frame;
    let mut best_score = f64::INFINITY;
    let mut best_rgba =
        framebuffer_to_rgba_bytes_cropped(emulator.bus.get_ppu().get_framebuffer(), 256, 256, 224);

    while emulator.frame_count() <= end_frame {
        if (emulator.frame_count() - start_frame) % sample_step == 0 {
            let rgba = framebuffer_to_rgba_bytes_cropped(
                emulator.bus.get_ppu().get_framebuffer(),
                256,
                256,
                224,
            );
            let score = weighted_startup_score(&rgba, &ref_rgba);
            eprintln!(
                "[STARFOX-POST-UNBLANK-SCAN] frame={} score={:.4} cpu_pc={:06X}",
                emulator.frame_count(),
                score,
                emulator.current_cpu_pc()
            );
            if score < best_score {
                best_score = score;
                best_frame = emulator.frame_count();
                best_rgba = rgba;
            }
        }
        if emulator.frame_count() == end_frame {
            break;
        }
        emulator.step_one_frame();
    }

    let best_path = Path::new("/tmp/starfox_post_unblank_best.png");
    save_rgba_png(best_path, &best_rgba, 256, 224).expect("failed to write best png");
    eprintln!(
        "[STARFOX-POST-UNBLANK-BEST] frame={} score={:.4} path={}",
        best_frame,
        best_score,
        best_path.display()
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_diagnostic_from_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }
    let save_state_path = std::env::var("STARFOX_SAVE_STATE_PATH")
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let print_trace_last = std::env::var("STARFOX_PRINT_TRACE_LAST")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0);
    let gsu_overrides = parse_star_fox_gsu_test_overrides();
    let diag_cpu_bytes = std::env::var("STARFOX_DIAG_CPU_BYTES")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .split(',')
                .filter_map(|part| {
                    let token = part
                        .trim()
                        .trim_start_matches("0x")
                        .trim_start_matches("0X");
                    u32::from_str_radix(token, 16).ok()
                })
                .collect::<Vec<_>>()
        })
        .filter(|list| !list.is_empty());

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let step_frames: u64 = std::env::var("STARFOX_STEP_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");
    apply_star_fox_gsu_test_overrides(&mut emulator, &gsu_overrides);
    let start_frame = emulator.frame_count();
    let mut exact_capture_stopped = false;
    for _ in 0..step_frames {
        if emulator.step_one_frame_inner() {
            exact_capture_stopped = Emulator::save_state_exact_capture_env_active();
            break;
        }
        if crate::shutdown::should_quit() {
            break;
        }
    }

    let end_frame = emulator.frame_count();
    let pc = ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32;
    let (sfr_low, sfr_high) = emulator.read_superfx_sfr_bytes_direct();
    let tm = emulator.bus.get_ppu().get_main_screen_designation();
    let inidisp = emulator.bus.get_ppu().screen_display;
    let brightness = emulator.bus.get_ppu().brightness;
    let bg_mode = emulator.bus.get_ppu().get_bg_mode();
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();
    eprintln!(
        "[STARFOX-STATE] state={} start_frame={} step_frames={} end_frame={} pc={:06X} sfr={:02X}{:02X} inidisp={:02X} bright={} tm={:02X} non_black={}",
        state_path.display(),
        start_frame,
        step_frames,
        end_frame,
        pc,
        sfr_high,
        sfr_low,
        inidisp,
        brightness,
        tm,
        non_black
    );
    if let Some(count) = print_trace_last {
        emulator.debugger.print_trace(count);
    }
    if std::env::var_os("STARFOX_DIAG_PERF").is_some() {
        let stats = &emulator.performance_stats;
        let frames = stats.total_frames.max(1) as f64;
        eprintln!(
            "[STARFOX-STATE-PERF] frames={} frame_avg_ms={:.3} cpu_ms={:.3} ppu_ms={:.3} dma_ms={:.3} sa1_ms={:.3} apu_ms={:.3} sync_ms={:.3} input_ms={:.3} render_ms={:.3} copy_ms={:.3}",
            stats.total_frames,
            stats.frame_time_avg.as_secs_f64() * 1000.0,
            stats.cpu_time_total.as_secs_f64() * 1000.0 / frames,
            stats.ppu_time_total.as_secs_f64() * 1000.0 / frames,
            stats.dma_time_total.as_secs_f64() * 1000.0 / frames,
            stats.sa1_time_total.as_secs_f64() * 1000.0 / frames,
            stats.apu_time_total.as_secs_f64() * 1000.0 / frames,
            stats.sync_time_total.as_secs_f64() * 1000.0 / frames,
            stats.input_time_total.as_secs_f64() * 1000.0 / frames,
            stats.render_time_total.as_secs_f64() * 1000.0 / frames,
            stats.copy_time_total.as_secs_f64() * 1000.0 / frames,
        );
    }
    if std::env::var_os("STARFOX_DIAG_PPU").is_some() {
        let ppu = emulator.bus.get_ppu();
        eprintln!(
            "[STARFOX-STATE-PPU] mode={} inidisp={:02X} bright={} tm={:02X}",
            bg_mode, inidisp, brightness, tm
        );
        eprintln!(
            "[STARFOX-STATE-PPU-SCROLL] bg1=({:04X},{:04X}) bg2=({:04X},{:04X}) bg3=({:04X},{:04X}) bg4=({:04X},{:04X})",
            ppu.bg1_hscroll,
            ppu.bg1_vscroll,
            ppu.bg2_hscroll,
            ppu.bg2_vscroll,
            ppu.bg3_hscroll,
            ppu.bg3_vscroll,
            ppu.bg4_hscroll,
            ppu.bg4_vscroll,
        );
        if let Some(gsu) = emulator.bus.superfx.as_ref() {
            if let Some((pc, pbr, scbr, height, bpp, mode, len)) =
                gsu.debug_selected_screen_snapshot_meta()
            {
                eprintln!(
                    "[STARFOX-STATE-SFX-SNAPSHOT] selected pc={:02X}:{:04X} scbr={:02X} h={} bpp={} mode={} len=0x{:X}",
                    pbr, pc, scbr, height, bpp, mode, len
                );
            } else {
                eprintln!("[STARFOX-STATE-SFX-SNAPSHOT] selected <none>");
            }
            if let Some((pc, pbr, scbr, height, bpp, mode, len)) =
                gsu.debug_latest_stop_snapshot_meta()
            {
                eprintln!(
                    "[STARFOX-STATE-SFX-SNAPSHOT] latest_stop pc={:02X}:{:04X} scbr={:02X} h={} bpp={} mode={} len=0x{:X}",
                    pbr, pc, scbr, height, bpp, mode, len
                );
            } else {
                eprintln!("[STARFOX-STATE-SFX-SNAPSHOT] latest_stop <none>");
            }
            if let Some((pc, pbr, scbr, height, bpp, mode, len)) =
                gsu.debug_selected_tile_snapshot_meta()
            {
                eprintln!(
                    "[STARFOX-STATE-SFX-SNAPSHOT] tile pc={:02X}:{:04X} scbr={:02X} h={} bpp={} mode={} len=0x{:X}",
                    pbr, pc, scbr, height, bpp, mode, len
                );
            } else {
                eprintln!("[STARFOX-STATE-SFX-SNAPSHOT] tile <none>");
            }
            if let Some((buffer, height, bpp, mode)) = gsu.screen_buffer_display_snapshot() {
                let nonzero = buffer.iter().filter(|&&b| b != 0).count();
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                buffer.hash(&mut hasher);
                eprintln!(
                    "[STARFOX-STATE-SFX-BUFFER] selected h={} bpp={} mode={} len=0x{:X} nonzero={} hash={:016X}",
                    height,
                    bpp,
                    mode,
                    buffer.len(),
                    nonzero,
                    hasher.finish(),
                );
            } else {
                eprintln!("[STARFOX-STATE-SFX-BUFFER] selected <none>");
            }
            if let Some((buffer, height, bpp, mode)) = gsu.screen_buffer_live() {
                let nonzero = buffer.iter().filter(|&&b| b != 0).count();
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                use std::hash::{Hash, Hasher};
                buffer.hash(&mut hasher);
                eprintln!(
                    "[STARFOX-STATE-SFX-BUFFER] live h={} bpp={} mode={} len=0x{:X} nonzero={} hash={:016X}",
                    height,
                    bpp,
                    mode,
                    buffer.len(),
                    nonzero,
                    hasher.finish(),
                );
            } else {
                eprintln!("[STARFOX-STATE-SFX-BUFFER] live <none>");
            }
            eprintln!(
                "[STARFOX-STATE-SFX-STOPS] recent={:?}",
                gsu.debug_recent_stop_snapshot_metas(16)
            );
        }
        if let Some((min_x, min_y, max_x, max_y, count)) = ppu.dbg_superfx_direct_pixel_bounds() {
            eprintln!(
                "[STARFOX-STATE-PPU-SFX-BOUNDS] min=({}, {}) max=({}, {}) nonzero_pixels={}",
                min_x, min_y, max_x, max_y, count
            );
        } else {
            eprintln!("[STARFOX-STATE-PPU-SFX-BOUNDS] <none>");
        }
        if let Some((sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color)) =
            ppu.dbg_superfx_direct_sample(128, 96)
        {
            eprintln!(
                "[STARFOX-STATE-PPU-SFX] x=128 y=96 sxy=({}, {}) tile_base=0x{:04X} row={} p0={:02X} p1={:02X} p2={:02X} p3={:02X} msb_color=0x{:02X} lsb_color=0x{:02X}",
                sx,
                sy,
                tile_base,
                row_in_tile,
                p0,
                p1,
                p2,
                p3,
                msb_color,
                lsb_color
            );
        } else {
            eprintln!("[STARFOX-STATE-PPU-SFX] x=128 y=96 <none>");
        }
        for bg in 0..=1usize {
            let (map_base, tile_base) = ppu.dbg_bg_bases(bg);
            let (map_nonzero, map_samples) = ppu.analyze_vram_region(map_base, 128);
            let (tile_nonzero, tile_samples) = ppu.analyze_vram_region(tile_base, 256);
            let map_entry = if map_samples.len() >= 2 {
                u16::from(map_samples[0]) | (u16::from(map_samples[1]) << 8)
            } else {
                0
            };
            let tile_id = map_entry & 0x03FF;
            let tile_addr = tile_base.wrapping_add(tile_id.wrapping_mul(16)) & 0x7FFF;
            let (ref_nonzero, ref_samples) = ppu.analyze_vram_region(tile_addr, 32);
            eprintln!(
                "[STARFOX-STATE-PPU-BG{}] map_base=0x{:04X} map_nonzero={} map_head={:02X?} tile_base=0x{:04X} tile_nonzero={} tile_head={:02X?} entry=0x{:04X} tile_id=0x{:03X} tile_addr=0x{:04X} tile_ref_nonzero={} tile_ref_head={:02X?}",
                bg + 1,
                map_base,
                map_nonzero,
                &map_samples[..map_samples.len().min(16)],
                tile_base,
                tile_nonzero,
                &tile_samples[..tile_samples.len().min(16)],
                map_entry,
                tile_id,
                tile_addr,
                ref_nonzero,
                &ref_samples[..ref_samples.len().min(16)]
            );
        }
    }
    if let Some(gsu) = emulator.bus.superfx.as_ref() {
        eprintln!(
            "[STARFOX-STATE-GSU] pbr={:02X} r1={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r9={:04X} r10={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} rombr={:02X} rambr={:02X} scbr={:02X} scmr={:02X} cbr={:04X} recent={:?}",
            gsu.debug_pbr(),
            gsu.debug_reg(1),
            gsu.debug_reg(4),
            gsu.debug_reg(5),
            gsu.debug_reg(6),
            gsu.debug_reg(7),
            gsu.debug_reg(9),
            gsu.debug_reg(10),
            gsu.debug_reg(12),
            gsu.debug_reg(13),
            gsu.debug_reg(14),
            gsu.debug_reg(15),
            gsu.debug_rombr(),
            gsu.debug_rambr(),
            gsu.debug_scbr(),
            gsu.debug_scmr(),
            gsu.debug_cbr(),
            gsu.debug_recent_pc_transfers()
        );
        if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some()
            || std::env::var_os("TRACE_SUPERFX_LAST_WRITERS").is_some()
        {
            if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some() {
                eprintln!(
                    "[STARFOX-STATE-GSU-REG] recent={:?}",
                    gsu.debug_recent_reg_writes()
                );
            }
            eprintln!(
                "[STARFOX-STATE-GSU-LAST] r0={:?} r1={:?} r3={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                gsu.debug_last_nontrivial_reg_write(0),
                gsu.debug_last_nontrivial_reg_write(1),
                gsu.debug_last_nontrivial_reg_write(3),
                gsu.debug_last_nontrivial_reg_write(4),
                gsu.debug_last_nontrivial_reg_write(5),
                gsu.debug_last_nontrivial_reg_write(6),
                gsu.debug_last_nontrivial_reg_write(7),
                gsu.debug_last_nontrivial_reg_write(8),
                gsu.debug_last_nontrivial_reg_write(9),
                gsu.debug_last_nontrivial_reg_write(11),
                gsu.debug_last_nontrivial_reg_write(12),
                gsu.debug_last_nontrivial_reg_write(13),
                gsu.debug_last_nontrivial_reg_write(14),
                gsu.debug_last_nontrivial_reg_write(15)
            );
            eprintln!(
                "[STARFOX-STATE-GSU-LAST-HIST] r0={:?} r1={:?} r3={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                gsu.debug_recent_nontrivial_reg_writes(0),
                gsu.debug_recent_nontrivial_reg_writes(1),
                gsu.debug_recent_nontrivial_reg_writes(3),
                gsu.debug_recent_nontrivial_reg_writes(4),
                gsu.debug_recent_nontrivial_reg_writes(5),
                gsu.debug_recent_nontrivial_reg_writes(6),
                gsu.debug_recent_nontrivial_reg_writes(7),
                gsu.debug_recent_nontrivial_reg_writes(8),
                gsu.debug_recent_nontrivial_reg_writes(9),
                gsu.debug_recent_nontrivial_reg_writes(11),
                gsu.debug_recent_nontrivial_reg_writes(12),
                gsu.debug_recent_nontrivial_reg_writes(13),
                gsu.debug_recent_nontrivial_reg_writes(14),
                gsu.debug_recent_nontrivial_reg_writes(15)
            );
            let reg_filters = parse_trace_superfx_recent_regs();
            if !reg_filters.is_empty() {
                let limit = trace_superfx_recent_regs_limit();
                for reg in reg_filters {
                    eprintln!(
                        "[STARFOX-STATE-GSU-RECENT] r{} last_any={:?} recent={:?}",
                        reg,
                        gsu.debug_last_reg_write(reg),
                        gsu.debug_recent_reg_writes_for_reg(reg, limit)
                    );
                }
            }
            if let Some(raw) = std::env::var_os("TRACE_SUPERFX_LOW_RAM_WORDS") {
                let raw = raw.to_string_lossy();
                for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    let token = token.trim_start_matches("0x").trim_start_matches("0X");
                    let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                        continue;
                    };
                    let value = gsu.debug_read_ram_word_short(addr);
                    let lo = gsu.debug_last_low_ram_write(addr);
                    let hi = gsu.debug_last_low_ram_write(addr.wrapping_add(1));
                    eprintln!(
                        "[STARFOX-STATE-GSU-RAM] addr={:04X} value={:04X} lo={:?} hi={:?}",
                        addr, value, lo, hi
                    );
                }
            }
            if let Some(raw) = std::env::var_os("TRACE_SUPERFX_DATA_ADDRS") {
                let raw = raw.to_string_lossy();
                for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    let (bank, addr) = if let Some((bank, addr)) = token.split_once(':') {
                        let bank = bank.trim_start_matches("0x").trim_start_matches("0X");
                        let addr = addr.trim_start_matches("0x").trim_start_matches("0X");
                        let (Some(bank), Some(addr)) = (
                            u8::from_str_radix(bank, 16).ok(),
                            u16::from_str_radix(addr, 16).ok(),
                        ) else {
                            continue;
                        };
                        (bank, addr)
                    } else {
                        let token = token.trim_start_matches("0x").trim_start_matches("0X");
                        let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                            continue;
                        };
                        (gsu.debug_rombr(), addr)
                    };
                    let mut bytes = [0u8; 8];
                    for (i, slot) in bytes.iter_mut().enumerate() {
                        *slot = gsu
                            .debug_read_data_source_byte(
                                &emulator.bus.rom,
                                bank,
                                addr.wrapping_add(i as u16),
                            )
                            .unwrap_or(0xFF);
                    }
                    eprintln!(
                        "[STARFOX-STATE-GSU-DATA] addr={:02X}:{:04X} bytes={:02X?}",
                        bank, addr, bytes
                    );
                }
            }
        }
    }
    if let Some(addrs) = diag_cpu_bytes.as_ref() {
        for &addr in addrs {
            let mut bytes = [0u8; 8];
            for (i, slot) in bytes.iter_mut().enumerate() {
                *slot = emulator.bus.read_u8(addr.wrapping_add(i as u32));
            }
            eprintln!(
                "[STARFOX-STATE-CODE] addr={:06X} bytes={:02X?}",
                addr, bytes
            );
        }
    }
    if exact_capture_stopped {
        eprintln!("[STARFOX-STATE-SAVED] <exact-capture>");
    } else if let Some(path) = save_state_path.as_ref() {
        emulator
            .save_state_to_file(path)
            .expect("failed to save Star Fox diagnostic state");
        eprintln!("[STARFOX-STATE-SAVED] {}", path.display());
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    crate::shutdown::clear_for_tests();
}
#[test]
fn star_fox_launch_packet_from_state_path_does_not_mutate_gsu_immediately() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    // Recreate the frame-152 CPU packet that hands work to the later GSU path.
    emulator.bus.write_u8(0x70_002C, 0x00);
    emulator.bus.write_u8(0x70_002D, 0x40);
    emulator.bus.write_u8(0x70_0062, 0x96);
    emulator.bus.write_u8(0x70_0063, 0xD8);
    emulator.bus.write_u8(0x70_0064, 0x14);
    emulator.bus.write_u8(0x00_3034, 0x01);
    emulator.bus.write_u8(0x00_303A, 0x39);

    let (pre_r9, pre_r13, pre_r14, pre_r15) = {
        let gsu = emulator.bus.superfx.as_ref().expect("missing SuperFX");
        (
            gsu.debug_reg(9),
            gsu.debug_reg(13),
            gsu.debug_reg(14),
            gsu.debug_reg(15),
        )
    };

    emulator.bus.write_u8(0x00_301E, 0x01);
    {
        let gsu = emulator.bus.superfx.as_ref().expect("missing SuperFX");
        assert_eq!(gsu.debug_reg(15), (pre_r15 & 0xFF00) | 0x0001);
        assert_eq!(gsu.debug_reg(9), pre_r9);
        assert_eq!(gsu.debug_reg(13), pre_r13);
        assert_eq!(gsu.debug_reg(14), pre_r14);
        assert!(!gsu.running());
    }

    emulator.bus.write_u8(0x00_301F, 0xB3);
    {
        let gsu = emulator.bus.superfx.as_ref().expect("missing SuperFX");
        assert!(gsu.running());
        assert_eq!(gsu.debug_reg(15), 0xB301);
        assert_eq!(gsu.debug_reg(9), pre_r9);
        assert_eq!(gsu.debug_reg(13), pre_r13);
        assert_eq!(gsu.debug_reg(14), pre_r14);
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    crate::shutdown::clear_for_tests();
}
#[test]
fn star_fox_gsu_only_from_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }
    let save_state_path = std::env::var("STARFOX_GSU_ONLY_SAVE_STATE_PATH")
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);

    let gsu_steps: usize = std::env::var("STARFOX_GSU_ONLY_STEPS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4096);
    let use_status_poll_late_wait_assist =
        std::env::var_os("STARFOX_GSU_ONLY_STATUS_POLL_LATE_WAIT_ASSIST").is_some();
    let gsu_overrides = parse_star_fox_gsu_test_overrides();

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let frame = emulator.frame_count();
    crate::cartridge::superfx::set_trace_superfx_exec_frame(frame);

    let rom = emulator.bus.rom.clone();
    let save_state_at_gsu_pc_range = std::env::var("SAVE_STATE_AT_GSU_PC_RANGE")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let save_state_at_superfx_ram_addr = std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDR_RANGE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("SAVE_STATE_AT_SUPERFX_RAM_ADDRS")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            std::env::var("SAVE_STATE_AT_SUPERFX_RAM_BYTE_EQ")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| {
            std::env::var("SAVE_STATE_AT_SUPERFX_RAM_WORD_EQ")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    apply_star_fox_gsu_test_overrides(&mut emulator, &gsu_overrides);
    let mut save_state_pc_hit = None;
    let mut save_state_ram_addr_hit = None;
    if let Some(gsu) = emulator.bus.superfx.as_mut() {
        let stop_on_save_hit =
            save_state_at_gsu_pc_range.is_some() || save_state_at_superfx_ram_addr.is_some();
        if use_status_poll_late_wait_assist {
            gsu.run_status_poll_until_stop_with_starfox_late_wait_assist(&rom, gsu_steps);
        } else {
            let mut remaining = gsu_steps;
            while remaining > 0 {
                let chunk = if stop_on_save_hit { 1 } else { remaining };
                gsu.debug_run_steps(&rom, chunk);
                save_state_pc_hit = gsu.debug_take_save_state_pc_hit();
                save_state_ram_addr_hit = gsu.debug_take_save_state_ram_addr_hit();
                if stop_on_save_hit
                    && (save_state_pc_hit.is_some() || save_state_ram_addr_hit.is_some())
                {
                    break;
                }
                remaining -= chunk;
            }
        }
        if save_state_pc_hit.is_none() {
            save_state_pc_hit = gsu.debug_take_save_state_pc_hit();
        }
        if save_state_ram_addr_hit.is_none() {
            save_state_ram_addr_hit = gsu.debug_take_save_state_ram_addr_hit();
        }
        eprintln!(
            "[STARFOX-GSU-ONLY] state={} frame={} steps={} mode={} overrides={:?} pbr={:02X} r1={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r9={:04X} r10={:04X} r11={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} sfr={:04X} running={} recent={:?}",
            state_path.display(),
            frame,
            gsu_steps,
            if use_status_poll_late_wait_assist {
                "status_poll_late_wait_assist"
            } else {
                "run_steps"
            },
            gsu_overrides.regs,
            gsu.debug_pbr(),
            gsu.debug_reg(1),
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
            gsu.running(),
            gsu.debug_recent_pc_transfers()
        );
        if let Some((pbr, pc)) = save_state_pc_hit {
            eprintln!("[STARFOX-GSU-ONLY-PC-HIT] {:02X}:{:04X}", pbr, pc);
        }
        if let Some((pbr, pc, addr)) = save_state_ram_addr_hit {
            eprintln!(
                "[STARFOX-GSU-ONLY-RAM-HIT] {:02X}:{:04X} addr={:04X}",
                pbr, pc, addr
            );
        }
        if std::env::var_os("TRACE_SUPERFX_LAST_WRITERS").is_some() {
            eprintln!(
                "[STARFOX-GSU-ONLY-LAST] r0={:?} r1={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r10={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
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
            eprintln!(
                "[STARFOX-GSU-ONLY-LAST-HIST] r1={:?} r2={:?} r4={:?} r6={:?} r10={:?} r11={:?} r12={:?} r13={:?} r15={:?}",
                gsu.debug_recent_nontrivial_reg_writes(1),
                gsu.debug_recent_nontrivial_reg_writes(2),
                gsu.debug_recent_nontrivial_reg_writes(4),
                gsu.debug_recent_nontrivial_reg_writes(6),
                gsu.debug_recent_nontrivial_reg_writes(10),
                gsu.debug_recent_nontrivial_reg_writes(11),
                gsu.debug_recent_nontrivial_reg_writes(12),
                gsu.debug_recent_nontrivial_reg_writes(13),
                gsu.debug_recent_nontrivial_reg_writes(15),
            );
            let reg_filters = parse_trace_superfx_recent_regs();
            if !reg_filters.is_empty() {
                let limit = trace_superfx_recent_regs_limit();
                for reg in reg_filters {
                    eprintln!(
                        "[STARFOX-GSU-ONLY-RECENT] r{} last_any={:?} recent={:?}",
                        reg,
                        gsu.debug_last_reg_write(reg),
                        gsu.debug_recent_reg_writes_for_reg(reg, limit)
                    );
                }
            }
        }
        if let Some(raw) = std::env::var_os("TRACE_SUPERFX_LOW_RAM_WORDS") {
            let raw = raw.to_string_lossy();
            for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let token = token.trim_start_matches("0x").trim_start_matches("0X");
                let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                    continue;
                };
                let value = gsu.debug_read_ram_word_short(addr);
                let lo = gsu.debug_last_low_ram_write(addr);
                let hi = gsu.debug_last_low_ram_write(addr.wrapping_add(1));
                eprintln!(
                    "[STARFOX-GSU-ONLY-RAM] addr={:04X} value={:04X} lo={:?} hi={:?}",
                    addr, value, lo, hi
                );
            }
        }
        if let Some(raw) = std::env::var_os("TRACE_SUPERFX_CODE_ADDRS") {
            let raw = raw.to_string_lossy();
            for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let (bank, addr) = if let Some((bank, addr)) = token.split_once(':') {
                    let bank = bank.trim_start_matches("0x").trim_start_matches("0X");
                    let addr = addr.trim_start_matches("0x").trim_start_matches("0X");
                    let (Some(bank), Some(addr)) = (
                        u8::from_str_radix(bank, 16).ok(),
                        u16::from_str_radix(addr, 16).ok(),
                    ) else {
                        continue;
                    };
                    (bank, addr)
                } else {
                    let token = token.trim_start_matches("0x").trim_start_matches("0X");
                    let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                        continue;
                    };
                    (gsu.debug_pbr(), addr)
                };
                let mut bytes = [0u8; 8];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = gsu
                        .debug_read_program_source_byte(&rom, bank, addr.wrapping_add(i as u16))
                        .unwrap_or(0xFF);
                }
                eprintln!(
                    "[STARFOX-GSU-ONLY-CODE] addr={:02X}:{:04X} bytes={:02X?}",
                    bank, addr, bytes
                );
            }
        }
        if let Some(raw) = std::env::var_os("TRACE_SUPERFX_DATA_ADDRS") {
            let raw = raw.to_string_lossy();
            for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let (bank, addr) = if let Some((bank, addr)) = token.split_once(':') {
                    let bank = bank.trim_start_matches("0x").trim_start_matches("0X");
                    let addr = addr.trim_start_matches("0x").trim_start_matches("0X");
                    let (Some(bank), Some(addr)) = (
                        u8::from_str_radix(bank, 16).ok(),
                        u16::from_str_radix(addr, 16).ok(),
                    ) else {
                        continue;
                    };
                    (bank, addr)
                } else {
                    let token = token.trim_start_matches("0x").trim_start_matches("0X");
                    let Some(addr) = u16::from_str_radix(token, 16).ok() else {
                        continue;
                    };
                    (gsu.debug_rombr(), addr)
                };
                let mut bytes = [0u8; 8];
                for (i, slot) in bytes.iter_mut().enumerate() {
                    *slot = gsu
                        .debug_read_data_source_byte(&rom, bank, addr.wrapping_add(i as u16))
                        .unwrap_or(0xFF);
                }
                eprintln!(
                    "[STARFOX-GSU-ONLY-DATA] addr={:02X}:{:04X} bytes={:02X?}",
                    bank, addr, bytes
                );
            }
        }
    }
    if let Some(path) = save_state_path.as_ref() {
        let should_save = Emulator::gsu_only_should_save_state(
            save_state_at_gsu_pc_range.is_some(),
            save_state_at_superfx_ram_addr.is_some(),
            save_state_pc_hit,
            save_state_ram_addr_hit,
        );
        if should_save {
            emulator
                .save_state_to_file(path)
                .expect("failed to save Star Fox GSU-only state");
            eprintln!("[STARFOX-GSU-ONLY-SAVED] {}", path.display());
        }
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_advance_state_path_and_save() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }
    let save_state_path = match std::env::var("STARFOX_SAVE_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let step_frames: u64 = std::env::var("STARFOX_STEP_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    for _ in 0..step_frames {
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
    }

    emulator
        .save_state_to_file(Path::new(&save_state_path))
        .expect("failed to save advanced Star Fox state");
    eprintln!(
        "[STARFOX-ADVANCE-SAVED] from={} to={} frame={}",
        state_path.display(),
        Path::new(&save_state_path).display(),
        emulator.frame_count()
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_advance_state_path_until_unblank_and_save() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }
    let save_state_path = match std::env::var("STARFOX_SAVE_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let max_frames: u64 = std::env::var("STARFOX_UNBLANK_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(32);
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let start_frame = emulator.frame_count();
    let mut stepped = 0u64;
    while stepped < max_frames && !emulator.has_unblanked_output() {
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
        stepped += 1;
    }

    emulator
        .save_state_to_file(Path::new(&save_state_path))
        .expect("failed to save advanced Star Fox state");
    let ppu = emulator.bus.get_ppu();
    eprintln!(
        "[STARFOX-ADVANCE-UNBLANK-SAVED] from={} to={} start_frame={} end_frame={} stepped={} unblanked={} inidisp={:02X} bright={} tm={:02X}",
        state_path.display(),
        Path::new(&save_state_path).display(),
        start_frame,
        emulator.frame_count(),
        stepped,
        emulator.has_unblanked_output(),
        ppu.screen_display,
        ppu.current_brightness(),
        ppu.get_main_screen_designation()
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_advance_state_path_state_only_and_save() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }
    let save_state_path = match std::env::var("STARFOX_SAVE_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let step_frames: u64 = std::env::var("STARFOX_STEP_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1);
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    for _ in 0..step_frames {
        if step_one_frame_state_only_for_test(&mut emulator) {
            break;
        }
    }

    emulator
        .save_state_to_file(Path::new(&save_state_path))
        .expect("failed to save advanced Star Fox state");
    eprintln!(
        "[STARFOX-ADVANCE-STATE-ONLY-SAVED] from={} to={} frame={} inidisp={:02X} bright={} tm={:02X}",
        state_path.display(),
        Path::new(&save_state_path).display(),
        emulator.frame_count(),
        emulator.bus.get_ppu().screen_display,
        emulator.bus.get_ppu().current_brightness(),
        emulator.bus.get_ppu().get_main_screen_designation()
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_inspect_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let pc = ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32;
    let ppu = emulator.bus.get_ppu();
    let tm = ppu.get_main_screen_designation();
    let inidisp = ppu.screen_display;
    let brightness = ppu.current_brightness();
    let bg_mode = ppu.get_bg_mode();
    let a = emulator.cpu.a();
    let x = emulator.cpu.x();
    let y = emulator.cpu.y();
    let (vram_addr, vram_increment, vram_mapping) = ppu.dbg_vram_regs();
    eprintln!(
        "[STARFOX-INSPECT] state={} frame={} pc={:06X} a={:04X} x={:04X} y={:04X} inidisp={:02X} bright={} tm={:02X} mode={} vram={:04X} inc={} map={}",
        state_path.display(),
        emulator.frame_count(),
        pc,
        a,
        x,
        y,
        inidisp,
        brightness,
        tm,
        bg_mode,
        vram_addr,
        vram_increment,
        vram_mapping
    );

    if let Some(raw) = std::env::var_os("STARFOX_DIAG_CPU_BYTES") {
        let raw = raw.to_string_lossy();
        for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let token = token.trim_start_matches("0x").trim_start_matches("0X");
            let Some(addr) = u32::from_str_radix(token, 16).ok() else {
                continue;
            };
            let mut bytes = [0u8; 8];
            for (i, slot) in bytes.iter_mut().enumerate() {
                *slot = emulator.bus.read_u8(addr.wrapping_add(i as u32));
            }
            eprintln!(
                "[STARFOX-INSPECT-CODE] addr={:06X} bytes={:02X?}",
                addr, bytes
            );
        }
    }

    if let Some(raw) = std::env::var_os("STARFOX_DIAG_DUMP_ADDRS") {
        let raw = raw.to_string_lossy();
        for token in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let token = token.trim_start_matches("0x").trim_start_matches("0X");
            let Some(addr) = u32::from_str_radix(token, 16).ok() else {
                continue;
            };
            let mut bytes = [0u8; 8];
            for (i, slot) in bytes.iter_mut().enumerate() {
                *slot = emulator.bus.read_u8(addr.wrapping_add(i as u32));
            }
            eprintln!(
                "[STARFOX-INSPECT-DATA] addr={:06X} bytes={:02X?}",
                addr, bytes
            );
        }
    }

    if let Some(gsu) = emulator.bus.superfx.as_ref() {
        eprintln!(
            "[STARFOX-INSPECT-GSU] pbr={:02X} r1={:04X} r10={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} scbr={:02X} scmr={:02X} por={:02X} running={}",
            gsu.debug_pbr(),
            gsu.debug_reg(1),
            gsu.debug_reg(10),
            gsu.debug_reg(12),
            gsu.debug_reg(13),
            gsu.debug_reg(14),
            gsu.debug_reg(15),
            gsu.debug_scbr(),
            gsu.debug_scmr(),
            gsu.debug_por(),
            gsu.running()
        );
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_sample_state_path_layers() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let step_frames = std::env::var("STARFOX_STEP_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let force_no_blank = std::env::var("STARFOX_SAMPLE_FORCE_NOBLANK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let render_before_sample = std::env::var("STARFOX_RENDER_BEFORE_SAMPLE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");
    for _ in 0..step_frames {
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
    }
    if force_no_blank {
        emulator.bus.get_ppu_mut().force_no_blank = true;
    }
    if render_before_sample {
        emulator.render();
    }

    let samples = std::env::var("STARFOX_SAMPLE_POINTS")
        .unwrap_or_else(|_| "128x112,64x64,192x64,128x176,240x184".to_string());
    let frame = emulator.frame_count();
    let scene_active = emulator.has_scene_activity_behind_forced_blank();
    let ppu = emulator.bus.get_ppu_mut();
    eprintln!(
        "[STARFOX-SAMPLE-STATE] state={} frame={} step_frames={} inidisp={:02X} tm={:02X} mode={} scene_active={} force_no_blank={} render_before_sample={}",
        state_path.display(),
        frame,
        step_frames,
        ppu.screen_display,
        ppu.effective_main_screen_designation(),
        ppu.get_bg_mode(),
        scene_active as u8,
        force_no_blank as u8,
        render_before_sample as u8
    );
    if let Some((min_x, min_y, max_x, max_y, count)) = ppu.dbg_superfx_direct_pixel_bounds() {
        eprintln!(
            "[STARFOX-SAMPLE-SFX-BOUNDS] min=({}, {}) max=({}, {}) nonzero_pixels={}",
            min_x, min_y, max_x, max_y, count
        );
    }
    for token in samples.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let Some((x_raw, y_raw)) = token.split_once('x') else {
            continue;
        };
        let Some(x) = x_raw.parse::<u16>().ok() else {
            continue;
        };
        let Some(y) = y_raw.parse::<u16>().ok() else {
            continue;
        };
        let enables = ppu.effective_main_screen_designation();
        let (bg_color, bg_pr, bg_id) = ppu.get_main_bg_pixel(x, y, enables);
        let (obj_color, obj_pr) = ppu.get_sprite_pixel(x, y);
        let final_color = ppu.get_pixel_color(x, y);
        let direct = ppu
            .dbg_superfx_direct_sample(x, y)
            .map(
                |(sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color)| {
                    format!(
                        " sfx=({},{} base=0x{:04X} row={} bytes=[{:02X},{:02X},{:02X},{:02X}] msb=0x{:02X} lsb=0x{:02X})",
                        sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color
                    )
                },
            )
            .unwrap_or_default();
        eprintln!(
            "[STARFOX-SAMPLE] state={} frame={} xy=({}, {}) TM={:02X} mode={} bg=(0x{:08X},pr={},id={}) obj=(0x{:08X},pr={}) final=0x{:08X}{}",
            state_path.display(),
            frame,
            x,
            y,
            enables,
            ppu.get_bg_mode(),
            bg_color,
            bg_pr,
            bg_id,
            obj_color,
            obj_pr,
            final_color,
            direct
        );
    }

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_auto_unblank_from_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");
    let prev_threshold = set_env_temp("COMPAT_AUTO_UNBLANK_FRAME", "120");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let before = emulator.bus.get_ppu().screen_display;
    let before_tm = emulator.bus.get_ppu().get_main_screen_designation();
    let scene_active = emulator.has_scene_activity_behind_forced_blank();
    emulator.maybe_auto_unblank();
    let after = emulator.bus.get_ppu().screen_display;
    let after_tm = emulator.bus.get_ppu().get_main_screen_designation();
    eprintln!(
        "[STARFOX-AUTO-UNBLANK] state={} frame={} scene_active={} before_inidisp={:02X} before_tm={:02X} after_inidisp={:02X} after_tm={:02X}",
        state_path.display(),
        emulator.frame_count(),
        scene_active as u8,
        before,
        before_tm,
        after,
        after_tm
    );

    assert!(scene_active, "expected scene activity behind forced blank");
    assert_eq!(before, 0x80, "expected forced blank before auto-unblank");
    assert_eq!(after & 0x80, 0, "expected forced blank cleared");
    assert_eq!(after & 0x0F, 0x0F, "expected brightness raised to 15");
    assert_eq!(after_tm, before_tm, "expected TM preserved");

    restore_env("COMPAT_AUTO_UNBLANK_FRAME", prev_threshold);
    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
#[ignore] // Diagnostic: confirms whether auto-unblank alone reveals non-black output
fn star_fox_auto_unblank_from_state_path_reveals_non_black_pixels() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");
    let prev_threshold = set_env_temp("COMPAT_AUTO_UNBLANK_FRAME", "120");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    emulator.maybe_auto_unblank();
    emulator.render();
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .take(256 * 224)
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();
    eprintln!(
        "[STARFOX-AUTO-UNBLANK-RENDER] state={} frame={} inidisp={:02X} tm={:02X} non_black={}",
        state_path.display(),
        emulator.frame_count(),
        emulator.bus.get_ppu().screen_display,
        emulator.bus.get_ppu().get_main_screen_designation(),
        non_black
    );

    restore_env("COMPAT_AUTO_UNBLANK_FRAME", prev_threshold);
    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);

    assert!(
        non_black > 0,
        "expected non-black pixels after auto-unblank and render"
    );
}
#[test]
#[ignore] // Diagnostic: used to rule out color math as the only cause of black output
fn star_fox_auto_unblank_from_state_path_reveals_non_black_pixels_without_color_math() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");
    let prev_threshold = set_env_temp("COMPAT_AUTO_UNBLANK_FRAME", "120");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    emulator.maybe_auto_unblank();
    {
        let ppu = emulator.bus.get_ppu_mut();
        ppu.cgwsel = 0;
        ppu.cgadsub = 0;
    }
    emulator.render();
    let non_black = emulator
        .bus
        .get_ppu()
        .get_framebuffer()
        .iter()
        .take(256 * 224)
        .filter(|&&p| (p & 0x00FF_FFFF) != 0)
        .count();
    eprintln!(
        "[STARFOX-AUTO-UNBLANK-NOMATH] state={} frame={} inidisp={:02X} tm={:02X} non_black={}",
        state_path.display(),
        emulator.frame_count(),
        emulator.bus.get_ppu().screen_display,
        emulator.bus.get_ppu().get_main_screen_designation(),
        non_black
    );

    restore_env("COMPAT_AUTO_UNBLANK_FRAME", prev_threshold);
    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);

    assert!(
        non_black > 0,
        "expected non-black pixels after auto-unblank with color math disabled"
    );
}
#[test]
fn star_fox_auto_unblank_persists_across_one_frame_from_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_compat = set_env_temp("COMPAT_BOOT_FALLBACK", "1");
    let prev_threshold = set_env_temp("COMPAT_AUTO_UNBLANK_FRAME", "120");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let before = emulator.bus.get_ppu().screen_display;
    emulator.maybe_auto_unblank();
    let after_unblank = emulator.bus.get_ppu().screen_display;
    let _ = step_one_frame_state_only_for_test(&mut emulator);
    let after_step = emulator.bus.get_ppu().screen_display;
    eprintln!(
        "[STARFOX-AUTO-UNBLANK-PERSIST] state={} before={:02X} after_unblank={:02X} after_step={:02X} frame={}",
        state_path.display(),
        before,
        after_unblank,
        after_step,
        emulator.frame_count()
    );

    restore_env("COMPAT_AUTO_UNBLANK_FRAME", prev_threshold);
    restore_env("COMPAT_BOOT_FALLBACK", prev_compat);
    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_startup_similarity_scan_from_state_path() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    let ref_path = Path::new("/tmp/starfox_startup_ref.png");
    if !rom_path.exists() || !ref_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let frame_step = std::env::var("STARFOX_SCAN_FRAME_STEP")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(30);
    let max_frames = std::env::var("STARFOX_SCAN_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(360);

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();

    let (ref_w, ref_h, ref_rgba) = read_png_rgba(ref_path).expect("failed to read startup ref");
    assert_eq!(ref_w, 256);
    assert_eq!(ref_h, 224);

    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");

    let mut best_frame = emulator.frame_count();
    let mut best_score = f64::INFINITY;
    let mut next_sample = emulator.frame_count();
    let end_frame = emulator.frame_count().saturating_add(max_frames);

    while emulator.frame_count() <= end_frame {
        if emulator.frame_count() >= next_sample {
            let rgba = framebuffer_to_rgba_bytes(emulator.framebuffer());
            let score = weighted_startup_score(&rgba, &ref_rgba);
            eprintln!(
                "[STARFOX-STARTUP-SCAN] frame={} score={:.4} cpu_pc={:06X}",
                emulator.frame_count(),
                score,
                emulator.current_cpu_pc()
            );
            if score < best_score {
                best_score = score;
                best_frame = emulator.frame_count();
            }
            next_sample = next_sample.saturating_add(frame_step);
        }
        if emulator.frame_count() == end_frame {
            break;
        }
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
    }

    eprintln!(
        "[STARFOX-STARTUP-BEST] frame={} score={:.4}",
        best_frame, best_score
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_dump_framebuffer_from_state_path_png() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }
    let state_path = match std::env::var("STARFOX_STATE_PATH") {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => return,
    };
    if !state_path.exists() {
        return;
    }

    let step_frames = std::env::var("STARFOX_STEP_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let output_path = std::env::var("STARFOX_OUTPUT_PNG")
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/starfox_state_framebuffer.png"));
    let use_gui_framebuffer = std::env::var("STARFOX_USE_GUI_FRAMEBUFFER")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let disable_color_math = std::env::var("STARFOX_DISABLE_COLOR_MATH")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator
        .load_state_from_file(&state_path)
        .expect("failed to load Star Fox save state");
    for _ in 0..step_frames {
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
    }
    if disable_color_math {
        let ppu = emulator.bus.get_ppu_mut();
        ppu.cgwsel = 0x00;
        ppu.cgadsub = 0x00;
    }
    let framebuffer = if use_gui_framebuffer {
        emulator.framebuffer()
    } else {
        emulator.bus.get_ppu().get_framebuffer()
    };
    write_framebuffer_png(&output_path, framebuffer, 256, 224).expect("failed to write png");
    eprintln!(
        "STARFOX_STATE_DUMP {} frame={} pc={:06X} disable_color_math={} gui={}",
        output_path.display(),
        emulator.frame_count(),
        ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32,
        disable_color_math,
        use_gui_framebuffer
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_startup_similarity_scan_from_cold_boot() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    let ref_path = Path::new("/tmp/starfox_startup_ref.png");
    if !rom_path.exists() || !ref_path.exists() {
        return;
    }

    let frame_step = std::env::var("STARFOX_SCAN_FRAME_STEP")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(30);
    let max_frames = std::env::var("STARFOX_SCAN_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(240);
    let dump_best = std::env::var("STARFOX_SCAN_DUMP_BEST")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();

    let (ref_w, ref_h, ref_rgba) = read_png_rgba(ref_path).expect("failed to read startup ref");
    assert_eq!(ref_w, 256);
    assert_eq!(ref_h, 224);

    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");

    let mut best_frame = 0u64;
    let mut best_score = f64::INFINITY;
    let mut best_rgba = Vec::new();
    let mut next_sample = 0u64;

    while emulator.frame_count() <= max_frames {
        if emulator.frame_count() >= next_sample {
            let rgba = framebuffer_to_rgba_bytes(emulator.framebuffer());
            let score = weighted_startup_score(&rgba, &ref_rgba);
            eprintln!(
                "[STARFOX-COLD-SCAN] frame={} score={:.4} cpu_pc={:06X}",
                emulator.frame_count(),
                score,
                emulator.current_cpu_pc()
            );
            if score < best_score {
                best_score = score;
                best_frame = emulator.frame_count();
                best_rgba = rgba;
            }
            next_sample = next_sample.saturating_add(frame_step);
        }
        if emulator.frame_count() == max_frames {
            break;
        }
        if emulator.step_one_frame_inner() || crate::shutdown::should_quit() {
            break;
        }
    }

    if let Some(path) = dump_best.as_ref() {
        if !best_rgba.is_empty() {
            let _ = save_rgba_png(path, &best_rgba, 256, 224);
        }
    }

    eprintln!(
        "[STARFOX-COLD-BEST] frame={} score={:.4}",
        best_frame, best_score
    );

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS", prev_headless);
}
#[test]
fn star_fox_leaves_initial_superfx_wait_within_120_frames() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "120");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let pc = ((emulator.cpu.pb() as u32) << 16) | emulator.cpu.pc() as u32;
    let (sfr_low, _) = emulator.read_superfx_sfr_bytes_direct();

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert_eq!(sfr_low & 0x20, 0, "pc={pc:06X} sfr_low={sfr_low:02X}");
    assert_ne!(pc, 0x7E4EFD, "stuck in early SuperFX wait loop");
    assert_ne!(pc, 0x7E4F02, "stuck in initial Star Fox polling loop");
}
#[test]
fn headless_run_updates_superfx_trace_frame_counter() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_frames = set_env_temp("HEADLESS_FRAMES", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");
    let prev_quiet = set_env_temp("QUIET", "1");

    crate::cartridge::superfx::set_trace_superfx_exec_frame(0);

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut emulator = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct Star Fox emulator");
    emulator.run();

    let trace_frame = crate::cartridge::superfx::debug_current_trace_superfx_exec_frame();

    restore_env("QUIET", prev_quiet);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS_FRAMES", prev_headless_frames);
    restore_env("HEADLESS", prev_headless);

    assert_eq!(trace_frame, 1);
}
#[test]
#[ignore] // Diagnostic regression: save/resume fidelity for the later Star Fox producer path
fn star_fox_save_resume_matches_continuous_framebuffer_at_300_frames_from_163() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_starfox_resume_163_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut baseline = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct baseline emulator");
    while baseline.frame_count() < 300 {
        assert!(
            !baseline.step_one_frame_inner(),
            "baseline emulation stopped early at frame {}",
            baseline.frame_count()
        );
    }
    let baseline_fb = baseline.bus.get_ppu().get_framebuffer().to_vec();

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut save_src = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct save-state emulator");
    while save_src.frame_count() < 163 {
        assert!(
            !save_src.step_one_frame_inner(),
            "save-source emulation stopped early at frame {}",
            save_src.frame_count()
        );
    }
    save_src
        .save_state_to_file(&save_path)
        .expect("failed to save Star Fox state at frame 163");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut resumed = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct resumed emulator");
    resumed
        .load_state_from_file(&save_path)
        .expect("failed to load saved Star Fox state");
    while resumed.frame_count() < 300 {
        assert!(
            !resumed.step_one_frame_inner(),
            "resumed emulation stopped early at frame {}",
            resumed.frame_count()
        );
    }

    let resumed_fb = resumed.bus.get_ppu().get_framebuffer();
    let diff_pixels = baseline_fb
        .iter()
        .zip(resumed_fb.iter())
        .filter(|(lhs, rhs)| lhs != rhs)
        .count();

    let _ = std::fs::remove_file(&save_path);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    restore_env("QUIET", prev_quiet);
    crate::shutdown::clear_for_tests();

    assert_eq!(
        diff_pixels, 0,
        "save/resume drifted by {diff_pixels} pixels at frame 300 from a frame-163 state"
    );
}
#[test]
#[ignore] // Diagnostic regression: shorter save/resume check for mid-boot producer fidelity
fn star_fox_save_resume_matches_continuous_framebuffer_at_170_frames_from_150() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_starfox_resume_150_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut baseline = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct baseline emulator");
    while baseline.frame_count() < 170 {
        assert!(
            !baseline.step_one_frame_inner(),
            "baseline emulation stopped early at frame {}",
            baseline.frame_count()
        );
    }
    let baseline_fb = baseline.bus.get_ppu().get_framebuffer().to_vec();

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut save_src = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct save-state emulator");
    while save_src.frame_count() < 150 {
        assert!(
            !save_src.step_one_frame_inner(),
            "save-source emulation stopped early at frame {}",
            save_src.frame_count()
        );
    }
    save_src
        .save_state_to_file(&save_path)
        .expect("failed to save Star Fox state at frame 150");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut resumed = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct resumed emulator");
    resumed
        .load_state_from_file(&save_path)
        .expect("failed to load saved Star Fox state");
    while resumed.frame_count() < 170 {
        assert!(
            !resumed.step_one_frame_inner(),
            "resumed emulation stopped early at frame {}",
            resumed.frame_count()
        );
    }

    let resumed_fb = resumed.bus.get_ppu().get_framebuffer();
    let diff_pixels = baseline_fb
        .iter()
        .zip(resumed_fb.iter())
        .filter(|(lhs, rhs)| lhs != rhs)
        .count();

    let _ = std::fs::remove_file(&save_path);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    restore_env("QUIET", prev_quiet);
    crate::shutdown::clear_for_tests();

    assert_eq!(
        diff_pixels, 0,
        "save/resume drifted by {diff_pixels} pixels at frame 170 from a frame-150 state"
    );
}
#[test]
#[ignore] // Diagnostic regression: cheap next-frame save/resume fidelity check
fn star_fox_save_resume_matches_next_frame_from_150() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_starfox_resume_next_150_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut continuous = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct continuous emulator");
    while continuous.frame_count() < 150 {
        assert!(
            !continuous.step_one_frame_inner(),
            "continuous emulation stopped early at frame {}",
            continuous.frame_count()
        );
    }
    continuous
        .save_state_to_file(&save_path)
        .expect("failed to save Star Fox state at frame 150");
    assert!(
        !continuous.step_one_frame_inner(),
        "continuous emulation stopped on frame transition from 150"
    );
    let expected_fb = continuous.bus.get_ppu().get_framebuffer().to_vec();

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut resumed = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct resumed emulator");
    resumed
        .load_state_from_file(&save_path)
        .expect("failed to load saved Star Fox state");
    assert!(
        !resumed.step_one_frame_inner(),
        "resumed emulation stopped on frame transition from 150"
    );

    let actual_fb = resumed.bus.get_ppu().get_framebuffer();
    let diff_pixels = expected_fb
        .iter()
        .zip(actual_fb.iter())
        .filter(|(lhs, rhs)| lhs != rhs)
        .count();

    let _ = std::fs::remove_file(&save_path);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    restore_env("QUIET", prev_quiet);
    crate::shutdown::clear_for_tests();

    assert_eq!(
        diff_pixels, 0,
        "save/resume drifted by {diff_pixels} pixels on the first frame after a frame-150 save"
    );
}
#[test]
#[ignore] // Diagnostic regression: producer-band raw state save/resume fidelity check
fn star_fox_save_resume_matches_state_at_164_frames_from_150() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_starfox_state_resume_164_from_150_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);
    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut continuous = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct continuous emulator");
    while continuous.frame_count() < 150 {
        assert!(
            !step_one_frame_state_only_for_test(&mut continuous),
            "continuous state-only emulation stopped early at frame {}",
            continuous.frame_count()
        );
    }
    continuous
        .save_state_to_file(&save_path)
        .expect("failed to save Star Fox state at frame 150");
    while continuous.frame_count() < 164 {
        assert!(
            !step_one_frame_state_only_for_test(&mut continuous),
            "continuous state-only emulation stopped early at frame {}",
            continuous.frame_count()
        );
    }

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut resumed = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct resumed emulator");
    resumed
        .load_state_from_file(&save_path)
        .expect("failed to load saved Star Fox state");
    while resumed.frame_count() < 164 {
        assert!(
            !step_one_frame_state_only_for_test(&mut resumed),
            "resumed state-only emulation stopped early at frame {}",
            resumed.frame_count()
        );
    }

    let cont_gsu = continuous.bus.superfx.as_ref().expect("continuous superfx");
    let resumed_gsu = resumed.bus.superfx.as_ref().expect("resumed superfx");
    for reg in [1usize, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15] {
        assert_eq!(
            cont_gsu.debug_reg(reg),
            resumed_gsu.debug_reg(reg),
            "register R{reg} diverged at frame 164"
        );
    }
    for addr in [0x021E_u16, 0x04C4, 0x1AB8, 0x1AE0, 0x1AE2, 0x29EC, 0x888C] {
        assert_eq!(
            cont_gsu.debug_read_ram_word_short(addr),
            resumed_gsu.debug_read_ram_word_short(addr),
            "RAM word {addr:04X} diverged at frame 164"
        );
    }

    let _ = std::fs::remove_file(&save_path);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    restore_env("QUIET", prev_quiet);
    crate::shutdown::clear_for_tests();
}
#[test]
#[ignore] // Diagnostic regression: producer-band save/resume fidelity check
fn star_fox_save_resume_matches_continuous_framebuffer_at_164_frames_from_150() {
    let rom_path = Path::new("roms/Star Fox (Japan).sfc");
    if !rom_path.exists() {
        return;
    }

    let _guard = env_lock().lock().unwrap();
    crate::shutdown::clear_for_tests();
    let save_path = std::env::temp_dir().join(format!(
        "codex_starfox_resume_164_from_150_{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&save_path);

    let prev_quiet = set_env_temp("QUIET", "1");
    let prev_headless = set_env_temp("HEADLESS", "1");
    let prev_headless_fast_render = set_env_temp("HEADLESS_FAST_RENDER", "1");
    let prev_headless_fast_render_last = set_env_temp("HEADLESS_FAST_RENDER_LAST", "1");

    let cart = Cartridge::load_from_file(rom_path).expect("failed to load Star Fox ROM");
    let mut continuous = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct continuous emulator");
    while continuous.frame_count() < 150 {
        assert!(
            !continuous.step_one_frame_inner(),
            "continuous emulation stopped early at frame {}",
            continuous.frame_count()
        );
    }
    continuous
        .save_state_to_file(&save_path)
        .expect("failed to save Star Fox state at frame 150");
    while continuous.frame_count() < 164 {
        assert!(
            !continuous.step_one_frame_inner(),
            "continuous emulation stopped early at frame {}",
            continuous.frame_count()
        );
    }
    let expected_fb = continuous.bus.get_ppu().get_framebuffer().to_vec();

    let cart = Cartridge::load_from_file(rom_path).expect("failed to reload Star Fox ROM");
    let mut resumed = Emulator::new(cart, "Star Fox".to_string(), Option::<PathBuf>::None)
        .expect("failed to construct resumed emulator");
    resumed
        .load_state_from_file(&save_path)
        .expect("failed to load saved Star Fox state");
    while resumed.frame_count() < 164 {
        assert!(
            !resumed.step_one_frame_inner(),
            "resumed emulation stopped early at frame {}",
            resumed.frame_count()
        );
    }

    let actual_fb = resumed.bus.get_ppu().get_framebuffer();
    let diff_pixels = expected_fb
        .iter()
        .zip(actual_fb.iter())
        .filter(|(lhs, rhs)| lhs != rhs)
        .count();

    let _ = std::fs::remove_file(&save_path);
    restore_env("HEADLESS_FAST_RENDER_LAST", prev_headless_fast_render_last);
    restore_env("HEADLESS_FAST_RENDER", prev_headless_fast_render);
    restore_env("HEADLESS", prev_headless);
    restore_env("QUIET", prev_quiet);
    crate::shutdown::clear_for_tests();

    assert_eq!(
        diff_pixels, 0,
        "save/resume drifted by {diff_pixels} pixels at frame 164 from a frame-150 state"
    );
}
