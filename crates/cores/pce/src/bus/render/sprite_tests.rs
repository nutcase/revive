// The pre-optimization pixel-first renderer, retained only for differential tests.
use super::*;

impl Bus {
    fn render_sprites_reference(
        &mut self,
        line_enabled: &[bool; FRAME_HEIGHT],
        line_display_starts: &[usize; FRAME_HEIGHT],
        line_display_widths: &[usize; FRAME_HEIGHT],
        options: SpriteRenderOptions,
    ) {
        if self.vdc.vram.is_empty() {
            return;
        }
        #[derive(Clone, Copy, Default)]
        struct LineSprite {
            x: i32,
            visible_width: usize,
            full_width: usize,
            src_tile_y: usize,
            row_in_tile: usize,
            pattern_base_index: usize,
            palette_base: usize,
            high_priority: bool,
            h_flip: bool,
            use_upper_cg_pair: bool,
        }

        let vram = if self.sprite_vram_snapshot.0.len() == self.vdc.vram.len()
            && !self.sprite_vram_snapshot.0.is_empty()
        {
            &self.sprite_vram_snapshot.0
        } else {
            &self.vdc.vram
        };
        let vram_mask = vram.len().saturating_sub(1);
        let mut overflow_detected = false;
        let mwr = self.vdc.registers[0x09];
        let sprite_dot_period = (mwr >> 2) & 0x03;
        let cg_mode_enabled = sprite_dot_period == 0x01;
        let SpriteRenderOptions {
            reverse_priority,
            no_sprite_line_limit,
            pattern_raw_index,
            row_interleaved,
            sprite_max_entries,
        } = options;

        for dest_row in 0..FRAME_HEIGHT {
            if !line_enabled[dest_row] {
                continue;
            }
            let Some(active_row) = self.vdc.active_row_for_output_row(dest_row) else {
                continue;
            };
            let line_display_start = line_display_starts[dest_row] as i32;
            let mut line_sprites = [LineSprite::default(); SPRITE_COUNT];
            let mut line_sprite_count = 0usize;
            let mut cell_slots_used = 0u8;
            let scanline_y = active_row as i32;

            for sprite_idx in 0..SPRITE_COUNT.min(sprite_max_entries) {
                let sprite = if reverse_priority {
                    SPRITE_COUNT - 1 - sprite_idx
                } else {
                    sprite_idx
                };
                let base = sprite * 4;
                let y_word = self.vdc.satb.get(base).copied().unwrap_or(0);
                let x_word = self.vdc.satb.get(base + 1).copied().unwrap_or(0);
                let pattern_word = self.vdc.satb.get(base + 2).copied().unwrap_or(0);
                let attr_word = self.vdc.satb.get(base + 3).copied().unwrap_or(0);

                // MAME sprite Y: src_y = (m_current_segment_start - sat_y) & 0x3FF
                // m_current_segment_start = 0x40 at first active line.
                // So sat_y=64 (0x40) → src_y=0 → first row at display row 0.
                // Screen Y = sat_y - 64 (no +1; the -1 in "raster_count - 1"
                // is already factored into m_current_segment_start).
                let y = (y_word & 0x03FF) as i32 - 64;
                let x = (x_word & 0x03FF) as i32 - 32 + line_display_start;
                let width_cells = if (attr_word & 0x0100) != 0 {
                    2usize
                } else {
                    1usize
                };
                let height_code = ((attr_word >> 12) & 0x03) as usize;
                let height_cells = match height_code {
                    0 => 1,
                    1 => 2,
                    _ => 4,
                };
                let full_width = width_cells * SPRITE_PATTERN_WIDTH;
                let full_height = height_cells * SPRITE_PATTERN_HEIGHT;
                if scanline_y < y || scanline_y >= y + full_height as i32 {
                    continue;
                }

                let required_cell_slots = width_cells as u8;
                if !no_sprite_line_limit && cell_slots_used >= 16 {
                    overflow_detected = true;
                    continue;
                }
                if cell_slots_used.saturating_add(required_cell_slots) > 16 {
                    overflow_detected = true;
                }
                cell_slots_used = cell_slots_used.saturating_add(required_cell_slots).min(16);

                let mut pattern_base_index = if pattern_raw_index {
                    (pattern_word & 0x03FF) as usize
                } else {
                    ((pattern_word >> 1) & 0x03FF) as usize
                };
                if width_cells == 2 {
                    pattern_base_index &= !0x0001;
                }
                // HuC6270 aligns tall sprites to their required tile group.
                // 32px high clears bit 1; 64px high clears bits 1 and 2.
                match height_code {
                    1 => pattern_base_index &= !0x0002,
                    2 | 3 => pattern_base_index &= !0x0006,
                    _ => {}
                }

                let v_flip = (attr_word & 0x8000) != 0;
                let local_y = (scanline_y - y) as usize;
                let src_y = if v_flip {
                    full_height - 1 - local_y
                } else {
                    local_y
                };
                let src_tile_y = src_y / SPRITE_PATTERN_HEIGHT;
                let row_in_tile = src_y % SPRITE_PATTERN_HEIGHT;

                if line_sprite_count < line_sprites.len() {
                    line_sprites[line_sprite_count] = LineSprite {
                        x,
                        visible_width: full_width,
                        full_width,
                        src_tile_y,
                        row_in_tile,
                        pattern_base_index,
                        palette_base: 0x100usize | (((attr_word & 0x000F) as usize) << 4),
                        high_priority: (attr_word & 0x0080) != 0,
                        h_flip: (attr_word & 0x0800) != 0,
                        use_upper_cg_pair: (pattern_word & 0x0001) != 0,
                    };
                    line_sprite_count += 1;
                }
            }

            self.sprite_line_counts[dest_row] = cell_slots_used;

            let line_display_start = line_display_starts[dest_row];
            let line_display_width = line_display_widths[dest_row];
            let line_display_end = line_display_start + line_display_width;
            for screen_x in line_display_start..line_display_end {
                let offset = dest_row * FRAME_WIDTH + screen_x;
                for sprite in line_sprites[..line_sprite_count].iter() {
                    if (screen_x as i32) < sprite.x
                        || (screen_x as i32) >= sprite.x + sprite.visible_width as i32
                    {
                        continue;
                    }

                    let local_x = (screen_x as i32 - sprite.x) as usize;
                    let src_x = if sprite.h_flip {
                        sprite.full_width - 1 - local_x
                    } else {
                        local_x
                    };
                    let src_tile_x = src_x / SPRITE_PATTERN_WIDTH;
                    let col_in_tile = src_x % SPRITE_PATTERN_WIDTH;
                    let pattern_index =
                        sprite.pattern_base_index + sprite.src_tile_y * 2 + src_tile_x;
                    let pattern_base = (pattern_index * SPRITE_PATTERN_WORDS) & vram_mask;

                    let (plane0_word, plane1_word, plane2_word, plane3_word) = if row_interleaved {
                        let row_base = (pattern_base + sprite.row_in_tile * 4) & vram_mask;
                        (
                            vram[row_base],
                            vram[(row_base + 1) & vram_mask],
                            vram[(row_base + 2) & vram_mask],
                            vram[(row_base + 3) & vram_mask],
                        )
                    } else {
                        (
                            vram[(pattern_base + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 16 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 32 + sprite.row_in_tile) & vram_mask],
                            vram[(pattern_base + 48 + sprite.row_in_tile) & vram_mask],
                        )
                    };
                    let shift = 15usize.saturating_sub(col_in_tile);
                    let mut plane0 = ((plane0_word >> shift) & 0x01) as u8;
                    let mut plane1 = ((plane1_word >> shift) & 0x01) as u8;
                    let mut plane2 = ((plane2_word >> shift) & 0x01) as u8;
                    let mut plane3 = ((plane3_word >> shift) & 0x01) as u8;

                    if cg_mode_enabled {
                        if sprite.use_upper_cg_pair {
                            plane0 = plane2;
                            plane1 = plane3;
                            plane2 = 0;
                            plane3 = 0;
                        } else {
                            plane2 = 0;
                            plane3 = 0;
                        }
                    }

                    let pixel = plane0 | (plane1 << 1) | (plane2 << 2) | (plane3 << 3);
                    if pixel == 0 {
                        continue;
                    }

                    let bg_opaque = self.bg_opaque[offset];
                    let bg_forces_front = self.bg_priority[offset];
                    if !bg_opaque || (sprite.high_priority && !bg_forces_front) {
                        let colour_index = (sprite.palette_base | pixel as usize) & 0x1FF;
                        self.framebuffer[offset] = self.vce.palette_rgb(colour_index);
                    }
                    // The first opaque sprite pixel wins, regardless of BG blend result.
                    break;
                }
            }
        }

        if overflow_detected {
            self.vdc.raise_status(VDC_STATUS_OR);
        }
    }
}

