#![cfg_attr(not(feature = "dev"), allow(dead_code))]

pub mod apu;
pub mod spc;

use rodio::{OutputStream, Sink, Source};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEFAULT_AUDIO_CHUNK_FRAMES: usize = 256;
const DEFAULT_AUDIO_BUFFER_FRAMES: usize = 32768;
const DEFAULT_AUDIO_TARGET_BUFFER_FRAMES: usize = 2048;
const DEFAULT_AUDIO_SAMPLE_RATE: u32 = 32000;
const DEFAULT_AUDIO_VOLUME: f32 = 0.7;

#[derive(Clone, Copy)]
struct AudioConfig {
    buffer_frames: usize,
    target_buffer_frames: usize,
    chunk_frames: usize,
    sample_rate: u32,
    default_volume: f32,
}

impl AudioConfig {
    fn from_env() -> Self {
        let buffer_frames =
            Self::read_usize_env("AUDIO_BUFFER_FRAMES", DEFAULT_AUDIO_BUFFER_FRAMES);
        let chunk_frames = Self::read_usize_env("AUDIO_CHUNK_FRAMES", DEFAULT_AUDIO_CHUNK_FRAMES);
        let target_buffer_frames = Self::read_usize_env(
            "AUDIO_TARGET_BUFFER_FRAMES",
            DEFAULT_AUDIO_TARGET_BUFFER_FRAMES,
        )
        .max(chunk_frames)
        .min(buffer_frames);
        Self {
            buffer_frames,
            target_buffer_frames,
            chunk_frames,
            sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
            default_volume: DEFAULT_AUDIO_VOLUME,
        }
    }

    fn read_usize_env(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(default)
    }
}

struct AudioRing {
    buffer: VecDeque<(i16, i16)>,
    max_frames: usize,
}

impl AudioRing {
    fn new(max_frames: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_frames),
            max_frames,
        }
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn clear(&mut self) {
        self.buffer.clear();
    }

    fn push_samples(&mut self, samples: &[(i16, i16)], target_frames: usize) {
        if samples.is_empty() {
            return;
        }
        let target_frames = target_frames.min(self.max_frames).max(samples.len());
        let needed = self.buffer.len().saturating_add(samples.len());
        if needed > target_frames {
            let drop = needed - target_frames;
            for _ in 0..drop {
                self.buffer.pop_front();
            }
        }
        self.buffer.extend(samples.iter().copied());
    }

    fn pop_into(&mut self, out: &mut Vec<(i16, i16)>, max: usize) -> usize {
        out.clear();
        let count = self.buffer.len().min(max);
        out.reserve(count);
        for _ in 0..count {
            if let Some(v) = self.buffer.pop_front() {
                out.push(v);
            }
        }
        count
    }
}

pub struct SnesAudioSource {
    ring: Arc<Mutex<AudioRing>>,
    sample_rate: u32,
    channels: u16,
    current_frame: Vec<(i16, i16)>,
    last_sample: (i16, i16),
    // Interleaved sample cursor (L,R,L,R...) within current_frame.
    // current_frame holds stereo frames; cursor counts i16 samples.
    sample_cursor: usize,
    chunk_frames: usize,
}

impl SnesAudioSource {
    fn new(ring: Arc<Mutex<AudioRing>>, sample_rate: u32, chunk_frames: usize) -> Self {
        let channels = 2; // Stereo

        Self {
            ring,
            sample_rate,
            channels,
            current_frame: Vec::with_capacity(chunk_frames),
            last_sample: (0, 0),
            sample_cursor: 0,
            chunk_frames: chunk_frames.max(1),
        }
    }

    fn generate_audio_frame(&mut self) {
        let mut got = 0usize;
        if let Ok(mut ring) = self.ring.lock() {
            got = ring.pop_into(&mut self.current_frame, self.chunk_frames);
        }

        if got > 0 {
            self.last_sample = self.current_frame[got - 1];
        }
        if got < self.chunk_frames {
            self.current_frame
                .resize(self.chunk_frames, self.last_sample);
        }
        self.sample_cursor = 0;
    }
}

