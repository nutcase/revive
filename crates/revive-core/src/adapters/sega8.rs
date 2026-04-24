use std::path::{Path, PathBuf};

use mastersystem_core::{Button as SmsButton, Emulator as SmsEmulator};
use sg1000_core::{Button as SgButton, Emulator as SgEmulator};

use super::common::write_byte;
use crate::paths::{readable_state_path, rom_stem, state_path};
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

pub struct Sg1000Adapter {
    emulator: SgEmulator,
    rom_path: PathBuf,
    title: String,
    audio_sample_rate_hz: u32,
}

impl Sg1000Adapter {
    pub fn load(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).map_err(|err| err.to_string())?;
        let emulator = SgEmulator::new(rom)?;
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            audio_sample_rate_hz: 44_100,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        const MAX_STEPS_PER_FRAME: usize = 2_000;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.emulator.step().frame_ready {
                return Ok(());
            }
        }
        Err("SG-1000 frame did not complete before step limit".to_string())
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
            channels: self.emulator.audio_output_channels(),
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
        let Some(button) = sg1000_button(button) else {
            return;
        };
        self.emulator.set_button_pressed(player, button, pressed);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.emulator.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.emulator.vram().len(),
                writable: true,
            },
        ]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.emulator.work_ram()),
            "vram" => Some(self.emulator.vram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.emulator.work_ram_mut(), offset, value),
            "vram" => write_byte(self.emulator.vram_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(SystemKind::Sg1000, &self.rom_path, slot, "sgs");
        self.emulator.save_state_to_file(&path)
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::Sg1000, &self.rom_path, slot, "sgs")?;
        self.emulator.load_state_from_file(&path)
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct MasterSystemAdapter {
    emulator: SmsEmulator,
    rom_path: PathBuf,
    title: String,
    audio_sample_rate_hz: u32,
}

impl MasterSystemAdapter {
    pub fn load(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).map_err(|err| err.to_string())?;
        let emulator = SmsEmulator::new(rom)?;
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            audio_sample_rate_hz: 44_100,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn step_frame(&mut self) -> Result<()> {
        const MAX_STEPS_PER_FRAME: usize = 2_000;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.emulator.step().frame_ready {
                return Ok(());
            }
        }
        Err("Master System frame did not complete before step limit".to_string())
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
            channels: self.emulator.audio_output_channels(),
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
        let Some(button) = mastersystem_button(button) else {
            return;
        };
        self.emulator.set_button_pressed(player, button, pressed);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.emulator.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "cart_ram",
                label: "Cartridge RAM",
                len: self.emulator.cart_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.emulator.vram().len(),
                writable: true,
            },
        ]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.emulator.work_ram()),
            "cart_ram" => Some(self.emulator.cart_ram()),
            "vram" => Some(self.emulator.vram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.emulator.work_ram_mut(), offset, value),
            "cart_ram" => write_byte(self.emulator.cart_ram_mut(), offset, value),
            "vram" => write_byte(self.emulator.vram_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(SystemKind::MasterSystem, &self.rom_path, slot, "smsst");
        self.emulator.save_state_to_file(&path)
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(SystemKind::MasterSystem, &self.rom_path, slot, "smsst")?;
        self.emulator.load_state_from_file(&path)
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        Ok(())
    }
}
fn sg1000_button(button: VirtualButton) -> Option<SgButton> {
    match button {
        VirtualButton::Up => Some(SgButton::Up),
        VirtualButton::Down => Some(SgButton::Down),
        VirtualButton::Left => Some(SgButton::Left),
        VirtualButton::Right => Some(SgButton::Right),
        VirtualButton::A => Some(SgButton::Button1),
        VirtualButton::B => Some(SgButton::Button2),
        VirtualButton::X
        | VirtualButton::Y
        | VirtualButton::L
        | VirtualButton::R
        | VirtualButton::Start
        | VirtualButton::Select
        | VirtualButton::C
        | VirtualButton::Z
        | VirtualButton::Mode => None,
    }
}

fn mastersystem_button(button: VirtualButton) -> Option<SmsButton> {
    match button {
        VirtualButton::Up => Some(SmsButton::Up),
        VirtualButton::Down => Some(SmsButton::Down),
        VirtualButton::Left => Some(SmsButton::Left),
        VirtualButton::Right => Some(SmsButton::Right),
        VirtualButton::A => Some(SmsButton::Button1),
        VirtualButton::B => Some(SmsButton::Button2),
        VirtualButton::X
        | VirtualButton::Y
        | VirtualButton::L
        | VirtualButton::R
        | VirtualButton::Start
        | VirtualButton::Select
        | VirtualButton::C
        | VirtualButton::Z
        | VirtualButton::Mode => None,
    }
}