fn options(bits: usize) -> SpriteRenderOptions {
    SpriteRenderOptions {
        reverse_priority: bits & 1 != 0,
        no_sprite_line_limit: bits & 2 != 0,
        pattern_raw_index: bits & 4 != 0,
        row_interleaved: bits & 8 != 0,
        sprite_max_entries: SPRITE_COUNT,
    }
}

fn fixture(scene: usize) -> Bus {
    let mut bus = Bus::new();
    let mut seed = 0x7654_3210u32;
    for word in &mut bus.vdc.vram {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        *word = if scene == 4 { 0 } else { seed as u16 };
    }
    for (index, color) in bus.vce.palette.iter_mut().enumerate() {
        *color = index as u16;
    }
    for index in 0..bus.framebuffer.len() {
        bus.framebuffer[index] = 0xFF12_3456;
        bus.bg_opaque[index] = index % 3 != 0;
        bus.bg_priority[index] = index % 7 < 2;
    }
    for sprite in 0..SPRITE_COUNT {
        let base = sprite * 4;
        let x = match scene {
            0 => (sprite * 37 % 540) as i32 - 28,
            1 => (sprite % 16 * 32) as i32,
            2 | 4 => 80,
            3 => 800, // Entirely offscreen: still consumes sprite slots.
            _ => (sprite * 23 % 550) as i32 - 31,
        };
        let y = if scene == 2 || scene == 4 {
            0
        } else {
            sprite / 16 * 60
        };
        bus.vdc.satb[base] = (y + 64) as u16;
        bus.vdc.satb[base + 1] = (x + 32) as u16;
        // Includes VRAM wraparound and both CG pairs.
        bus.vdc.satb[base + 2] = (0x3F0 + sprite * 7) as u16;
        bus.vdc.satb[base + 3] = ((sprite & 15) as u16)
            | if sprite % 2 == 0 { 0x0100 } else { 0 }
            | if sprite % 3 == 0 { 0x0080 } else { 0 }
            | if sprite % 4 < 2 { 0x0800 } else { 0 }
            | if sprite % 5 < 2 { 0x8000 } else { 0 }
            | (((sprite % 4) as u16) << 12);
    }
    if scene == 5 {
        bus.sprite_vram_snapshot.0 = bus.vdc.vram.clone();
        bus.vdc.vram.fill(0);
    } else if scene == 6 {
        // A mismatching snapshot must fall back to current VRAM.
        bus.sprite_vram_snapshot.0 = vec![0; 16];
        bus.vdc.registers[0x0C] = 0x0201;
        bus.vdc.registers[0x0D] = 179;
        bus.vdc.registers[0x0E] = 4;
    } else if scene == 7 {
        bus.vdc.vram.clear();
    }
    bus
}

