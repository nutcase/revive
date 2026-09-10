use super::*;

pub(super) struct ObjScanline {
    pub(super) pixels: [(Option<ObjPixel>, Option<ObjPixel>); GBA_LCD_WIDTH as usize],
    pub(super) window: [bool; GBA_LCD_WIDTH as usize],
}
impl Default for ObjScanline {
    fn default() -> Self {
        Self {
            pixels: [(None, None); GBA_LCD_WIDTH as usize],
            window: [false; GBA_LCD_WIDTH as usize],
        }
    }
}
impl GbaEmulator {
    pub(super) fn render_objects_for_line(
        &self,
        y: u32,
        dispcnt: u16,
        mosaic: MosaicState,
        obj_attrs: &[ObjAttributes; 128],
        obj_affine: &[ObjAffineParams; 32],
    ) -> ObjScanline {
        let line = y;
        let mode = dispcnt & 0x7;
        let one_d_mapping = (dispcnt & (1 << 6)) != 0;
        let obj_char_base = if mode >= 3 {
            OBJ_CHAR_BASE_BITMAP
        } else {
            OBJ_CHAR_BASE_TEXT
        };
        let obj_mosaic_h = mosaic.obj_h as i32;
        let obj_mosaic_v = mosaic.obj_v as i32;

        let mut output = ObjScanline::default();
        for (obj_index, attrs) in obj_attrs.iter().enumerate() {
            let attr0 = attrs.attr0;
            let attr1 = attrs.attr1;
            let attr2 = attrs.attr2;

            let affine = (attr0 & (1 << 8)) != 0;
            if !affine && (attr0 & (1 << 9)) != 0 {
                continue;
            }

            let obj_mode = ((attr0 >> 10) & 0x3) as u8;
            if obj_mode == 3
                || (obj_mode == 2 && dispcnt & (1 << 15) == 0)
                || (obj_mode != 2 && dispcnt & (1 << 12) == 0)
            {
                continue;
            }
            let semi_transparent = obj_mode == 1;
            let mosaic_enabled = (attr0 & (1 << 12)) != 0;

            let shape = ((attr0 >> 14) & 0x3) as u8;
            let size = ((attr1 >> 14) & 0x3) as u8;
            let (width, height) = match obj_dimensions(shape, size) {
                Some(dim) => dim,
                None => continue,
            };

            let x_raw = (attr1 & 0x01FF) as i32;
            let y_raw = (attr0 & 0x00FF) as i32;
            let obj_x = if x_raw >= 240 { x_raw - 512 } else { x_raw };
            let obj_y = if y_raw >= 160 { y_raw - 256 } else { y_raw };

            let color_8bpp = (attr0 & (1 << 13)) != 0;
            let mut tile_index = (attr2 & 0x03FF) as u32;
            let tile_span = if color_8bpp { 2 } else { 1 };

            let double_size = affine && (attr0 & (1 << 9)) != 0;
            let draw_w = (width as i32) * if double_size { 2 } else { 1 };
            let draw_h = (height as i32) * if double_size { 2 } else { 1 };
            if (y as i32) < obj_y || (y as i32) >= obj_y + draw_h {
                continue;
            }
            // OAM metadata is decoded once per object, and offscreen pixels
            // are never visited. Preserve OAM order and the best two pixels.
            for x in obj_x.max(0)..(obj_x + draw_w).min(GBA_LCD_WIDTH as i32) {
                let x = x as u32;
                let (sx, sy) = if affine {
                    let double_size = (attr0 & (1 << 9)) != 0;
                    let draw_w = if double_size { width * 2 } else { width } as i32;
                    let draw_h = if double_size { height * 2 } else { height } as i32;

                    let mut rel_x = x as i32 - obj_x;
                    let mut rel_y = y as i32 - obj_y;
                    if rel_x < 0 || rel_y < 0 || rel_x >= draw_w || rel_y >= draw_h {
                        continue;
                    }
                    if mosaic_enabled {
                        rel_x -= rel_x % obj_mosaic_h;
                        rel_y -= rel_y % obj_mosaic_v;
                    }

                    let dx = rel_x - (draw_w / 2);
                    let dy = rel_y - (draw_h / 2);
                    let affine_index = ((attr1 >> 9) & 0x1F) as usize;
                    let params = &obj_affine[affine_index];
                    let sx_fp = i64::from(params.pa) * i64::from(dx)
                        + i64::from(params.pb) * i64::from(dy)
                        + i64::from((width as i32 * 256) / 2);
                    let sy_fp = i64::from(params.pc) * i64::from(dx)
                        + i64::from(params.pd) * i64::from(dy)
                        + i64::from((height as i32 * 256) / 2);

                    if sx_fp < 0
                        || sy_fp < 0
                        || sx_fp >= i64::from(width * 256)
                        || sy_fp >= i64::from(height * 256)
                    {
                        continue;
                    }

                    ((sx_fp >> 8) as u32, (sy_fp >> 8) as u32)
                } else {
                    let mut sx = x as i32 - obj_x;
                    let mut sy = y as i32 - obj_y;
                    if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                        continue;
                    }
                    if mosaic_enabled {
                        sx -= sx % obj_mosaic_h;
                        sy -= sy % obj_mosaic_v;
                    }

                    if (attr1 & (1 << 12)) != 0 {
                        sx = width as i32 - 1 - sx;
                    }
                    if (attr1 & (1 << 13)) != 0 {
                        sy = height as i32 - 1 - sy;
                    }
                    (sx as u32, sy as u32)
                };

                if color_8bpp {
                    tile_index &= !1;
                }
                let tile_x = sx / 8;
                let tile_y = sy / 8;
                let local_x = sx & 7;
                let local_y = sy & 7;

                let row_step = if one_d_mapping {
                    let tiles_w = width / 8;
                    tiles_w * tile_span
                } else {
                    32
                };

                let tile_number = tile_index + tile_y * row_step + tile_x * tile_span;
                let tile_addr = obj_char_base + tile_number * 32;

                let color = if color_8bpp {
                    let index = self
                        .bus
                        .scanline_obj_vram_read8(line, tile_addr + local_y * 8 + local_x);
                    if index == 0 {
                        continue;
                    }
                    self.bus
                        .scanline_pram_read16(line, 0x200 + u32::from(index) * 2)
                } else {
                    let byte = self
                        .bus
                        .scanline_obj_vram_read8(line, tile_addr + local_y * 4 + (local_x / 2));
                    let index = if (local_x & 1) == 0 {
                        byte & 0x0F
                    } else {
                        byte >> 4
                    };
                    if index == 0 {
                        continue;
                    }
                    let palette_bank = (attr2 >> 12) & 0x0F;
                    let palette_index = palette_bank * 16 + u16::from(index);
                    self.bus
                        .scanline_pram_read16(line, 0x200 + u32::from(palette_index) * 2)
                };

                if obj_mode == 2 {
                    output.window[x as usize] = true;
                    continue;
                }
                let (best, second) = &mut output.pixels[x as usize];
                let candidate = ObjPixel {
                    obj_index,
                    color,
                    priority: ((attr2 >> 10) & 0x3) as u8,
                    semi_transparent,
                };
                if best
                    .as_ref()
                    .is_none_or(|current| obj_pixel_in_front(&candidate, current))
                {
                    *second = *best;
                    *best = Some(candidate);
                } else if second
                    .as_ref()
                    .is_none_or(|current| obj_pixel_in_front(&candidate, current))
                {
                    *second = Some(candidate);
                }
            }
        }

        output
    }
}
