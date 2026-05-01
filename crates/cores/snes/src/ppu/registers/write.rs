#![allow(static_mut_refs)]

use crate::ppu::{
    trace_cgram_write_config, trace_cgram_write_match, trace_vram_write_config,
    trace_vram_write_match, Ppu, IMPORTANT_WRITE_LIMIT,
};

impl Ppu {
    pub(crate) fn write(&mut self, addr: u16, mut value: u8) {
        // デバッグ：全PPUレジスタ書き込み（抑制可能）
        if crate::debug_flags::ppu_write() {
            static mut TOTAL_PPU_WRITES: u32 = 0;
            unsafe {
                TOTAL_PPU_WRITES += 1;
                if TOTAL_PPU_WRITES <= 50 || TOTAL_PPU_WRITES.is_multiple_of(100) {
                    println!(
                        "PPU Write #{}: 0x21{:02X} = 0x{:02X}",
                        TOTAL_PPU_WRITES, addr, value
                    );
                }
            }
        }

        // デバッグ：重要なPPUレジスタ書き込みをログ
        static mut IMPORTANT_WRITES: u32 = 0;
        static mut VRAM_DATA_WRITES: u32 = 0;
        static mut CGRAM_DATA_WRITES: u32 = 0;
        let is_important = matches!(
            addr,
            0x00 | 0x01 | 0x2C | 0x2D | 0x2E | 0x2F | 0x30 | 0x31 | 0x32 | 0x33
        );
        let is_vram_data = matches!(addr, 0x18 | 0x19); // VRAM data registers
        let is_cgram_data = addr == 0x22; // CGRAM data register

        if is_important && (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose()) {
            unsafe {
                IMPORTANT_WRITES += 1;
                if IMPORTANT_WRITES <= IMPORTANT_WRITE_LIMIT {
                    println!("PPU Important Write: 0x21{:02X} = 0x{:02X}", addr, value);
                }
            }
        }

        // Monitor VRAM data writes more closely
        if is_vram_data && (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose()) {
            unsafe {
                VRAM_DATA_WRITES += 1;
                // Show more VRAM writes to detect patterns
                if VRAM_DATA_WRITES <= 50 || VRAM_DATA_WRITES.is_multiple_of(100) || value != 0x00 {
                    println!(
                        "VRAM DATA[{}]: 0x21{:02X} = 0x{:02X} (addr=0x{:04X}) [{}]",
                        VRAM_DATA_WRITES,
                        addr,
                        value,
                        self.vram_addr,
                        if value == 0x00 { "clear" } else { "data" }
                    );
                }

                // Detect potential graphics loading patterns
                if value != 0x00 && self.vram_addr <= 0x8000 && VRAM_DATA_WRITES.is_multiple_of(500)
                {
                    println!(
                        "GRAPHICS LOADING: {} non-zero VRAM writes detected at 0x{:04X}",
                        VRAM_DATA_WRITES, self.vram_addr
                    );
                }
            }
        }

        // Monitor CGRAM data writes
        if is_cgram_data && (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose())
        {
            unsafe {
                CGRAM_DATA_WRITES += 1;
                if CGRAM_DATA_WRITES <= 100 || CGRAM_DATA_WRITES.is_multiple_of(50) || value != 0x00
                {
                    let color_index = self.cgram_addr >> 1;
                    let is_high = (self.cgram_addr & 1) == 1;
                    println!(
                        "CGRAM DATA[{}]: 0x2122 = 0x{:02X} (addr=0x{:02X}) [color {} {}]",
                        CGRAM_DATA_WRITES,
                        value,
                        self.cgram_addr,
                        color_index,
                        if is_high { "HIGH" } else { "LOW" }
                    );
                }

                // Detect complete palette loading
                if CGRAM_DATA_WRITES > 0 && CGRAM_DATA_WRITES.is_multiple_of(32) {
                    println!(
                        "PALETTE PROGRESS: {} colors potentially loaded",
                        CGRAM_DATA_WRITES / 2
                    );
                }
            }
        }

        match addr {
            0x00 => {
                // INIDISP - Forced blank and brightness
                let prev_display = self.screen_display;
                let defer_update =
                    crate::debug_flags::strict_ppu_timing() && self.in_active_display();

                // Optional: globally lock the display ON (for stubborn titles like SMW when APU upload is stubbed)
                if std::env::var("FORCE_INIDISP_ON")
                    .map(|v| v == "1" || v.to_lowercase() == "true")
                    .unwrap_or(false)
                {
                    let mut patched = value & 0x0F; // brightness only
                    if patched == 0 {
                        patched = 0x0F;
                    }
                    value = patched; // ensure forced blank bit cleared
                }

                // Optional: ignore CPU writes to INIDISP (debug workaround for stubborn blanking)
                if self.write_ctx == 0 && crate::debug_flags::ignore_inidisp_cpu() {
                    return;
                }

                // CPU writes (write_ctx == 0): log first few to catch unintended values (e.g., 0x9X)
                if self.write_ctx == 0 && std::env::var_os("TRACE_INIDISP_CPU").is_some() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static COUNT: AtomicU32 = AtomicU32::new(0);
                    let n = COUNT.fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        println!(
                            "[INIDISP-CPU][{}] scanline={} value=0x{:02X} (prev=0x{:02X} forced_blank={})",
                            n + 1,
                            self.scanline,
                            value,
                            prev_display,
                            (value & 0x80) != 0
                        );
                    }
                }
                // Targeted INIDISP trace for specific frame range
                if let Ok(range) = std::env::var("TRACE_INIDISP_RANGE") {
                    let parts: Vec<u64> = range.split('-').filter_map(|s| s.parse().ok()).collect();
                    if parts.len() == 2 && self.frame >= parts[0] && self.frame <= parts[1] {
                        let who = match self.write_ctx {
                            0 => "CPU",
                            1 => "MDMA",
                            2 => "HDMA",
                            _ => "?",
                        };
                        eprintln!(
                            "[INIDISP-TRACE] {} frame={} sl={} val=0x{:02X} blank={} bright={} (prev=0x{:02X})",
                            who, self.frame, self.scanline, value,
                            (value & 0x80) != 0, value & 0x0F, prev_display
                        );
                    }
                }

                // DMA/HDMA writes to INIDISP ($2100) are valid on real hardware.
                // We only block them when explicitly requested for debugging.
                if self.write_ctx != 0 {
                    if crate::debug_flags::block_inidisp_dma() {
                        return;
                    }
                    if std::env::var_os("DEBUG_INIDISP_DMA").is_some()
                        && !crate::debug_flags::quiet()
                    {
                        let source = match self.write_ctx {
                            1 => "MDMA",
                            2 => "HDMA",
                            _ => "unknown",
                        };
                        let ch = self.debug_dma_channel.unwrap_or(0xFF);
                        println!(
                            "[INIDISP-DMA] {} ch={} scanline={} cyc={} value=0x{:02X} blank={} brightness={}",
                            source,
                            if ch == 0xFF { -1 } else { ch as i32 },
                            self.scanline,
                            self.cycle,
                            value,
                            ((value & 0x80) != 0) as u8,
                            value & 0x0F,
                        );
                    }
                }

                // Optional debug override: force display on with max brightness
                if std::env::var_os("FORCE_MAX_BRIGHTNESS").is_some() {
                    self.screen_display = 0x0F;
                    self.brightness = 0x0F;
                    return;
                }

                let applied_value = value;
                if defer_update {
                    self.latched_inidisp = Some(applied_value);
                } else {
                    self.screen_display = applied_value;
                    self.brightness = applied_value & 0x0F;
                    self.maybe_reset_oam_on_inidisp(prev_display, applied_value);
                }
                let log_value = if defer_update {
                    applied_value
                } else {
                    self.screen_display
                };
                let forced_blank_prev = (prev_display & 0x80) != 0;
                let forced_blank_new = (log_value & 0x80) != 0;
                if !crate::debug_flags::quiet() && crate::debug_flags::trace_ppu_inidisp() {
                    println!(
                        "TRACE_PPU_INIDISP: prev=0x{:02X} new=0x{:02X} forced_blank {}→{} brightness {}→{} (latched={})",
                        prev_display,
                        log_value,
                        forced_blank_prev,
                        forced_blank_new,
                        prev_display & 0x0F,
                        log_value & 0x0F,
                        defer_update
                    );
                }
            }
            0x01 => {
                // OBSEL ($2101): Sprite size and name base
                // bits 5-7: sprite size, bits 3-4: name select, bits 0-2: name base high bits
                self.sprite_size = (value >> 5) & 0x07;
                // Name select (nn) is stored raw; the secondary 8KB table starts at
                // (nn + 1) * 8KB words from the name base. With nn=0, it is contiguous.
                self.sprite_name_select = ((value >> 3) & 0x03) as u16;
                // Base address for tiles 0x000..0x0FF (8K-word = 16KB-byte steps).
                self.sprite_name_base = ((value & 0x07) as u16) << 13;
                if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                    println!(
                        "PPU: Sprite size: {}, name base: 0x{:04X}, select: 0x{:04X}",
                        self.sprite_size, self.sprite_name_base, self.sprite_name_select
                    );
                }
            }
            0x02 => {
                // OAMADDL ($2102)
                // Sets OAM *word* address (low 8 bits). Internal address becomes (word<<1).
                self.oam_addr = (self.oam_addr & 0x0100) | (value as u16);
                self.oam_addr &= 0x01FF;
                self.oam_internal_addr = (self.oam_addr & 0x01FF) << 1;
                self.refresh_oam_eval_base_from_internal_addr();
                // Start small OAM data gap (for MDMA/CPU) after address change
                self.oam_data_gap_ticks = crate::debug_flags::oam_gap_after_oamadd();
            }
            0x03 => {
                // OAMADDH ($2103)
                // SNESdev wiki:
                // - bit0: OAM word address bit8
                // - bit7: OBJ priority rotation enable
                self.oam_priority_rotation_enabled = (value & 0x80) != 0;
                self.oam_addr = (self.oam_addr & 0x00FF) | (((value as u16) & 0x01) << 8);
                self.oam_addr &= 0x01FF;
                self.oam_internal_addr = (self.oam_addr & 0x01FF) << 1;
                self.refresh_oam_eval_base_from_internal_addr();
                self.oam_data_gap_ticks = crate::debug_flags::oam_gap_after_oamadd();
            }
            0x04 => {
                // OAMDATA ($2104)
                // SNESdev wiki:
                // - Low table (internal < 0x200): writes are staged; the *odd* byte write commits a word.
                // - High table (internal >= 0x200): direct byte writes; internal increments by 1 each time.
                if !self.can_write_oam_now() {
                    if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                        println!("PPU TIMING: Skip OAMDATA write outside VBlank (strict)");
                    }
                    self.oam_rejects = self.oam_rejects.saturating_add(1);
                    if self.oam_data_gap_ticks > 0 && self.write_ctx != 2 {
                        self.oam_gap_blocks = self.oam_gap_blocks.saturating_add(1);
                    }
                    if crate::debug_flags::timing_rejects()
                        && self.last_reject_frame_oam != self.frame
                    {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let reason = if self.oam_data_gap_ticks > 0 && self.write_ctx != 2 {
                            "[gap]"
                        } else {
                            ""
                        };
                        println!(
                            "⛔ OAM REJECT: y={} x={} ctx={} addr=$2104 {}",
                            self.scanline, self.cycle, who, reason
                        );
                        self.last_reject_frame_oam = self.frame;
                    }
                    return;
                }
                let internal = self.oam_internal_addr & 0x03FF;
                if internal < 0x200 {
                    if (internal & 1) == 0 {
                        self.oam_write_latch = value;
                    } else {
                        let even = (internal & !1) as usize;
                        let odd = internal as usize;
                        if even < self.oam.len() {
                            self.oam[even] = self.oam_write_latch;
                        }
                        if odd < self.oam.len() {
                            self.oam[odd] = value;
                        }
                        self.oam_writes_total = self.oam_writes_total.saturating_add(2);
                        self.oam_dirty = true;
                    }
                } else {
                    let mapped = (0x200 | (internal & 0x001F)) as usize;
                    if mapped < self.oam.len() {
                        self.oam[mapped] = value;
                    }
                    self.oam_writes_total = self.oam_writes_total.saturating_add(1);
                    self.oam_dirty = true;
                }
                self.oam_internal_addr = (internal + 1) & 0x03FF;
                self.refresh_oam_eval_base_from_internal_addr();
            }
            0x05 => {
                // BGMODE: bit0-2: mode, bit4-7: tile size for BG1..BG4 (1=16x16)
                let requested_mode = value & 0x07;
                self.bg_mode = requested_mode;
                // Mode 1 BG3 priority bit (bit3)
                self.mode1_bg3_priority = (value & 0x08) != 0;
                self.bg_tile_16[0] = (value & 0x10) != 0;
                self.bg_tile_16[1] = (value & 0x20) != 0;
                self.bg_tile_16[2] = (value & 0x40) != 0;
                self.bg_tile_16[3] = (value & 0x80) != 0;

                if (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose())
                    && !crate::debug_flags::quiet()
                {
                    println!(
                        "PPU: BG Mode set to {} (reg $2105 = 0x{:02X}), BG3prio={}, tile16: [{},{},{},{}]",
                        self.bg_mode,
                        value,
                        self.mode1_bg3_priority,
                        self.bg_tile_16[0],
                        self.bg_tile_16[1],
                        self.bg_tile_16[2],
                        self.bg_tile_16[3]
                    );
                    if self.bg_mode == 2 {
                        println!(
                            "  Mode 2 activated: BG1 & BG2 use 4bpp tiles with offset-per-tile"
                        );
                    }
                }
                self.update_line_render_state();
                self.bg_cache_dirty = true;
            }
            0x06 => {
                self.bg_mosaic = value;
                self.mosaic_size = ((value >> 4) & 0x0F) + 1; // ビット4-7がモザイクサイズ（0-15 → 1-16）
            }
            0x07 => {
                // BG1SC ($2107):
                // - bits 0-1: screen size
                // - bits 2-7: tilemap base in units of 0x400 bytes (1KB)
                // Tilemap base is stored as VRAM *word* address.
                // Common reference formula (SNESdev): base word address = (value & 0xFC) << 8.
                self.bg1_tilemap_base = ((value as u16) & 0xFC) << 8;
                self.bg_screen_size[0] = value & 0x03;
                self.bg_cache_dirty = true;
            }
            0x08 => {
                // BG2SC ($2108): store base as VRAM word address (see $2107)
                self.bg2_tilemap_base = ((value as u16) & 0xFC) << 8;
                self.bg_screen_size[1] = value & 0x03;
                self.bg_cache_dirty = true;
                if (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose())
                    && !crate::debug_flags::quiet()
                {
                    println!(
                        "PPU: BG2 tilemap base: 0x{:04X}, size={}",
                        self.bg2_tilemap_base, self.bg_screen_size[1]
                    );
                }
            }
            0x09 => {
                // BG3SC ($2109): store base as VRAM word address (see $2107)
                self.bg3_tilemap_base = ((value as u16) & 0xFC) << 8;
                self.bg_screen_size[2] = value & 0x03;
                self.bg_cache_dirty = true;
            }
            0x0A => {
                // BG4SC ($210A): store base as VRAM word address (see $2107)
                self.bg4_tilemap_base = ((value as u16) & 0xFC) << 8;
                self.bg_screen_size[3] = value & 0x03;
                self.bg_cache_dirty = true;
            }
            0x0B => {
                // BG12NBA ($210B): Character (tile) data area designation.
                // Bits 0-3: BG1 base, bits 4-7: BG2 base.
                // Unit is 0x2000 bytes (8 KiB); VRAM is word-addressed.
                // => base_word = nibble * 0x1000 words (0x2000 bytes)
                let bg1 = (value & 0x0F) as u16;
                let bg2 = ((value >> 4) & 0x0F) as u16;
                self.bg1_tile_base = bg1 << 12;
                self.bg2_tile_base = bg2 << 12;
                self.bg_cache_dirty = true;
            }
            0x0C => {
                // BG34NBA ($210C): Character (tile) data area designation.
                // Bits 0-3: BG3 base, bits 4-7: BG4 base.
                // Unit is 0x2000 bytes (8 KiB); VRAM is word-addressed.
                let bg3 = (value & 0x0F) as u16;
                let bg4 = ((value >> 4) & 0x0F) as u16;
                self.bg3_tile_base = bg3 << 12;
                self.bg4_tile_base = bg4 << 12;
                self.bg_cache_dirty = true;
                if (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose())
                    && !crate::debug_flags::quiet()
                {
                    println!(
                        "PPU: BG3 tile base: 0x{:04X}, BG4 tile base: 0x{:04X}",
                        self.bg3_tile_base, self.bg4_tile_base
                    );
                }
            }
            0x0D => {
                // $210D also maps to M7HOFS (Mode 7); uses the Mode 7 latch (shared with $211B-$2120).
                self.write_m7hofs(value);
                self.write_bghofs(0, value);
            }
            0x0E => {
                // $210E also maps to M7VOFS (Mode 7); uses the Mode 7 latch (shared with $211B-$2120).
                self.write_m7vofs(value);
                self.write_bgvofs(0, value);
            }
            0x0F => {
                self.write_bghofs(1, value);
            }
            0x10 => {
                self.write_bgvofs(1, value);
            }
            0x11 => {
                self.write_bghofs(2, value);
            }
            0x12 => {
                self.write_bgvofs(2, value);
            }
            0x13 => {
                self.write_bghofs(3, value);
            }
            0x14 => {
                self.write_bgvofs(3, value);
            }
            0x15 => {
                // $2115: VRAM Address Increment/Mapping
                // NOTE: VMAIN is a normal control register; in the common (non-strict) path
                // it takes effect immediately. Any deferral is debug-only behind STRICT_PPU_TIMING.
                if crate::debug_flags::strict_ppu_timing() {
                    // In STRICT timing, defer changes to a safe sub-window.
                    // Always record last written for summaries.
                    self.vram_last_vmain = value;
                    if self.can_commit_vmain_now() {
                        // Defer the visible effect by a small number of dots (debug-only)
                        self.vmain_effect_pending = Some(value);
                        self.vmain_effect_ticks = crate::debug_flags::vmain_effect_delay_dots();
                    } else {
                        self.latched_vmain = Some(value);
                    }
                } else {
                    // Immediate apply (default)
                    self.vram_mapping = value;
                    self.vram_last_vmain = value;
                    self.vram_increment = match value & 0x03 {
                        0 => 1,
                        1 => 32,
                        _ => 128,
                    };
                    self.vmain_effect_pending = None;
                    self.vmain_effect_ticks = 0;
                    self.latched_vmain = None;
                }
                if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                    static mut VMAIN_LOG_CNT: u32 = 0;
                    unsafe {
                        VMAIN_LOG_CNT += 1;
                        if VMAIN_LOG_CNT <= 8 {
                            let inc = match value & 0x03 {
                                0 => 1,
                                1 => 32,
                                _ => 128,
                            };
                            let fg = (value >> 2) & 0x03;
                            let inc_on_high = (value & 0x80) != 0;
                            println!("VMAIN write: 0x{:02X} (inc={}, FGmode={}, inc_on_{}, pending_commit={})",
                                value, inc, fg, if inc_on_high {"HIGH"} else {"LOW"},
                                crate::debug_flags::strict_ppu_timing() && !self.can_commit_vmain_now());
                        }
                    }
                }
            }
            0x16 => {
                if self.can_commit_vmadd_now() {
                    self.vram_addr = (self.vram_addr & 0xFF00) | (value as u16);
                    // SNESdev wiki: On VMADD write, vram_latch = [VMADD]
                    self.reload_vram_read_latch();
                } else {
                    self.latched_vmadd_lo = Some(value);
                }
                if crate::debug_flags::boot_verbose() {
                    static mut VRAM_ADDR_SET_COUNT: u32 = 0;
                    unsafe {
                        VRAM_ADDR_SET_COUNT += 1;
                        if VRAM_ADDR_SET_COUNT <= 10 {
                            println!(
                                "VRAM address LOW write: 0x{:02X} (pending_commit={})",
                                value,
                                !self.can_commit_vmadd_now()
                            );
                        }
                    }
                }
            }
            0x17 => {
                if self.can_commit_vmadd_now() {
                    self.vram_addr = (self.vram_addr & 0x00FF) | ((value as u16) << 8);
                    // SNESdev wiki: On VMADD write, vram_latch = [VMADD]
                    self.reload_vram_read_latch();
                } else {
                    self.latched_vmadd_hi = Some(value);
                }
                if crate::debug_flags::boot_verbose() {
                    static mut VRAM_ADDR_SET_COUNT_HIGH: u32 = 0;
                    unsafe {
                        VRAM_ADDR_SET_COUNT_HIGH += 1;
                        if VRAM_ADDR_SET_COUNT_HIGH <= 10 {
                            println!(
                                "VRAM address HIGH write: 0x{:02X} (pending_commit={})",
                                value,
                                !self.can_commit_vmadd_now()
                            );
                        }
                    }
                }
            }
            0x18 => {
                // VRAM Data Write (Low byte) - $2118
                // STRICT: 許可はVBlankまたはHBlank中（HDMA先頭含む）の安全ドットのみ
                if !self.is_vram_write_safe_dot() {
                    if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                        println!("PPU TIMING: Skip VMDATAL write outside blank (strict)");
                    }
                    self.vram_rejects = self.vram_rejects.saturating_add(1);
                    if self.vmain_data_gap_ticks > 0 && self.write_ctx != 2 {
                        self.vram_gap_blocks = self.vram_gap_blocks.saturating_add(1);
                    }
                    if crate::debug_flags::timing_rejects()
                        && self.last_reject_frame_vram != self.frame
                    {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let reason = if self.vmain_data_gap_ticks > 0 && self.write_ctx != 2 {
                            "[gap]"
                        } else {
                            ""
                        };
                        println!(
                            "⛔ VRAM REJECT: y={} x={} ctx={} addr=$2118 {}",
                            self.scanline, self.cycle, who, reason
                        );
                        self.last_reject_frame_vram = self.frame;
                    }
                    // Even if the VRAM write is ignored, VMADD still increments depending on VMAIN.
                    // (SNESdev wiki: "VMADD will always increment ... even if the VRAM write is ignored.")
                    if (self.vram_mapping & 0x80) == 0 {
                        self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment);
                    }
                    return;
                }
                if crate::debug_flags::boot_verbose() {
                    static mut VRAM_DIRECT_WRITE_LOW_COUNT: u32 = 0;
                    unsafe {
                        VRAM_DIRECT_WRITE_LOW_COUNT += 1;
                        if VRAM_DIRECT_WRITE_LOW_COUNT <= 20 {
                            println!(
                                "VRAM write $2118 = 0x{:02X} (addr=0x{:04X})",
                                value, self.vram_addr
                            );
                        }
                    }
                }
                let masked_addr = self.vram_remap_word_addr(self.vram_addr); // apply FG mapping
                                                                             // VRAM is word-addressed (0x0000-0x7FFF), but stored as bytes (0x0000-0xFFFF)
                let vram_index = ((masked_addr & 0x7FFF) as usize * 2) & 0xFFFF; // Low byte at even address
                if let Some(cfg) = trace_vram_write_config() {
                    if trace_vram_write_match(cfg, masked_addr, self.frame) {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let ch = self.debug_dma_channel.unwrap_or(0xFF);
                        println!(
                            "[TRACE_VRAM_WRITE] frame={} sl={} cyc={} ctx={} ch={} reg=$2118 raw=0x{:04X} masked=0x{:04X} val=0x{:02X} VMAIN=0x{:02X} inc={} range=0x{:04X}-0x{:04X}",
                            self.frame,
                            self.scanline,
                            self.cycle,
                            who,
                            ch,
                            self.vram_addr,
                            masked_addr,
                            value,
                            self.vram_mapping,
                            self.vram_increment,
                            cfg.start_addr,
                            cfg.end_addr
                        );
                    }
                }

                // burn-in-test.sfc DMA MEMORY: detect unexpected writes into the test region.
                if self.burnin_vram_trace_armed
                    && std::env::var_os("TRACE_BURNIN_DMA_MEMORY").is_some()
                    && (0x5000..0x5800).contains(&masked_addr)
                {
                    let dma_ch = self.debug_dma_channel.unwrap_or(0xFF);
                    let is_known = self.write_ctx == 1 && dma_ch == 6;
                    if !is_known {
                        let n = self.burnin_vram_trace_cnt_2118;
                        self.burnin_vram_trace_cnt_2118 =
                            self.burnin_vram_trace_cnt_2118.saturating_add(1);
                        if n < 64 {
                            let who = match self.write_ctx {
                                2 => "HDMA",
                                1 => "MDMA",
                                _ => "CPU",
                            };
                            println!(
                                "[BURNIN-VRAM-WRITE] {} ch={} frame={} sl={} cyc={} vblank={} hblank={} fblank={} vis_h={} VMAIN={:02X} inc={} raw={:04X} masked={:04X} $2118={:02X}",
                                who,
                                dma_ch,
                                self.frame,
                                self.scanline,
                                self.cycle,
                                self.v_blank as u8,
                                self.h_blank as u8,
                                ((self.screen_display & 0x80) != 0) as u8,
                                self.get_visible_height(),
                                self.vram_mapping,
                                self.vram_increment,
                                self.vram_addr,
                                masked_addr,
                                value
                            );
                        }
                    }
                }

                // Summary counters (bucketed by masked word address high bits)
                let bucket = ((masked_addr >> 12) & 0x7) as usize; // 0..7
                if bucket < self.vram_write_buckets.len() {
                    self.vram_write_buckets[bucket] =
                        self.vram_write_buckets[bucket].saturating_add(1);
                }
                self.vram_write_low_count = self.vram_write_low_count.saturating_add(1);
                self.vram_writes_total_low = self.vram_writes_total_low.saturating_add(1);

                // Debug output disabled for performance

                if vram_index < self.vram.len() {
                    self.vram[vram_index] = value;
                    self.bg_cache_dirty = true;
                } else {
                    println!(
                        "WARNING: VRAM write out of bounds! index=0x{:05X} >= len=0x{:05X}",
                        vram_index,
                        self.vram.len()
                    );
                }
                // アドレスインクリメントモード（bit 7）
                // bit7=0 -> LOW($2118) 書き込み後にインクリメント
                // bit7=1 -> HIGH($2119) 書き込み後にインクリメント
                if (self.vram_mapping & 0x80) == 0 {
                    self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment);
                }
            }
            0x19 => {
                // VRAM Data Write (High byte) - $2119
                if !self.is_vram_write_safe_dot() {
                    if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                        println!("PPU TIMING: Skip VMDATAH write outside blank (strict)");
                    }
                    self.vram_rejects = self.vram_rejects.saturating_add(1);
                    if self.vmain_data_gap_ticks > 0 && self.write_ctx != 2 {
                        self.vram_gap_blocks = self.vram_gap_blocks.saturating_add(1);
                    }
                    if crate::debug_flags::timing_rejects()
                        && self.last_reject_frame_vram != self.frame
                    {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let reason = if self.vmain_data_gap_ticks > 0 && self.write_ctx != 2 {
                            "[gap]"
                        } else {
                            ""
                        };
                        println!(
                            "⛔ VRAM REJECT: y={} x={} ctx={} addr=$2119 {}",
                            self.scanline, self.cycle, who, reason
                        );
                        self.last_reject_frame_vram = self.frame;
                    }
                    // Even if the VRAM write is ignored, VMADD still increments depending on VMAIN.
                    if (self.vram_mapping & 0x80) != 0 {
                        self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment);
                    }
                    return;
                }
                if crate::debug_flags::boot_verbose() {
                    static mut VRAM_DIRECT_WRITE_HIGH_COUNT: u32 = 0;
                    unsafe {
                        VRAM_DIRECT_WRITE_HIGH_COUNT += 1;
                        if VRAM_DIRECT_WRITE_HIGH_COUNT <= 20 {
                            println!(
                                "VRAM write $2119 = 0x{:02X} (addr=0x{:04X})",
                                value, self.vram_addr
                            );
                        }
                    }
                }
                let masked_addr = self.vram_remap_word_addr(self.vram_addr);
                let vram_index = (((masked_addr & 0x7FFF) as usize) * 2 + 1) & 0xFFFF; // High byte at odd address
                if let Some(cfg) = trace_vram_write_config() {
                    if trace_vram_write_match(cfg, masked_addr, self.frame) {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let ch = self.debug_dma_channel.unwrap_or(0xFF);
                        println!(
                            "[TRACE_VRAM_WRITE] frame={} sl={} cyc={} ctx={} ch={} reg=$2119 raw=0x{:04X} masked=0x{:04X} val=0x{:02X} VMAIN=0x{:02X} inc={} range=0x{:04X}-0x{:04X}",
                            self.frame,
                            self.scanline,
                            self.cycle,
                            who,
                            ch,
                            self.vram_addr,
                            masked_addr,
                            value,
                            self.vram_mapping,
                            self.vram_increment,
                            cfg.start_addr,
                            cfg.end_addr
                        );
                    }
                }

                if self.burnin_vram_trace_armed
                    && std::env::var_os("TRACE_BURNIN_DMA_MEMORY").is_some()
                    && (0x5000..0x5800).contains(&masked_addr)
                {
                    let dma_ch = self.debug_dma_channel.unwrap_or(0xFF);
                    let is_known = self.write_ctx == 1 && dma_ch == 6;
                    if !is_known {
                        let n = self.burnin_vram_trace_cnt_2119;
                        self.burnin_vram_trace_cnt_2119 =
                            self.burnin_vram_trace_cnt_2119.saturating_add(1);
                        if n < 64 {
                            let who = match self.write_ctx {
                                2 => "HDMA",
                                1 => "MDMA",
                                _ => "CPU",
                            };
                            println!(
                                "[BURNIN-VRAM-WRITE] {} ch={} frame={} sl={} cyc={} vblank={} hblank={} fblank={} vis_h={} VMAIN={:02X} inc={} raw={:04X} masked={:04X} $2119={:02X}",
                                who,
                                dma_ch,
                                self.frame,
                                self.scanline,
                                self.cycle,
                                self.v_blank as u8,
                                self.h_blank as u8,
                                ((self.screen_display & 0x80) != 0) as u8,
                                self.get_visible_height(),
                                self.vram_mapping,
                                self.vram_increment,
                                self.vram_addr,
                                masked_addr,
                                value
                            );
                        }
                    }
                }

                // Summary counters
                let bucket = ((masked_addr >> 12) & 0x7) as usize; // 0..7
                if bucket < self.vram_write_buckets.len() {
                    self.vram_write_buckets[bucket] =
                        self.vram_write_buckets[bucket].saturating_add(1);
                }
                self.vram_write_high_count = self.vram_write_high_count.saturating_add(1);
                self.vram_writes_total_high = self.vram_writes_total_high.saturating_add(1);

                if vram_index < self.vram.len() {
                    self.vram[vram_index] = value;
                    self.bg_cache_dirty = true;
                } else {
                    println!(
                        "WARNING: VRAM high write out of bounds! index=0x{:05X} >= len=0x{:05X}",
                        vram_index,
                        self.vram.len()
                    );
                }

                // Increment when bit7 of VMAIN is 1 (increment on HIGH)
                if (self.vram_mapping & 0x80) != 0 {
                    self.vram_addr = self.vram_addr.wrapping_add(self.vram_increment);
                }
            }

            0x21 => {
                // CGADD - set color index (word address). In strict timing, defer to HBlank mid-window.
                if crate::debug_flags::ppu_write() && !crate::debug_flags::quiet() {
                    static mut CGADD_WRITE_COUNT: u32 = 0;
                    unsafe {
                        CGADD_WRITE_COUNT += 1;
                        if CGADD_WRITE_COUNT <= 64 {
                            println!(
                                "[PPU] CGADD write[{}]: value=0x{:02X}",
                                CGADD_WRITE_COUNT, value
                            );
                        }
                    }
                }
                if self.can_commit_cgadd_now() {
                    self.cgram_addr = value;
                    self.cgram_second = false;
                    self.cgram_read_second = false;
                } else {
                    self.latched_cgadd = Some(value);
                }
            }
            0x22 => {
                // CGDATA - staged writes: commit only on HIGH byte
                if !self.can_write_cgram_now() {
                    if crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose() {
                        println!("PPU TIMING: Skip CGDATA write outside VBlank (strict)");
                    }
                    self.cgram_rejects = self.cgram_rejects.saturating_add(1);
                    if self.cgram_data_gap_ticks > 0 && self.write_ctx != 2 {
                        self.cgram_gap_blocks = self.cgram_gap_blocks.saturating_add(1);
                    }
                    if crate::debug_flags::timing_rejects()
                        && self.last_reject_frame_cgram != self.frame
                    {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        let reason = if self.cgram_data_gap_ticks > 0 && self.write_ctx != 2 {
                            "[gap]"
                        } else {
                            ""
                        };
                        println!(
                            "⛔ CGRAM REJECT: y={} x={} ctx={} addr=$2122 {}",
                            self.scanline, self.cycle, who, reason
                        );
                        self.last_reject_frame_cgram = self.frame;
                    }
                    if let Some(cfg) = trace_cgram_write_config() {
                        if trace_cgram_write_match(cfg, self.frame, self.cgram_addr) {
                            let who = match self.write_ctx {
                                2 => "HDMA",
                                1 => "MDMA",
                                _ => "CPU",
                            };
                            println!(
                                "[TRACE_CGRAM_WRITE][REJECT] frame={} sl={} cyc={} ctx={} addr=0x{:02X} val=0x{:02X} second={} gap={}",
                                self.frame,
                                self.scanline,
                                self.cycle,
                                who,
                                self.cgram_addr,
                                value,
                                self.cgram_second as u8,
                                self.cgram_data_gap_ticks
                            );
                        }
                    }
                    return;
                }
                if !self.cgram_second {
                    // LOW byte stage: latch only
                    self.cgram_latch_lo = value;
                    self.cgram_second = true;
                    if let Some(cfg) = trace_cgram_write_config() {
                        if trace_cgram_write_match(cfg, self.frame, self.cgram_addr) {
                            let who = match self.write_ctx {
                                2 => "HDMA",
                                1 => "MDMA",
                                _ => "CPU",
                            };
                            println!(
                                "[TRACE_CGRAM_WRITE][LOW] frame={} sl={} cyc={} ctx={} addr=0x{:02X} val=0x{:02X}",
                                self.frame,
                                self.scanline,
                                self.cycle,
                                who,
                                self.cgram_addr,
                                value
                            );
                        }
                    }
                    static mut CGRAM_WRITE_COUNT: u32 = 0;
                    unsafe {
                        CGRAM_WRITE_COUNT += 1;
                        if (crate::debug_flags::ppu_write() || crate::debug_flags::boot_verbose())
                            && CGRAM_WRITE_COUNT <= 10
                            && !crate::debug_flags::quiet()
                        {
                            println!(
                                "CGRAM write[{}]: color=0x{:02X}, LOW byte, value=0x{:02X}",
                                CGRAM_WRITE_COUNT, self.cgram_addr, value
                            );
                        }
                    }
                } else {
                    // HIGH byte stage: commit both bytes
                    let base = (self.cgram_addr as usize) * 2;
                    if base + 1 < self.cgram.len() {
                        let quiet = crate::debug_flags::quiet();
                        // SNES CGRAM is 15-bit BGR; bit7 of the high byte is ignored.
                        let hi = value & 0x7F;
                        self.cgram[base] = self.cgram_latch_lo;
                        self.cgram[base + 1] = hi;
                        self.cgram_writes_total = self.cgram_writes_total.saturating_add(1);
                        self.update_cgram_rgb_cache(self.cgram_addr);
                        if let Some(cfg) = trace_cgram_write_config() {
                            if trace_cgram_write_match(cfg, self.frame, self.cgram_addr) {
                                let who = match self.write_ctx {
                                    2 => "HDMA",
                                    1 => "MDMA",
                                    _ => "CPU",
                                };
                                let stored = ((hi as u16) << 8) | (self.cgram_latch_lo as u16);
                                println!(
                                    "[TRACE_CGRAM_WRITE][HIGH] frame={} sl={} cyc={} ctx={} addr=0x{:02X} lo=0x{:02X} hi=0x{:02X} stored=0x{:04X}",
                                    self.frame,
                                    self.scanline,
                                    self.cycle,
                                    who,
                                    self.cgram_addr,
                                    self.cgram_latch_lo,
                                    hi,
                                    stored
                                );
                            }
                        }

                        // Debug the actual CGRAM storage
                        static mut CGRAM_STORE_DEBUG: u32 = 0;
                        unsafe {
                            CGRAM_STORE_DEBUG += 1;
                            if crate::debug_flags::ppu_write() && CGRAM_STORE_DEBUG <= 5 && !quiet {
                                let stored_color =
                                    ((hi as u16) << 8) | (self.cgram_latch_lo as u16);
                                println!("🎨 CGRAM STORED[{}]: addr={}, base={}, cgram[{}]=0x{:02X}, cgram[{}]=0x{:02X}, color=0x{:04X}",
                                        CGRAM_STORE_DEBUG, self.cgram_addr, base, base, self.cgram_latch_lo, base+1, hi, stored_color);
                            }
                        }
                        static mut CGRAM_WRITE_COUNT: u32 = 0;
                        unsafe {
                            CGRAM_WRITE_COUNT += 1;
                            if crate::debug_flags::ppu_write() && CGRAM_WRITE_COUNT <= 10 && !quiet
                            {
                                println!(
                                    "CGRAM write[{}]: color=0x{:02X}, HIGH byte, value=0x{:02X} (masked 0x{:02X})",
                                    CGRAM_WRITE_COUNT, self.cgram_addr, value, hi
                                );
                            }
                        }
                    }
                    // increment address after high byte
                    self.cgram_addr = self.cgram_addr.wrapping_add(1);
                    self.cgram_second = false;
                }
            }
            0x2C => {
                let strict_latched =
                    crate::debug_flags::strict_ppu_timing() && self.in_active_display();
                if strict_latched {
                    self.latched_tm = Some(value);
                } else {
                    self.main_screen_designation = value;
                    // Remember non-zero values for rendering (workaround for timing issues)
                    if value != 0 {
                        self.main_screen_designation_last_nonzero = value;
                    }
                    self.update_line_render_state();
                }
                if crate::debug_flags::trace_ppu_tm() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 512 {
                        let who = match self.write_ctx {
                            2 => "HDMA",
                            1 => "MDMA",
                            _ => "CPU",
                        };
                        println!(
                            "[TRACE_PPU_TM] {} frame={} sl={} cyc={} $212C=0x{:02X} (latched={}) raw_tm=0x{:02X} last_nonzero=0x{:02X}",
                            who,
                            self.frame,
                            self.scanline,
                            self.cycle,
                            value,
                            strict_latched as u8,
                            self.main_screen_designation,
                            self.main_screen_designation_last_nonzero
                        );
                    }
                }
                if crate::debug_flags::ppu_write() && !crate::debug_flags::quiet() {
                    static mut TM_DEBUG_COUNT: u32 = 0;
                    unsafe {
                        if TM_DEBUG_COUNT < 30 {
                            TM_DEBUG_COUNT += 1;
                            println!("PPU[$212C][{}]: scanline={} value=0x{:02X} (BG1:{} BG2:{} BG3:{} BG4:{} OBJ:{}) vblank={} using=0x{:02X}",
                                     TM_DEBUG_COUNT, self.scanline, value,
                                     (value & 1) != 0,
                                     (value & 2) != 0,
                                     (value & 4) != 0,
                                     (value & 8) != 0,
                                     (value & 16) != 0,
                                     self.v_blank,
                                     if value != 0 { value } else { self.main_screen_designation_last_nonzero });
                        }
                    }
                }
            }
            0x2D => {
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_ts = Some(value);
                } else {
                    self.sub_screen_designation = value;
                    self.update_line_render_state();
                }
            }
            0x2E => {
                // TMW - window mask enable (main)
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_tmw = Some(value & 0x1F);
                } else {
                    self.tmw_mask = value & 0x1F; // BG1..4 + OBJ
                }
            }
            0x2F => {
                // TSW - window mask enable (sub)
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_tsw = Some(value & 0x1F);
                } else {
                    self.tsw_mask = value & 0x1F;
                }
            }
            // ウィンドウ座標設定
            0x26 => {
                self.window1_left = value;
            }
            0x27 => {
                self.window1_right = value;
            }
            0x28 => {
                self.window2_left = value;
            }
            0x29 => {
                self.window2_right = value;
            }
            // BGウィンドウマスク設定
            0x23 => {
                self.window_bg_mask[0] = value & 0x0F; // BG1
                self.window_bg_mask[1] = (value >> 4) & 0x0F; // BG2
            }
            0x24 => {
                self.window_bg_mask[2] = value & 0x0F; // BG3
                self.window_bg_mask[3] = (value >> 4) & 0x0F; // BG4
            }
            0x25 => {
                self.window_obj_mask = value & 0x0F; // スプライト
                self.window_color_mask = (value >> 4) & 0x0F; // カラー
            }
            0x2A => {
                // WBGLOG: BG1..BG4 window logic (00=OR,01=AND,10=XOR,11=XNOR)
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_wbglog = Some(value);
                } else {
                    self.bg_window_logic[0] = value & 0x03;
                    self.bg_window_logic[1] = (value >> 2) & 0x03;
                    self.bg_window_logic[2] = (value >> 4) & 0x03;
                    self.bg_window_logic[3] = (value >> 6) & 0x03;
                }
            }
            0x2B => {
                // WOBJLOG: OBJ/COL window logic (00=OR,01=AND,10=XOR,11=XNOR)
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_wobjlog = Some(value);
                } else {
                    self.obj_window_logic = value & 0x03;
                    self.color_window_logic = (value >> 2) & 0x03;
                }
            }

            // カラー演算制御
            0x30 => {
                // CGWSEL: Color math gating + subscreen/fixed select
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_cgwsel = Some(value);
                } else {
                    self.cgwsel = value;
                    self.color_math_control = value; // legacy
                    self.update_line_render_state();
                }
            }
            0x31 => {
                // CGADSUB: Add/Sub + halve + layer enables
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_cgadsub = Some(value);
                } else {
                    self.cgadsub = value;
                    self.color_math_designation = value; // legacy: lower 6 bits as layer mask
                    self.update_line_render_state();
                }
            }
            0x32 => {
                // 固定色データ設定
                let intensity = value & 0x1F; // 強度（0-31）
                let mut next = self.fixed_color;

                // COLDATA ($2132): bits7/6/5 are enable flags for B/G/R components.
                // When set, the corresponding component of the fixed color is updated to INTENSITY.
                // Multiple components may be updated in a single write.
                //
                // The fixed color uses the same RGB555 layout as CGRAM:
                // bit0-4: Red, bit5-9: Green, bit10-14: Blue.
                if (value & 0x20) != 0 {
                    // Red: bits0-4
                    next = (next & !0x001F) | (intensity as u16);
                }
                if (value & 0x40) != 0 {
                    // Green: bits5-9
                    next = (next & !0x03E0) | ((intensity as u16) << 5);
                }
                if (value & 0x80) != 0 {
                    // Blue: bits10-14
                    next = (next & !0x7C00) | ((intensity as u16) << 10);
                }
                if crate::debug_flags::strict_ppu_timing() && self.in_active_display() {
                    self.latched_fixed_color = Some(next);
                } else {
                    self.fixed_color = next;
                }
            }
            0x33 => {
                // SETINI (pseudo hires, EXTBG, interlace)
                let vblank_start = self.vblank_start_line();
                if crate::debug_flags::strict_ppu_timing() && self.scanline < vblank_start {
                    // Defer any change during visible region (including HBlank) to line start
                    self.latched_setini = Some(value);
                } else {
                    self.setini = value;
                    self.pseudo_hires = (value & 0x08) != 0;
                    self.extbg = (value & 0x40) != 0;
                    self.overscan = (value & 0x04) != 0;
                    self.obj_interlace = (value & 0x02) != 0;
                    self.interlace = (value & 0x01) != 0;
                    self.update_line_render_state();
                }
            }

            // Mode 7 設定
            0x1A => {
                // M7SEL: bit7=R (0:wrap 1:fill), bit6=F (0:transparent 1:char0), bit1=Y flip, bit0=X flip
                self.m7sel = value;
            }
            // Mode 7レジスタ（2回書き込みで16ビット値を構成）
            0x1B..=0x20 => {
                // Mode 7 registers share a single latch. Writing low then high yields the intended
                // 16-bit value after the second write, but each write updates the register.
                let idx = (addr - 0x1B) as usize; // 0..5 (A,B,C,D,CenterX,CenterY)
                let combined_u16 = self.mode7_combine(value);
                let combined_i16 = combined_u16 as i16;
                match idx {
                    0 => {
                        self.mode7_matrix_a = combined_i16;
                        self.update_mode7_mul_result();
                    }
                    1 => {
                        self.mode7_matrix_b = combined_i16;
                        // $2134-$2136 uses the last 8-bit value written to M7B.
                        self.mode7_mul_b = value as i8;
                        self.update_mode7_mul_result();
                        if crate::debug_flags::trace_mode7_regs() && !crate::debug_flags::quiet() {
                            static M7B_LOG: std::sync::atomic::AtomicU32 =
                                std::sync::atomic::AtomicU32::new(0);
                            let n = M7B_LOG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if n < 4 {
                                println!("PPU: Mode 7 matrix B set to {}", self.mode7_matrix_b);
                            }
                        }
                    }
                    2 => {
                        self.mode7_matrix_c = combined_i16;
                    }
                    3 => {
                        self.mode7_matrix_d = combined_i16;
                    }
                    4 => {
                        self.mode7_center_x = Self::sign_extend13(combined_u16);
                    }
                    5 => {
                        self.mode7_center_y = Self::sign_extend13(combined_u16);
                    }
                    _ => {}
                }
                if idx == 0
                    && crate::debug_flags::trace_mode7_regs()
                    && !crate::debug_flags::quiet()
                {
                    println!("PPU: Mode 7 matrix A set to {}", self.mode7_matrix_a);
                }
            }

            _ => {}
        }
    }
}
