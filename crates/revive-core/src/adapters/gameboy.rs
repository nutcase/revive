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

use super::common::{
    fixed_audio_spec, load_state_slot, save_state_slot, write_byte, write_file, write_optional_file,
};
use crate::paths::rom_stem;
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

pub struct GameBoyAdapter {
    emulator: GbEmulator,
    rom_path: PathBuf,
    title: String,
    system: SystemKind,
    pressed_mask: u8,
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
        FrameView {
            width: GB_LCD_WIDTH as usize,
            height: GB_LCD_HEIGHT as usize,
            format: PixelFormat::Rgba8888,
            data: self.emulator.frame_rgba8888(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        fixed_audio_spec(self.audio_sample_rate_hz, 2)
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
        let mut regions = vec![
            MemoryRegion {
                id: "wram",
                label: "Work RAM",
                len: gameboy_work_ram_len(self.system),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "Video RAM",
                len: gameboy_video_ram_len(self.system),
                writable: true,
            },
            MemoryRegion {
                id: "oam",
                label: "Object Attribute Memory",
                len: self.emulator.oam().len(),
                writable: true,
            },
            MemoryRegion {
                id: "hram",
                label: "High RAM",
                len: self.emulator.high_ram().len(),
                writable: true,
            },
        ];
        if let Some(ram) = self.emulator.backup_data() {
            regions.push(MemoryRegion {
                id: "cart_ram",
                label: "Cartridge RAM",
                len: ram.len(),
                writable: true,
            });
        }
        regions
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "wram" => Some(&self.emulator.work_ram()[..gameboy_work_ram_len(self.system)]),
            "vram" => Some(&self.emulator.video_ram()[..gameboy_video_ram_len(self.system)]),
            "oam" => Some(self.emulator.oam()),
            "hram" => Some(self.emulator.high_ram()),
            "cart_ram" => self.emulator.backup_data(),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "wram" => write_byte(
                &mut self.emulator.work_ram_mut()[..gameboy_work_ram_len(self.system)],
                offset,
                value,
            ),
            "vram" => write_byte(
                &mut self.emulator.video_ram_mut()[..gameboy_video_ram_len(self.system)],
                offset,
                value,
            ),
            "oam" => write_byte(self.emulator.oam_mut(), offset, value),
            "hram" => write_byte(self.emulator.high_ram_mut(), offset, value),
            "cart_ram" => self
                .emulator
                .backup_data_mut()
                .map(|ram| write_byte(ram, offset, value))
                .unwrap_or(false),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, _slot: u8) -> Result<()> {
        Err("Game Boy save states are not exposed by ../gameboy yet".to_string())
    }

    pub fn load_state_from_slot(&mut self, _slot: u8) -> Result<()> {
        Err("Game Boy save states are not exposed by ../gameboy yet".to_string())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        write_optional_file(
            &self.rom_path.with_extension("sav"),
            self.emulator.backup_data(),
        )
    }
}

pub struct GameBoyAdvanceAdapter {
    emulator: GbaEmulator,
    frame_buffer: GbaFrameBuffer,
    rom_path: PathBuf,
    title: String,
    pressed_mask: u16,
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
        FrameView {
            width: GBA_LCD_WIDTH as usize,
            height: GBA_LCD_HEIGHT as usize,
            format: PixelFormat::Rgba8888,
            data: self.frame_buffer.pixels(),
        }
    }

    pub fn audio_spec(&self) -> AudioSpec {
        fixed_audio_spec(self.audio_sample_rate_hz, 2)
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
        vec![
            MemoryRegion {
                id: "ewram",
                label: "External Work RAM",
                len: self.emulator.ewram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "iwram",
                label: "Internal Work RAM",
                len: self.emulator.iwram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "pram",
                label: "Palette RAM",
                len: self.emulator.pram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "vram",
                label: "Video RAM",
                len: self.emulator.vram().len(),
                writable: true,
            },
            MemoryRegion {
                id: "oam",
                label: "Object Attribute Memory",
                len: self.emulator.oam().len(),
                writable: true,
            },
        ]
    }

    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        match region_id {
            "ewram" => Some(self.emulator.ewram()),
            "iwram" => Some(self.emulator.iwram()),
            "pram" => Some(self.emulator.pram()),
            "vram" => Some(self.emulator.vram()),
            "oam" => Some(self.emulator.oam()),
            _ => None,
        }
    }

    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        match region_id {
            "ewram" => write_byte(self.emulator.ewram_mut(), offset, value),
            "iwram" => write_byte(self.emulator.iwram_mut(), offset, value),
            "pram" => write_byte(self.emulator.pram_mut(), offset, value),
            "vram" => write_byte(self.emulator.vram_mut(), offset, value),
            "oam" => write_byte(self.emulator.oam_mut(), offset, value),
            _ => false,
        }
    }

    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        save_state_slot(
            SystemKind::GameBoyAdvance,
            &self.rom_path,
            slot,
            "gbas",
            |path| {
                let state_data = self.emulator.save_state();
                write_file(path, &state_data)
            },
        )
    }

    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let mut state_data = Vec::new();
        load_state_slot(
            SystemKind::GameBoyAdvance,
            &self.rom_path,
            slot,
            "gbas",
            |path| {
                state_data = std::fs::read(path).map_err(|err| err.to_string())?;
                Ok(())
            },
        )?;
        self.emulator
            .load_state(&state_data)
            .map_err(|err| err.to_string())?;
        self.emulator.set_keyinput_pressed_mask(self.pressed_mask);
        self.emulator.render_frame_rgba8888(&mut self.frame_buffer);
        Ok(())
    }

    pub fn flush_persistent_save(&mut self) -> Result<()> {
        write_optional_file(
            &self.rom_path.with_extension("sav"),
            self.emulator.backup_data().as_deref(),
        )
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

fn gameboy_work_ram_len(system: SystemKind) -> usize {
    match system {
        SystemKind::GameBoyColor => 32 * 1024,
        _ => 8 * 1024,
    }
}

fn gameboy_video_ram_len(system: SystemKind) -> usize {
    match system {
        SystemKind::GameBoyColor => 16 * 1024,
        _ => 8 * 1024,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gameboy_adapter_exposes_writable_memory_regions() {
        let mut adapter = GameBoyAdapter {
            emulator: GbEmulator::new(GbModel::Dmg),
            rom_path: PathBuf::from("dummy.gb"),
            title: "dummy".to_string(),
            system: SystemKind::GameBoy,
            pressed_mask: 0,
            audio_sample_rate_hz: 44_100,
        };

        assert!(adapter
            .memory_regions()
            .iter()
            .any(|region| region.id == "wram"));
        assert!(adapter.write_memory_byte("wram", 0x12, 0x34));
        assert_eq!(adapter.read_memory("wram").unwrap()[0x12], 0x34);
    }

    #[test]
    fn gba_adapter_exposes_writable_work_ram_regions() {
        let mut adapter = GameBoyAdvanceAdapter {
            emulator: GbaEmulator::new(),
            frame_buffer: GbaFrameBuffer::new(),
            rom_path: PathBuf::from("dummy.gba"),
            title: "dummy".to_string(),
            pressed_mask: 0,
            audio_sample_rate_hz: 44_100,
        };

        assert!(adapter
            .memory_regions()
            .iter()
            .any(|region| region.id == "ewram"));
        assert!(adapter.write_memory_byte("iwram", 0x20, 0x56));
        assert_eq!(adapter.read_memory("iwram").unwrap()[0x20], 0x56);
    }
}
