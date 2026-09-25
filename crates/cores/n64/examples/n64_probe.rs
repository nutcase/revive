//! Opt-in local-cartridge integration and timing probe; never bundles game data.
use std::{io::Write, path::PathBuf, time::Instant};
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("usage: n64_probe ROM [frames] [output-directory]")?,
    );
    let frames: usize = args
        .next()
        .as_deref()
        .unwrap_or("3600")
        .parse()
        .map_err(|_| "Invalid frame count")?;
    if frames == 0 {
        return Err("Frame count must be greater than zero".into());
    }
    let output = args.next().map(PathBuf::from);
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let rom = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut core = n64_core::Emulator::load(rom.clone(), temp.path())?;
    assert!(n64_core::Emulator::load(rom.clone(), temp.path()).is_err());
    let initial_save = core.persistent_save().to_vec();
    let fps = core.frame_rate_hz();
    let start = Instant::now();
    let mut audio = vec![];
    let mut nonzero_audio = 0usize;
    let mut timings = Vec::new();
    for frame in 0..frames {
        core.set_button(0, 3, (900..910).contains(&frame));
        core.set_button(
            0,
            0,
            (1200..1210).contains(&frame)
                || (1500..1510).contains(&frame)
                || (frame >= 4000 && frame % 300 < 10),
        );
        core.set_stick(
            0,
            0,
            if (6000..8500).contains(&frame) {
                -32767
            } else {
                0
            },
        );
        let tick = Instant::now();
        core.step_frame()?;
        timings.push(tick.elapsed().as_secs_f64() * 1000.0);
        core.drain_audio(&mut audio);
        nonzero_audio += audio.iter().filter(|&&v| v != 0).count();
        if (frame + 1) % 600 == 0 {
            let (rgba, w, h) = core.frame();
            println!(
                "frame={} size={}x{} elapsed={:.3}s nonzero_audio={}",
                frame + 1,
                w,
                h,
                start.elapsed().as_secs_f64(),
                nonzero_audio
            );
            if let Some(dir) = &output {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                let mut file = std::fs::File::create(dir.join(format!("frame-{}.ppm", frame + 1)))
                    .map_err(|e| e.to_string())?;
                write!(file, "P6\n{w} {h}\n255\n").map_err(|e| e.to_string())?;
                for p in rgba.as_chunks::<4>().0.iter() {
                    file.write_all(&p[..3]).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    assert!(nonzero_audio > 0);
    let (rgba, _, _) = core.frame();
    assert!(rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[0] != 0 || p[1] != 0 || p[2] != 0));
    println!(
        "game-written persistent bytes={}",
        initial_save
            .iter()
            .zip(core.persistent_save())
            .filter(|(a, b)| a != b)
            .count()
    );
    // Verify guest byte addressing and state restoration without advancing the CPU.
    let offset = 0x700000;
    let original = core.ram()[offset..offset + 4].to_vec();
    for (i, v) in [0x12, 0x34, 0x56, 0x78].into_iter().enumerate() {
        assert!(core.write_ram(offset + i, v));
    }
    let state = core.save_state()?;
    assert_eq!(&core.ram()[offset..offset + 4], &[0x12, 0x34, 0x56, 0x78]);
    core.write_ram(offset, 0xFF);
    core.load_state(&state)?;
    assert_eq!(core.ram()[offset], 0x12);
    let mut corrupt = state.clone();
    corrupt[100] ^= 1;
    assert!(core.load_state(&corrupt).is_err());
    assert!(core.load_state(&state[..100]).is_err());
    for (i, v) in original.into_iter().enumerate() {
        core.write_ram(offset + i, v);
    }
    let mut save = core.persistent_save().to_vec();
    save[0] ^= 0x5A;
    core.load_persistent_save(&save)?;
    core.set_audio_enabled(false);
    core.step_frame()?;
    core.drain_audio(&mut audio);
    assert!(audio.is_empty());
    drop(core);
    let mut reopened = n64_core::Emulator::load(rom, temp.path())?;
    reopened.load_persistent_save(&save)?;
    assert_eq!(reopened.persistent_save(), save);
    reopened.step_frame()?;
    assert!(timings.iter().all(|v| v.is_finite() && *v >= 0.0));
    timings.sort_by(f64::total_cmp);
    println!(
        "PASS state/RAM/checksum/save/reopen/audio; target={fps:.2}Hz mean={:.3}ms p95={:.3}ms",
        timings.iter().sum::<f64>() / timings.len() as f64,
        timings[timings.len() * 95 / 100]
    );
    Ok(())
}