#[test]
fn sprite_spans_match_pixel_reference() {
    let enabled = std::array::from_fn(|y| y % 7 != 3);
    let starts = std::array::from_fn(|y| y % 33);
    let widths = std::array::from_fn(|y| [1, 17, 255, 320, 480][y % 5]);
    for scene in 0..8 {
        for flags in 0..16 {
            for cg_mode in [0, 4] {
                let mut actual = fixture(scene);
                actual.vdc.registers[0x09] = cg_mode;
                let mut reference = actual.clone();
                let mut opts = options(flags);
                if scene == 6 {
                    opts.sprite_max_entries = 19;
                }
                actual.render_sprites_with_options(&enabled, &starts, &widths, opts);
                reference.render_sprites_reference(&enabled, &starts, &widths, opts);
                let context = format!("scene={scene}, flags={flags}, cg={cg_mode}");
                assert!(
                    actual.framebuffer == reference.framebuffer,
                    "pixels: {context}"
                );
                assert_eq!(
                    actual.sprite_line_counts, reference.sprite_line_counts,
                    "{context}"
                );
                assert_eq!(
                    actual.vdc.status_bits(),
                    reference.vdc.status_bits(),
                    "{context}"
                );
                // Include VDC state, source snapshots and background masks, not only pixels.
                assert!(
                    bincode::encode_to_vec(&actual, bincode::config::standard()).unwrap()
                        == bincode::encode_to_vec(&reference, bincode::config::standard()).unwrap(),
                    "state: {context}"
                );
            }
        }
    }
}

