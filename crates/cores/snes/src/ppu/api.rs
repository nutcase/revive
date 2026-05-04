use super::Ppu;

impl Ppu {
    /// デバッグ用: BG1 のタイルマップ／タイルベースアドレスを取得
    #[allow(dead_code)]
    pub fn dbg_bg1_bases(&self) -> (u16, u16) {
        (self.bg1_tilemap_base, self.bg1_tile_base)
    }

    #[allow(dead_code)]
    pub fn dbg_bg_bases(&self, bg: usize) -> (u16, u16) {
        match bg {
            0 => (self.bg1_tilemap_base, self.bg1_tile_base),
            1 => (self.bg2_tilemap_base, self.bg2_tile_base),
            2 => (self.bg3_tilemap_base, self.bg3_tile_base),
            _ => (self.bg4_tilemap_base, self.bg4_tile_base),
        }
    }

    /// デバッグ用: VRAM 関連レジスタを取得
    pub fn dbg_vram_regs(&self) -> (u16, u16, u8) {
        (self.vram_addr, self.vram_increment, self.vram_mapping)
    }

    // Raw memory accessors (headless debug dump)
    #[allow(dead_code)]
    pub fn get_vram(&self) -> &[u8] {
        &self.vram
    }

    #[allow(dead_code)]
    pub fn get_cgram(&self) -> &[u8] {
        &self.cgram
    }

    #[allow(dead_code)]
    pub fn get_oam(&self) -> &[u8] {
        &self.oam
    }

    // Convenience dumps (head portion) for debugging
    pub fn dump_vram_head(&self, n: usize) -> Vec<u8> {
        let cnt = n.min(self.vram.len());
        self.vram[..cnt].to_vec()
    }

    pub fn dump_cgram_head(&self, n: usize) -> Vec<u16> {
        let mut out = Vec::new();
        let cnt = n.min(16).min(self.cgram.len() / 2);
        for i in 0..cnt {
            let lo = self.cgram[i * 2] as u16;
            let hi = self.cgram[i * 2 + 1] as u16;
            out.push((hi << 8) | lo);
        }
        out
    }

    pub fn dump_oam_head(&self, n: usize) -> Vec<u8> {
        let cnt = n.min(self.oam.len());
        self.oam[..cnt].to_vec()
    }

    #[allow(dead_code)]
    pub fn get_subscreen_buffer(&self) -> &[u32] {
        &self.subscreen_buffer
    }

    // Debug helper: expose current OAM address and internal address.
    #[inline]
    pub fn dbg_oam_addrs(&self) -> (u16, u16) {
        (self.oam_addr, self.oam_internal_addr)
    }

    pub fn is_forced_blank(&self) -> bool {
        (self.screen_display & 0x80) != 0
    }

    pub fn current_brightness(&self) -> u8 {
        self.brightness & 0x0F
    }

    pub fn get_main_screen_designation(&self) -> u8 {
        self.main_screen_designation
    }

    pub fn get_bg_mode(&self) -> u8 {
        self.bg_mode
    }

    // Headless init counters summary
    pub fn get_init_counters(&self) -> (u32, u64, u64, u64, u64) {
        (
            self.important_writes_count,
            self.vram_writes_total_low,
            self.vram_writes_total_high,
            self.cgram_writes_total,
            self.oam_writes_total,
        )
    }

    // prepare_line_opt_luts moved to rendering.rs

    // update_line_render_state moved to rendering.rs

    // read_bg_tilemap_entry_word moved to rendering.rs

    // invalidate_bg_caches moved to rendering.rs

    // get_bg_map_entry_cached moved to rendering.rs

    // sample_bg_cached moved to rendering.rs

