use super::dsp::SAMPLE_RATE;
use super::dsp_helpers;

const WAV_DUMP_CAPACITY: usize = SAMPLE_RATE * 60 * 2;

pub(super) struct AudioDump {
    main: Option<Vec<i16>>,
    dry: Option<Vec<i16>>,
    echo: Option<Vec<i16>>,
    voice7: Option<Vec<i16>>,
}

impl AudioDump {
    pub(super) fn new_from_env() -> Self {
        let enabled = std::env::var_os("DUMP_AUDIO_WAV").is_some();
        let new_buffer = || {
            if enabled {
                Some(Vec::with_capacity(WAV_DUMP_CAPACITY))
            } else {
                None
            }
        };

        Self {
            main: new_buffer(),
            dry: new_buffer(),
            echo: new_buffer(),
            voice7: new_buffer(),
        }
    }

    pub(super) fn push_main(&mut self, left: i16, right: i16) {
        Self::push_pair(&mut self.main, left, right);
    }

    pub(super) fn push_dry(&mut self, left: i32, right: i32) {
        Self::push_pair(
            &mut self.dry,
            dsp_helpers::clamp(left) as i16,
            dsp_helpers::clamp(right) as i16,
        );
    }

    pub(super) fn push_echo(&mut self, left: i32, right: i32) {
        Self::push_pair(
            &mut self.echo,
            dsp_helpers::clamp(left) as i16,
            dsp_helpers::clamp(right) as i16,
        );
    }

    pub(super) fn push_voice7(&mut self, left: i32, right: i32) {
        Self::push_pair(
            &mut self.voice7,
            dsp_helpers::clamp(left) as i16,
            dsp_helpers::clamp(right) as i16,
        );
    }

    fn push_pair(buf: &mut Option<Vec<i16>>, left: i16, right: i16) {
        if let Some(buf) = buf {
            buf.push(left);
            buf.push(right);
        }
    }

    pub(super) fn write_all(&self) {
        let Some(main) = self.main.as_ref() else {
            return;
        };
        if main.is_empty() {
            eprintln!("[WAV] no samples to dump");
            return;
        }
        let Ok(path) = std::env::var("DUMP_AUDIO_WAV") else {
            return;
        };

        Self::write_wav(&path, main, 32000);
        let base = path.trim_end_matches(".wav");
        if let Some(ref dry) = self.dry {
            Self::write_wav(&format!("{}_dry.wav", base), dry, 32000);
        }
        if let Some(ref echo) = self.echo {
            Self::write_wav(&format!("{}_echo.wav", base), echo, 32000);
        }
        if let Some(ref voice7) = self.voice7 {
            Self::write_wav(&format!("{}_v7.wav", base), voice7, 32000);
        }
    }

    fn write_wav(path: &str, samples: &[i16], sample_rate: u32) {
        use std::io::Write;
        let num_channels: u16 = 2;
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
        let block_align = num_channels * bits_per_sample / 8;
        let data_size = (samples.len() * 2) as u32;
        let file_size = 36 + data_size;

        let mut f = match std::fs::File::create(path) {
            Ok(f) => std::io::BufWriter::new(f),
            Err(e) => {
                eprintln!("[WAV] failed to create {}: {}", path, e);
                return;
            }
        };
        let _ = f.write_all(b"RIFF");
        let _ = f.write_all(&file_size.to_le_bytes());
        let _ = f.write_all(b"WAVE");
        let _ = f.write_all(b"fmt ");
        let _ = f.write_all(&16u32.to_le_bytes()); // chunk size
        let _ = f.write_all(&1u16.to_le_bytes()); // PCM
        let _ = f.write_all(&num_channels.to_le_bytes());
        let _ = f.write_all(&sample_rate.to_le_bytes());
        let _ = f.write_all(&byte_rate.to_le_bytes());
        let _ = f.write_all(&block_align.to_le_bytes());
        let _ = f.write_all(&bits_per_sample.to_le_bytes());
        let _ = f.write_all(b"data");
        let _ = f.write_all(&data_size.to_le_bytes());
        for &s in samples {
            let _ = f.write_all(&s.to_le_bytes());
        }
        eprintln!(
            "[WAV] wrote {} samples ({:.1}s) to {}",
            samples.len() / 2,
            samples.len() as f64 / 2.0 / sample_rate as f64,
            path
        );
    }
}