#[test]
fn background_hidden_winner_still_blocks_later_sprites() {
    let mut bus = Bus::new();
    bus.framebuffer.fill(0xFF12_3456);
    bus.bg_opaque.fill(true);
    bus.bg_priority.fill(false);
    bus.vce.palette[0x101] = 0x1FF;
    for tile in 0..2 {
        for row in 0..16 {
            bus.vdc.vram[tile * SPRITE_PATTERN_WORDS + row] = 0xFFFF;
        }
        bus.vdc.satb[tile * 4] = 64;
        bus.vdc.satb[tile * 4 + 1] = 32;
        bus.vdc.satb[tile * 4 + 2] = (tile * 2) as u16;
    }
    // Front sprite is behind BG, next sprite is above BG.
    bus.vdc.satb[7] = 0x0080;
    let enabled = std::array::from_fn(|y| y == 0);
    let starts = [0; FRAME_HEIGHT];
    let widths = [256; FRAME_HEIGHT];
    bus.render_sprites_with_options(&enabled, &starts, &widths, options(0));
    assert_eq!(bus.framebuffer[0], 0xFF12_3456);
    // A transparent pixel in the first sprite must let the second sprite win.
    bus.vdc.vram[0] = 0x7FFF;
    bus.render_sprites_with_options(&enabled, &starts, &widths, options(0));
    assert_eq!(bus.framebuffer[0], bus.vce.palette_rgb(0x101));
    assert_eq!(bus.framebuffer[1], 0xFF12_3456);
}

#[test]
#[ignore = "manual release benchmark; run with --ignored --nocapture --test-threads=1"]
fn benchmark_pce_sprite_spans() {
    use std::{hint::black_box, time::Instant};
    let enabled = [true; FRAME_HEIGHT];
    let starts = [0; FRAME_HEIGHT];
    let widths = [512; FRAME_HEIGHT];
    for (scene, label) in [
        (0, "spread"),
        (1, "dense"),
        (2, "overlap"),
        (3, "offscreen"),
        (4, "transparent"),
    ] {
        let initial = fixture(scene);
        let mut reference = initial.clone();
        let mut actual = initial;
        for _ in 0..20 {
            reference.render_sprites_reference(&enabled, &starts, &widths, options(0));
            actual.render_sprites_with_options(&enabled, &starts, &widths, options(0));
        }
        assert!(reference.framebuffer == actual.framebuffer);
        let mut measurements = [Vec::new(), Vec::new()];
        for round in 0..6 {
            // Alternate order to reduce warm-up/frequency ordering bias.
            for which in [round % 2, 1 - round % 2] {
                let start = Instant::now();
                for _ in 0..200 {
                    if which == 0 {
                        reference.render_sprites_reference(
                            black_box(&enabled),
                            black_box(&starts),
                            black_box(&widths),
                            black_box(options(0)),
                        );
                        black_box(&reference.framebuffer);
                    } else {
                        actual.render_sprites_with_options(
                            black_box(&enabled),
                            black_box(&starts),
                            black_box(&widths),
                            black_box(options(0)),
                        );
                        black_box(&actual.framebuffer);
                    }
                }
                measurements[which].push(start.elapsed().as_secs_f64() * 1e6 / 200.0);
            }
        }
        let medians = measurements.map(|mut values| {
            values.sort_by(f64::total_cmp);
            (values[2] + values[3]) / 2.0
        });
        assert!(reference.framebuffer == actual.framebuffer);
        println!(
            "{label}: reference={:.3} us/frame, spans={:.3} us/frame, reduction={:.1}%",
            medians[0],
            medians[1],
            100.0 * (1.0 - medians[1] / medians[0])
        );
    }
}
