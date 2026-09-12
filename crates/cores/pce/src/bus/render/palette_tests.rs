use super::*;

#[test]
#[ignore = "manual release benchmark; compare before/after binaries with identical fixtures"]
fn benchmark_pce_palette_frame_rendering() {
    use std::{hint::black_box, time::Instant};
    for (label, sprites, animate) in [
        ("background", false, false),
        ("background-sprites", true, false),
        ("palette-animation", true, true),
    ] {
        let mut bus = Bus::new();
        let ctrl = VDC_CTRL_ENABLE_BACKGROUND_LEGACY
            | if sprites {
                VDC_CTRL_ENABLE_SPRITES_LEGACY
            } else {
                0
            };
        bus.vdc.registers[0x04] = ctrl;
        bus.vdc.registers[0x05] = ctrl;
        bus.vdc.registers[0x0A] = 0;
        bus.vdc.registers[0x0B] = 31; // 256 visible pixels.
        let mut seed = 0x3141_5926u32;
        for word in &mut bus.vdc.vram {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *word = seed as u16;
        }
        for index in 0..2048 {
            bus.vdc.vram[index] = (0x200 + index % 256) as u16 | ((index % 16) as u16) << 12;
        }
        for (index, raw) in bus.vce.palette.iter_mut().enumerate() {
            *raw = ((index * 313) & 511) as u16;
        }
        for sprite in 0..SPRITE_COUNT {
            let base = sprite * 4;
            bus.vdc.satb[base] = (64 + (sprite / 16) * 60) as u16;
            bus.vdc.satb[base + 1] = (16 + (sprite % 16) * 17) as u16;
            bus.vdc.satb[base + 2] = ((128 + sprite * 2) * 2) as u16;
            bus.vdc.satb[base + 3] = 0x3180 | (sprite % 16) as u16;
        }
        let mut samples = Vec::with_capacity(300);
        for frame in 0..330 {
            let start = Instant::now();
            if animate {
                bus.vce.set_address((frame % 512) as u16);
                for index in 0..64 {
                    let raw = ((frame * 7 + index * 13) & 511) as u16;
                    bus.vce.write_data_low(raw as u8);
                    bus.vce.write_data_high((raw >> 8) as u8);
                }
            }
            black_box(&mut bus).render_frame_from_vram();
            black_box(&bus.framebuffer);
            if frame >= 30 {
                samples.push(start.elapsed().as_secs_f64() * 1e6);
            }
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        samples.sort_by(f64::total_cmp);
        let p95 = samples[284];
        let bytes = bincode::encode_to_vec(&bus, bincode::config::standard()).unwrap();
        let checksum = bytes.iter().fold(0x811C9DC5u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
        });
        assert!(bus.framebuffer.iter().any(|&pixel| pixel != 0));
        println!("{label}: mean={mean:.3} us, p95={p95:.3} us, state={checksum:08X}");
    }
}
