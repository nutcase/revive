use crate::bus::{GbBus, INT_LCD_STAT, INT_VBLANK};

pub const GB_FRAME_CYCLES: u32 = 70_224;
pub const GB_LCD_WIDTH: u32 = 160;
pub const GB_LCD_HEIGHT: u32 = 144;

const CYCLES_PER_LINE: u32 = 456;
const VISIBLE_LINES: u8 = 144;
const TOTAL_LINES: u8 = 154;

const MODE2_CYCLES: u32 = 80;
const MODE3_CYCLES: u32 = 172;

const LCDC_ENABLE: u8 = 0x80;
const STAT_INT_HBLANK: u8 = 0x08;
const STAT_INT_VBLANK: u8 = 0x10;
const STAT_INT_OAM: u8 = 0x20;
const STAT_INT_LYC: u8 = 0x40;
const BG_WINDOW_ENABLE: u8 = 0x01;
const OBJ_ENABLE: u8 = 0x02;
const OBJ_SIZE_8X16: u8 = 0x04;
const BG_TILEMAP_SELECT: u8 = 0x08;
const TILE_DATA_UNSIGNED: u8 = 0x10;
const WINDOW_ENABLE: u8 = 0x20;
const WINDOW_TILEMAP_SELECT: u8 = 0x40;
const FRAMEBUFFER_SIZE: usize = (GB_LCD_WIDTH as usize) * (GB_LCD_HEIGHT as usize) * 4;

#[derive(Debug)]
pub struct GbPpu {
    line_cycles: u32,
    stat_irq_latched: bool,
    frame_rgba8888: [u8; FRAMEBUFFER_SIZE],
}

#[derive(Debug, Clone, Copy)]
struct SpriteCandidate {
    oam_index: usize,
    x: i32,
    y: i32,
    tile: u8,
    attrs: u8,
}

#[derive(Debug, Clone, Copy)]
struct BgPixel {
    color_index: u8,
    palette: u8,
    priority: bool,
}

impl Default for GbPpu {
    fn default() -> Self {
        Self {
            line_cycles: 0,
            stat_irq_latched: false,
            frame_rgba8888: [0; FRAMEBUFFER_SIZE],
        }
    }
}

impl GbPpu {
    pub fn frame_rgba8888(&self) -> &[u8] {
        &self.frame_rgba8888
    }

    pub fn reset(&mut self, bus: &mut GbBus) {
        self.line_cycles = 0;
        self.stat_irq_latched = false;
        self.clear_framebuffer_to_color([0xE0, 0xF8, 0xD0, 0xFF]);
        bus.set_ppu_ly(0);
        bus.set_ppu_mode(0);
        self.update_stat(bus);
    }

    pub fn step(&mut self, cycles: u32, bus: &mut GbBus) -> bool {
        if (bus.ppu_lcdc() & LCDC_ENABLE) == 0 {
            self.line_cycles = 0;
            self.clear_framebuffer_to_color([0xE0, 0xF8, 0xD0, 0xFF]);
            bus.set_ppu_ly(0);
            bus.set_ppu_mode(0);
            self.stat_irq_latched = false;
            self.update_stat(bus);
            return false;
        }

        let mut frame_ready = false;
        let mut remaining = cycles;

        while remaining > 0 {
            let prev_mode = bus.ppu_stat() & 0x03;
            let prev_ly = bus.ppu_ly();
            let step = (CYCLES_PER_LINE - self.line_cycles).min(remaining);
            self.line_cycles += step;
            remaining -= step;

            self.update_stat(bus);
            let mode = bus.ppu_stat() & 0x03;
            let ly = bus.ppu_ly();
            if prev_mode != 0 && mode == 0 && ly < VISIBLE_LINES && ly == prev_ly {
                bus.step_hblank_hdma();
            }

            if self.line_cycles >= CYCLES_PER_LINE {
                self.line_cycles -= CYCLES_PER_LINE;

                let current_ly = bus.ppu_ly();
                if current_ly < VISIBLE_LINES {
                    self.render_scanline(current_ly, bus);
                }
                let next_ly = if current_ly + 1 >= TOTAL_LINES {
                    frame_ready = true;
                    0
                } else {
                    current_ly + 1
                };
                bus.set_ppu_ly(next_ly);

                if next_ly == VISIBLE_LINES {
                    bus.request_interrupt(INT_VBLANK);
                }

                self.update_stat(bus);
            }
        }

        frame_ready
    }

