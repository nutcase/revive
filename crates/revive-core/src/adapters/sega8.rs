use std::path::{Path, PathBuf};

use mastersystem_core::{Button as SmsButton, Emulator as SmsEmulator};
use sg1000_core::{Button as SgButton, Emulator as SgEmulator};

use super::common::write_byte;
use crate::paths::{readable_state_path, rom_stem, state_path};
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

trait Sega8Emulator: Sized {
    const SYSTEM: SystemKind;
    const STATE_EXT: &'static str;
    const FRAME_TIMEOUT_LABEL: &'static str;

    fn from_rom(rom: Vec<u8>) -> Result<Self>;
    fn step_frame_ready(&mut self) -> bool;
    fn frame_width(&self) -> usize;
    fn frame_height(&self) -> usize;
    fn frame_buffer(&self) -> &[u8];
    fn audio_output_channels(&self) -> u8;
    fn set_audio_output_sample_rate_hz(&mut self, hz: u32);
    fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16>;
    fn save_state_to_file(&self, path: &Path) -> Result<()>;
    fn load_state_from_file(&mut self, path: &Path) -> Result<()>;
}

impl Sega8Emulator for SgEmulator {
    const SYSTEM: SystemKind = SystemKind::Sg1000;
    const STATE_EXT: &'static str = "sgs";
    const FRAME_TIMEOUT_LABEL: &'static str = "SG-1000";

    fn from_rom(rom: Vec<u8>) -> Result<Self> {
        SgEmulator::new(rom)
    }

    fn step_frame_ready(&mut self) -> bool {
        self.step().frame_ready
    }

    fn frame_width(&self) -> usize {
        SgEmulator::frame_width(self)
    }

    fn frame_height(&self) -> usize {
        SgEmulator::frame_height(self)
    }

    fn frame_buffer(&self) -> &[u8] {
        SgEmulator::frame_buffer(self)
    }

    fn audio_output_channels(&self) -> u8 {
        SgEmulator::audio_output_channels(self)
    }

    fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        SgEmulator::set_audio_output_sample_rate_hz(self, hz);
    }

    fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        SgEmulator::drain_audio_samples(self, max_samples)
    }

    fn save_state_to_file(&self, path: &Path) -> Result<()> {
        SgEmulator::save_state_to_file(self, path)
    }

    fn load_state_from_file(&mut self, path: &Path) -> Result<()> {
        SgEmulator::load_state_from_file(self, path)
    }
}

impl Sega8Emulator for SmsEmulator {
    const SYSTEM: SystemKind = SystemKind::MasterSystem;
    const STATE_EXT: &'static str = "smsst";
    const FRAME_TIMEOUT_LABEL: &'static str = "Master System";

    fn from_rom(rom: Vec<u8>) -> Result<Self> {
        SmsEmulator::new(rom)
    }

    fn step_frame_ready(&mut self) -> bool {
        self.step().frame_ready
    }

    fn frame_width(&self) -> usize {
        SmsEmulator::frame_width(self)
    }

    fn frame_height(&self) -> usize {
        SmsEmulator::frame_height(self)
    }

    fn frame_buffer(&self) -> &[u8] {
        SmsEmulator::frame_buffer(self)
    }

    fn audio_output_channels(&self) -> u8 {
        SmsEmulator::audio_output_channels(self)
    }

    fn set_audio_output_sample_rate_hz(&mut self, hz: u32) {
        SmsEmulator::set_audio_output_sample_rate_hz(self, hz);
    }

    fn drain_audio_samples(&mut self, max_samples: usize) -> Vec<i16> {
        SmsEmulator::drain_audio_samples(self, max_samples)
    }

    fn save_state_to_file(&self, path: &Path) -> Result<()> {
        SmsEmulator::save_state_to_file(self, path)
    }

    fn load_state_from_file(&mut self, path: &Path) -> Result<()> {
        SmsEmulator::load_state_from_file(self, path)
    }
}

struct Sega8Adapter<E: Sega8Emulator> {
    emulator: E,
    rom_path: PathBuf,
    title: String,
    audio_sample_rate_hz: u32,
}

