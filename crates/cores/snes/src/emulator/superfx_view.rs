use super::{Emulator, SCREEN_HEIGHT, SCREEN_WIDTH};

impl Emulator {
    pub(super) fn starfox_gui_autocontrast_enabled(&self) -> bool {
        self.rom_title.to_ascii_uppercase().contains("STAR FOX")
            && std::env::var("STARFOX_GUI_AUTOCONTRAST")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
    }

    pub(super) fn populate_starfox_autocontrast_framebuffer(&mut self) -> bool {
        if !self.starfox_gui_autocontrast_enabled() {
            return false;
        }
        let source = self.bus.get_ppu().get_framebuffer();
        if source.is_empty() {
            return false;
        }

        // The top border in the current Star Fox output is often saturated noise.
        // Build the contrast curve from the central visible region and ignore near-black
        // backdrop pixels so the logo area controls exposure.
        let mut histogram = [0u32; 256];
        let mut sample_count = 0u32;
        for y in 8..SCREEN_HEIGHT.min(192) {
            for x in 16..SCREEN_WIDTH.saturating_sub(16) {
                let pixel = source[y * SCREEN_WIDTH + x];
                let r = ((pixel >> 16) & 0xFF) as u16;
                let g = ((pixel >> 8) & 0xFF) as u16;
                let b = (pixel & 0xFF) as u16;
                let luma = ((r * 77 + g * 150 + b * 29) >> 8) as u8;
                if luma <= 3 {
                    continue;
                }
                histogram[luma as usize] += 1;
                sample_count += 1;
            }
        }
        if sample_count == 0 {
            self.frame_buffer
                .resize(SCREEN_WIDTH * SCREEN_HEIGHT, 0xFF000000);
            let copy_len = self.frame_buffer.len().min(source.len());
            self.frame_buffer[..copy_len].copy_from_slice(&source[..copy_len]);
            if copy_len < self.frame_buffer.len() {
                self.frame_buffer[copy_len..].fill(0xFF000000);
            }
            return true;
        }

        let low_rank = sample_count / 20;
        let high_rank = sample_count.saturating_sub((sample_count / 50).max(1));
        let mut acc = 0u32;
        let mut low = 0u8;
        let mut high = 255u8;
        for (idx, &count) in histogram.iter().enumerate() {
            acc += count;
            if acc >= low_rank {
                low = idx as u8;
                break;
            }
        }
        acc = 0;
        for (idx, &count) in histogram.iter().enumerate() {
            acc += count;
            if acc >= high_rank {
                high = idx as u8;
                break;
            }
        }
        if high <= low {
            self.frame_buffer.copy_from_slice(source);
            return true;
        }

        let span = u32::from(high - low).max(1);
        self.frame_buffer.resize(source.len(), 0xFF000000);
        for (dst, &pixel) in self.frame_buffer.iter_mut().zip(source.iter()) {
            let r = ((pixel >> 16) & 0xFF) as u32;
            let g = ((pixel >> 8) & 0xFF) as u32;
            let b = (pixel & 0xFF) as u32;
            let luma = ((r * 77 + g * 150 + b * 29) >> 8).min(255);
            let stretched = luma.saturating_sub(u32::from(low)).saturating_mul(255) / span;
            let gain = if luma == 0 {
                0.0
            } else {
                (stretched as f32 / luma as f32).clamp(0.0, 6.0)
            };
            let boost = 1.15f32;
            let rr = ((r as f32 * gain * boost).round() as u32).min(255);
            let gg = ((g as f32 * gain * boost).round() as u32).min(255);
            let bb = ((b as f32 * gain * boost).round() as u32).min(255);
            *dst = 0xFF000000 | (rr << 16) | (gg << 8) | bb;
        }
        true
    }

    pub(super) fn starfox_gui_superfx_fallback_enabled(&self) -> bool {
        self.rom_title.to_ascii_uppercase().contains("STAR FOX")
            && std::env::var("STARFOX_GUI_FALLBACK")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
    }

    pub(super) fn starfox_gui_superfx_fallback_force_enabled(&self) -> bool {
        self.rom_title.to_ascii_uppercase().contains("STAR FOX")
            && std::env::var("STARFOX_GUI_DIRECT_ONLY")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
    }