    fn render_scanline(&mut self, ly: u8, bus: &GbBus) {
        let ly_usize = usize::from(ly);
        let lcdc = bus.ppu_lcdc();
        let cgb_mode = bus.cgb_mode();
        let bg_enabled = (lcdc & BG_WINDOW_ENABLE) != 0;
        let window_enabled = bg_enabled && (lcdc & WINDOW_ENABLE) != 0;
        let scx = bus.ppu_scx();
        let scy = bus.ppu_scy();
        let wx = bus.ppu_wx();
        let wy = bus.ppu_wy();
        let bg_palette = bus.ppu_bg_palette();
        let mut bg_color_indices = [0u8; GB_LCD_WIDTH as usize];
        let mut bg_priority = [false; GB_LCD_WIDTH as usize];
        let window_x_start = i32::from(wx) - 7;

        for x in 0..GB_LCD_WIDTH as usize {
            let x_i32 = x as i32;
            let use_window = window_enabled && ly >= wy && x_i32 >= window_x_start;
            let (map_base, px, py) = if use_window {
                let px = (x_i32 - window_x_start).clamp(0, 255) as u8;
                let py = ly.wrapping_sub(wy);
                let map_base = if (lcdc & WINDOW_TILEMAP_SELECT) != 0 {
                    0x9C00
                } else {
                    0x9800
                };
                (map_base, px, py)
            } else {
                let px = (x as u8).wrapping_add(scx);
                let py = ly.wrapping_add(scy);
                let map_base = if (lcdc & BG_TILEMAP_SELECT) != 0 {
                    0x9C00
                } else {
                    0x9800
                };
                (map_base, px, py)
            };

            let pixel = if bg_enabled {
                self.fetch_bg_window_pixel(bus, lcdc, map_base, px, py, cgb_mode)
            } else {
                BgPixel {
                    color_index: 0,
                    palette: 0,
                    priority: false,
                }
            };
            bg_color_indices[x] = pixel.color_index;
            bg_priority[x] = pixel.priority;
            let color = if cgb_mode {
                cgb_palette_color(bus, true, pixel.palette, pixel.color_index)
            } else {
                dmg_palette_color(bg_palette, pixel.color_index)
            };
            self.write_pixel_rgba(x, ly_usize, color);
        }

        if (lcdc & OBJ_ENABLE) != 0 {
            self.render_sprites_for_line(ly, bus, lcdc, cgb_mode, &bg_color_indices, &bg_priority);
        }
    }

