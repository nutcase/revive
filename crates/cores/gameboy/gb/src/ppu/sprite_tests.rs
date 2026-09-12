// Original pixel-first renderer retained only as a differential reference.
use super::*;

impl GbPpu {
    fn render_scanline_reference(&mut self, ly: u8, bus: &GbBus) {
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
            self.render_sprites_for_line_reference(
                ly,
                bus,
                lcdc,
                cgb_mode,
                &bg_color_indices,
                &bg_priority,
            );
        }
    }

    fn render_sprites_for_line_reference(
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
}

fn fixture(cgb: bool, tall: bool, scene: usize) -> GbBus {
    let mut bus = GbBus::default();
    bus.set_cgb_mode(cgb);
    bus.write8(0xFF40, 0xF3 | if tall { OBJ_SIZE_8X16 } else { 0 });
    bus.write8(0xFF47, 0xE4);
    bus.write8(0xFF48, 0xE4);
    bus.write8(0xFF49, 0x1B);
    bus.write8(0xFF4A, 31);
    bus.write8(0xFF4B, 83);
    for (addr, data) in [(0xFF68, 0xFF69), (0xFF6A, 0xFF6B)] {
        bus.write8(addr, 0x80);
        for index in 0..64 {
            bus.write8(data, (index * 37 + 13) as u8);
        }
    }
    let mut seed = 0x1234ABCDu32;
    for byte in bus.video_ram_mut() {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        *byte = if scene == 3 { 0 } else { seed as u8 };
    }
    for (index, sprite) in bus.oam_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let x = match scene {
            1 => 80,
            2 => {
                if index < 10 {
                    0
                } else {
                    8
                }
            }
            4 => 4 + index % 3,
            5 => 165 + index % 3,
            _ => (index * 29) % 180,
        };
        sprite[0] = if scene == 6 {
            0
        } else if scene == 1 || scene == 2 {
            16
        } else {
            (16 + index / 10 * 32) as u8
        };
        sprite[1] = x as u8;
        sprite[2] = (index * 17 + 1) as u8; // Odd tile indices exercise 8x16 alignment.
        sprite[3] = (index * 53 + scene * 11) as u8;
    }
    bus
}

fn state(ppu: &GbPpu) -> Vec<u8> {
    let mut writer = crate::state::StateWriter::new();
    ppu.serialize_state(&mut writer);
    writer.into_vec()
}

#[test]
fn sprite_spans_match_original_pixels_and_state() {
    for cgb in [false, true] {
        for tall in [false, true] {
            for scene in 0..8 {
                let mut bus = fixture(cgb, tall, scene);
                let mut actual = GbPpu::default();
                let mut reference = GbPpu::default();
                actual.frame_rgba8888.fill(0x5A);
                reference.frame_rgba8888.fill(0x5A);
                for ly in 0..144u8 {
                    let bg =
                        std::array::from_fn::<_, 160, _>(|x| ((x + usize::from(ly)) % 4) as u8);
                    let priority = std::array::from_fn::<_, 160, _>(|x| x % 3 == 0);
                    actual.render_sprites_for_line(ly, &bus, bus.ppu_lcdc(), cgb, &bg, &priority);
                    reference.render_sprites_for_line_reference(
                        ly,
                        &bus,
                        bus.ppu_lcdc(),
                        cgb,
                        &bg,
                        &priority,
                    );
                }
                assert_eq!(
                    state(&actual),
                    state(&reference),
                    "sprite pass: cgb={cgb}, tall={tall}, scene={scene}"
                );
                for ly in 0..144u8 {
                    // Re-fetch changed VRAM, OAM and palette values on later rows.
                    bus.video_ram_mut()[usize::from(ly) * 7] ^= ly;
                    bus.oam_mut()[3] = ly;
                    bus.write8(0xFF48, ly);
                    bus.write8(0xFF6A, ly & 63);
                    bus.write8(0xFF6B, ly.wrapping_mul(13));
                    bus.write8(0xFF43, ly);
                    actual.render_scanline(ly, &bus);
                    reference.render_scanline_reference(ly, &bus);
                }
                assert_eq!(
                    state(&actual),
                    state(&reference),
                    "full rendering: cgb={cgb}, tall={tall}, scene={scene}"
                );
                // Restoring a frame introduces no persistent sprite cache.
                let saved = state(&actual);
                actual
                    .deserialize_state(&mut crate::state::StateReader::new(&saved))
                    .unwrap();
                reference
                    .deserialize_state(&mut crate::state::StateReader::new(&saved))
                    .unwrap();
                actual.render_scanline(0, &bus);
                reference.render_scanline_reference(0, &bus);
                assert_eq!(state(&actual), state(&reference));
            }
        }
    }
}