    pub(super) fn superfx_gui_direct_x_offset(
        buffer: &[u8],
        height: usize,
        bpp: u8,
        mode: u8,
        frame: u64,
    ) -> i32 {
        std::env::var("SUPERFX_DIRECT_X_OFFSET")
            .ok()
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or_else(|| {
                if height == 128 && bpp == 2 {
                    0
                } else {
                    crate::ppu::Ppu::default_superfx_direct_x_offset(
                        buffer,
                        height as u16,
                        bpp,
                        mode,
                        frame,
                    )
                }
            })
    }

    pub(super) fn superfx_gui_direct_y_offset(
        &self,
        buffer: &[u8],
        height: usize,
        bpp: u8,
        mode: u8,
        frame: u64,
    ) -> i32 {
        std::env::var("SUPERFX_DIRECT_Y_OFFSET")
            .ok()
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or_else(|| {
                if let Some(offset) = crate::ppu::Ppu::forced_blank_superfx_direct_y_offset(
                    height as u16,
                    bpp,
                    mode,
                    self.bus.get_ppu().is_forced_blank(),
                ) {
                    return offset;
                }
                crate::ppu::Ppu::default_superfx_direct_y_offset(
                    buffer,
                    height as u16,
                    bpp,
                    mode,
                    frame,
                )
            })
    }

    pub(super) fn superfx_gui_direct_swap_xy() -> bool {
        std::env::var("SUPERFX_DIRECT_SWAP_XY")
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    }

    pub(super) fn superfx_gui_direct_row_major() -> bool {
        std::env::var("SUPERFX_DIRECT_ROW_MAJOR")
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    }

    pub(super) fn superfx_gui_direct_pixel_addr(
        x: usize,
        y: usize,
        height: usize,
        bpp: usize,
        _mode: usize,
    ) -> Option<(usize, usize, usize)> {
        if x >= 256 || y >= height {
            return None;
        }

        let row_in_tile = y & 7;
        let bit = 7 - (x & 7);
        let bytes_per_tile = match bpp {
            2 => 16,
            4 => 32,
            8 => 64,
            _ => return None,
        };

        if Self::superfx_gui_direct_row_major() {
            let tile_base = ((y >> 3) * 32 + (x >> 3)) * bytes_per_tile;
            return Some((tile_base, row_in_tile, bit));
        }

        let cn = match height {
            128 => ((x & 0xF8) << 1) + ((y & 0xF8) >> 3),
            160 => ((x & 0xF8) << 1) + ((x & 0xF8) >> 1) + ((y & 0xF8) >> 3),
            192 => ((x & 0xF8) << 1) + (x & 0xF8) + ((y & 0xF8) >> 3),
            256 => ((y & 0x80) << 2) + ((x & 0x80) << 1) + ((y & 0x78) << 1) + ((x & 0x78) >> 3),
            _ => return None,
        };
        Some((cn * bytes_per_tile, row_in_tile, bit))
    }

    pub(super) fn superfx_gui_sample_pixel(
        buffer: &[u8],
        x: usize,
        y: usize,
        height: usize,
        bpp: usize,
        mode: usize,
    ) -> u8 {
        let Some((tile_base, row_in_tile, bit)) =
            Self::superfx_gui_direct_pixel_addr(x, y, height, bpp, mode)
        else {
            return 0;
        };

        match bpp {
            2 => {
                let row = tile_base + row_in_tile * 2;
                if row + 1 >= buffer.len() {
                    return 0;
                }
                ((buffer[row] >> bit) & 1) | (((buffer[row + 1] >> bit) & 1) << 1)
            }
            4 => {
                let row01 = tile_base + row_in_tile * 2;
                let row23 = tile_base + 16 + row_in_tile * 2;
                if row23 + 1 >= buffer.len() {
                    return 0;
                }
                ((buffer[row01] >> bit) & 1)
                    | (((buffer[row01 + 1] >> bit) & 1) << 1)
                    | (((buffer[row23] >> bit) & 1) << 2)
                    | (((buffer[row23 + 1] >> bit) & 1) << 3)
            }
            8 => {
                let row01 = tile_base + row_in_tile * 2;
                let row23 = tile_base + 16 + row_in_tile * 2;
                let row45 = tile_base + 32 + row_in_tile * 2;
                let row67 = tile_base + 48 + row_in_tile * 2;
                if row67 + 1 >= buffer.len() {
                    return 0;
                }
                ((buffer[row01] >> bit) & 1)
                    | (((buffer[row01 + 1] >> bit) & 1) << 1)
                    | (((buffer[row23] >> bit) & 1) << 2)
                    | (((buffer[row23 + 1] >> bit) & 1) << 3)
                    | (((buffer[row45] >> bit) & 1) << 4)
                    | (((buffer[row45 + 1] >> bit) & 1) << 5)
                    | (((buffer[row67] >> bit) & 1) << 6)
                    | (((buffer[row67 + 1] >> bit) & 1) << 7)
            }
            _ => 0,
        }
    }

