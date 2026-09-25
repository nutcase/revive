use std::error::Error;
use std::io;

use revive_core::CoreInstance;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};

pub(crate) struct AudioOutput {
    stream: AudioStreamOwner,
    sample_rate_hz: u32,
    channels: usize,
    playing: bool,
    underruns: usize,
    source_gaps: usize,
    refill_ms: usize,
    debug: bool,
}

impl AudioOutput {
    pub(crate) fn clear(&mut self) {
        let _ = self.stream.pause();
        let _ = self.stream.clear();
        self.playing = false;
    }
}

pub(crate) fn open_audio_output(
    sdl: &sdl3::Sdl,
    core: &mut CoreInstance,
) -> Result<AudioOutput, Box<dyn Error>> {
    let audio = sdl.audio().map_err(sdl_error)?;
    let spec = core.audio_spec();
    let desired = AudioSpec {
        freq: Some(spec.sample_rate_hz as i32),
        channels: Some(spec.channels as i32),
        format: Some(AudioFormat::s16_sys()),
    };
    let device = audio
        .open_playback_device(&desired)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let stream = device
        .open_device_stream(Some(&desired))
        .map_err(|err| io::Error::other(err.to_string()))?;
    let (_, output_spec) = stream
        .get_format()
        .map_err(|err| io::Error::other(err.to_string()))?;
    let output_spec = output_spec.unwrap_or(desired);
    let obtained_freq_hz = output_spec
        .freq
        .unwrap_or(spec.sample_rate_hz as i32)
        .max(8_000) as u32;
    let obtained_channels = output_spec.channels.unwrap_or(spec.channels as i32).max(1) as usize;
    core.configure_audio_output(obtained_freq_hz);
    // N64 delivers audio in DMA bursts, not one fixed block per video frame.
    // Start paused so the first callback cannot consume an empty stream.
    stream
        .pause()
        .map_err(|err| io::Error::other(err.to_string()))?;
    println!(
        "Audio       : {} Hz, {} ch",
        obtained_freq_hz, obtained_channels
    );

    Ok(AudioOutput {
        stream,
        sample_rate_hz: obtained_freq_hz,
        channels: obtained_channels,
        playing: false,
        underruns: 0,
        source_gaps: 0,
        refill_ms: 50,
        debug: std::env::var_os("REVIVE_AUDIO_DEBUG").is_some(),
    })
}

pub(crate) fn feed_audio(
    output: &mut AudioOutput,
    core: &mut CoreInstance,
    scratch: &mut Vec<i16>,
) {
    core.drain_audio_i16(scratch);
    output.feed(scratch);
    scratch.clear();
}

impl AudioOutput {
    fn feed(&mut self, samples: &[i16]) {
        let bytes_per_second = self.sample_rate_hz as usize * self.channels * 2;
        let queued = self.stream.queued_bytes().unwrap_or(0) as usize;
        if self.playing && queued == 0 {
            if samples.is_empty() {
                // Games can stop the AI DMA entirely during scene transitions.
                self.source_gaps += 1;
            } else {
                self.underruns += 1;
                self.refill_ms = (self.refill_ms + 25).min(100);
                if self.debug {
                    eprintln!(
                        "Audio buffer empty #{}; refilling {} ms",
                        self.underruns, self.refill_ms
                    );
                }
            }
            // Preserve the stream's resampler history across an empty queue.
            let _ = self.stream.pause();
            self.playing = false;
        } else if queued > bytes_per_second / 4 {
            // Recover from a stalled device without leaving seconds of stale audio.
            self.clear();
        }
        if !samples.is_empty() {
            if let Err(err) = self.stream.put_data_i16(samples) {
                eprintln!("Audio queue failed: {err}");
                return;
            }
        }
        // Start with 50 ms for DMA bursts; increase to at most 100 ms after
        // scheduling stalls. Refill after pause/underrun instead of repeatedly
        // playing tiny fragments.
        if !self.playing
            && self.stream.queued_bytes().unwrap_or(0) as usize
                >= bytes_per_second * self.refill_ms / 1000
        {
            match self.stream.resume() {
                Ok(()) => self.playing = true,
                Err(err) => eprintln!("Audio resume failed: {err}"),
            }
        }
    }
}

fn sdl_error(message: sdl3::Error) -> io::Error {
    io::Error::other(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore = "plays a local ROM through the real SDL audio device; set REVIVE_AUDIO_TEST_ROM"]
    fn local_rom_audio_delivery() {
        let path =
            std::fs::canonicalize(std::env::var_os("REVIVE_AUDIO_TEST_ROM").unwrap()).unwrap();
        // Run this opt-in test alone: the temporary working directory isolates saves.
        let old_dir = std::env::current_dir().unwrap();
        let save_dir =
            std::env::temp_dir().join(format!("revive-audio-test-{}", std::process::id()));
        std::fs::create_dir_all(&save_dir).unwrap();
        std::env::set_current_dir(&save_dir).unwrap();
        let sdl = sdl3::init().unwrap();
        let mut core = CoreInstance::load_rom(&path, None).unwrap();
        let mut output = open_audio_output(&sdl, &mut core).unwrap();
        // Startup and explicit pause must buffer, even when samples are ready.
        assert!(output.stream.device_paused().unwrap());
        output.feed(&vec![
            0;
            output.sample_rate_hz as usize / 100 * output.channels
        ]);
        assert!(!output.playing);
        assert!(output.stream.device_paused().unwrap());
        output.clear();
        assert_eq!(output.stream.queued_bytes().unwrap(), 0);
        let mut clock = crate::frame_clock::FrameClock::with_rate(core.frame_rate_hz());
        let mut scratch = vec![];
        let mut empty = 0;
        let mut delivered_samples = 0usize;
        let frames: usize = std::env::var("REVIVE_AUDIO_TEST_FRAMES")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(900);
        let mut max_core_ms = 0.0f64;
        let start = Instant::now();
        for frame in 0..frames {
            let tick = Instant::now();
            core.step_frame().unwrap();
            max_core_ms = max_core_ms.max(tick.elapsed().as_secs_f64() * 1000.0);
            let queued = output.stream.queued_bytes().unwrap();
            if queued == 0 {
                empty += 1;
            }
            if frame % 60 == 0 {
                println!("audio frame={frame} queued={queued} empty={empty} max_core_ms={max_core_ms:.2} elapsed={:.3}", start.elapsed().as_secs_f64());
            }
            core.drain_audio_i16(&mut scratch);
            delivered_samples += scratch.len();
            if queued == 0 || scratch.is_empty() {
                println!(
                    "audio gap frame={frame} samples={} playing={} core_ms={:.2}",
                    scratch.len(),
                    output.playing,
                    tick.elapsed().as_secs_f64() * 1000.0
                );
            }
            output.feed(&scratch);
            scratch.clear();
            clock.wait();
        }
        println!(
            "audio delivery: empty={empty}/{frames} underruns={} source_gaps={} refill_ms={} max_core_ms={max_core_ms:.2}",
            output.underruns, output.source_gaps, output.refill_ms
        );
        assert!(delivered_samples > 0);
        drop(core);
        std::env::set_current_dir(old_dir).unwrap();
        std::fs::remove_dir_all(save_dir).unwrap();
    }
}