#[test]
fn sprite_priority_differs_between_dmg_and_cgb_and_respects_hidden_winners() {
    for cgb in [false, true] {
        let mut bus = fixture(cgb, false, 0);
        bus.oam_mut().fill(0);
        bus.video_ram_mut()[0..16].fill(0xFF); // Opaque color 3.
        bus.oam_mut()[0..8].copy_from_slice(&[16, 12, 0, 0x80, 16, 10, 0, 0]);
        let mut ppu = GbPpu::default();
        ppu.frame_rgba8888.fill(0x5A);
        ppu.render_sprites_for_line(0, &bus, bus.ppu_lcdc(), cgb, &[1; 160], &[false; 160]);
        let pixel = &ppu.frame_rgba8888[4 * 4..5 * 4];
        if cgb {
            assert_eq!(pixel, &[0x5A; 4]); // First OAM object wins, but BG hides it.
        } else {
            assert_eq!(pixel, &dmg_palette_color(bus.ppu_obj_palette0(), 3)); // Lower X wins.
        }
        // Equal X must use OAM order on either model.
        bus.oam_mut()[5] = 12;
        ppu.frame_rgba8888.fill(0x5A);
        ppu.render_sprites_for_line(0, &bus, bus.ppu_lcdc(), cgb, &[1; 160], &[false; 160]);
        assert_eq!(&ppu.frame_rgba8888[16..20], &[0x5A; 4]);
    }
}

#[test]
fn horizontally_hidden_objects_still_consume_the_ten_object_limit() {
    for cgb in [false, true] {
        let mut bus = fixture(cgb, false, 2);
        bus.video_ram_mut().fill(0xFF);
        let mut ppu = GbPpu::default();
        ppu.frame_rgba8888.fill(0x5A);
        ppu.render_sprites_for_line(0, &bus, bus.ppu_lcdc(), cgb, &[0; 160], &[false; 160]);
        assert!(ppu.frame_rgba8888.iter().all(|&byte| byte == 0x5A));
    }
}

#[test]
#[ignore = "manual release benchmark"]
fn benchmark_gb_sprite_spans() {
    use std::{hint::black_box, time::Instant};
    for cgb in [false, true] {
        for (scene, label) in [
            (0, "spread"),
            (1, "overlap"),
            (2, "offscreen"),
            (3, "transparent"),
            (6, "empty"),
        ] {
            let bus = fixture(cgb, true, scene);
            for full_frame in [false, true] {
                let mut ppus = [GbPpu::default(), GbPpu::default()];
                let mut times = [Vec::new(), Vec::new()];
                for round in 0..8 {
                    for which in [round % 2, 1 - round % 2] {
                        let start = Instant::now();
                        for _ in 0..100 {
                            for ly in 0..144u8 {
                                let ppu = black_box(&mut ppus[which]);
                                let bus = black_box(&bus);
                                if full_frame {
                                    if which == 0 {
                                        ppu.render_scanline_reference(ly, bus);
                                    } else {
                                        ppu.render_scanline(ly, bus);
                                    }
                                } else if which == 0 {
                                    ppu.render_sprites_for_line_reference(
                                        ly,
                                        bus,
                                        bus.ppu_lcdc(),
                                        cgb,
                                        &[0; 160],
                                        &[false; 160],
                                    );
                                } else {
                                    ppu.render_sprites_for_line(
                                        ly,
                                        bus,
                                        bus.ppu_lcdc(),
                                        cgb,
                                        &[0; 160],
                                        &[false; 160],
                                    );
                                }
                            }
                            black_box(ppus[which].frame_rgba8888());
                        }
                        if round >= 2 {
                            times[which].push(start.elapsed().as_secs_f64() * 1e6 / 100.0);
                        }
                    }
                }
                assert_eq!(state(&ppus[0]), state(&ppus[1]));
                let medians = times.map(|mut times| {
                    times.sort_by(f64::total_cmp);
                    (times[2] + times[3]) / 2.0
                });
                println!(
                    "cgb={cgb} {label} full={full_frame}: reference={:.3} us, spans={:.3} us, reduction={:.1}%",
                    medians[0],
                    medians[1],
                    100.0 * (1.0 - medians[1] / medians[0])
                );
            }
        }
    }
}
