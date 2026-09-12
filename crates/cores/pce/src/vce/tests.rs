use super::*;

// Original arithmetic, independent of the lookup-table builder.
fn reference_rgb(raw: u16, scale: u16) -> u32 {
    let blue = (raw & 0x0007) as u8;
    let red = ((raw >> 3) & 0x0007) as u8;
    let green = ((raw >> 6) & 0x0007) as u8;
    let component = |value: u8| -> u8 {
        if scale == 0 {
            return 0;
        }
        let expanded = (value as u16 * 255) / 0x07;
        let scaled = (expanded * scale) / 0x07;
        scaled.min(255) as u8
    };
    let r = component(red);
    let g = component(green);
    let b = component(blue);
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn configured_scale() -> u16 {
    #[cfg(not(feature = "runtime-debug-flags"))]
    {
        7
    }
    #[cfg(feature = "runtime-debug-flags")]
    {
        use std::sync::OnceLock;
        static SCALE: OnceLock<u16> = OnceLock::new();
        *SCALE.get_or_init(|| {
            std::env::var("PCE_FORCE_BRIGHTNESS")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
                .map(|v| u16::from(v & 0x0F))
                .unwrap_or(7)
        })
    }
}

#[test]
fn lookup_matches_every_color_and_brightness() {
    for scale in 0..16 {
        let table = make_rgb_table(scale);
        for raw in 0..=u16::MAX {
            assert_eq!(
                table[usize::from(raw & 0x1FF)],
                reference_rgb(raw, scale),
                "raw={raw}, scale={scale}"
            );
        }
    }
}

#[test]
fn configured_palette_lookup_matches_reference() {
    let mut vce = Vce::new();
    // Includes ignored upper bits in direct/debug writes and decoded palettes.
    for raw in 0..=u16::MAX {
        let index = usize::from(raw & 0x1FF);
        vce.palette[index] = raw;
        assert_eq!(
            vce.palette_rgb(index),
            reference_rgb(raw, configured_scale())
        );
    }
    for index in [512, 1024, usize::MAX] {
        assert_eq!(vce.palette_rgb(index), 0);
    }
}

#[test]
fn lookup_tracks_byte_writes_direct_writes_reset_and_legacy_states() {
    // Mirrors the pre-change serialized layout, without derived host data.
    #[derive(bincode::Encode, bincode::Decode)]
    struct LegacyVce {
        palette: [u16; 512],
        control: u16,
        address: u16,
        data_latch: u16,
        write_phase: VcePhase,
        read_phase: VcePhase,
    }
    let scale = configured_scale();
    let mut vce = Vce::new();
    vce.set_address(511);
    vce.write_data_low(0x6B);
    assert_eq!(vce.palette_rgb(511), reference_rgb(0x6B, scale));
    vce.write_data_high(0xFF);
    assert_eq!(vce.palette_rgb(511), reference_rgb(0x16B, scale));
    assert_eq!(vce.address_index(), 0);
    vce.palette[0] = 0xFEA5;
    assert_eq!(vce.palette_rgb(0), reference_rgb(0xFEA5, scale));
    let legacy = LegacyVce {
        palette: vce.palette,
        control: vce.control,
        address: vce.address,
        data_latch: vce.data_latch,
        write_phase: vce.write_phase,
        read_phase: vce.read_phase,
    };
    let config = bincode::config::standard();
    let bytes = bincode::encode_to_vec(&legacy, config).unwrap();
    assert_eq!(bincode::encode_to_vec(&vce, config).unwrap(), bytes);
    vce.reset();
    assert_eq!(vce.palette_rgb(0), 0);
    assert_eq!(vce.palette_rgb(511), 0);
    let (restored, consumed): (Vce, usize) = bincode::decode_from_slice(&bytes, config).unwrap();
    assert_eq!(consumed, bytes.len());
    assert_eq!(restored.palette_rgb(0), reference_rgb(0xFEA5, scale));
    assert_eq!(restored.palette_rgb(511), reference_rgb(0x16B, scale));
    assert_eq!(bincode::encode_to_vec(&restored, config).unwrap(), bytes);
    let (decoded_old, _): (LegacyVce, usize) =
        bincode::decode_from_slice(&bincode::encode_to_vec(&restored, config).unwrap(), config)
            .unwrap();
    assert_eq!(decoded_old.palette, restored.palette);
}

#[test]
#[ignore = "manual release benchmark"]
fn benchmark_pce_palette_lookup() {
    use std::{hint::black_box, time::Instant};
    let mut vce = Vce::new();
    for (index, raw) in vce.palette.iter_mut().enumerate() {
        *raw = ((index * 313) & 511) as u16;
    }
    let mut outputs = [vec![0u32; 512 * 240], vec![0u32; 512 * 240]];
    let mut samples = [Vec::new(), Vec::new()];
    for round in 0..8 {
        for which in [round % 2, 1 - round % 2] {
            let start = Instant::now();
            for _ in 0..100 {
                for (pixel, out) in outputs[which].iter_mut().enumerate() {
                    let index = black_box(pixel & 511);
                    *out = if which == 0 {
                        reference_rgb(
                            vce.palette.get(index).copied().unwrap_or(0),
                            configured_scale(),
                        )
                    } else {
                        vce.palette_rgb(index)
                    };
                }
                black_box(&outputs[which]);
            }
            if round >= 2 {
                samples[which].push(start.elapsed().as_secs_f64() * 1e6 / 100.0);
            }
        }
    }
    assert_eq!(outputs[0], outputs[1]);
    let medians = samples.map(|mut values| {
        values.sort_by(f64::total_cmp);
        (values[2] + values[3]) / 2.0
    });
    println!(
        "palette 122880 pixels: reference={:.3} us, lookup={:.3} us, reduction={:.1}%",
        medians[0],
        medians[1],
        100.0 * (1.0 - medians[1] / medians[0])
    );
}