    fn render_sprites_for_line(
        &mut self,
        ly: u8,
        bus: &GbBus,
        lcdc: u8,
        cgb_mode: bool,
        bg_color_indices: &[u8],
        bg_priority: &[bool],
    ) {
        let sprite_height = if (lcdc & OBJ_SIZE_8X16) != 0 { 16 } else { 8 };
        let mut visible = [None; 10];
        let mut visible_count = 0usize;

        for sprite_index in 0..40usize {
            let base = sprite_index * 4;
            let y = i32::from(bus.ppu_read_oam(base)).wrapping_sub(16);
            let x = i32::from(bus.ppu_read_oam(base + 1)).wrapping_sub(8);
            let tile = bus.ppu_read_oam(base + 2);
            let attrs = bus.ppu_read_oam(base + 3);
            let ly_i32 = i32::from(ly);
            if ly_i32 < y || ly_i32 >= y + sprite_height {
                continue;
            }

            if visible_count < visible.len() {
                visible[visible_count] = Some(SpriteCandidate {
                    oam_index: sprite_index,
                    x,
                    y,
                    tile,
                    attrs,
                });
                visible_count += 1;
            } else {
                break;
            }
        }

        let obp0 = bus.ppu_obj_palette0();
        let obp1 = bus.ppu_obj_palette1();
        let ly_usize = usize::from(ly);
        for x in 0..GB_LCD_WIDTH as usize {
            let mut winning: Option<(u8, u8)> = None;
            let mut winning_x = i32::MAX;
            let mut winning_oam = usize::MAX;
            let x_i32 = x as i32;

            for candidate in visible.iter().flatten() {
                if x_i32 < candidate.x || x_i32 >= candidate.x + 8 {
                    continue;
                }
                let Some(color_index) =
                    self.sprite_color_index_at(bus, candidate, sprite_height, x_i32, ly, cgb_mode)
                else {
                    continue;
                };
                if color_index == 0 {
                    continue;
                }

                let better = if cgb_mode {
                    candidate.oam_index < winning_oam
                } else {
                    candidate.x < winning_x
                        || (candidate.x == winning_x && candidate.oam_index < winning_oam)
                };
                if better {
                    winning = Some((color_index, candidate.attrs));
                    winning_x = candidate.x;
                    winning_oam = candidate.oam_index;
                }
            }

            let Some((color_index, attrs)) = winning else {
                continue;
            };
            let obj_behind_bg = (attrs & 0x80) != 0;
            if cgb_mode && bg_priority[x] && bg_color_indices[x] != 0 {
                continue;
            }
            if obj_behind_bg && bg_color_indices[x] != 0 {
                continue;
            }

            let color = if cgb_mode {
                cgb_palette_color(bus, false, attrs & 0x07, color_index)
            } else {
                let palette = if (attrs & 0x10) != 0 { obp1 } else { obp0 };
                dmg_palette_color(palette, color_index)
            };
            self.write_pixel_rgba(x, ly_usize, color);
        }
    }

    fn sprite_color_index_at(
        &self,
        bus: &GbBus,
        candidate: &SpriteCandidate,
        sprite_height: i32,
        x: i32,
        ly: u8,
        cgb_mode: bool,
    ) -> Option<u8> {
        let mut sprite_x = x - candidate.x;
        let mut sprite_y = i32::from(ly) - candidate.y;
        if !(0..8).contains(&sprite_x) || !(0..sprite_height).contains(&sprite_y) {
            return None;
        }

        if (candidate.attrs & 0x20) != 0 {
            sprite_x = 7 - sprite_x;
        }
        if (candidate.attrs & 0x40) != 0 {
            sprite_y = (sprite_height - 1) - sprite_y;
        }

        let mut tile = candidate.tile;
        if sprite_height == 16 {
            tile &= 0xFE;
            if sprite_y >= 8 {
                tile = tile.wrapping_add(1);
                sprite_y -= 8;
            }
        }

        let tile_addr = 0x8000 + u16::from(tile) * 16;
        let line_offset = (sprite_y as u16) * 2;
        let bank = if cgb_mode && (candidate.attrs & 0x08) != 0 {
            1
        } else {
            0
        };
        let low = bus.ppu_read_vram_bank(bank, tile_addr + line_offset);
        let high = bus.ppu_read_vram_bank(bank, tile_addr + line_offset + 1);
        let bit = 7 - (sprite_x as u8);
        let color = (((high >> bit) & 0x01) << 1) | ((low >> bit) & 0x01);
        Some(color)
    }

