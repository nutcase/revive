use std::path::{Path, PathBuf};

use pce::emulator::Emulator as PceEmulator;

use super::common::{argb8888_u32_frame_as_bgra8888_bytes, write_byte};
use crate::paths::{readable_state_path, rom_stem, state_path};
use crate::system::{
    AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton,
};

pub struct PceAdapter {
    emulator: PceEmulator,
    rom_path: PathBuf,
    title: String,
    hucard: bool,
    pad_state: u8,
    latest_frame: Vec<u32>,
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

        let byte_len = width * raw_height * std::mem::size_of::<u32>();
        let frame_bytes = &argb8888_u32_frame_as_bgra8888_bytes(&self.latest_frame)[..byte_len];

        FrameView {
            width,
            height: raw_height,
            format: PixelFormat::Bgra8888,
            data: frame_bytes,
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
        self.emulator.drain_audio_samples_into(out);
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
        let path = readable_state_path(SystemKind::Pce, &self.rom_path, slot, "pcst")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pce_frame_uses_core_display_height() {
        let emulator = PceEmulator::new();
        let width = emulator.display_width();
        let height = emulator.display_height();
        let mut latest_frame = vec![0; width * height];
        latest_frame[width * height - 1] = 0xFF12_3456;
        let mut adapter = PceAdapter {
            emulator,
            rom_path: PathBuf::from("dummy.pce"),
            title: "dummy".to_string(),
            hucard: true,
            pad_state: 0xFF,
            latest_frame,
        };

        let frame = adapter.frame();

        assert_eq!(frame.height, height);
        assert_eq!(
            frame.data.len(),
            width * height * std::mem::size_of::<u32>()
        );
        assert_eq!(
            &frame.data[frame.data.len() - 4..],
            &[0x56, 0x34, 0x12, 0xFF]
        );
    }
}
