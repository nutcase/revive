use super::common::{fixed_audio_spec, load_state_slot, save_state_slot};
use crate::{
    paths::rom_stem,
    ps1_disc::Ps1Disc,
    system::{AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub struct Ps1Adapter {
    // Close native file handles before the extracted disc is removed.
    emulator: ps1_core::Emulator,
    _disc: Ps1Disc,
    rom_path: PathBuf,
    card_path: PathBuf,
    saved_card: Vec<u8>,
    frames_since_save: u32,
    title: String,
}
impl Ps1Adapter {
    pub fn load(path: &Path) -> Result<Self> {
        let disc = Ps1Disc::open(path)?;
        let save_dir = Path::new("states").join("ps1").join(rom_stem(path));
        std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
        let save_dir = std::fs::canonicalize(save_dir).map_err(|e| e.to_string())?;
        let card_path = save_dir.join("memory-card.mcd");
        let mut emulator = ps1_core::Emulator::load(&disc.path, &save_dir)?;
        match std::fs::read(&card_path) {
            Ok(data) => emulator.load_memory_card(&data)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Cannot load PS1 memory card: {e}")),
        }
        let saved_card = emulator.memory_card().to_vec();
        Ok(Self {
            emulator,
            _disc: disc,
            rom_path: path.into(),
            card_path,
            saved_card,
            frames_since_save: 0,
            title: rom_stem(path),
        })
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn frame_rate_hz(&self) -> f64 {
        self.emulator.frame_rate_hz()
    }
    pub fn set_audio_output_enabled(&mut self, enabled: bool) {
        self.emulator.set_audio_enabled(enabled);
    }
    pub fn step_frame(&mut self) -> Result<()> {
        self.emulator.step_frame()?;
        self.frames_since_save += 1;
        if self.frames_since_save >= 60 {
            self.flush_persistent_save()?;
            self.frames_since_save = 0;
        }
        Ok(())
    }
    pub fn frame(&mut self) -> FrameView<'_> {
        let (data, width, height) = self.emulator.frame();
        FrameView {
            width,
            height,
            format: PixelFormat::Rgba8888,
            data,
        }
    }
    pub fn audio_spec(&self) -> AudioSpec {
        fixed_audio_spec(44_100, 2)
    }
    // SDL's audio stream converts from the declared 44.1 kHz source format.
    pub fn configure_audio_output(&mut self, _sample_rate_hz: u32) {}
    pub fn drain_audio_i16(&mut self, out: &mut Vec<i16>) {
        self.emulator.drain_audio(out);
    }
    pub fn set_stick(&mut self, _player: u8, _x: i16, _y: i16) {}

    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if !(1..=2).contains(&player) {
            return;
        }
        if let Some(id) = button_id(button) {
            self.emulator.set_button(u32::from(player - 1), id, pressed);
        }
    }
    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![MemoryRegion {
            id: "ram",
            label: "PS1 Main RAM",
            len: self.emulator.ram().len(),
            writable: true,
        }]
    }
    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        (region_id == "ram").then(|| self.emulator.ram())
    }
    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        region_id == "ram" && self.emulator.write_ram(offset, value)
    }
    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let data = self.emulator.save_state()?;
        save_state_slot(
            SystemKind::PlayStation,
            &self.rom_path,
            slot,
            "psst",
            |path| atomic_write(path, &data),
        )
    }
    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        load_state_slot(
            SystemKind::PlayStation,
            &self.rom_path,
            slot,
            "psst",
            |path| {
                let data = std::fs::read(path).map_err(|e| e.to_string())?;
                self.emulator.load_state(&data)
            },
        )
    }
    pub fn flush_persistent_save(&mut self) -> Result<()> {
        let card = self.emulator.memory_card();
        if card != self.saved_card || !self.card_path.exists() {
            atomic_write(&self.card_path, card)?;
            self.saved_card.clear();
            self.saved_card.extend_from_slice(card);
        }
        Ok(())
    }
}
fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Save path has no parent")?)
        .map_err(|e| e.to_string())?;
    temp.write_all(data).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn button_id(button: VirtualButton) -> Option<u32> {
    Some(match button {
        VirtualButton::B => 0,
        VirtualButton::Y => 1,
        VirtualButton::Select => 2,
        VirtualButton::Start => 3,
        VirtualButton::Up => 4,
        VirtualButton::Down => 5,
        VirtualButton::Left => 6,
        VirtualButton::Right => 7,
        VirtualButton::A => 8,
        VirtualButton::X => 9,
        VirtualButton::L => 10,
        VirtualButton::R => 11,
        VirtualButton::L2 => 12,
        VirtualButton::R2 => 13,
        _ => return None,
    })
}
