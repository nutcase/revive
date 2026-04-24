use std::path::{Path, PathBuf};

use nes_emulator::Nes;

use super::common::write_byte;
use crate::paths::{ensure_readable_nes_state_path, rom_stem};
use crate::system::{AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, VirtualButton};

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
        ensure_readable_nes_state_path(&self.rom_path, slot)?;
        self.nes.load_state(slot).map_err(|err| err.to_string())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        self.nes.save_sram().map_err(|err| err.to_string())
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
