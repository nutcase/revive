use super::Emulator;
use std::sync::OnceLock;

impl Emulator {
    pub(super) fn read_superfx_sfr_bytes_direct(&mut self) -> (u8, u8) {
        let mdr = self.bus.mdr;
        if let Some(gsu) = self.bus.superfx.as_mut() {
            (
                gsu.read_register(0x3030, mdr),
                gsu.read_register(0x3031, mdr),
            )
        } else {
            (self.bus.read_u8(0x00_3030), self.bus.read_u8(0x00_3031))
        }
    }

    pub(super) fn dump_starfox_diag(&mut self, label: &str) {
        let pc = ((self.cpu.pb() as u32) << 16) | self.cpu.pc() as u32;
        let a = self.cpu.a();
        let x = self.cpu.x();
        let y = self.cpu.y();
        let sp = self.cpu.sp();
        let d = self.cpu.dp();
        let db = self.cpu.db();
        let p = self.cpu.core.state().p.bits();
        let emu = self.cpu.core.state().emulation_mode;
        let mut bytes = [0u8; 8];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = self.bus.read_u8(pc.wrapping_add(i as u32));
        }
        let (sfr_low, sfr_high) = self.read_superfx_sfr_bytes_direct();
        let inidisp = self.bus.get_ppu().screen_display;
        let brightness = self.bus.get_ppu().brightness;
        let tm = self.bus.get_ppu().get_main_screen_designation();
        let bg_mode = self.bus.get_ppu().get_bg_mode();
        let non_black = self
            .bus
            .get_ppu()
            .get_framebuffer()
            .iter()
            .filter(|&&p| (p & 0x00FF_FFFF) != 0)
            .count();
        let _ = self.bus.read_u8(0x00_2137);
        let ophct_lo = self.bus.read_u8(0x00_213C);
        let ophct_hi = self.bus.read_u8(0x00_213C) & 0x01;
        eprintln!(
            "[STARFOX{}] pc={pc:06X} a={:04X} x={:04X} y={:04X} sp={:04X} d={:04X} db={:02X} p={:02X} e={} sfr={:02X}{:02X} inidisp={:02X} bright={} tm={:02X} mode={} non_black={} ophct={:01X}{:02X} code={:02X?}",
            label, a, x, y, sp, d, db, p, emu as u8, sfr_high, sfr_low, inidisp, brightness, tm, bg_mode, non_black, ophct_hi, ophct_lo, bytes
        );
        if std::env::var_os("STARFOX_DIAG_PPU").is_some() {
            let ppu = self.bus.get_ppu();
            ppu.debug_ppu_state();
            let cgram = ppu.dump_cgram_head(16);
            eprintln!("[STARFOX{}-PPU] CGRAM_HEAD={:04X?}", label, cgram);
            if let Some((min_x, min_y, max_x, max_y, count)) = ppu.dbg_superfx_direct_pixel_bounds()
            {
                eprintln!(
                    "[STARFOX{}-PPU-SFX-BOUNDS] min=({}, {}) max=({}, {}) nonzero_pixels={}",
                    label, min_x, min_y, max_x, max_y, count
                );
            } else {
                eprintln!("[STARFOX{}-PPU-SFX-BOUNDS] <none>", label);
            }
            if let Some((sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color)) =
                ppu.dbg_superfx_direct_sample(128, 96)
            {
                eprintln!(
                    "[STARFOX{}-PPU-SFX] x=128 y=96 sxy=({}, {}) tile_base=0x{:04X} row={} p0={:02X} p1={:02X} p2={:02X} p3={:02X} msb_color=0x{:02X} lsb_color=0x{:02X}",
                    label,
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
                eprintln!("[STARFOX{}-PPU-SFX] x=128 y=96 <none>", label);
            }
            if let Some((sx, sy, tile_base, row_in_tile, p0, p1, p2, p3, msb_color, lsb_color)) =
                ppu.dbg_superfx_direct_sample_unoffset(128, 96)
            {
                eprintln!(
                    "[STARFOX{}-PPU-SFX-RAW] x=128 y=96 sxy=({}, {}) tile_base=0x{:04X} row={} p0={:02X} p1={:02X} p2={:02X} p3={:02X} msb_color=0x{:02X} lsb_color=0x{:02X}",
                    label,
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
                eprintln!("[STARFOX{}-PPU-SFX-RAW] x=128 y=96 <none>", label);
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
                    label,
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
        if let Some(gsu) = self.bus.superfx.as_ref() {
            let nonzero_range = gsu.debug_nonzero_game_ram_range();
            eprintln!(
                "[STARFOX{}-GSU] pbr={:02X} r0={:04X} r1={:04X} r2={:04X} r3={:04X} r4={:04X} r5={:04X} r6={:04X} r7={:04X} r8={:04X} r9={:04X} r12={:04X} r13={:04X} r14={:04X} r15={:04X} rombr={:02X} rambr={:02X} scbr={:02X} scmr={:02X} cbr={:04X} cfgr={:02X} colr={:02X} por={:02X} src=r{} dst=r{} game_ram_nonzero={} screen_nonzero={} range={:?}",
                label,
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
            if let Some((buffer, height, bpp, mode)) = gsu.screen_buffer_snapshot() {
                let first_nonzero = buffer.iter().position(|&byte| byte != 0);
                let head_len = buffer.len().min(64);
                let around = first_nonzero.map(|idx| {
                    let start = idx.saturating_sub(16);
                    let end = (idx + 48).min(buffer.len());
                    (idx, buffer[start..end].to_vec())
                });
                eprintln!(
                    "[STARFOX{}-GSU-SCREEN] len={} height={} bpp={} mode={} first_nonzero={:?} head={:02X?} around={:?}",
                    label,
                    buffer.len(),
                    height,
                    bpp,
                    mode,
                    first_nonzero,
                    &buffer[..head_len],
                    around
                );
            }
            if let Some((buffer, height, bpp, mode)) = gsu.tile_buffer_snapshot() {
                let first_nonzero = buffer.iter().position(|&byte| byte != 0);
                let head_len = buffer.len().min(64);
                let around = first_nonzero.map(|idx| {
                    let start = idx.saturating_sub(16);
                    let end = (idx + 48).min(buffer.len());
                    (idx, buffer[start..end].to_vec())
                });
                eprintln!(
                    "[STARFOX{}-GSU-TILE] len={} height={} bpp={} mode={} first_nonzero={:?} head={:02X?} around={:?}",
                    label,
                    buffer.len(),
                    height,
                    bpp,
                    mode,
                    first_nonzero,
                    &buffer[..head_len],
                    around
                );
            }
            if std::env::var_os("TRACE_SUPERFX_PROFILE").is_some() {
                eprintln!(
                    "[STARFOX{}-GSU-PROFILE] top={:?} top_alt={:?}",
                    label,
                    gsu.debug_top_profile(24),
                    gsu.debug_top_profile_by_alt(24)
                );
            }
            if std::env::var_os("TRACE_SUPERFX_PC_TRACE").is_some()
                || std::env::var_os("TRACE_SUPERFX_LAST_TRANSFERS").is_some()
            {
                eprintln!(
                    "[STARFOX{}-GSU-PC] recent={:?}",
                    label,
                    gsu.debug_recent_pc_transfers()
                );
            }
            if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some()
                || std::env::var_os("TRACE_SUPERFX_LAST_WRITERS").is_some()
            {
                if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some() {
                    eprintln!(
                        "[STARFOX{}-GSU-REG] recent={:?}",
                        label,
                        gsu.debug_recent_reg_writes()
                    );
                }
                eprintln!(
                    "[STARFOX{}-GSU-LAST] r0={:?} r1={:?} r4={:?} r5={:?} r6={:?} r7={:?} r8={:?} r9={:?} r10={:?} r11={:?} r12={:?} r13={:?} r14={:?} r15={:?}",
                    label,
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
                    label,
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
                    label,
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
                    label,
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
                            label, addr, value, lo, hi
                        );
                    }
                }
                if std::env::var_os("TRACE_SUPERFX_REG_FLOW").is_some() {
                    let recent_exec = gsu.debug_recent_exec_trace();
                    eprintln!("[STARFOX{}-GSU-EXEC-LEN] {}", label, recent_exec.len());
                    for entry in recent_exec.iter().rev().take(64) {
                        eprintln!("[STARFOX{}-GSU-EXEC] {:?}", label, entry);
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
                let mut dump = [0u8; 16];
                for (i, byte) in dump.iter_mut().enumerate() {
                    *byte = self.bus.read_u8(addr.wrapping_add(i as u32));
                }
                eprintln!("[STARFOX{}-DUMP] {:06X}: {:02X?}", label, addr, dump);
            }
        }
    }

    pub(super) fn maybe_dump_starfox_diag_at(&mut self) {
        static CFG: OnceLock<Option<u64>> = OnceLock::new();
        let Some(frame) = *CFG.get_or_init(|| {
            std::env::var("STARFOX_DIAG_AT")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
        }) else {
            return;
        };

        if self.frame_count == frame {
            let label = frame.to_string();
            self.dump_starfox_diag(&label);
        }
    }
}