    fn fetch_bg_window_pixel(
        &self,
        bus: &GbBus,
        lcdc: u8,
        tile_map_base: u16,
        px: u8,
        py: u8,
        cgb_mode: bool,
    ) -> BgPixel {
        let tile_x = u16::from(px / 8);
        let tile_y = u16::from(py / 8);
        let tile_index_addr = tile_map_base + tile_y * 32 + tile_x;
        let tile_num = bus.ppu_read_vram_bank(0, tile_index_addr);
        let attrs = if cgb_mode {
            bus.ppu_read_vram_bank(1, tile_index_addr)
        } else {
            0
        };

        let mut tile_pixel_x = px % 8;
        let mut tile_pixel_y = py % 8;
        if cgb_mode {
            if (attrs & 0x20) != 0 {
                tile_pixel_x = 7 - tile_pixel_x;
            }
            if (attrs & 0x40) != 0 {
                tile_pixel_y = 7 - tile_pixel_y;
            }
        }
        let tile_line = u16::from(tile_pixel_y);

        let tile_data_addr = if (lcdc & TILE_DATA_UNSIGNED) != 0 {
            0x8000 + u16::from(tile_num) * 16
        } else {
            let signed = i16::from(tile_num as i8);
            (0x9000i32 + i32::from(signed) * 16) as u16
        };
        let bank = if cgb_mode && (attrs & 0x08) != 0 {
            1
        } else {
            0
        };
        let low = bus.ppu_read_vram_bank(bank, tile_data_addr + tile_line * 2);
        let high = bus.ppu_read_vram_bank(bank, tile_data_addr + tile_line * 2 + 1);
        let bit = 7 - tile_pixel_x;
        let color_index = (((high >> bit) & 0x01) << 1) | ((low >> bit) & 0x01);
        BgPixel {
            color_index,
            palette: attrs & 0x07,
            priority: (attrs & 0x80) != 0,
        }
    }

    fn write_pixel_rgba(&mut self, x: usize, y: usize, rgba: [u8; 4]) {
        let index = (y * GB_LCD_WIDTH as usize + x) * 4;
        self.frame_rgba8888[index] = rgba[0];
        self.frame_rgba8888[index + 1] = rgba[1];
        self.frame_rgba8888[index + 2] = rgba[2];
        self.frame_rgba8888[index + 3] = rgba[3];
    }

    fn clear_framebuffer_to_color(&mut self, rgba: [u8; 4]) {
        for chunk in self.frame_rgba8888.chunks_exact_mut(4) {
            chunk[0] = rgba[0];
            chunk[1] = rgba[1];
            chunk[2] = rgba[2];
            chunk[3] = rgba[3];
        }
    }

    fn update_stat(&mut self, bus: &mut GbBus) {
        if (bus.ppu_lcdc() & LCDC_ENABLE) == 0 {
            bus.set_ppu_mode(0);
            bus.set_ppu_lyc_flag(bus.ppu_ly() == bus.ppu_lyc());
            self.stat_irq_latched = false;
            return;
        }

        let ly = bus.ppu_ly();
        let mode = if ly >= VISIBLE_LINES {
            1
        } else if self.line_cycles < MODE2_CYCLES {
            2
        } else if self.line_cycles < MODE2_CYCLES + MODE3_CYCLES {
            3
        } else {
            0
        };
        bus.set_ppu_mode(mode);

        let lyc_equal = ly == bus.ppu_lyc();
        bus.set_ppu_lyc_flag(lyc_equal);

        let stat = bus.ppu_stat();
        let mode_irq_enabled = match mode {
            0 => (stat & STAT_INT_HBLANK) != 0,
            1 => (stat & STAT_INT_VBLANK) != 0,
            2 => (stat & STAT_INT_OAM) != 0,
            _ => false,
        };
        let lyc_irq_enabled = lyc_equal && (stat & STAT_INT_LYC) != 0;
        let stat_signal = mode_irq_enabled || lyc_irq_enabled;

        if stat_signal && !self.stat_irq_latched {
            bus.request_interrupt(INT_LCD_STAT);
        }
        self.stat_irq_latched = stat_signal;
    }
}

