use std::path::{Path, PathBuf};

use megadrive_core::{Button as MdButton, Cartridge as MdCartridge};

use super::common::{
    fixed_audio_spec, load_state_slot, replace_audio_buffer, save_state_slot, write_byte,
};
use crate::paths::rom_stem;
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

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
        fixed_audio_spec(self.audio_sample_rate_hz, 2)
    }

    pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_sample_rate_hz = sample_rate_hz;
        self.emulator
            .set_audio_output_sample_rate_hz(sample_rate_hz);
    }

    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        let max_samples = ((self.audio_sample_rate_hz as usize) / 20).max(1024) * 2;
        replace_audio_buffer(out, self.emulator.drain_audio_samples(max_samples));
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
        save_state_slot(
            SystemKind::MegaDrive,
            &self.rom_path,
            slot,
            "mdst",
            |path| self.emulator.save_state_to_file(path),
        )
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        load_state_slot(
            SystemKind::MegaDrive,
            &self.rom_path,
            slot,
            "mdst",
            |path| self.emulator.load_state_from_file(path),
        )
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        Ok(())
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
