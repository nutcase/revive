//! Headless audio diagnostic for Dodge Danpei 2 (FME-7/Sunsoft 5B).
//! Run with: cargo test -p nes-emulator --test audio_diag -- --nocapture

use nes_emulator::Nes;

#[test]
fn diagnose_dodge_danpei_audio() {
    let rom_path = "roms/Honoo no Toukyuuji - Dodge Danpei 2 (Japan).nes";
    if !std::path::Path::new(rom_path).exists() {
        eprintln!("ROM not found, skipping");
        return;
    }

    let mut nes = Nes::new();
    nes.load_rom(rom_path).unwrap();

    let total_frames = 600;

    // Per-frame stats
    struct FrameStats {
        noise_duty: f64,
        noise_vol_avg: f64,
        noise_period: u16,
        noise_envelope_disable: bool,
        exp_rms: f64,
        pulse1_active: f64,
        pulse2_active: f64,
        triangle_active: f64,
    }

    let mut all_frames: Vec<FrameStats> = Vec::new();

    println!("Running {} frames headless...\n", total_frames);
    println!(
        "{:>5} {:>6} {:>4} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7}",
        "Frame", "NsDuty", "NsVl", "NsPer", "NsEnv", "P1Dty", "P2Dty", "TriDt", "ExpRMS"
    );

    for frame_idx in 0..total_frames {
        let mut noise_active = 0u64;
        let mut noise_vol_sum = 0u64;
        let mut noise_vol_count = 0u64;
        let mut pulse1_active = 0u64;
        let mut pulse2_active = 0u64;
        let mut triangle_active = 0u64;
        let mut total_cycles = 0u64;
        let mut exp_sum_sq = 0.0f64;

        let (last_noise_period, last_noise_envdis) = loop {
            let frame_done = nes.step();
            total_cycles += 1;

            let diag = nes.audio_diag_full();
            // noise
            if diag.noise_enabled && diag.noise_length > 0 && diag.noise_vol > 0 {
                noise_active += 1;
                noise_vol_sum += diag.noise_vol as u64;
                noise_vol_count += 1;
            }
            // pulse1
            if diag.pulse1_enabled && diag.pulse1_length > 0 {
                pulse1_active += 1;
            }
            // pulse2
            if diag.pulse2_enabled && diag.pulse2_length > 0 {
                pulse2_active += 1;
            }
            // triangle
            if diag.triangle_enabled && diag.triangle_length > 0 {
                triangle_active += 1;
            }
            // expansion
            exp_sum_sq += (diag.expansion as f64).powi(2);

            if frame_done {
                break (diag.noise_period, diag.noise_envelope_disable);
            }
        };

        let noise_duty = if total_cycles > 0 {
            noise_active as f64 / total_cycles as f64
        } else {
            0.0
        };
        let noise_vol_avg = if noise_vol_count > 0 {
            noise_vol_sum as f64 / noise_vol_count as f64
        } else {
            0.0
        };
        let exp_rms = (exp_sum_sq / total_cycles.max(1) as f64).sqrt();
        let p1_duty = if total_cycles > 0 {
            pulse1_active as f64 / total_cycles as f64
        } else {
            0.0
        };
        let p2_duty = if total_cycles > 0 {
            pulse2_active as f64 / total_cycles as f64
        } else {
            0.0
        };
        let tri_duty = if total_cycles > 0 {
            triangle_active as f64 / total_cycles as f64
        } else {
            0.0
        };

        all_frames.push(FrameStats {
            noise_duty,
            noise_vol_avg,
            noise_period: last_noise_period,
            noise_envelope_disable: last_noise_envdis,
            exp_rms,
            pulse1_active: p1_duty,
            pulse2_active: p2_duty,
            triangle_active: tri_duty,
        });

        if (frame_idx + 1) % 20 == 0 {
            println!(
                "{:5} {:6.3} {:4.1} {:5} {:>5} {:5.3} {:5.3} {:5.3} {:7.5}",
                frame_idx + 1,
                noise_duty,
                noise_vol_avg,
                last_noise_period,
                if last_noise_envdis { "const" } else { "env" },
                p1_duty,
                p2_duty,
                tri_duty,
                exp_rms
            );
        }
    }

    // Summary
    let noise_on_frames = all_frames.iter().filter(|f| f.noise_duty > 0.01).count();
    let p1_on_frames = all_frames.iter().filter(|f| f.pulse1_active > 0.01).count();
    let p2_on_frames = all_frames.iter().filter(|f| f.pulse2_active > 0.01).count();
    let tri_on_frames = all_frames
        .iter()
        .filter(|f| f.triangle_active > 0.01)
        .count();
    let exp_on_frames = all_frames.iter().filter(|f| f.exp_rms > 0.001).count();

    println!(
        "\n=== Channel Activity Summary ({} frames) ===",
        total_frames
    );
    println!(
        "Pulse 1:   {:3}/{} frames active ({:.1}%)",
        p1_on_frames,
        total_frames,
        p1_on_frames as f64 / total_frames as f64 * 100.0
    );
    println!(
        "Pulse 2:   {:3}/{} frames active ({:.1}%)",
        p2_on_frames,
        total_frames,
        p2_on_frames as f64 / total_frames as f64 * 100.0
    );
    println!(
        "Triangle:  {:3}/{} frames active ({:.1}%)",
        tri_on_frames,
        total_frames,
        tri_on_frames as f64 / total_frames as f64 * 100.0
    );
    println!(
        "Noise:     {:3}/{} frames active ({:.1}%)",
        noise_on_frames,
        total_frames,
        noise_on_frames as f64 / total_frames as f64 * 100.0
    );
    println!(
        "Expansion: {:3}/{} frames active ({:.1}%)",
        exp_on_frames,
        total_frames,
        exp_on_frames as f64 / total_frames as f64 * 100.0
    );

    // Noise-active frames: show period distribution
    let mut period_counts: std::collections::HashMap<u16, u32> = std::collections::HashMap::new();
    let mut env_const_count = 0u32;
    let mut env_env_count = 0u32;
    for f in all_frames.iter().filter(|f| f.noise_duty > 0.01) {
        *period_counts.entry(f.noise_period).or_insert(0) += 1;
        if f.noise_envelope_disable {
            env_const_count += 1;
        } else {
            env_env_count += 1;
        }
    }
    let mut periods: Vec<_> = period_counts.into_iter().collect();
    periods.sort_by_key(|&(p, _)| p);
    println!("\nNoise period distribution (when active):");
    for (p, c) in &periods {
        println!("  period {:5}: {} frames", p, c);
    }
    println!(
        "Noise volume mode: const={}, envelope={}",
        env_const_count, env_env_count
    );

    // Average noise volume when active
    let active_vols: Vec<f64> = all_frames
        .iter()
        .filter(|f| f.noise_duty > 0.01)
        .map(|f| f.noise_vol_avg)
        .collect();
    if !active_vols.is_empty() {
        let avg_vol: f64 = active_vols.iter().sum::<f64>() / active_vols.len() as f64;
        let max_vol = active_vols.iter().cloned().fold(0.0f64, f64::max);
        println!(
            "Noise avg volume (when active): {:.1}, max: {:.1}",
            avg_vol, max_vol
        );
    }
}
