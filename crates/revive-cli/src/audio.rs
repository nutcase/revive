use std::error::Error;
use std::io;

use revive_core::CoreInstance;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};

pub(crate) struct AudioOutput {
    stream: AudioStreamOwner,
    sample_rate_hz: u32,
    channels: usize,
}

impl AudioOutput {
    pub(crate) fn clear(&self) {
        let _ = self.stream.clear();
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
    stream
        .resume()
        .map_err(|err| io::Error::other(err.to_string()))?;
    println!(
        "Audio       : {} Hz, {} ch",
        obtained_freq_hz, obtained_channels
    );

    Ok(AudioOutput {
        stream,
        sample_rate_hz: obtained_freq_hz,
        channels: obtained_channels,
    })
}

pub(crate) fn feed_audio(output: &AudioOutput, core: &mut CoreInstance, scratch: &mut Vec<i16>) {
    let target_frames = ((output.sample_rate_hz as usize) / 32).clamp(512, 2048);
    let max_buffered_bytes = target_frames
        .saturating_mul(3)
        .saturating_mul(output.channels)
        .saturating_mul(std::mem::size_of::<i16>());

    if output
        .stream
        .queued_bytes()
        .ok()
        .is_some_and(|queued| queued as usize > max_buffered_bytes)
    {
        let _ = output.stream.clear();
    }

    core.drain_audio_i16(scratch);
    if !scratch.is_empty() {
        let _ = output.stream.put_data_i16(scratch);
        scratch.clear();
    }
}

fn sdl_error(message: sdl3::Error) -> io::Error {
    io::Error::other(message.to_string())
}
