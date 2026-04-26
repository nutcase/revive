use std::path::{Path, PathBuf};

use mastersystem_core::{Button as SmsButton, Emulator as SmsEmulator};
use sg1000_core::{Button as SgButton, Emulator as SgEmulator};

use super::common::{
    fixed_audio_spec, load_state_slot, replace_audio_buffer, save_state_slot, write_byte,
};
use crate::paths::rom_stem;
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

trait Sega8Platform {
    fn set_virtual_button(&mut self, player: u8, button: VirtualButton, pressed: bool);
    fn memory_regions(&self) -> Vec<MemoryRegion>;
    fn read_memory(&self, region_id: &str) -> Option<&[u8]>;
    fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool;
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

impl Sega8Platform for SgEmulator {
    fn set_virtual_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(button) = sg1000_button(button) else {
            return;
        };
        self.set_button_pressed(player, button, pressed);
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.vram().len(),
                writable: true,
            },
        ]
    }

    fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.work_ram()),
            "vram" => Some(self.vram()),
            _ => None,
        }
    }

    fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.work_ram_mut(), offset, value),
            "vram" => write_byte(self.vram_mut(), offset, value),
            _ => false,
        }
    }
}

impl Sega8Platform for SmsEmulator {
    fn set_virtual_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        let Some(button) = mastersystem_button(button) else {
            return;
        };
        self.set_button_pressed(player, button, pressed);
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: self.work_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "cart_ram",
                label: "Cartridge RAM",
                len: self.cart_ram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "VDP VRAM",
                len: self.vram().len(),
                writable: true,
            },
        ]
    }

    fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(self.work_ram()),
            "cart_ram" => Some(self.cart_ram()),
            "vram" => Some(self.vram()),
            _ => None,
        }
    }

    fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(self.work_ram_mut(), offset, value),
            "cart_ram" => write_byte(self.cart_ram_mut(), offset, value),
            "vram" => write_byte(self.vram_mut(), offset, value),
            _ => false,
        }
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
        fixed_audio_spec(
            self.audio_sample_rate_hz,
            self.emulator.audio_output_channels(),
        )
    }

    fn configure_audio_output(&mut self, sample_rate_hz: u32) {
        self.audio_sample_rate_hz = sample_rate_hz;
        self.emulator
            .set_audio_output_sample_rate_hz(sample_rate_hz);
    }

    fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        let max_samples = ((self.audio_sample_rate_hz as usize) / 20).max(1024) * 2;
        replace_audio_buffer(out, self.emulator.drain_audio_samples(max_samples));
    }

    fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        save_state_slot(E::SYSTEM, &self.rom_path, slot, E::STATE_EXT, |path| {
            self.emulator.save_state_to_file(path)
        })
    }

    fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        load_state_slot(E::SYSTEM, &self.rom_path, slot, E::STATE_EXT, |path| {
            self.emulator.load_state_from_file(path)
        })
    }
}

impl<E> Sega8Adapter<E>
where
    E: Sega8Emulator + Sega8Platform,
{
    fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        self.emulator.set_virtual_button(player, button, pressed);
    }

    fn memory_regions(&self) -> Vec<MemoryRegion> {
        self.emulator.memory_regions()
    }

    fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        self.emulator.read_memory(region_id)
    }

    fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        self.emulator.write_memory_byte(region_id, offset, value)
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

            pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
                self.0.set_button(player, button, pressed);
            }

            pub fn memory_regions(&self) -> Vec<MemoryRegion> {
                self.0.memory_regions()
            }

            pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
                self.0.read_memory(region_id)
            }

            pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
                self.0.write_memory_byte(region_id, offset, value)
            }
        }
    };
}

pub struct Sg1000Adapter(Sega8Adapter<SgEmulator>);

impl_common_adapter!(Sg1000Adapter);

pub struct MasterSystemAdapter(Sega8Adapter<SmsEmulator>);

impl_common_adapter!(MasterSystemAdapter);

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