    // Summarize VRAM writes since last call, including FG mode info. Resets counters.
    pub fn take_vram_write_summary(&mut self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let fg_mode = (self.vram_last_vmain >> 2) & 0x03;
        let inc = match self.vram_last_vmain & 0x03 {
            0 => 1,
            1 => 32,
            _ => 128,
        };
        let inc_on = if (self.vram_last_vmain & 0x80) != 0 {
            "HIGH"
        } else {
            "LOW"
        };
        parts.push(format!(
            "VMAIN fg={} inc={} inc_on_{}",
            fg_mode, inc, inc_on
        ));
        parts.push(format!(
            "writes: low={} high={}",
            self.vram_write_low_count, self.vram_write_high_count
        ));
        // Buckets 0..7 => 0x0000..0x7000 (word address)
        let mut bucket_strs: Vec<String> = Vec::new();
        for i in 0..8 {
            let base = i * 0x1000;
            let cnt = self.vram_write_buckets[i];
            if cnt > 0 {
                bucket_strs.push(format!("{:04X}-{:04X}:{}", base, base + 0x0FFF, cnt));
            }
        }
        if bucket_strs.is_empty() {
            parts.push("buckets: none".to_string());
        } else {
            parts.push(format!("buckets: {}", bucket_strs.join(", ")));
        }

        // Reject counters and concise gap blocks (timing tune)
        parts.push(format!(
            "rejects: vram={} cgram={} oam={}",
            self.vram_rejects, self.cgram_rejects, self.oam_rejects
        ));
        parts.push(format!(
            "gaps: vram={} cgram={} oam={}",
            self.vram_gap_blocks, self.cgram_gap_blocks, self.oam_gap_blocks
        ));

        // Reset counters
        self.vram_write_buckets = [0; 8];
        self.vram_write_low_count = 0;
        self.vram_write_high_count = 0;
        self.vram_rejects = 0;
        self.cgram_rejects = 0;
        self.oam_rejects = 0;
        self.vram_gap_blocks = 0;
        self.cgram_gap_blocks = 0;
        self.oam_gap_blocks = 0;

        parts.join(" | ")
    }

    // Summarize per-frame render metrics and reset counters
    pub fn take_render_metrics_summary(&mut self) -> String {
        if !crate::debug_flags::render_metrics() {
            return "RENDER_METRICS: off".to_string();
        }
        let s = format!(
            "RENDER_METRICS: clip_in={} clip_out={} add={} add/2={} sub={} sub/2={} masked_bg={} masked_obj={} obj_add={} obj_add/2={} obj_sub={} obj_sub/2={} obj_clip_in={} obj_clip_out={} win_xor={} win_xnor={} math_blocked={} math_blocked_obj={} math_blocked_bd={} m7_wrap={} m7_clip={} m7_fill={} m7_bg1={} m7_bg2={} m7_edge={}",
            self.dbg_clip_inside,
            self.dbg_clip_outside,
            self.dbg_math_add,
            self.dbg_math_add_half,
            self.dbg_math_sub,
            self.dbg_math_sub_half,
            self.dbg_masked_bg,
            self.dbg_masked_obj,
            self.dbg_math_obj_add,
            self.dbg_math_obj_add_half,
            self.dbg_math_obj_sub,
            self.dbg_math_obj_sub_half,
            self.dbg_clip_obj_inside,
            self.dbg_clip_obj_outside,
            self.dbg_win_xor_applied,
            self.dbg_win_xnor_applied,
            self.dbg_math_blocked,
            self.dbg_math_blocked_obj,
            self.dbg_math_blocked_backdrop,
            self.dbg_m7_wrap,
            self.dbg_m7_clip,
            self.dbg_m7_fill,
            self.dbg_m7_bg1,
            self.dbg_m7_bg2,
            self.dbg_m7_edge
        );
        self.dbg_clip_inside = 0;
        self.dbg_clip_outside = 0;
        self.dbg_math_add = 0;
        self.dbg_math_add_half = 0;
        self.dbg_math_sub = 0;
        self.dbg_math_sub_half = 0;
        self.dbg_masked_bg = 0;
        self.dbg_masked_obj = 0;
        self.dbg_math_obj_add = 0;
        self.dbg_math_obj_add_half = 0;
        self.dbg_math_obj_sub = 0;
        self.dbg_math_obj_sub_half = 0;
        self.dbg_clip_obj_inside = 0;
        self.dbg_clip_obj_outside = 0;
        self.dbg_win_xor_applied = 0;
        self.dbg_win_xnor_applied = 0;
        self.dbg_math_blocked = 0;
        self.dbg_math_blocked_obj = 0;
        self.dbg_math_blocked_backdrop = 0;
        self.dbg_m7_wrap = 0;
        self.dbg_m7_clip = 0;
        self.dbg_m7_fill = 0;
        self.dbg_m7_bg1 = 0;
        self.dbg_m7_bg2 = 0;
        self.dbg_m7_edge = 0;
        s
    }

