use std::path::{Path, PathBuf};

use snes_emulator::cartridge::Cartridge as SnesCartridge;
use snes_emulator::emulator::Emulator as SnesEmulator;

use super::common::{
    argb8888_u32_frame_as_bgra8888_bytes, load_state_slot, save_state_slot, write_byte,
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
    audio_frame_remainder: u32,
}

impl SnesAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let cartridge = SnesCartridge::load_from_file(path).map_err(|err| err.to_string())?;
        let title = snes_display_title(&cartridge, path);
        let mut srm_path = path.to_path_buf();
        srm_path.set_extension("srm");

        let previous_audio_backend = std::env::var("AUDIO_BACKEND").ok();
        std::env::set_var("AUDIO_BACKEND", "sdl_callback");
        let emulator_result = SnesEmulator::new(cartridge, title.clone(), Some(srm_path));
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
            audio_frame_remainder: 0,
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
        AudioSpec {
            sample_rate_hz: self.audio_sample_rate_hz,
            channels: 2,
        }
    }

    pub fn configure_audio_output(&mut self, _sample_rate_hz: u32) {}

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        out.clear();
        let numerator = self
            .audio_sample_rate_hz
            .saturating_add(self.audio_frame_remainder);
        let frames = (numerator / 60).max(1);
        self.audio_frame_remainder = numerator % 60;
        out.resize(frames as usize * 2, 0);
        self.audio_source.fill_interleaved_i16(out);
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
fn restore_env_var(name: &str, previous: Option<String>) {
    if let Some(value) = previous {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}

fn snes_display_title(cartridge: &SnesCartridge, path: &Path) -> String {
    let header_title = cartridge.header.title.trim_matches('\0').trim();
    if header_title.is_empty() || header_title.chars().all(|ch| ch == '\0' || ch == ' ') {
        rom_stem(path)
    } else {
        header_title.to_string()
    }
}
