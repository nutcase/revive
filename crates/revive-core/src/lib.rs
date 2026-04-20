use std::path::{Path, PathBuf};

use emulator_core::{EmulatorCore, RomImage};
use emulator_gb::{
    GbEmulator, GbModel, GB_KEY_A, GB_KEY_B, GB_KEY_DOWN, GB_KEY_LEFT, GB_KEY_RIGHT, GB_KEY_SELECT,
    GB_KEY_START, GB_KEY_UP, GB_LCD_HEIGHT, GB_LCD_WIDTH,
};
use emulator_gba::{
    GbaEmulator, GbaFrameBuffer, GBA_KEY_A, GBA_KEY_B, GBA_KEY_DOWN, GBA_KEY_L, GBA_KEY_LEFT,
    GBA_KEY_R, GBA_KEY_RIGHT, GBA_KEY_SELECT, GBA_KEY_START, GBA_KEY_UP, GBA_LCD_HEIGHT,
    GBA_LCD_WIDTH,
};
use megadrive_core::{Button as MdButton, Cartridge as MdCartridge};
use nes_emulator::Nes;
use pce::emulator::Emulator as PceEmulator;
use snes_emulator::cartridge::Cartridge as SnesCartridge;
use snes_emulator::emulator::Emulator as SnesEmulator;

pub type Result<T> = std::result::Result<T, String>;
const PCE_VISIBLE_HEIGHT: usize = 208;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKind {
    Nes,
    Snes,
    MegaDrive,
    Pce,
    GameBoy,
    GameBoyColor,
    GameBoyAdvance,
}

impl SystemKind {
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "auto" => None,
            "nes" | "fc" | "famicom" => Some(Self::Nes),
            "snes" | "sfc" | "super-famicom" | "superfamicom" => Some(Self::Snes),
            "md" | "genesis" | "megadrive" | "mega-drive" => Some(Self::MegaDrive),
            "pce" | "pcengine" | "pc-engine" | "tg16" | "turbografx" | "turbografx-16" => {
                Some(Self::Pce)
            }
            "gb" | "gameboy" | "game-boy" => Some(Self::GameBoy),
            "gbc" | "gameboycolor" | "game-boy-color" | "gameboy-color" => Some(Self::GameBoyColor),
            "gba" | "gameboyadvance" | "game-boy-advance" | "gameboy-advance" => {
                Some(Self::GameBoyAdvance)
            }
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Nes => "NES",
            Self::Snes => "SNES",
            Self::MegaDrive => "Mega Drive",
            Self::Pce => "PC Engine",
            Self::GameBoy => "Game Boy",
            Self::GameBoyColor => "Game Boy Color",
            Self::GameBoyAdvance => "Game Boy Advance",
        }
    }

    fn state_dir(self) -> &'static str {
        match self {
            Self::Nes => "nes",
            Self::Snes => "snes",
            Self::MegaDrive => "megadrive",
            Self::Pce => "pce",
            Self::GameBoy => "gb",
            Self::GameBoyColor => "gbc",
            Self::GameBoyAdvance => "gba",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
    C,
    Z,
    Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb24,
}

pub struct FrameView<'a> {
    pub width: usize,
    pub height: usize,
    pub format: PixelFormat,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub id: &'static str,
    pub label: &'static str,
    pub len: usize,
    pub writable: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioSpec {
    pub sample_rate_hz: u32,
    pub channels: u8,
}

impl Default for AudioSpec {
    fn default() -> Self {
        Self {
            sample_rate_hz: 44_100,
            channels: 2,
        }
    }
}

pub enum CoreInstance {
    Nes(NesAdapter),
    Snes(Box<SnesAdapter>),
    MegaDrive(MegaDriveAdapter),
    Pce(Box<PceAdapter>),
    GameBoy(GameBoyAdapter),
    GameBoyAdvance(Box<GameBoyAdvanceAdapter>),
}

impl CoreInstance {
    pub fn load_rom(path: &Path, system: Option<SystemKind>) -> Result<Self> {
        let system = match system {
            Some(system) => system,
            None => detect_system(path)?,
        };

        match system {
            SystemKind::Nes => NesAdapter::load(path).map(Self::Nes),
            SystemKind::Snes => {
                SnesAdapter::load(path).map(|adapter| Self::Snes(Box::new(adapter)))
            }
            SystemKind::MegaDrive => MegaDriveAdapter::load(path).map(Self::MegaDrive),
            SystemKind::Pce => PceAdapter::load(path).map(|adapter| Self::Pce(Box::new(adapter))),
            SystemKind::GameBoy => {
                GameBoyAdapter::load(path, GbModel::Dmg, SystemKind::GameBoy).map(Self::GameBoy)
            }
            SystemKind::GameBoyColor => {
                GameBoyAdapter::load(path, GbModel::Cgb, SystemKind::GameBoyColor)
                    .map(Self::GameBoy)
            }
            SystemKind::GameBoyAdvance => GameBoyAdvanceAdapter::load(path)
                .map(|adapter| Self::GameBoyAdvance(Box::new(adapter))),
        }
    }

