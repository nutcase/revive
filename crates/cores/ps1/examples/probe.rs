//! Headless HLE smoke test. No firmware or game data is included.
//! probe DISC FRAMES OUTPUT_DIR [STATE|-] [FRAME:BUTTON:0|1 ...]
//! Button ids follow libretro (0=Cross, 3=Start, 8=Circle, 4..7=directions).
use std::{io::Write, path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 4 {
        return Err("probe DISC FRAMES OUTPUT_DIR [STATE|-] [FRAME:BUTTON:0|1 ...]".into());
    }
    let path = std::fs::canonicalize(&args[1])?;
    let frames: usize = args[2].parse()?;
    let output = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&output)?;
    let mut core = ps1_core::Emulator::load(&path, &std::fs::canonicalize(&output)?)?;
    if let Some(state) = args.get(4).filter(|s| s.as_str() != "-") {
        core.load_state(&std::fs::read(state)?)?;
    }
    let mut events = Vec::new();
    for event in args.iter().skip(5) {
        let fields: Vec<_> = event.split(':').collect();
        if fields.len() != 3 {
            return Err("Expected FRAME:BUTTON:0|1".into());
        }
        events.push((
            fields[0].parse::<usize>()?,
            fields[1].parse::<u32>()?,
            fields[2] == "1",
        ));
    }
    let start = Instant::now();
    let mut samples = Vec::new();
    let mut audio_samples = 0;
    let mut nonzero_audio = 0;
    for frame in 0..frames {
        for &(_, id, down) in events.iter().filter(|e| e.0 == frame) {
            core.set_button(0, id, down);
        }
        core.step_frame()?;
        core.drain_audio(&mut samples);
        audio_samples += samples.len();
        nonzero_audio += samples.iter().filter(|&&s| s != 0).count();
        if (frame + 1) % 300 == 0 || frame + 1 == frames {
            let (data, w, h) = core.frame();
            let mut file =
                std::fs::File::create(output.join(format!("frame-{:05}.ppm", frame + 1)))?;
            write!(file, "P6\n{w} {h}\n255\n")?;
            for pixel in data.chunks_exact(4) {
                file.write_all(&pixel[..3])?;
            }
            println!(
                "frame={} geometry={}x{} elapsed={:.2}s",
                frame + 1,
                w,
                h,
                start.elapsed().as_secs_f64()
            );
        }
    }
    std::fs::write(output.join("state.psst"), core.save_state()?)?;
    std::fs::write(output.join("memory-card.mcd"), core.memory_card())?;
    println!("HLE frames={frames} seconds={:.3} native_fps={:.5} samples={audio_samples} nonzero_audio={nonzero_audio}", start.elapsed().as_secs_f64(), core.frame_rate_hz());
    Ok(())
}