    pub(super) fn superfx_gui_palette_bank() -> u8 {
        std::env::var("SUPERFX_DIRECT_PALETTE_BANK")
            .ok()
            .and_then(|v| v.trim().parse::<u8>().ok())
            .filter(|&v| v < 8)
            .unwrap_or(0)
    }

    pub(super) fn superfx_gui_cgram_color(&self, index: u8, bpp: u8) -> Option<u32> {
        let cgram = self.bus.get_ppu().get_cgram();
        let bank = Self::superfx_gui_palette_bank();
        let palette_index = match bpp {
            2 | 4 => bank.saturating_mul(16).saturating_add(index),
            8 => index,
            _ => return None,
        } as usize;
        let lo = *cgram.get(palette_index * 2)?;
        let hi = *cgram.get(palette_index * 2 + 1)? & 0x7F;
        let color = u16::from(lo) | (u16::from(hi) << 8);
        let r5 = (color & 0x1F) as u32;
        let g5 = ((color >> 5) & 0x1F) as u32;
        let b5 = ((color >> 10) & 0x1F) as u32;
        let r8 = (r5 << 3) | (r5 >> 2);
        let g8 = (g5 << 3) | (g5 >> 2);
        let b8 = (b5 << 3) | (b5 >> 2);
        Some(0xFF000000 | (r8 << 16) | (g8 << 8) | b8)
    }

    pub(super) fn superfx_gui_color(&self, index: u8, bpp: u8) -> u32 {
        if index == 0 {
            return 0xFF000000;
        }
        if let Some(color) = self.superfx_gui_cgram_color(index, bpp) {
            if (color & 0x00FF_FFFF) != 0 {
                return color;
            }
        }

        let max = ((1u16 << bpp.min(8)) - 1).max(1) as u32;
        let intensity = ((u32::from(index) * 255) / max) as u8;
        0xFF000000
            | (u32::from(intensity) << 16)
            | (u32::from(intensity) << 8)
            | u32::from(intensity)
    }

    pub(super) fn populate_superfx_gui_fallback_framebuffer(&mut self) -> bool {
        if !self.starfox_gui_superfx_fallback_enabled() {
            return false;
        }

        let ppu_all_black = {
            let fb = self.bus.get_ppu().get_framebuffer();
            fb.iter().all(|&px| px == 0xFF000000 || px == 0x00000000)
        };
        if !ppu_all_black && !self.starfox_gui_superfx_fallback_force_enabled() {
            return false;
        }

        let selected = if Self::superfx_direct_use_tile_snapshot() {
            self.bus
                .superfx_tile_buffer_snapshot()
                .or_else(|| self.bus.superfx_screen_buffer_display_snapshot())
        } else if Self::superfx_direct_use_live_buffer() {
            self.bus
                .superfx_screen_buffer_live()
                .or_else(|| self.bus.superfx_screen_buffer_display_snapshot())
        } else {
            self.bus.superfx_screen_buffer_display_snapshot()
        };
        let (buffer, height, bpp, mode) = selected.unwrap_or_default();
        let height = height as usize;
        let bpp_usize = bpp as usize;
        let mode_usize = (mode & 0x03) as usize;
        if buffer.is_empty() || !matches!(bpp_usize, 2 | 4 | 8) || height == 0 {
            return false;
        }

        let x_offset =
            Self::superfx_gui_direct_x_offset(&buffer, height, bpp, mode, self.frame_count);
        let y_offset =
            self.superfx_gui_direct_y_offset(&buffer, height, bpp, mode, self.frame_count);
        self.frame_buffer.fill(0xFF000000);
        let mut any_visible = false;
        for y in 0..SCREEN_HEIGHT {
            let sfx_y = y as i32 + y_offset;
            if !(0..height as i32).contains(&sfx_y) {
                continue;
            }
            for x in 0..SCREEN_WIDTH {
                let sfx_x = x as i32 + x_offset;
                if !(0..256).contains(&sfx_x) {
                    continue;
                }
                let mut sx = sfx_x as usize;
                let mut sy = sfx_y as usize;
                if Self::superfx_gui_direct_swap_xy() {
                    std::mem::swap(&mut sx, &mut sy);
                }
                let color_index =
                    Self::superfx_gui_sample_pixel(&buffer, sx, sy, height, bpp_usize, mode_usize);
                if color_index == 0 {
                    continue;
                }
                any_visible = true;
                self.frame_buffer[y * SCREEN_WIDTH + x] = self.superfx_gui_color(color_index, bpp);
            }
        }

        any_visible
    }