    pub fn system(&self) -> SystemKind {
        match self {
            Self::Nes(_) => SystemKind::Nes,
            Self::Snes(_) => SystemKind::Snes,
            Self::MegaDrive(_) => SystemKind::MegaDrive,
            Self::Pce(_) => SystemKind::Pce,
            Self::GameBoy(adapter) => adapter.system(),
            Self::GameBoyAdvance(_) => SystemKind::GameBoyAdvance,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Self::Nes(adapter) => adapter.title(),
            Self::Snes(adapter) => adapter.title(),
            Self::MegaDrive(adapter) => adapter.title(),
            Self::Pce(adapter) => adapter.title(),
            Self::GameBoy(adapter) => adapter.title(),
            Self::GameBoyAdvance(adapter) => adapter.title(),
        }
    }

    pub fn step_frame(&mut self) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.step_frame(),
            Self::Snes(adapter) => adapter.step_frame(),
            Self::MegaDrive(adapter) => adapter.step_frame(),
            Self::Pce(adapter) => adapter.step_frame(),
            Self::GameBoy(adapter) => adapter.step_frame(),
            Self::GameBoyAdvance(adapter) => adapter.step_frame(),
        }
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        match self {
            Self::Nes(adapter) => adapter.frame(),
            Self::Snes(adapter) => adapter.frame(),
            Self::MegaDrive(adapter) => adapter.frame(),
            Self::Pce(adapter) => adapter.frame(),
            Self::GameBoy(adapter) => adapter.frame(),
            Self::GameBoyAdvance(adapter) => adapter.frame(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        match self {
            Self::Nes(adapter) => adapter.audio_spec(),
            Self::Snes(adapter) => adapter.audio_spec(),
            Self::MegaDrive(adapter) => adapter.audio_spec(),
            Self::Pce(adapter) => adapter.audio_spec(),
            Self::GameBoy(adapter) => adapter.audio_spec(),
            Self::GameBoyAdvance(adapter) => adapter.audio_spec(),
        }
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        match self {
            Self::Nes(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::Snes(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::MegaDrive(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::Pce(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::GameBoy(adapter) => adapter.configure_audio_output(sample_rate_hz),
            Self::GameBoyAdvance(adapter) => adapter.configure_audio_output(sample_rate_hz),
        }
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        match self {
            Self::Nes(adapter) => adapter.drain_audio_i16(out),
            Self::Snes(adapter) => adapter.drain_audio_i16(out),
            Self::MegaDrive(adapter) => adapter.drain_audio_i16(out),
            Self::Pce(adapter) => adapter.drain_audio_i16(out),
            Self::GameBoy(adapter) => adapter.drain_audio_i16(out),
            Self::GameBoyAdvance(adapter) => adapter.drain_audio_i16(out),
        }
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        match self {
            Self::Nes(adapter) => adapter.set_button(player, button, pressed),
            Self::Snes(adapter) => adapter.set_button(player, button, pressed),
            Self::MegaDrive(adapter) => adapter.set_button(player, button, pressed),
            Self::Pce(adapter) => adapter.set_button(player, button, pressed),
            Self::GameBoy(adapter) => adapter.set_button(player, button, pressed),
            Self::GameBoyAdvance(adapter) => adapter.set_button(player, button, pressed),
        }
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        match self {
            Self::Nes(adapter) => adapter.memory_regions(),
            Self::Snes(adapter) => adapter.memory_regions(),
            Self::MegaDrive(adapter) => adapter.memory_regions(),
            Self::Pce(adapter) => adapter.memory_regions(),
            Self::GameBoy(adapter) => adapter.memory_regions(),
            Self::GameBoyAdvance(adapter) => adapter.memory_regions(),
        }
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match self {
            Self::Nes(adapter) => adapter.read_memory(region_id),
            Self::Snes(adapter) => adapter.read_memory(region_id),
            Self::MegaDrive(adapter) => adapter.read_memory(region_id),
            Self::Pce(adapter) => adapter.read_memory(region_id),
            Self::GameBoy(adapter) => adapter.read_memory(region_id),
            Self::GameBoyAdvance(adapter) => adapter.read_memory(region_id),
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match self {
            Self::Nes(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::Snes(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::MegaDrive(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::Pce(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::GameBoy(adapter) => adapter.write_memory_byte(region_id, offset, value),
            Self::GameBoyAdvance(adapter) => adapter.write_memory_byte(region_id, offset, value),
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.save_state_to_slot(slot),
            Self::Snes(adapter) => adapter.save_state_to_slot(slot),
            Self::MegaDrive(adapter) => adapter.save_state_to_slot(slot),
            Self::Pce(adapter) => adapter.save_state_to_slot(slot),
            Self::GameBoy(adapter) => adapter.save_state_to_slot(slot),
            Self::GameBoyAdvance(adapter) => adapter.save_state_to_slot(slot),
        }
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.load_state_from_slot(slot),
            Self::Snes(adapter) => adapter.load_state_from_slot(slot),
            Self::MegaDrive(adapter) => adapter.load_state_from_slot(slot),
            Self::Pce(adapter) => adapter.load_state_from_slot(slot),
            Self::GameBoy(adapter) => adapter.load_state_from_slot(slot),
            Self::GameBoyAdvance(adapter) => adapter.load_state_from_slot(slot),
        }
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        match self {
            Self::Nes(adapter) => adapter.flush_persistent_save(),
            Self::Snes(adapter) => adapter.flush_persistent_save(),
            Self::MegaDrive(adapter) => adapter.flush_persistent_save(),
            Self::Pce(adapter) => adapter.flush_persistent_save(),
            Self::GameBoy(adapter) => adapter.flush_persistent_save(),
            Self::GameBoyAdvance(adapter) => adapter.flush_persistent_save(),
        }
    }
}

pub fn detect_system(path: &Path) -> Result<SystemKind> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match ext.as_str() {
        "nes" => return Ok(SystemKind::Nes),
        "sfc" | "smc" => return Ok(SystemKind::Snes),
        "md" | "gen" | "genesis" => return Ok(SystemKind::MegaDrive),
        "pce" => return Ok(SystemKind::Pce),
        "gb" => return Ok(SystemKind::GameBoy),
        "gbc" => return Ok(SystemKind::GameBoyColor),
        "gba" => return Ok(SystemKind::GameBoyAdvance),
        "bin" => {}
        _ => {
            return Err(format!(
                "could not infer system from extension '.{ext}'; pass --system"
            ));
        }
    }

    let data = std::fs::read(path).map_err(|err| err.to_string())?;
    if data.len() >= 0x104 && &data[0x100..0x104] == b"SEGA" {
        Ok(SystemKind::MegaDrive)
    } else {
        Err("ambiguous .bin ROM; pass --system".to_string())
    }
}

pub struct NesAdapter {
    nes: Nes,
    rom_path: PathBuf,
    title: String,
    controllers: [u8; 2],
    audio_sample_rate_hz: u32,
}

impl NesAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let path_str = path
            .to_str()
            .ok_or_else(|| format!("ROM path is not valid UTF-8: {}", path.display()))?;
        let mut nes = Nes::new();
        nes.load_rom(path_str).map_err(|err| err.to_string())?;
        Ok(Self {
            nes,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            controllers: [0, 0],
            audio_sample_rate_hz: 44_100,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        const MAX_STEPS_PER_FRAME: usize = 50_000;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.nes.step() {
                return Ok(());
            }
        }
        Err("NES frame did not complete before step limit".to_string())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        FrameView {
            width: 256,
            height: 240,
            format: PixelFormat::Rgb24,
            data: self.nes.get_frame_buffer(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        AudioSpec {
            sample_rate_hz: self.audio_sample_rate_hz,
            channels: 2,
        }
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_sample_rate_hz = sample_rate_hz;
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        out.clear();
        for sample in self.nes.get_audio_buffer() {
            let value = f32_to_i16(sample);
            out.push(value);
            out.push(value);
        }
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(mask) = nes_button_mask(button) else {
            return;
        };
        let index = match player {
            1 => 0,
            2 => 1,
            _ => return,
        };
        if pressed {
            self.controllers[index] |= mask;
        } else {
            self.controllers[index] &= !mask;
        }
        self.nes.set_controller(self.controllers[0]);
        self.nes.set_controller2(self.controllers[1]);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![MemoryRegion {
            id: "cpu_ram",
            label: "CPU RAM",
            len: self.nes.ram().len(),
            writable: true,
        }];
        if let Some(prg_ram) = self.nes.prg_ram() {
            regions.push(MemoryRegion {
                id: "prg_ram",
                label: "PRG RAM / SRAM",
                len: prg_ram.len(),
                writable: true,
            });
        }
        regions
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "cpu_ram" => Some(self.nes.ram()),
            "prg_ram" => self.nes.prg_ram(),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "cpu_ram" => write_byte(self.nes.ram_mut(), offset, value),
            "prg_ram" => self
                .nes
                .prg_ram_mut()
                .map(|ram| write_byte(ram, offset, value))
                .unwrap_or(false),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        self.nes
            .save_state(slot, &rom_stem(&self.rom_path))
            .map_err(|err| err.to_string())
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        self.nes.load_state(slot).map_err(|err| err.to_string())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        self.nes.save_sram().map_err(|err| err.to_string())
    }
}

pub struct SnesAdapter {
    emulator: SnesEmulator,
    rom_path: PathBuf,
    title: String,
    key_states: snes_emulator::input::KeyStates,
    rgb_frame: Vec<u8>,
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
            rgb_frame: Vec::new(),
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
        self.rgb_frame.clear();
        self.rgb_frame.reserve(fb.len() * 3);
        for &pixel in fb {
            self.rgb_frame.push(((pixel >> 16) & 0xFF) as u8);
            self.rgb_frame.push(((pixel >> 8) & 0xFF) as u8);
            self.rgb_frame.push((pixel & 0xFF) as u8);
        }
        FrameView {
            width: 256,
            height: 224,
            format: PixelFormat::Rgb24,
            data: &self.rgb_frame,
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
        let path = state_path(SystemKind::Snes, &self.rom_path, slot, "sns");
        self.emulator.save_state_to_file(&path)
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::Snes, &self.rom_path, slot, "sns");
        self.emulator.load_state_from_file(&path)
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        self.emulator.flush_sram();
        Ok(())
    }
}

pub struct MegaDriveAdapter {
    emulator: megadrive_core::Emulator,
    rom_path: PathBuf,
    title: String,
    audio_sample_rate_hz: u32,
}

impl MegaDriveAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).map_err(|err| err.to_string())?;
        let cartridge = MdCartridge::from_bytes(rom).map_err(|err| err.to_string())?;
        let mut emulator = megadrive_core::Emulator::new(cartridge);
        emulator.set_controller_type(1, megadrive_core::ControllerType::SixButton);
        emulator.set_controller_type(2, megadrive_core::ControllerType::SixButton);
        let title = md_display_title(&emulator, path);
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title,
            audio_sample_rate_hz: 44_100,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        const MAX_STEPS_PER_FRAME: usize = 100_000;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.emulator.step().frame_ready {
                return Ok(());
            }
        }
        Err("Mega Drive frame did not complete before step limit".to_string())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        FrameView {
            width: self.emulator.frame_width(),
            height: self.emulator.frame_height(),
            format: PixelFormat::Rgb24,
            data: self.emulator.frame_buffer(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        AudioSpec {
            sample_rate_hz: self.audio_sample_rate_hz,
            channels: 2,
        }
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_sample_rate_hz = sample_rate_hz;
        self.emulator
            .set_audio_output_sample_rate_hz(sample_rate_hz);
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        let max_samples = ((self.audio_sample_rate_hz as usize) / 20).max(1024) * 2;
        *out = self.emulator.drain_audio_samples(max_samples);
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(button) = md_button(button) else {
            return;
        };
        match player {
            1 => self.emulator.set_button_pressed(button, pressed),
            2 => self.emulator.set_button2_pressed(button, pressed),
            _ => {}
        }
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![MemoryRegion {
            id: "wram",
            label: "Work RAM",
            len: self.emulator.work_ram().len(),
            writable: true,
        }]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.emulator.work_ram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.emulator.work_ram_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(SystemKind::MegaDrive, &self.rom_path, slot, "mdst");
        self.emulator.save_state_to_file(&path)
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::MegaDrive, &self.rom_path, slot, "mdst");
        self.emulator.load_state_from_file(&path)
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct PceAdapter {
    emulator: PceEmulator,
    rom_path: PathBuf,
    title: String,
    hucard: bool,
    pad_state: u8,
    latest_frame: Vec<u32>,
    rgb_frame: Vec<u8>,
}

impl PceAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).map_err(|err| err.to_string())?;
        let mut emulator = PceEmulator::new();
        let hucard = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("pce"))
            .unwrap_or(false);

        if hucard {
            emulator
                .load_hucard(&rom)
                .map_err(|err| format!("failed to load HuCard: {err}"))?;
            load_pce_persistent_saves(&mut emulator, path);
        } else {
            emulator.load_program(0xC000, &rom);
        }

        emulator.set_audio_batch_size(256);
        emulator.reset();
        emulator.bus.set_joypad_input(0xFF);

        let frame_len = emulator.display_width() * emulator.display_height();
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            hucard,
            pad_state: 0xFF,
            latest_frame: vec![0; frame_len],
            rgb_frame: Vec::with_capacity(frame_len * 3),
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        const MAX_TICKS_PER_FRAME: usize = 150_000;
        for _ in 0..MAX_TICKS_PER_FRAME {
            self.emulator.tick();
            if self.emulator.take_frame_into(&mut self.latest_frame) {
                return Ok(());
            }
        }
        Err("PC Engine frame did not complete before tick limit".to_string())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        let width = self.emulator.display_width();
        let raw_height = self.emulator.display_height();
        let expected = width * raw_height;
        if self.latest_frame.len() != expected {
            self.latest_frame.resize(expected, 0);
        }

        let height = pce_visible_height(raw_height);
        self.rgb_frame.clear();
        self.rgb_frame.reserve(width * height * 3);
        for row in self.latest_frame.chunks_exact(width).take(height) {
            for &pixel in row {
                self.rgb_frame.push(((pixel >> 16) & 0xFF) as u8);
                self.rgb_frame.push(((pixel >> 8) & 0xFF) as u8);
                self.rgb_frame.push((pixel & 0xFF) as u8);
            }
        }

        FrameView {
            width,
            height,
            format: PixelFormat::Rgb24,
            data: &self.rgb_frame,
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        AudioSpec {
            sample_rate_hz: 44_100,
            channels: 1,
        }
    }

    pub fn configure_audio_output(&mut self, _sample_rate_hz: u32) {}

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        *out = self.emulator.drain_audio_samples();
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if player != 1 {
            return;
        }
        let Some(bit) = pce_button_bit(button) else {
            return;
        };
        if pressed {
            self.pad_state &= !(1 << bit);
        } else {
            self.pad_state |= 1 << bit;
        }
        self.emulator.bus.set_joypad_input(self.pad_state);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        let mut regions = vec![MemoryRegion {
            id: "wram",
            label: "Work RAM",
            len: self.emulator.work_ram().len(),
            writable: true,
        }];
        if let Some(cart_ram) = self.emulator.backup_ram() {
            regions.push(MemoryRegion {
                id: "cart_ram",
                label: "HuCard Backup RAM",
                len: cart_ram.len(),
                writable: true,
            });
        }
        regions.push(MemoryRegion {
            id: "bram",
            label: "BRAM",
            len: self.emulator.bram().len(),
            writable: true,
        });
        regions
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.emulator.work_ram()),
            "cart_ram" => self.emulator.backup_ram(),
            "bram" => Some(self.emulator.bram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.emulator.work_ram_mut(), offset, value),
            "cart_ram" => self
                .emulator
                .backup_ram_mut()
                .map(|ram| write_byte(ram, offset, value))
                .unwrap_or(false),
            "bram" => write_byte(self.emulator.bram_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(SystemKind::Pce, &self.rom_path, slot, "pcst");
        self.emulator
            .save_state_to_file(&path)
            .map_err(|err| err.to_string())
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::Pce, &self.rom_path, slot, "pcst");
        self.emulator
            .load_state_from_file(&path)
            .map_err(|err| err.to_string())?;
        self.emulator.bus.set_joypad_input(self.pad_state);
        Ok(())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        if !self.hucard {
            return Ok(());
        }
        if let Some(snapshot) = self.emulator.save_backup_ram() {
            std::fs::write(self.rom_path.with_extension("sav"), snapshot)
                .map_err(|err| err.to_string())?;
        }
        std::fs::write(
            self.rom_path.with_extension("brm"),
            self.emulator.save_bram(),
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn pce_visible_height(raw_height: usize) -> usize {
    raw_height.min(PCE_VISIBLE_HEIGHT)
}

pub struct GameBoyAdapter {
    emulator: GbEmulator,
    rom_path: PathBuf,
    title: String,
    system: SystemKind,
    pressed_mask: u8,
    rgb_frame: Vec<u8>,
    audio_sample_rate_hz: u32,
}

impl GameBoyAdapter {
    pub fn load(path: &Path, model: GbModel, system: SystemKind) -> Result<Self> {
        let rom = RomImage::from_file(path).map_err(|err| err.to_string())?;
        let mut emulator = GbEmulator::new(model);
        emulator.load_rom(rom).map_err(|err| err.to_string())?;
        load_gameboy_backup(&mut emulator, path);
        let audio_sample_rate_hz = emulator.debug_audio_sample_rate_hz().max(8_000);
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            system,
            pressed_mask: 0,
            rgb_frame: Vec::with_capacity((GB_LCD_WIDTH * GB_LCD_HEIGHT * 3) as usize),
            audio_sample_rate_hz,
        })
    }

    pub fn system(&self) -> SystemKind {
        self.system
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        self.emulator.step_frame().map_err(|err| err.to_string())?;
        self.audio_sample_rate_hz = self.emulator.debug_audio_sample_rate_hz().max(8_000);
        Ok(())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        rgba8888_to_rgb24(self.emulator.frame_rgba8888(), &mut self.rgb_frame);
        FrameView {
            width: GB_LCD_WIDTH as usize,
            height: GB_LCD_HEIGHT as usize,
            format: PixelFormat::Rgb24,
            data: &self.rgb_frame,
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
        self.emulator.take_audio_samples_i16_into(out);
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if player != 1 {
            return;
        }
        let Some(mask) = gameboy_button_mask(button) else {
            return;
        };
        if pressed {
            self.pressed_mask |= mask;
        } else {
            self.pressed_mask &= !mask;
        }
        self.emulator.set_keyinput_pressed_mask(self.pressed_mask);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        self.emulator
            .backup_data()
            .map(|ram| {
                vec![MemoryRegion {
                    id: "cart_ram",
                    label: "Cartridge RAM",
                    len: ram.len(),
                    writable: false,
                }]
            })
            .unwrap_or_default()
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "cart_ram" => self.emulator.backup_data(),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, _region_id: &str, _offset: usize, _value: u8) -> bool {
        false
    }

    pub fn save_state_to_slot(&mut self, _slot: u8) -> Result<()> {
        Err("Game Boy save states are not exposed by ../gameboy yet".to_string())
    }

    pub fn load_state_from_slot(&mut self, _slot: u8) -> Result<()> {
        Err("Game Boy save states are not exposed by ../gameboy yet".to_string())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        if let Some(save_data) = self.emulator.backup_data() {
            std::fs::write(self.rom_path.with_extension("sav"), save_data)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

pub struct GameBoyAdvanceAdapter {
    emulator: GbaEmulator,
    frame_buffer: GbaFrameBuffer,
    rom_path: PathBuf,
    title: String,
    pressed_mask: u16,
    rgb_frame: Vec<u8>,
    audio_sample_rate_hz: u32,
}

impl GameBoyAdvanceAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let rom = RomImage::from_file(path).map_err(|err| err.to_string())?;
        let mut emulator = GbaEmulator::new();
        load_gameboy_advance_bios(&mut emulator);
        emulator.load_rom(rom).map_err(|err| err.to_string())?;
        load_gameboy_advance_backup(&mut emulator, path);
        let audio_sample_rate_hz = emulator.debug_audio_sample_rate_hz().max(8_000);
        Ok(Self {
            emulator,
            frame_buffer: GbaFrameBuffer::new(),
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            pressed_mask: 0,
            rgb_frame: Vec::with_capacity((GBA_LCD_WIDTH * GBA_LCD_HEIGHT * 3) as usize),
            audio_sample_rate_hz,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        self.emulator
            .step_frame_with_render(&mut self.frame_buffer)
            .map_err(|err| err.to_string())?;
        self.audio_sample_rate_hz = self.emulator.debug_audio_sample_rate_hz().max(8_000);
        Ok(())
    }

    pub fn frame(&mut self) -> FrameView<'_> {
        rgba8888_to_rgb24(self.frame_buffer.pixels(), &mut self.rgb_frame);
        FrameView {
            width: GBA_LCD_WIDTH as usize,
            height: GBA_LCD_HEIGHT as usize,
            format: PixelFormat::Rgb24,
            data: &self.rgb_frame,
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
        self.emulator.take_audio_samples_i16_into(out);
    }

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if player != 1 {
            return;
        }
        let Some(mask) = gameboy_advance_button_mask(button) else {
            return;
        };
        if pressed {
            self.pressed_mask |= mask;
        } else {
            self.pressed_mask &= !mask;
        }
        self.emulator.set_keyinput_pressed_mask(self.pressed_mask);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        Vec::new()
    }

    pub fn read_memory(&self, _region_id: &str) -> Option<&[u8]> {
        None
    }

    pub fn write_memory_byte(&mut self, _region_id: &str, _offset: usize, _value: u8) -> bool {
        false
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(SystemKind::GameBoyAdvance, &self.rom_path, slot, "gbas");
        let state_data = self.emulator.save_state();
        std::fs::write(path, state_data).map_err(|err| err.to_string())
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::GameBoyAdvance, &self.rom_path, slot, "gbas");
        let state_data = std::fs::read(path).map_err(|err| err.to_string())?;
        self.emulator
            .load_state(&state_data)
            .map_err(|err| err.to_string())?;
        self.emulator.set_keyinput_pressed_mask(self.pressed_mask);
        self.emulator.render_frame_rgba8888(&mut self.frame_buffer);
        Ok(())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        if let Some(save_data) = self.emulator.backup_data() {
            std::fs::write(self.rom_path.with_extension("sav"), save_data)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

fn f32_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

fn nes_button_mask(button: VirtualButton) -> Option<u8> {
    match button {
        VirtualButton::A => Some(0x01),
        VirtualButton::B => Some(0x02),
        VirtualButton::Select => Some(0x04),
        VirtualButton::Start => Some(0x08),
        VirtualButton::Up => Some(0x10),
        VirtualButton::Down => Some(0x20),
        VirtualButton::Left => Some(0x40),
        VirtualButton::Right => Some(0x80),
        _ => None,
    }
}

fn md_button(button: VirtualButton) -> Option<MdButton> {
    match button {
        VirtualButton::Up => Some(MdButton::Up),
        VirtualButton::Down => Some(MdButton::Down),
        VirtualButton::Left => Some(MdButton::Left),
        VirtualButton::Right => Some(MdButton::Right),
        VirtualButton::A => Some(MdButton::A),
        VirtualButton::B => Some(MdButton::B),
        VirtualButton::C => Some(MdButton::C),
        VirtualButton::Start => Some(MdButton::Start),
        VirtualButton::X => Some(MdButton::X),
        VirtualButton::Y => Some(MdButton::Y),
        VirtualButton::Z => Some(MdButton::Z),
        VirtualButton::Mode => Some(MdButton::Mode),
        VirtualButton::L | VirtualButton::R | VirtualButton::Select => None,
    }
}

fn pce_button_bit(button: VirtualButton) -> Option<u8> {
    match button {
        VirtualButton::Up => Some(0),
        VirtualButton::Right => Some(1),
        VirtualButton::Down => Some(2),
        VirtualButton::Left => Some(3),
        VirtualButton::A => Some(4),
        VirtualButton::B => Some(5),
        VirtualButton::Select => Some(6),
        VirtualButton::Start => Some(7),
        VirtualButton::X
        | VirtualButton::Y
        | VirtualButton::L
        | VirtualButton::R
        | VirtualButton::C
        | VirtualButton::Z
        | VirtualButton::Mode => None,
    }
}

fn gameboy_button_mask(button: VirtualButton) -> Option<u8> {
    match button {
        VirtualButton::A => Some(GB_KEY_A),
        VirtualButton::B => Some(GB_KEY_B),
        VirtualButton::Select => Some(GB_KEY_SELECT),
        VirtualButton::Start => Some(GB_KEY_START),
        VirtualButton::Right => Some(GB_KEY_RIGHT),
        VirtualButton::Left => Some(GB_KEY_LEFT),
        VirtualButton::Up => Some(GB_KEY_UP),
        VirtualButton::Down => Some(GB_KEY_DOWN),
        VirtualButton::X
        | VirtualButton::Y
        | VirtualButton::L
        | VirtualButton::R
        | VirtualButton::C
        | VirtualButton::Z
        | VirtualButton::Mode => None,
    }
}

fn gameboy_advance_button_mask(button: VirtualButton) -> Option<u16> {
    match button {
        VirtualButton::A => Some(GBA_KEY_A),
        VirtualButton::B => Some(GBA_KEY_B),
        VirtualButton::Select => Some(GBA_KEY_SELECT),
        VirtualButton::Start => Some(GBA_KEY_START),
        VirtualButton::Right => Some(GBA_KEY_RIGHT),
        VirtualButton::Left => Some(GBA_KEY_LEFT),
        VirtualButton::Up => Some(GBA_KEY_UP),
        VirtualButton::Down => Some(GBA_KEY_DOWN),
        VirtualButton::L => Some(GBA_KEY_L),
        VirtualButton::R => Some(GBA_KEY_R),
        VirtualButton::X
        | VirtualButton::Y
        | VirtualButton::C
        | VirtualButton::Z
        | VirtualButton::Mode => None,
    }
}

fn write_byte(memory: &mut [u8], offset: usize, value: u8) -> bool {
    if let Some(slot) = memory.get_mut(offset) {
        *slot = value;
        true
    } else {
        false
    }
}

fn rgba8888_to_rgb24(rgba: &[u8], rgb: &mut Vec<u8>) {
    rgb.clear();
    rgb.reserve(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        rgb.push(pixel[0]);
        rgb.push(pixel[1]);
        rgb.push(pixel[2]);
    }
}

fn load_pce_persistent_saves(emulator: &mut PceEmulator, rom_path: &Path) {
    let backup_path = rom_path.with_extension("sav");
    if backup_path.exists() {
        match std::fs::read(&backup_path) {
            Ok(bytes) => {
                if let Err(err) = emulator.load_backup_ram(&bytes) {
                    eprintln!(
                        "warning: failed to load PC Engine backup RAM from {}: {err}",
                        backup_path.display()
                    );
                }
            }
            Err(err) => eprintln!(
                "warning: could not read PC Engine backup RAM file {}: {err}",
                backup_path.display()
            ),
        }
    }

    let bram_path = rom_path.with_extension("brm");
    if bram_path.exists() {
        match std::fs::read(&bram_path) {
            Ok(bytes) => {
                if let Err(err) = emulator.load_bram(&bytes) {
                    eprintln!(
                        "warning: failed to load PC Engine BRAM from {}: {err}",
                        bram_path.display()
                    );
                }
            }
            Err(err) => eprintln!(
                "warning: could not read PC Engine BRAM file {}: {err}",
                bram_path.display()
            ),
        }
    }
}

fn load_gameboy_backup(emulator: &mut GbEmulator, rom_path: &Path) {
    let save_path = rom_path.with_extension("sav");
    if let Ok(bytes) = std::fs::read(save_path) {
        emulator.load_backup_data(&bytes);
    }
}

fn load_gameboy_advance_backup(emulator: &mut GbaEmulator, rom_path: &Path) {
    let save_path = rom_path.with_extension("sav");
    if let Ok(bytes) = std::fs::read(save_path) {
        emulator.load_backup_data(&bytes);
    }
}

fn load_gameboy_advance_bios(emulator: &mut GbaEmulator) {
    let candidates = std::env::var_os("GBA_BIOS")
        .map(PathBuf::from)
        .into_iter()
        .chain([PathBuf::from(
            "/Users/takamatsu/dev/gameboy/bios/gba_bios.bin",
        )]);
    for path in candidates {
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                emulator.load_bios(&bytes);
                return;
            }
            _ => {}
        }
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

fn md_display_title(emulator: &megadrive_core::Emulator, path: &Path) -> String {
    let header = emulator.header();
    for candidate in [&header.domestic_title, &header.overseas_title] {
        let title = candidate.trim();
        if !title.is_empty() {
            return title.to_string();
        }
    }
    rom_stem(path)
}

fn rom_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game")
        .to_string()
}

fn state_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let stem = rom_stem(rom_path);
    let path = Path::new("states")
        .join(system.state_dir())
        .join(stem)
        .join(format!("slot{slot}.{ext}"));
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    path
}

fn readable_state_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let path = state_path(system, rom_path, slot, ext);
    if path.exists() {
        path
    } else {
        legacy_state_path(system, rom_path, slot, ext)
    }
}

fn legacy_state_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let stem = rom_stem(rom_path);
    Path::new("states")
        .join(system.state_dir())
        .join(format!("{stem}.slot{slot}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_system_from_standard_extensions() {
        assert_eq!(
            detect_system(Path::new("game.nes")).unwrap(),
            SystemKind::Nes
        );
        assert_eq!(
            detect_system(Path::new("game.sfc")).unwrap(),
            SystemKind::Snes
        );
        assert_eq!(
            detect_system(Path::new("game.smc")).unwrap(),
            SystemKind::Snes
        );
        assert_eq!(
            detect_system(Path::new("game.md")).unwrap(),
            SystemKind::MegaDrive
        );
        assert_eq!(
            detect_system(Path::new("game.gen")).unwrap(),
            SystemKind::MegaDrive
        );
        assert_eq!(
            detect_system(Path::new("game.pce")).unwrap(),
            SystemKind::Pce
        );
        assert_eq!(
            detect_system(Path::new("game.gb")).unwrap(),
            SystemKind::GameBoy
        );
        assert_eq!(
            detect_system(Path::new("game.gbc")).unwrap(),
            SystemKind::GameBoyColor
        );
        assert_eq!(
            detect_system(Path::new("game.gba")).unwrap(),
            SystemKind::GameBoyAdvance
        );
    }

    #[test]
    fn detects_megadrive_bin_header() {
        let path = std::env::temp_dir().join(format!("revive-md-{}.bin", std::process::id()));
        let mut rom = vec![0; 0x200];
        rom[0x100..0x104].copy_from_slice(b"SEGA");
        std::fs::write(&path, rom).unwrap();

        let detected = detect_system(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(detected, SystemKind::MegaDrive);
    }

    #[test]
    fn pce_video_crops_bottom_overscan() {
        assert_eq!(pce_visible_height(240), 208);
        assert_eq!(pce_visible_height(224), 208);
        assert_eq!(pce_visible_height(216), 208);
        assert_eq!(pce_visible_height(200), 200);
    }
}