impl<E: Sega8Emulator> Sega8Adapter<E> {
    fn load(path: &Path) -> Result<Self> {
        let rom = std::fs::read(path).map_err(|err| err.to_string())?;
        let emulator = E::from_rom(rom)?;
        Ok(Self {
            emulator,
            rom_path: path.to_path_buf(),
            title: rom_stem(path),
            audio_sample_rate_hz: 44_100,
        })
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn step_frame(&mut self) -> Result<()> {
        const MAX_STEPS_PER_FRAME: usize = 2_000;
        for _ in 0..MAX_STEPS_PER_FRAME {
            if self.emulator.step_frame_ready() {
                return Ok(());
            }
        }
        Err(format!(
            "{} frame did not complete before step limit",
            E::FRAME_TIMEOUT_LABEL
        ))
    }

    fn frame(&mut self) -> FrameView<'_> {
        FrameView {
            width: self.emulator.frame_width(),
            height: self.emulator.frame_height(),
            format: PixelFormat::Rgb24,
            data: self.emulator.frame_buffer(),
        }
    }

    fn audio_spec(&self) -> AudioSpec {
        AudioSpec {
            sample_rate_hz: self.audio_sample_rate_hz,
            channels: self.emulator.audio_output_channels(),
        }
    }

    fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_sample_rate_hz = sample_rate_hz;
        self.emulator
            .set_audio_output_sample_rate_hz(sample_rate_hz);
    }

    fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        let max_samples = ((self.audio_sample_rate_hz as usize) / 20).max(1024) * 2;
        *out = self.emulator.drain_audio_samples(max_samples);
    }

    fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let path = state_path(E::SYSTEM, &self.rom_path, slot, E::STATE_EXT);
        self.emulator.save_state_to_file(&path)
    }

    fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let path = readable_state_path(E::SYSTEM, &self.rom_path, slot, E::STATE_EXT)?;
        self.emulator.load_state_from_file(&path)
    }
}

macro_rules! impl_common_adapter {
    ($adapter:ident) => {
        impl $adapter {
            pub fn load(path: &Path) -> Result<Self> {
                Sega8Adapter::load(path).map(Self)
            }

            pub fn title(&self) -> &str {
                self.0.title()
            }

            pub fn step_frame(&mut self) -> Result<()> {
                self.0.step_frame()
            }

            pub fn frame(&mut self) -> FrameView<'_> {
                self.0.frame()
            }

            pub fn audio_spec(&self) -> AudioSpec {
                self.0.audio_spec()
            }

            pub fn configure_audio_output(&mut self, sample_rate_hz: u32) {
                self.0.configure_audio_output(sample_rate_hz);
            }

            pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
                self.0.drain_audio_i16(out);
            }

            pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
                self.0.save_state_to_slot(slot)
            }

            pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
                self.0.load_state_from_slot(slot)
            }

            pub fn flush_persistent_save(&mut self) -> Result<()> {
                Ok(())
            }
        }
    };
}

pub struct Sg1000Adapter(Sega8Adapter<SgEmulator>);

impl_common_adapter!(Sg1000Adapter);

impl Sg1000Adapter {
    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(button) = sg1000_button(button) else {
            return;
        };
        self.0.emulator.set_button_pressed(player, button, pressed);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.0.emulator.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.0.emulator.vram().len(),
                writable: true,
            },
        ]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.0.emulator.work_ram()),
            "vram" => Some(self.0.emulator.vram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.0.emulator.work_ram_mut(), offset, value),
            "vram" => write_byte(self.0.emulator.vram_mut(), offset, value),
            _ => false,
        }
    }
}

pub struct MasterSystemAdapter(Sega8Adapter<SmsEmulator>);

impl_common_adapter!(MasterSystemAdapter);

impl MasterSystemAdapter {
    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(button) = mastersystem_button(button) else {
            return;
        };
        self.0.emulator.set_button_pressed(player, button, pressed);
    }

    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.0.emulator.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "cart_ram",
                label: "Cartridge RAM",
                len: self.0.emulator.cart_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.0.emulator.vram().len(),
                writable: true,
            },
        ]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.0.emulator.work_ram()),
            "cart_ram" => Some(self.0.emulator.cart_ram()),
            "vram" => Some(self.0.emulator.vram()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.0.emulator.work_ram_mut(), offset, value),
            "cart_ram" => write_byte(self.0.emulator.cart_ram_mut(), offset, value),
            "vram" => write_byte(self.0.emulator.vram_mut(), offset, value),
            _ => false,
        }
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