    pub(super) fn superfx_direct_force_height() -> Option<u16> {
        std::env::var("SUPERFX_DIRECT_FORCE_HEIGHT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
    }

    pub(super) fn superfx_direct_force_bpp() -> Option<u8> {
        std::env::var("SUPERFX_DIRECT_FORCE_BPP")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
    }

    pub(super) fn superfx_direct_force_mode() -> Option<u8> {
        std::env::var("SUPERFX_DIRECT_FORCE_MODE")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
    }

    pub(super) fn superfx_direct_use_live_buffer() -> bool {
        std::env::var("SUPERFX_DIRECT_USE_LIVE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub(super) fn superfx_direct_use_tile_snapshot() -> bool {
        std::env::var("SUPERFX_DIRECT_USE_TILE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub(super) fn superfx_tile_use_stop_snapshot() -> bool {
        std::env::var("SUPERFX_TILE_USE_STOP")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub(super) fn superfx_bypass_bg1_window_enabled() -> bool {
        std::env::var("SUPERFX_BYPASS_BG1_WINDOW")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    pub(super) fn superfx_authoritative_bg1_source_enabled(&self) -> bool {
        std::env::var("SUPERFX_AUTHORITATIVE_BG1_SOURCE")
            .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
            .unwrap_or_else(|_| self.rom_title.to_ascii_uppercase().contains("STAR FOX"))
    }

    pub(super) fn sync_superfx_direct_buffer(&mut self) {
        #[cfg(not(test))]
        if !self.bus.is_superfx_active() {
            let ppu = self.bus.get_ppu_mut();
            ppu.clear_superfx_tile_buffer();
            ppu.clear_superfx_direct_buffer();
            ppu.superfx_bypass_bg1_window = false;
            ppu.set_superfx_authoritative_bg1_source(false);
            return;
        }

        let tile_buffer = if Self::superfx_tile_use_stop_snapshot() {
            self.bus.superfx_screen_buffer_display_snapshot()
        } else {
            self.bus.superfx_tile_buffer_snapshot()
        };
        if let Some((buffer, _height, bpp, mode)) = tile_buffer {
            self.bus
                .get_ppu_mut()
                .set_superfx_tile_buffer(buffer, bpp, mode);
        } else {
            self.bus.get_ppu_mut().clear_superfx_tile_buffer();
        }

        let direct_buffer = if Self::superfx_direct_use_tile_snapshot() {
            self.bus
                .superfx_tile_buffer_snapshot()
                .or_else(|| self.bus.superfx_screen_buffer_display_snapshot())
        } else if Self::superfx_direct_use_live_buffer() && self.bus.is_superfx_active() {
            self.bus
                .superfx_screen_buffer_live()
                .or_else(|| self.bus.superfx_screen_buffer_display_snapshot())
        } else {
            self.bus.superfx_screen_buffer_display_snapshot()
        };
        self.bus.get_ppu_mut().superfx_bypass_bg1_window =
            Self::superfx_bypass_bg1_window_enabled();
        let authoritative_bg1_source = self.superfx_authoritative_bg1_source_enabled();
        self.bus
            .get_ppu_mut()
            .set_superfx_authoritative_bg1_source(authoritative_bg1_source);
        if let Some((buffer, mut height, mut bpp, mut mode)) = direct_buffer {
            if let Some(forced) = Self::superfx_direct_force_height() {
                height = forced;
            }
            if let Some(forced) = Self::superfx_direct_force_bpp() {
                bpp = forced;
            }
            if let Some(forced) = Self::superfx_direct_force_mode() {
                mode = forced;
            }
            self.bus
                .get_ppu_mut()
                .set_superfx_direct_buffer(buffer, height, bpp, mode);
        } else {
            self.bus.get_ppu_mut().clear_superfx_direct_buffer();
        }
    }
}
