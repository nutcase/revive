#![allow(static_mut_refs)]

mod read;
mod write;

use super::Ppu;

impl Ppu {
    #[inline]
    pub(crate) fn sign_extend13(value: u16) -> i16 {
        // Mode 7 center/offset registers are 13-bit signed.
        let v = value & 0x1FFF;
        if (v & 0x1000) != 0 {
            (v | 0xE000) as i16
        } else {
            v as i16
        }
    }

    #[inline]
    pub(crate) fn mode7_combine(&mut self, value: u8) -> u16 {
        // SNESdev: Mode 7 registers share a single latch. Each write forms:
        //   reg = (value<<8) | mode7_latch; mode7_latch = value
        let combined = ((value as u16) << 8) | (self.mode7_latch as u16);
        self.mode7_latch = value;
        combined
    }

    #[inline]
    pub(crate) fn write_m7hofs(&mut self, value: u8) {
        // $210D also maps to M7HOFS and uses the Mode 7 latch (not BG scroll latch).
        self.mode7_hofs = Self::sign_extend13(self.mode7_combine(value));
    }

    #[inline]
    pub(crate) fn write_m7vofs(&mut self, value: u8) {
        // $210E also maps to M7VOFS and uses the Mode 7 latch (not BG scroll latch).
        self.mode7_vofs = Self::sign_extend13(self.mode7_combine(value));
    }

