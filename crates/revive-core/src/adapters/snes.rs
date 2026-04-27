use std::path::{Path, PathBuf};

use snes_emulator::cartridge::Cartridge as SnesCartridge;
use snes_emulator::emulator::Emulator as SnesEmulator;

use super::common::{
    argb8888_u32_frame_as_bgra8888_bytes, fixed_audio_spec, load_state_slot, save_state_slot,
    write_byte,
};
use crate::paths::rom_stem;
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

pub struct SnesAdapter {
    emulator: SnesEmulator,
    rom_path: PathBuf,
    title: String,
    key_states: snes_emulator::input::KeyStates,
    audio_source: snes_emulator::audio::SnesAudioCallbackSource,
    audio_sample_rate_hz: u32,
    audio_output_sample_rate_hz: u32,
    native_audio_frame_remainder: u64,
    output_audio_frame_remainder: u64,
    audio_resample_buffer: Vec<i16>,
}

impl SnesAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_audio(path, true)
    }

    pub fn load_with_audio(path: &Path, audio_enabled: bool) -> Result<Self> {
        let cartridge = SnesCartridge::load_from_file(path).map_err(|err| err.to_string())?;
        let title = snes_display_title(&cartridge, path);
        let mut srm_path = path.to_path_buf();
        srm_path.set_extension("srm");

        let previous_audio_backend = std::env::var("AUDIO_BACKEND").ok();
        let previous_no_audio = std::env::var("NO_AUDIO").ok();
        let previous_apu_boot_overrides = clear_env_vars(APU_BOOT_OVERRIDE_ENV_VARS);
        if audio_enabled {
            std::env::set_var("AUDIO_BACKEND", "sdl_callback");
            std::env::remove_var("NO_AUDIO");
        } else {
            std::env::remove_var("AUDIO_BACKEND");
            std::env::set_var("NO_AUDIO", "1");
        }
        let emulator_result = SnesEmulator::new(cartridge, title.clone(), Some(srm_path));
        restore_env_vars(previous_apu_boot_overrides);
        restore_env_var("NO_AUDIO", previous_no_audio);
        restore_env_var("AUDIO_BACKEND", previous_audio_backend);
        let emulator = emulator_result.map_err(|err| err.to_string())?;

        let audio_sample_rate_hz = emulator.audio_sample_rate();
        let audio_source = emulator.audio_callback_source();

        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title,
            key_states: snes_emulator::input::KeyStates::default(),
            audio_source,
            audio_sample_rate_hz,
            audio_output_sample_rate_hz: audio_sample_rate_hz,
            native_audio_frame_remainder: 0,
            output_audio_frame_remainder: 0,
            audio_resample_buffer: Vec::new(),
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        self.emulator
            .step_one_frame_with_render_and_audio(true, true);
        Ok(())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        let fb = self.emulator.framebuffer();
        FrameView {
            width: 256,
            height: 224,
            format: PixelFormat::Bgra8888,
            data: argb8888_u32_frame_as_bgra8888_bytes(fb),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        fixed_audio_spec(self.audio_sample_rate_hz, 2)
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_output_sample_rate_hz = sample_rate_hz.max(8_000);
        self.output_audio_frame_remainder = 0;
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        out.clear();
        let native_frames = snes_frame_audio_frames(
            self.audio_sample_rate_hz,
            &mut self.native_audio_frame_remainder,
        );
        let output_frames = snes_frame_audio_frames(
            self.audio_output_sample_rate_hz,
            &mut self.output_audio_frame_remainder,
        );

        if self.audio_output_sample_rate_hz == self.audio_sample_rate_hz {
            out.resize(native_frames * 2, 0);
            self.audio_source.fill_interleaved_i16(out);
            return;
        }

        self.audio_resample_buffer.resize(native_frames * 2, 0);
        self.audio_source
            .fill_interleaved_i16(&mut self.audio_resample_buffer);
        out.resize(output_frames * 2, 0);
        resample_stereo_i16(
            &self.audio_resample_buffer,
            native_frames,
            out,
            output_frames,
        );
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if player != 1 {
            return;
        }
        match button {
            VirtualButton::Up => self.key_states.up = pressed,
            VirtualButton::Down => self.key_states.down = pressed,
            VirtualButton::Left => self.key_states.left = pressed,
            VirtualButton::Right => self.key_states.right = pressed,
            VirtualButton::A => self.key_states.a = pressed,
            VirtualButton::B => self.key_states.b = pressed,
            VirtualButton::X => self.key_states.x = pressed,
            VirtualButton::Y => self.key_states.y = pressed,
            VirtualButton::L => self.key_states.l = pressed,
            VirtualButton::R => self.key_states.r = pressed,
            VirtualButton::Start => self.key_states.start = pressed,
            VirtualButton::Select => self.key_states.select = pressed,
            VirtualButton::C | VirtualButton::Z | VirtualButton::Mode => return,
        }
        self.emulator.set_key_states(&self.key_states);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![MemoryRegion {
            id: "wram",
            label: "WRAM",
            len: self.emulator.wram().len(),
            writable: true,
        }];
        if !self.emulator.sram().is_empty() {
            regions.push(MemoryRegion {
                id: "sram",
                label: "SRAM",
                len: self.emulator.sram().len(),
                writable: true,
            });
        }
        regions
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.emulator.wram()),
            "sram" => Some(self.emulator.sram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.emulator.wram_mut(), offset, value),
            "sram" => write_byte(self.emulator.sram_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        save_state_slot(SystemKind::Snes, &self.rom_path, slot, "sns", |path| {
            self.emulator.save_state_to_file(path)
        })
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        load_state_slot(SystemKind::Snes, &self.rom_path, slot, "sns", |path| {
            self.emulator.load_state_from_file(path)
        })
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        self.emulator.flush_sram();
        Ok(())
    }
}