impl Iterator for SnesAudioSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        // Generate new frame if we've consumed the current one
        if self.sample_cursor >= self.current_frame.len().saturating_mul(2) {
            self.generate_audio_frame();
        }

        let is_right_channel = self.sample_cursor % 2 == 1;

        if self.current_frame.is_empty() {
            return Some(if is_right_channel {
                self.last_sample.1
            } else {
                self.last_sample.0
            });
        }

        let sample_index = self.sample_cursor / 2;

        if sample_index >= self.current_frame.len() {
            return Some(if is_right_channel {
                self.last_sample.1
            } else {
                self.last_sample.0
            });
        }

        let sample = if is_right_channel {
            self.current_frame[sample_index].1
        } else {
            self.current_frame[sample_index].0
        };

        self.sample_cursor += 1;
        Some(sample)
    }
}

impl Source for SnesAudioSource {
    fn current_frame_len(&self) -> Option<usize> {
        None // Infinite source
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None // Infinite duration
    }
}

pub struct SnesAudioCallbackSource {
    ring: Arc<Mutex<AudioRing>>,
    current_frame: Vec<(i16, i16)>,
    last_sample: (i16, i16),
    volume: Arc<Mutex<f32>>,
}

impl SnesAudioCallbackSource {
    fn new(ring: Arc<Mutex<AudioRing>>, volume: Arc<Mutex<f32>>) -> Self {
        Self {
            ring,
            current_frame: Vec::new(),
            last_sample: (0, 0),
            volume,
        }
    }

    pub fn fill_interleaved_i16(&mut self, out: &mut [i16]) {
        let frame_count = out.len() / 2;
        let got = if let Ok(mut ring) = self.ring.lock() {
            ring.pop_into(&mut self.current_frame, frame_count)
        } else {
            self.current_frame.clear();
            0
        };
        let volume = self
            .volume
            .lock()
            .map(|v| *v)
            .unwrap_or(DEFAULT_AUDIO_VOLUME);

        for frame in 0..frame_count {
            let sample = if frame < got {
                self.current_frame[frame]
            } else {
                self.last_sample
            };
            if frame < got {
                self.last_sample = sample;
            }

            let offset = frame * 2;
            out[offset] = scale_i16(sample.0, volume);
            out[offset + 1] = scale_i16(sample.1, volume);
        }

        if out.len() % 2 == 1 {
            if let Some(last) = out.last_mut() {
                *last = 0;
            }
        }
    }
}