    // apply_color_math moved to rendering.rs

    // is_color_math_enabled moved to rendering.rs

    // fixed_color_to_rgb moved to rendering.rs

    // blend_colors moved to rendering.rs

    // apply_mosaic moved to rendering.rs

    // is_mosaic_enabled moved to rendering.rs

    // mode7_transform moved to rendering.rs

    // mode7_world_xy_int moved to rendering.rs

    // render_main_screen_pixel_with_layer_internal moved to rendering.rs

    // render_main_screen_pixel_with_layer moved to rendering.rs

    // render_main_screen_pixel_with_layer_cached moved to rendering.rs

    // get_main_bg_pixel moved to rendering.rs

    // render_sub_screen_pixel moved to rendering.rs

    // render_sub_screen_pixel_with_layer_internal moved to rendering.rs

    // render_sub_screen_pixel_with_layer moved to rendering.rs

    // render_sub_screen_pixel_with_layer_cached moved to rendering.rs

    // get_sub_bg_pixel moved to rendering.rs

    // apply_color_math_screens moved to rendering.rs

    pub fn nmi_pending(&self) -> bool {
        // CPU側へ通知するNMIリクエストは「ラッチ」(edge)で管理する。
        // nmi_flag は $4210(RDNMI) のbit7用で、読み出しでクリアされる。
        // NOTE: $4200 bit7 controls whether the edge is latched,
        // but once latched it should remain pending even if NMI is later disabled.
        self.nmi_latched
    }

    // Expose minimal NMI latch control for $4200 edge cases
    pub fn is_nmi_latched(&self) -> bool {
        self.nmi_latched
    }
    pub fn latch_nmi_now(&mut self) {
        self.nmi_latched = true;
    }

    pub fn get_scanline(&self) -> u16 {
        self.scanline
    }

    pub fn get_frame(&self) -> u64 {
        self.frame
    }

    // Accessors for HVB flags
    pub fn is_vblank(&self) -> bool {
        self.v_blank
    }

    pub fn is_hblank(&self) -> bool {
        self.h_blank
    }

    pub fn get_cycle(&self) -> u16 {
        self.cycle
    }

    pub(crate) fn dots_this_scanline(&self, scanline: u16) -> u16 {
        self.dots_per_scanline(scanline)
    }

    pub(crate) fn remaining_master_cycles_in_frame(&self) -> u64 {
        const MASTER_CYCLES_PER_DOT: u64 = 4;
        self.remaining_dots_in_frame() as u64 * MASTER_CYCLES_PER_DOT
    }

    // --- Write context control (called by Bus before/after DMA bursts) ---
    #[inline]
    pub fn begin_mdma_context(&mut self) {
        self.write_ctx = 1;
    }
    #[inline]
    pub fn end_mdma_context(&mut self) {
        self.write_ctx = 0;
        self.debug_dma_channel = None;
    }
    #[inline]
    pub fn begin_hdma_context(&mut self) {
        self.write_ctx = 2;
    }
    #[inline]
    pub fn end_hdma_context(&mut self) {
        self.write_ctx = 0;
        self.debug_dma_channel = None;
    }

    // Debug helper: mark which DMA channel is currently active
    #[inline]
    pub fn set_debug_dma_channel(&mut self, ch: Option<u8>) {
        self.debug_dma_channel = ch;
    }

    #[inline]
    pub fn arm_burnin_vram_trace(&mut self) {
        self.burnin_vram_trace_armed = true;
        self.burnin_vram_trace_cnt_2118 = 0;
        self.burnin_vram_trace_cnt_2119 = 0;
    }

    // Mark HBlank head guard window for HDMA operations
    pub fn on_hblank_start_guard(&mut self) {
        let hb = self.first_hblank_dot();
        const HDMA_HEAD_GUARD: u16 = 6;
        self.hdma_head_busy_until = hb.saturating_add(HDMA_HEAD_GUARD);
    }

    #[allow(dead_code)]
    pub fn clear_nmi(&mut self) {
        // NMIラッチだけを解除し、RDNMIフラグ（nmi_flag）は保持する。
        // 実機では $4210 読み出しでクリアされるため、CPU側のポーリングに委ねる。
        self.nmi_latched = false;
    }

