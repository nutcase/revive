use std::error::Error;
use std::io;

use revive_core::CoreInstance;
use sdl2::audio::{AudioQueue, AudioSpecDesired};

pub(crate) fn open_audio_queue(
    sdl: &sdl2::Sdl,
    core: &mut CoreInstance,
) -> Result<AudioQueue<i16>, Box<dyn Error>> {
    let audio = sdl.audio().map_err(sdl_error)?;
    let spec = core.audio_spec();
    let desired = AudioSpecDesired {
        freq: Some(spec.sample_rate_hz as i32),
        channels: Some(spec.channels),
        samples: Some(1024),
    };
    let queue = audio
        .open_queue::<i16, _>(None, &desired)
        .map_err(|err| io::Error::other(err.to_string()))?;
    let obtained = queue.spec();
    core.configure_audio_output(obtained.freq.max(8_000) as u32);
    queue.resume();
    println!(
        "Audio       : {} Hz, {} ch",
        obtained.freq, obtained.channels
    );
    Ok(queue)
}

pub(crate) fn feed_audio(
    queue: &mut AudioQueue<i16>,
    core: &mut CoreInstance,
    scratch: &mut Vec<i16>,
) -> Result<(), Box<dyn Error>> {
    let spec = queue.spec();
    let channels = usize::from(spec.channels.max(1));
    let queued_i16 = queue.size() as usize / std::mem::size_of::<i16>();
    let queued_frames = queued_i16 / channels;
    let target_frames = ((spec.freq.max(8_000) as usize) / 30).clamp(512, 2048);

    core.drain_audio_i16(scratch);
    if queued_frames < target_frames && !scratch.is_empty() {
        queue
            .queue_audio(scratch)
            .map_err(|err| io::Error::other(err.to_string()))?;
    }
    Ok(())
}

fn sdl_error(message: String) -> io::Error {
    io::Error::other(message)
}