const APU_BOOT_OVERRIDE_ENV_VARS: &[&str] = &["APU_BOOT_HLE", "APU_SKIP_BOOT", "APU_FAKE_UPLOAD"];

fn clear_env_vars(names: &'static [&'static str]) -> Vec<(&'static str, Option<String>)> {
    names
        .iter()
        .map(|&name| {
            let previous = std::env::var(name).ok();
            std::env::remove_var(name);
            (name, previous)
        })
        .collect()
}

fn restore_env_vars(vars: Vec<(&'static str, Option<String>)>) {
    for (name, previous) in vars {
        restore_env_var(name, previous);
    }
}

fn restore_env_var(name: &str, previous: Option<String>) {
    if let Some(value) = previous {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}

fn snes_frame_audio_frames(sample_rate_hz: u32, remainder: &mut u64) -> usize {
    const MASTER_CLOCK_NTSC: u64 = 21_477_272;
    const CYCLES_PER_FRAME: u64 = 341 * 262 * 4;
    let numerator = (sample_rate_hz as u64)
        .saturating_mul(CYCLES_PER_FRAME)
        .saturating_add(*remainder);
    let frames = (numerator / MASTER_CLOCK_NTSC).max(1);
    *remainder = numerator % MASTER_CLOCK_NTSC;
    frames as usize
}

fn resample_stereo_i16(
    input: &[i16],
    input_frames: usize,
    output: &mut [i16],
    output_frames: usize,
) {
    if input_frames == 0 || output_frames == 0 {
        return;
    }
    if input_frames == 1 || output_frames == 1 {
        let left = input.first().copied().unwrap_or(0);
        let right = input.get(1).copied().unwrap_or(left);
        for frame in output.chunks_exact_mut(2) {
            frame[0] = left;
            frame[1] = right;
        }
        return;
    }

    let in_span = input_frames - 1;
    let out_span = output_frames - 1;
    for out_index in 0..output_frames {
        let pos_num = out_index * in_span;
        let base = pos_num / out_span;
        let frac_num = pos_num % out_span;
        let next = (base + 1).min(in_span);
        let out = out_index * 2;
        let base = base * 2;
        let next = next * 2;
        output[out] = lerp_i16(input[base], input[next], frac_num, out_span);
        output[out + 1] = lerp_i16(input[base + 1], input[next + 1], frac_num, out_span);
    }
}

fn lerp_i16(a: i16, b: i16, numerator: usize, denominator: usize) -> i16 {
    let a = a as i64;
    let b = b as i64;
    let numerator = numerator as i64;
    let denominator = denominator as i64;
    let value = (a * (denominator - numerator) + b * numerator) / denominator;
    value.clamp(i16::MIN as i64, i16::MAX as i64) as i16
}

fn snes_display_title(cartridge: &SnesCartridge, path: &Path) -> String {
    let header_title = cartridge.header.title.trim_matches('\0').trim();
    if header_title.is_empty() || header_title.chars().all(|ch| ch == '\0' || ch == ' ') {
        rom_stem(path)
    } else {
        header_title.to_string()
    }
}