    #[inline]
    pub(crate) fn write_bghofs(&mut self, bg_num: usize, value: u8) {
        // BGnHOFS ($210D/$210F/$2111/$2113)
        // SNESdev wiki: BGnHOFS = (value<<8) | (bgofs_latch & ~7) | (bghofs_latch & 7)
        let lo = (self.bgofs_latch & !0x07) | (self.bghofs_latch & 0x07);
        let ofs = (((value as u16) << 8) | (lo as u16)) & 0x03FF;
        match bg_num {
            0 => self.bg1_hscroll = ofs,
            1 => self.bg2_hscroll = ofs,
            2 => self.bg3_hscroll = ofs,
            _ => self.bg4_hscroll = ofs,
        }
        self.bgofs_latch = value;
        self.bghofs_latch = value;

        if crate::debug_flags::trace_ppu_scroll() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: AtomicU32 = AtomicU32::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 512 {
                let who = match self.write_ctx {
                    2 => "HDMA",
                    1 => "MDMA",
                    _ => "CPU",
                };
                let (h, v) = match bg_num {
                    0 => (self.bg1_hscroll, self.bg1_vscroll),
                    1 => (self.bg2_hscroll, self.bg2_vscroll),
                    2 => (self.bg3_hscroll, self.bg3_vscroll),
                    _ => (self.bg4_hscroll, self.bg4_vscroll),
                };
                println!(
                    "[TRACE_PPU_SCROLL] {} frame={} sl={} cyc={} BG{}HOFS write=0x{:02X} -> h={} v={} (bgofs_latch=0x{:02X} bghofs_latch=0x{:02X})",
                    who,
                    self.frame,
                    self.scanline,
                    self.cycle,
                    bg_num + 1,
                    value,
                    h,
                    v,
                    self.bgofs_latch,
                    self.bghofs_latch
                );
            }
        }
    }

    #[inline]
    pub(crate) fn write_bgvofs(&mut self, bg_num: usize, value: u8) {
        // BGnVOFS ($210E/$2110/$2112/$2114)
        // SNESdev wiki: BGnVOFS = (value<<8) | bgofs_latch
        let ofs = (((value as u16) << 8) | (self.bgofs_latch as u16)) & 0x03FF;
        match bg_num {
            0 => self.bg1_vscroll = ofs,
            1 => self.bg2_vscroll = ofs,
            2 => self.bg3_vscroll = ofs,
            _ => self.bg4_vscroll = ofs,
        }
        self.bgofs_latch = value;

        if crate::debug_flags::trace_ppu_scroll() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CNT: AtomicU32 = AtomicU32::new(0);
            let n = CNT.fetch_add(1, Ordering::Relaxed);
            if n < 512 {
                let who = match self.write_ctx {
                    2 => "HDMA",
                    1 => "MDMA",
                    _ => "CPU",
                };
                let (h, v) = match bg_num {
                    0 => (self.bg1_hscroll, self.bg1_vscroll),
                    1 => (self.bg2_hscroll, self.bg2_vscroll),
                    2 => (self.bg3_hscroll, self.bg3_vscroll),
                    _ => (self.bg4_hscroll, self.bg4_vscroll),
                };
                println!(
                    "[TRACE_PPU_SCROLL] {} frame={} sl={} cyc={} BG{}VOFS write=0x{:02X} -> h={} v={} (bgofs_latch=0x{:02X})",
                    who,
                    self.frame,
                    self.scanline,
                    self.cycle,
                    bg_num + 1,
                    value,
                    h,
                    v,
                    self.bgofs_latch
                );
            }
        }
    }

    // evaluate_sprites_for_scanline moved to sprites.rs

    // get_sprite_status moved to sprites.rs

    // get_cached_sprites_for_scanline moved to sprites.rs

    // parse_sprites moved to sprites.rs

    // get_sprite_pixel_size moved to sprites.rs

    // render_sprite_tile moved to sprites.rs

    // calculate_sprite_tile_number moved to sprites.rs

    // get_sprite_bpp moved to sprites.rs

    // sprite_name_select_gap_words moved to sprites.rs

    // sprite_tile_base_word_addr moved to sprites.rs

    // render_sprite_2bpp moved to sprites.rs

    // render_sprite_4bpp moved to sprites.rs

    // render_sprite_8bpp moved to sprites.rs

    // --- Palette methods moved to palette.rs ---

    // --- Window mask methods moved to window.rs ---

    // VRAM address remapping helper (VMAIN bits 3-2 "Full Graphic Mode")
    //
    // SNESdev wiki:
    // - mode 0: none
    // - mode 1: rotate low 8 bits by 3 (2bpp)  : aaaaaaaaYYYccccc -> aaaaaaaacccccYYY
    // - mode 2: rotate low 9 bits by 3 (4bpp)  : aaaaaaaYYYcccccP -> aaaaaaacccccPYYY
    // - mode 3: rotate low 10 bits by 3 (8bpp) : aaaaaaYYYcccccPP -> aaaaaacccccPPYYY
    pub(crate) fn vram_remap_word_addr(&self, addr: u16) -> u16 {
        let mode = (self.vram_mapping >> 2) & 0x03;
        if mode == 0 {
            return addr & 0x7FFF;
        }

        let rotate_bits = match mode {
            1 => 8u8,
            2 => 9u8,
            _ => 10u8,
        };
        let mask: u16 = (1u16 << rotate_bits) - 1;
        let low = addr & mask;
        let y = low >> (rotate_bits - 3);
        let rest = low & ((1u16 << (rotate_bits - 3)) - 1);
        let remapped = (addr & !mask) | (rest << 3) | y;
        remapped & 0x7FFF
    }

    #[inline]
    pub(crate) fn reload_vram_read_latch(&mut self) {
        // SNESdev wiki: VRAM reads via $2139/$213A are only valid during VBlank or forced blank.
        // Outside those periods, the latch is not updated (returns invalid/old data).
        if !self.can_read_vram_now() {
            return;
        }

        let masked = self.vram_remap_word_addr(self.vram_addr) as usize;
        let idx = masked.saturating_mul(2);
        if idx + 1 < self.vram.len() {
            self.vram_read_buf_lo = self.vram[idx];
            self.vram_read_buf_hi = self.vram[idx + 1];
        } else {
            self.vram_read_buf_lo = 0;
            self.vram_read_buf_hi = 0;
        }
    }

    /// Mode 7 乗算結果を更新（$2134-$2136）
    pub(crate) fn update_mode7_mul_result(&mut self) {
        // 実機では基本 16x8 符号付き積。デバッグで 16x16 や固定値にも切り替え可能。
        let prod = if let Some(forced) = crate::debug_flags::force_m7_product() {
            forced as i32
        } else {
            let a = self.mode7_matrix_a as i32;
            if crate::debug_flags::m7_mul_full16() {
                let b = self.mode7_matrix_b as i32;
                a * b
            } else {
                let b = self.mode7_mul_b as i32; // last 8-bit value written to M7B
                a * b
            }
        };
        self.mode7_mul_result = (prod as u32) & 0x00FF_FFFF;
    }
}
