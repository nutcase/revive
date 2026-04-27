use std::collections::VecDeque;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};

use revive_core::CoreInstance;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

const DEVICE_BUFFER_SAMPLES: u16 = 512;

struct SharedAudioBuffer {
    samples: VecDeque<i16>,
}

impl SharedAudioBuffer {
    fn push_interleaved(&mut self, samples: &[i16]) {
        self.samples.extend(samples.iter().copied());
    }

    fn trim_oldest(&mut self, max_samples: usize) {
        while self.samples.len() > max_samples {
            self.samples.pop_front();
        }
    }
}

struct PlaybackCallback {
    shared: Arc<Mutex<SharedAudioBuffer>>,
    channels: usize,
    last_frame: Vec<i16>,
}

impl AudioCallback for PlaybackCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        if self.last_frame.len() != self.channels {
            self.last_frame.resize(self.channels, 0);
        }

        if let Ok(mut shared) = self.shared.try_lock() {
            for frame in out.chunks_exact_mut(self.channels) {
                let mut filled = true;
                for sample in frame.iter_mut() {
                    if let Some(value) = shared.samples.pop_front() {
                        *sample = value;
                    } else {
                        filled = false;
                        break;
                    }
                }

                if filled {
                    self.last_frame[..self.channels].copy_from_slice(frame);
                } else {
                    frame.copy_from_slice(&self.last_frame[..self.channels]);
                }
            }
            return;
        }

        for frame in out.chunks_exact_mut(self.channels) {
            frame.copy_from_slice(&self.last_frame[..self.channels]);
        }
    }
}

pub(crate) struct AudioOutput {
    device: AudioDevice<PlaybackCallback>,
    shared: Arc<Mutex<SharedAudioBuffer>>,
    sample_rate_hz: u32,
    channels: usize,
}

impl AudioOutput {
    fn push_interleaved(&self, samples: &[i16], max_buffered_frames: usize) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.push_interleaved(samples);
            shared.trim_oldest(max_buffered_frames.saturating_mul(self.channels));
        }
    }
}

pub(crate) fn open_audio_output(
    sdl: &sdl2::Sdl,
    core: &mut CoreInstance,
) -> Result<AudioOutput, Box<dyn Error>> {
    let audio = sdl.audio().map_err(sdl_error)?;
    let spec = core.audio_spec();
    let desired = AudioSpecDesired {
        freq: Some(spec.sample_rate_hz as i32),
        channels: Some(spec.channels),
        samples: Some(DEVICE_BUFFER_SAMPLES),
    };

    let shared = Arc::new(Mutex::new(SharedAudioBuffer {
        samples: VecDeque::new(),
    }));
    let callback_shared = Arc::clone(&shared);
    let device = audio
        .open_playback(None, &desired, move |obtained| PlaybackCallback {
            shared: Arc::clone(&callback_shared),
            channels: usize::from(obtained.channels.max(1)),
            last_frame: vec![0; usize::from(obtained.channels.max(1))],
        })
        .map_err(|err| io::Error::other(err.to_string()))?;

    let obtained = device.spec();
    let obtained_freq_hz = obtained.freq.max(8_000) as u32;
    let obtained_channels = usize::from(obtained.channels.max(1));
    core.configure_audio_output(obtained_freq_hz);
    device.resume();
    println!(
        "Audio       : {} Hz, {} ch",
        obtained.freq, obtained.channels
    );

    Ok(AudioOutput {
        device,
        shared,
        sample_rate_hz: obtained_freq_hz,
        channels: obtained_channels,
    })
}

pub(crate) fn feed_audio(output: &AudioOutput, core: &mut CoreInstance, scratch: &mut Vec<i16>) {
    let _keep_device_alive = &output.device;
    let target_frames = ((output.sample_rate_hz as usize) / 32).clamp(512, 2048);
    let max_buffered_frames = target_frames.saturating_mul(3);

    core.drain_audio_i16(scratch);
    if !scratch.is_empty() {
        output.push_interleaved(scratch, max_buffered_frames);
    }
}

fn sdl_error(message: String) -> io::Error {
    io::Error::other(message)
}

#[cfg(test)]
mod tests {
    use super::SharedAudioBuffer;
    use std::collections::VecDeque;

    #[test]
    fn trim_oldest_keeps_newly_pushed_samples() {
        let mut buffer = SharedAudioBuffer {
            samples: VecDeque::from(vec![0, 0, 0, 0]),
        };

        buffer.push_interleaved(&[10, 11, 12, 13]);
        buffer.trim_oldest(4);

        assert_eq!(
            buffer.samples.into_iter().collect::<Vec<_>>(),
            vec![10, 11, 12, 13]
        );
    }
}
