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