fn scale_i16(sample: i16, volume: f32) -> i16 {
    (sample as f32 * volume.clamp(0.0, 1.0)).clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

pub struct AudioSystem {
    // In headless/silent mode these are None to avoid device init
    _output_stream: Option<OutputStream>,
    sink: Option<Sink>,
    apu_handle: Arc<Mutex<crate::audio::apu::Apu>>,
    ring: Arc<Mutex<AudioRing>>,
    enabled: bool,
    volume: f32,
    volume_shared: Arc<Mutex<f32>>,
    sample_rate: u32,
    frame_sample_rem_acc: u64,
    frame_scratch: Vec<(i16, i16)>,
    target_buffer_frames: usize,
    chunk_frames: usize,
    rodio_output_enabled: bool,
}

impl AudioSystem {
    pub fn new() -> Result<Self, String> {
        let (output_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| format!("Failed to create audio output stream: {}", e))?;

        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;
        let config = AudioConfig::from_env();

        Ok(Self::from_config(
            config,
            Some(output_stream),
            Some(sink),
            true,
            config.default_volume,
            true,
        ))
    }

    // Construct a silent audio system that does not touch the host audio device.
    // Used for HEADLESS runs and environments without audio.
    pub fn new_silent() -> Self {
        let config = AudioConfig::from_env();
        Self::from_config(config, None, None, false, 0.0, false)
    }

    // Construct an enabled audio system whose samples are consumed by an
    // external host backend, such as SDL AudioCallback.
    pub fn new_external_output() -> Self {
        let config = AudioConfig::from_env();
        Self::from_config(config, None, None, true, config.default_volume, false)
    }

    fn from_config(
        config: AudioConfig,
        output_stream: Option<OutputStream>,
        sink: Option<Sink>,
        enabled: bool,
        volume: f32,
        rodio_output_enabled: bool,
    ) -> Self {
        // Create a dummy APU for now - will be replaced when emulator starts
        let apu = Arc::new(Mutex::new(crate::audio::apu::Apu::new()));
        let ring = Arc::new(Mutex::new(AudioRing::new(config.buffer_frames)));
        let volume = volume.clamp(0.0, 1.0);
        Self {
            _output_stream: output_stream,
            sink,
            apu_handle: apu,
            ring,
            enabled,
            volume,
            volume_shared: Arc::new(Mutex::new(volume)),
            sample_rate: config.sample_rate,
            frame_sample_rem_acc: 0,
            frame_scratch: Vec::new(),
            target_buffer_frames: config.target_buffer_frames,
            chunk_frames: config.chunk_frames,
            rodio_output_enabled,
        }
    }

    pub fn set_apu(&mut self, apu: Arc<Mutex<crate::audio::apu::Apu>>) {
        self.apu_handle = apu.clone();
        if let Ok(mut ring) = self.ring.lock() {
            ring.clear();
        }
        self.frame_sample_rem_acc = 0;

        if self.enabled && self.rodio_output_enabled {
            self.restart_audio();
        }
    }

    pub fn start(&mut self) {
        if self.enabled && self.rodio_output_enabled {
            self.restart_audio();
        }
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if let Some(s) = &self.sink {
            s.stop();
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled && self.rodio_output_enabled {
            self.restart_audio();
        } else if !enabled {
            if let Some(s) = &self.sink {
                s.pause();
            }
            self.clear_buffer();
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Ok(mut shared) = self.volume_shared.lock() {
            *shared = self.volume;
        }
        if let Some(s) = &self.sink {
            s.set_volume(self.volume);
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.volume
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn callback_source(&self) -> SnesAudioCallbackSource {
        SnesAudioCallbackSource::new(self.ring.clone(), self.volume_shared.clone())
    }

    pub fn mix_frame_from_apu(&mut self, apu: &mut crate::audio::apu::Apu) {
        self.mix_frame_from_apu_with_output(apu, true);
    }

    pub fn drain_frame_from_apu(&mut self, apu: &mut crate::audio::apu::Apu) {
        self.mix_frame_from_apu_with_output(apu, false);
    }

    fn mix_frame_from_apu_with_output(
        &mut self,
        apu: &mut crate::audio::apu::Apu,
        emit_output: bool,
    ) {
        if !self.enabled {
            return;
        }
        let sample_rate_u32 = apu.get_sample_rate();
        let sample_rate = sample_rate_u32 as u64;
        // Match emulator timing (NTSC 341*262 dots, 4 master cycles per dot).
        const MASTER_CLOCK_NTSC: u64 = 21_477_272;
        const CYCLES_PER_FRAME: u64 = 341 * 262 * 4;
        let numerator = sample_rate
            .saturating_mul(CYCLES_PER_FRAME)
            .saturating_add(self.frame_sample_rem_acc);
        let frame_size = (numerator / MASTER_CLOCK_NTSC) as usize;
        self.frame_sample_rem_acc = numerator % MASTER_CLOCK_NTSC;
        if frame_size == 0 {
            return;
        }

        if self.frame_scratch.len() < frame_size {
            self.frame_scratch.resize(frame_size, (0, 0));
        }
        let scratch = &mut self.frame_scratch[..frame_size];
        apu.generate_audio_samples(scratch);
        if emit_output {
            if let Ok(mut ring) = self.ring.lock() {
                ring.push_samples(scratch, self.target_buffer_frames);
            }
        }
        self.sample_rate = sample_rate_u32;
    }

    pub fn clear_buffer(&mut self) {
        if let Ok(mut ring) = self.ring.lock() {
            ring.clear();
        }
        if let Ok(mut apu) = self.apu_handle.lock() {
            apu.clear_audio_output_buffer();
        }
        self.frame_sample_rem_acc = 0;
        if self.enabled && self.rodio_output_enabled && self.sink.is_some() {
            self.restart_audio();
        }
    }

    fn restart_audio(&mut self) {
        if !self.rodio_output_enabled {
            return;
        }

        // Ensure we have a stream/sink; create if missing
        if self._output_stream.is_none() || self.sink.is_none() {
            if let Ok((output_stream, stream_handle)) = OutputStream::try_default() {
                if let Ok(sink) = Sink::try_new(&stream_handle) {
                    self._output_stream = Some(output_stream);
                    self.sink = Some(sink);
                } else {
                    // Could not create sink; keep silent
                    return;
                }
            } else {
                // Could not create output stream; keep silent
                return;
            }
        }

        if let Some(s) = &self.sink {
            s.stop();
        }

        if let Some(s) = &self.sink {
            if let Ok(mut ring) = self.ring.lock() {
                ring.clear();
            }
            let audio_source =
                SnesAudioSource::new(self.ring.clone(), self.sample_rate, self.chunk_frames);
            s.append(audio_source);
            s.set_volume(self.volume);
            s.play();
        }
    }
}

impl Drop for AudioSystem {
    fn drop(&mut self) {
        if let Some(s) = &self.sink {
            s.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_audio_system() -> AudioSystem {
        let config = AudioConfig {
            buffer_frames: 2048,
            target_buffer_frames: 1024,
            chunk_frames: DEFAULT_AUDIO_CHUNK_FRAMES,
            sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
            default_volume: 0.0,
        };
        AudioSystem::from_config(config, None, None, true, 0.0, false)
    }

    fn queued_frames(audio: &AudioSystem) -> usize {
        audio.ring.lock().unwrap().len()
    }

    #[test]
    fn clear_buffer_discards_queued_audio() {
        let mut audio = test_audio_system();
        let mut apu = crate::audio::apu::Apu::new();

        audio.mix_frame_from_apu(&mut apu);
        assert!(queued_frames(&audio) > 0);

        audio.clear_buffer();

        assert_eq!(queued_frames(&audio), 0);
    }

    #[test]
    fn mix_frame_caps_queued_audio_to_target_latency() {
        let mut audio = test_audio_system();
        let mut apu = crate::audio::apu::Apu::new();

        for _ in 0..10 {
            audio.mix_frame_from_apu(&mut apu);
        }

        assert!(queued_frames(&audio) <= audio.target_buffer_frames);
    }

    #[test]
    fn drain_frame_from_apu_does_not_queue_audio() {
        let mut audio = test_audio_system();
        let mut apu = crate::audio::apu::Apu::new();

        audio.drain_frame_from_apu(&mut apu);

        assert_eq!(queued_frames(&audio), 0);
    }

    #[test]
    fn callback_source_drains_ring_and_pads_underflow() {
        let mut audio = test_audio_system();
        audio.set_volume(0.5);
        audio
            .ring
            .lock()
            .unwrap()
            .push_samples(&[(1000, -1000), (2000, -2000)], audio.target_buffer_frames);

        let mut source = audio.callback_source();
        let mut out = [0i16; 6];

        source.fill_interleaved_i16(&mut out);

        assert_eq!(out, [500, -500, 1000, -1000, 1000, -1000]);
        assert_eq!(queued_frames(&audio), 0);
    }
}