fn dmg_palette_color(palette: u8, color_index: u8) -> [u8; 4] {
    let shade = (palette >> (color_index.saturating_mul(2))) & 0x03;
    match shade {
        0 => [0xE0, 0xF8, 0xD0, 0xFF],
        1 => [0x88, 0xC0, 0x70, 0xFF],
        2 => [0x34, 0x68, 0x56, 0xFF],
        _ => [0x08, 0x18, 0x20, 0xFF],
    }
}

fn cgb_palette_color(bus: &GbBus, background: bool, palette: u8, color_index: u8) -> [u8; 4] {
    let base = (palette & 0x07) * 8 + (color_index & 0x03) * 2;
    let low = if background {
        bus.cgb_bg_palette_byte(base)
    } else {
        bus.cgb_obj_palette_byte(base)
    };
    let high = if background {
        bus.cgb_bg_palette_byte(base.wrapping_add(1))
    } else {
        bus.cgb_obj_palette_byte(base.wrapping_add(1))
    };
    let rgb15 = u16::from(low) | (u16::from(high) << 8);
    let red5 = (rgb15 & 0x1F) as u8;
    let green5 = ((rgb15 >> 5) & 0x1F) as u8;
    let blue5 = ((rgb15 >> 10) & 0x1F) as u8;
    [
        (red5 << 3) | (red5 >> 2),
        (green5 << 3) | (green5 >> 2),
        (blue5 << 3) | (blue5 >> 2),
        0xFF,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::INT_LCD_STAT;

    fn make_test_rom() -> Vec<u8> {
        let mut rom = vec![0; 0x8000];
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    #[test]
    fn enters_vblank_at_line_144_and_sets_interrupt() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.write8(0xFF40, 0x80);
        bus.write8(0xFFFF, INT_VBLANK);

        let mut ppu = GbPpu::default();
        ppu.reset(&mut bus);

        for _ in 0..144 {
            ppu.step(CYCLES_PER_LINE, &mut bus);
        }

        assert_eq!(bus.ppu_ly(), 144);
        assert_eq!(bus.ppu_stat() & 0x03, 1);
        assert_eq!(bus.pending_interrupts() & INT_VBLANK, INT_VBLANK);
    }

    #[test]
    fn lyc_stat_interrupt_is_generated() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.write8(0xFF40, 0x80);
        bus.write8(0xFF41, 0x40);
        bus.write8(0xFF45, 1);
        bus.write8(0xFFFF, INT_LCD_STAT);

        let mut ppu = GbPpu::default();
        ppu.reset(&mut bus);
        ppu.step(CYCLES_PER_LINE, &mut bus);

        assert_eq!(bus.ppu_ly(), 1);
        assert_eq!(bus.pending_interrupts() & INT_LCD_STAT, INT_LCD_STAT);
    }

    #[test]
    fn mode_oam_stat_interrupt_is_generated() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.write8(0xFF40, 0x80);
        bus.write8(0xFF41, 0x20);
        bus.write8(0xFFFF, INT_LCD_STAT);

        let mut ppu = GbPpu::default();
        ppu.reset(&mut bus);
        ppu.step(CYCLES_PER_LINE + 1, &mut bus);

        assert_eq!(bus.ppu_ly(), 1);
        assert_eq!(bus.pending_interrupts() & INT_LCD_STAT, INT_LCD_STAT);
    }

    #[test]
    fn lcd_disabled_forces_ly_zero() {
        let mut bus = GbBus::default();
        bus.load_cartridge(&make_test_rom())
            .expect("cartridge should load");
        bus.write8(0xFF40, 0x80);

        let mut ppu = GbPpu::default();
        ppu.reset(&mut bus);
        ppu.step(CYCLES_PER_LINE * 10, &mut bus);
        assert!(bus.ppu_ly() > 0);

        bus.write8(0xFF40, 0x00);
        ppu.step(4, &mut bus);
        assert_eq!(bus.ppu_ly(), 0);
        assert_eq!(bus.ppu_stat() & 0x03, 0);
    }
}
