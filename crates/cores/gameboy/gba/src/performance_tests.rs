use super::*;
use std::hint::black_box;
use std::time::Instant;

// Synthetic fixtures keep timing comparisons reproducible without a commercial ROM.
#[test]
#[ignore = "release performance measurement; run with --ignored --nocapture"]
fn benchmark_gba_frames_and_snapshots() {
    let mut emu = GbaEmulator::new();
    let rom = RomImage::from_bytes([0xFE, 0xFF, 0xFF, 0xEA].repeat(128)).unwrap();
    emu.load_rom(rom).unwrap(); // ARM: B .
    emu.bus.write16(0x0400_0000, 3 | (1 << 10));
    emu.bus.write16(0x0400_0020, 0x100);
    emu.bus.write16(0x0400_0026, 0x100);
    emu.bus.vram_mut().fill(0x1F);
    let mut frame = GbaFrameBuffer::new();
    let mut audio = Vec::new();
    for _ in 0..30 {
        emu.step_frame_with_render(&mut frame).unwrap();
        emu.take_audio_samples_i16_into(&mut audio);
    }
    let mut timings = Vec::new();
    for _ in 0..300 {
        let start = Instant::now();
        emu.step_frame_with_render(&mut frame).unwrap();
        emu.take_audio_samples_i16_into(&mut audio);
        black_box(frame.pixels());
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timings.sort_by(f64::total_cmp);
    eprintln!(
        "GBA mode3 frame: mean={:.4}ms p95={:.4}ms checksum={:X}",
        timings.iter().sum::<f64>() / timings.len() as f64,
        timings[284],
        frame.pixels().iter().map(|&v| u64::from(v)).sum::<u64>()
    );

    for writes_per_line in [false, true] {
        let start = Instant::now();
        for n in 0..300u16 {
            emu.bus.write16(0x0600_0000, n);
            for line in 0..160 {
                if writes_per_line {
                    emu.bus.write16(0x0600_0000, n.wrapping_add(line));
                    emu.bus.write16(0x0601_0000, n.wrapping_add(line));
                }
                emu.snapshot_scanline_renderer_state(line);
            }
            black_box(emu.bus.scanline_bg_bitmap_vram_read16(159, 0x0600_0000));
        }
        eprintln!(
            "GBA snapshots writes_per_line={writes_per_line}: {:.4}ms/frame",
            start.elapsed().as_secs_f64() * 1000.0 / 300.0
        );
    }
}

#[test]
fn vram_snapshot_invalidation_covers_mirrors_mutable_access_clear_and_load() {
    let mut bus = GbaBus::default();
    bus.write16(0x0601_0000, 0x1234); // BG/OBJ overlap
    bus.snapshot_scanline_bg_bitmap_vram(0);
    bus.snapshot_scanline_obj_vram(0);
    bus.write8(0x0601_8000, 0x56); // mirror + duplicated byte write
    bus.snapshot_scanline_bg_bitmap_vram(1);
    bus.snapshot_scanline_obj_vram(1);
    assert_eq!(bus.scanline_bg_bitmap_vram_read16(0, 0x0601_0000), 0x1234);
    assert_eq!(bus.scanline_obj_vram_read8(0, 0x0601_8000), 0x34);
    assert_eq!(bus.scanline_bg_bitmap_vram_read16(1, 0x0601_0000), 0x5656);
    assert_eq!(bus.scanline_obj_vram_read8(1, 0x0601_0000), 0x56);
    bus.vram_mut()[0x10000] = 0x78;
    bus.snapshot_scanline_obj_vram(2);
    bus.snapshot_scanline_bg_bitmap_vram(2);
    assert_eq!(bus.scanline_obj_vram_read8(2, 0x0601_0000), 0x78);
    assert_eq!(bus.scanline_bg_bitmap_vram_read16(2, 0x0601_0000), 0x5678);
    bus.clear_vram();
    let mut writer = state::StateWriter::new();
    bus.serialize_state(&mut writer);
    let data = writer.into_vec();
    let mut loaded = GbaBus::default();
    loaded
        .deserialize_state(&mut state::StateReader::new(&data))
        .unwrap();
    assert_eq!(loaded.scanline_obj_vram_read8(2, 0x0601_0000), 0x78);
    loaded.snapshot_scanline_obj_vram(3);
    loaded.snapshot_scanline_bg_bitmap_vram(3);
    assert_eq!(loaded.scanline_obj_vram_read8(3, 0x0601_0000), 0);
    assert_eq!(loaded.scanline_bg_bitmap_vram_read16(3, 0x0601_0000), 0);
    // Re-saving preserves the expanded byte layout, regardless of sharing.
    let mut writer = state::StateWriter::new();
    bus.deserialize_state(&mut state::StateReader::new(&data))
        .unwrap();
    bus.serialize_state(&mut writer);
    assert_eq!(writer.into_vec(), data);
}

fn patterned_renderer() -> GbaEmulator {
    let mut emu = GbaEmulator::new();
    let mut seed = 0x12345678u32;
    for byte in emu.bus.vram_mut() {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        *byte = seed as u8;
    }
    for i in 0..512u32 {
        emu.bus
            .write16(0x0500_0000 + i * 2, (i as u16).wrapping_mul(73) & 0x7fff);
    }
    emu
}

fn varied_objects(seed: u16) -> ([ObjAttributes; 128], [ObjAffineParams; 32]) {
    let attrs = std::array::from_fn(|i| {
        let i = i as u16;
        ObjAttributes {
            // Cover disabled, affine/double-size, all modes and shapes,
            // wrapped coordinates, both color depths and mosaic.
            attr0: (i.wrapping_mul(17).wrapping_add(seed) & 255)
                | ((i & 3) << 8)
                | (((i / 4) & 3) << 10)
                | (((i / 3) & 1) << 12)
                | (((i / 5) & 1) << 13)
                | (((i / 7) & 3) << 14),
            attr1: (i.wrapping_mul(37).wrapping_add(seed) & 511)
                | (((i / 2) & 31) << 9)
                | (((i / 11) & 3) << 14),
            attr2: (i.wrapping_mul(19) & 1023) | ((i & 3) << 10) | ((i & 15) << 12),
        }
    });
    let matrices = std::array::from_fn(|i| {
        let i = i as i32;
        ObjAffineParams {
            pa: 256 - i * 13,
            pb: i * 17 - 128,
            pc: i * 11 - 64,
            pd: 256 - i * 7,
        }
    });
    (attrs, matrices)
}

#[test]
fn sprite_rasterizer_matches_pixel_reference_including_object_windows() {
    let emu = patterned_renderer();
    for seed in [0, 97, 211] {
        let (attrs, affine) = varied_objects(seed);
        for dispcnt in [0x9000, 0x9040, 0x9043, 0x8044, 0x1045, 0] {
            for y in (0..160).step_by(7) {
                let mosaic = MosaicState::from_register((y as u16).wrapping_mul(0x359));
                let line = emu.render_objects_for_line(y, dispcnt, mosaic, &attrs, &affine);
                for x in 0..240 {
                    let expected = if dispcnt & 0x1000 != 0 {
                        emu.sample_obj_pixels(y, dispcnt, x, y, mosaic, &attrs, &affine)
                    } else {
                        (None, None)
                    };
                    assert_eq!(
                        line.pixels[x as usize], expected,
                        "seed={seed} dispcnt={dispcnt:x} x={x} y={y}"
                    );
                    assert_eq!(
                        line.window[x as usize],
                        dispcnt & 0x8000 != 0
                            && emu.sample_objwin_hit(y, dispcnt, x, y, mosaic, &attrs, &affine)
                    );
                }
            }
        }
    }
}

#[test]
fn text_tile_row_cache_matches_reference_across_scroll_flip_depth_and_map_size() {
    let emu = patterned_renderer();
    for size in 0..4u16 {
        for eight_bit in [0, 0x80] {
            for y in [0, 7, 8, 159, 255, 256, 511] {
                for scroll in [0, 1, 7, 255, 511] {
                    let layer = TextBgLayer {
                        bg: 0,
                        cnt: (size << 14) | eight_bit | 0x1004,
                        hofs: scroll,
                        vofs: 253,
                        priority: 0,
                    };
                    let mut cache = TextBgRowCache::default();
                    for x in 0..240 {
                        // Repeated sample coordinates exercise mosaic reuse.
                        let sx = x - x % 3;
                        assert_eq!(
                            emu.sample_text_bg_color_cached(0, &layer, sx, y, &mut cache),
                            emu.sample_text_bg_color(0, &layer, sx, y)
                        );
                    }
                }
            }
        }
    }
}

fn halt_fixture(timer_control: Option<u16>) -> GbaEmulator {
    let mut emu = GbaEmulator::new();
    let program = [
        0xE59F100Cu32, // LDR r1, =IF
        0xE3E00000,    // MVN r0, #0
        0xE1C100B0,    // STRH r0, [r1]: clear IF
        0xEF000002,    // SWI Halt
        0xEAFFFFFB,    // B back to MVN
        0x04000202,
    ];
    emu.load_rom(
        RomImage::from_bytes(program.into_iter().flat_map(u32::to_le_bytes).collect()).unwrap(),
    )
    .unwrap();
    emu.bus.write16(0x0400_0004, 0xA038); // VBlank/HBlank/VCount IRQs, match line160
    emu.bus.write16(0x0400_0200, 0xffff); // IE; IME stays off, HALT still wakes
    if let Some(control) = timer_control {
        emu.bus.write16(0x0400_0100, 0xffed);
        emu.bus.write16(0x0400_0102, control);
        emu.bus.write16(0x0400_0104, 0xfffe);
        emu.bus.write16(0x0400_0106, 0x00c4); // cascade + IRQ
        // FIFO A DMA on timer0: exercise timer-triggered transfers at boundaries.
        for i in 0..32 {
            emu.bus.write32(0x0300_2000 + i * 4, 0x12345678 + i);
        }
        emu.bus.write16(0x0400_0084, 0x80);
        emu.bus.write16(0x0400_0082, 0x0304);
        emu.bus.write32(0x0400_00BC, 0x0300_2000);
        emu.bus.write32(0x0400_00C0, 0x0400_00A0);
        emu.bus.write16(0x0400_00C6, 0xB640);
    }
    emu
}

#[test]
fn halted_event_steps_preserve_state_pixels_and_pcm() {
    for timer in [None, Some(0x00c0), Some(0x00c1), Some(0x00c2), Some(0x00c3)] {
        for rendered in [false, true] {
            let mut fast = halt_fixture(timer);
            let mut reference = halt_fixture(timer);
            reference.single_cycle_halt = true;
            let mut fast_frame = GbaFrameBuffer::new();
            let mut reference_frame = GbaFrameBuffer::new();
            for _ in 0..2 {
                let a = if rendered {
                    fast.step_frame_with_render(&mut fast_frame)
                } else {
                    fast.step_frame()
                }
                .unwrap();
                let b = if rendered {
                    reference.step_frame_with_render(&mut reference_frame)
                } else {
                    reference.step_frame()
                }
                .unwrap();
                assert_eq!(a.cycles, b.cycles);
                assert_eq!(fast_frame.pixels(), reference_frame.pixels());
                let mut a = Vec::new();
                let mut b = Vec::new();
                fast.take_audio_samples_i16_into(&mut a);
                reference.take_audio_samples_i16_into(&mut b);
                assert_eq!(a, b, "timer={timer:?}");
                let a = fast.save_state();
                let b = reference.save_state();
                assert_eq!(a.len(), b.len());
                assert_eq!(
                    a.iter().zip(&b).position(|(a, b)| a != b),
                    None,
                    "timer={timer:?}, render={rendered}"
                );
            }
        }
    }
}

#[test]
fn streamed_state_matches_legacy_layout_and_crc_tracks_rom_replacement() {
    let mut emu = halt_fixture(None);
    let mut payload = state::StateWriter::new();
    emu.write_state_payload(&mut payload);
    let payload = payload.into_vec();
    let mut expected = Vec::new();
    expected.extend_from_slice(b"GBAS");
    expected.extend_from_slice(&1u32.to_le_bytes());
    expected.extend_from_slice(&state::crc32(emu.bus.rom_bytes()).to_le_bytes());
    expected.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    expected.extend_from_slice(&payload);
    assert_eq!(emu.save_state(), expected);
    assert_eq!(emu.state_payload_len(), payload.len());
    emu.load_state(&expected).unwrap();
    emu.load_rom(RomImage::from_bytes(vec![1; 512]).unwrap())
        .unwrap();
    assert_eq!(emu.load_state(&expected), Err("ROM CRC mismatch"));
    assert_eq!(emu.rom_crc32(), state::crc32(emu.bus.rom_bytes()));
}

#[test]
#[ignore = "release performance measurement"]
fn benchmark_followup_render_halt_and_state() {
    let emu = patterned_renderer();
    let (attrs, affine) = varied_objects(97);
    let mosaic = MosaicState::from_register(0x1234);
    for reference in [true, false] {
        let start = Instant::now();
        let mut checksum = 0u64;
        for _ in 0..20 {
            for y in 0..160 {
                if reference {
                    for x in 0..240 {
                        let pair = emu.sample_obj_pixels(y, 0x9040, x, y, mosaic, &attrs, &affine);
                        checksum += pair.0.map_or(0, |p| u64::from(p.color));
                        checksum += u64::from(
                            emu.sample_objwin_hit(y, 0x9040, x, y, mosaic, &attrs, &affine),
                        );
                        black_box(pair);
                    }
                } else {
                    let line = emu.render_objects_for_line(y, 0x9040, mosaic, &attrs, &affine);
                    for x in 0..240 {
                        checksum += line.pixels[x].0.map_or(0, |p| u64::from(p.color));
                        checksum += u64::from(line.window[x]);
                    }
                    black_box(line);
                }
            }
        }
        eprintln!(
            "OBJ reference={reference}: {:.4}ms/frame checksum={checksum}",
            start.elapsed().as_secs_f64() * 1000.0 / 20.0
        );
    }
    for reference in [true, false] {
        let start = Instant::now();
        let mut checksum = 0u64;
        let layer = TextBgLayer {
            bg: 0,
            cnt: 0x9004,
            hofs: 7,
            vofs: 255,
            priority: 0,
        };
        for _ in 0..100 {
            for y in 0..160 {
                let mut cache = TextBgRowCache::default();
                for x in 0..240 {
                    let color = if reference {
                        emu.sample_text_bg_color(y, &layer, x, y)
                    } else {
                        emu.sample_text_bg_color_cached(y, &layer, x, y, &mut cache)
                    };
                    checksum += u64::from(black_box(color).unwrap_or(0));
                }
            }
        }
        eprintln!(
            "BG reference={reference}: {:.4}ms/frame checksum={checksum}",
            start.elapsed().as_secs_f64() * 1000.0 / 100.0
        );
    }
    for reference in [true, false] {
        let mut emu = halt_fixture(None);
        emu.single_cycle_halt = reference;
        let mut audio = Vec::new();
        let start = Instant::now();
        for _ in 0..100 {
            black_box(emu.step_frame().unwrap());
            emu.take_audio_samples_i16_into(&mut audio);
        }
        eprintln!(
            "HALT reference={reference}: {:.4}ms/frame state_crc={:X}",
            start.elapsed().as_secs_f64() * 1000.0 / 100.0,
            state::crc32(&emu.save_state())
        );
    }
    for reference in [true, false] {
        let start = Instant::now();
        for _ in 0..100 {
            let size = if reference {
                let mut writer = state::StateWriter::new();
                emu.write_state_payload(&mut writer);
                black_box(writer.into_vec()).len()
            } else {
                emu.state_payload_len()
            };
            black_box(size);
        }
        eprintln!(
            "state size reference={reference}: {:.4}ms/call",
            start.elapsed().as_secs_f64() * 1000.0 / 100.0
        );
    }
}