    // Lightweight usage stats (counts non-zero bytes)
    pub fn vram_usage(&self) -> usize {
        self.vram.iter().filter(|&&b| b != 0).count()
    }

    /// Analyze VRAM content distribution
    pub fn analyze_vram_content(&self) -> (usize, usize, Vec<(usize, u8)>) {
        let mut nonzero_count = 0;
        let mut unique_values = std::collections::HashSet::new();
        let mut samples = Vec::new();

        for (i, &byte) in self.vram.iter().enumerate() {
            if byte != 0 {
                nonzero_count += 1;
                unique_values.insert(byte);
                if samples.len() < 20 {
                    samples.push((i, byte));
                }
            }
        }

        (nonzero_count, unique_values.len(), samples)
    }

    /// Analyze specific VRAM region (word address)
    pub fn analyze_vram_region(&self, word_addr: u16, word_count: usize) -> (usize, Vec<u8>) {
        // Apply VRAM mirroring: addresses 0x8000-0xFFFF mirror to 0x0000-0x7FFF
        let mirrored_addr = word_addr & 0x7FFF;
        let byte_start = (mirrored_addr as usize) * 2;
        let byte_end = (byte_start + word_count * 2).min(self.vram.len());
        let mut nonzero = 0;
        let mut samples = Vec::new();

        for i in byte_start..byte_end {
            if self.vram[i] != 0 {
                nonzero += 1;
                if samples.len() < 16 {
                    samples.push(self.vram[i]);
                }
            }
        }

        (nonzero, samples)
    }

    /// Get VRAM distribution by 4KB blocks
    pub fn get_vram_distribution(&self) -> Vec<(usize, usize)> {
        let block_size = 4096; // 4KB blocks
        let mut distribution = Vec::new();

        for block in 0..(self.vram.len() / block_size) {
            let start = block * block_size;
            let end = (start + block_size).min(self.vram.len());
            let nonzero = self.vram[start..end].iter().filter(|&&b| b != 0).count();
            if nonzero > 0 {
                distribution.push((block * block_size / 2, nonzero)); // word address
            }
        }

        distribution
    }

    pub fn cgram_usage(&self) -> usize {
        self.cgram.iter().filter(|&&b| b != 0).count()
    }

    /// Count non-zero color entries in CGRAM (each color is 2 bytes)
    #[allow(dead_code)]
    pub fn count_nonzero_colors(&self) -> usize {
        self.cgram
            .chunks_exact(2)
            .filter(|chunk| chunk[0] != 0 || chunk[1] != 0)
            .count()
    }

    /// Get BG configuration for debugging
    pub fn get_bg_config(&self, bg_num: u8) -> (u16, u16, bool, u8) {
        let index = (bg_num.saturating_sub(1)) as usize;
        if index >= 4 {
            return (0, 0, false, 0);
        }
        let tile_base = match bg_num {
            1 => self.bg1_tile_base,
            2 => self.bg2_tile_base,
            3 => self.bg3_tile_base,
            4 => self.bg4_tile_base,
            _ => 0,
        };
        let tilemap_base = match bg_num {
            1 => self.bg1_tilemap_base,
            2 => self.bg2_tilemap_base,
            3 => self.bg3_tilemap_base,
            4 => self.bg4_tilemap_base,
            _ => 0,
        };
        (
            tile_base,
            tilemap_base,
            self.bg_tile_16[index],
            self.bg_screen_size[index],
        )
    }

    pub fn get_setini(&self) -> u8 {
        self.setini
    }

    // write_cgram_color moved to palette.rs

    /// Write tilemap entry directly to VRAM (bypassing timing checks)
    #[allow(dead_code)]
    pub fn write_vram_word(&mut self, word_addr: u16, low_byte: u8, high_byte: u8) {
        // VRAM is 32KB words; wrap addresses the way hardware mirrors the 15-bit address.
        let addr = (word_addr as usize) & 0x7FFF; // 15-bit
        let byte_addr = addr * 2;
        if byte_addr + 1 < self.vram.len() {
            self.vram[byte_addr] = low_byte;
            self.vram[byte_addr + 1] = high_byte;
            self.bg_cache_dirty = true;
        }
    }

    pub fn oam_usage(&self) -> usize {
        self.oam.iter().filter(|&&b| b != 0).count()
    }
}
