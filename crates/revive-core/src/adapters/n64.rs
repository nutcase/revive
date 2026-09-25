use super::common::{fixed_audio_spec, load_state_slot, save_state_slot};
use crate::{
    paths::rom_stem,
    system::{AudioSpec, FrameView, MemoryRegion, PixelFormat, Result, SystemKind, VirtualButton},
};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub struct N64Adapter {
    emulator: n64_core::Emulator,
    rom_path: PathBuf,
    save_path: PathBuf,
    saved_data: Vec<u8>,
    frames_since_save: u32,
    title: String,
}
impl N64Adapter {
    pub fn load(path: &Path) -> Result<Self> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if !(4096..=64 * 1024 * 1024).contains(&size) {
            return Err("N64 cartridge must be 4 KiB–64 MiB".into());
        }
        let rom = std::fs::read(path).map_err(|e| e.to_string())?;
        let save_dir = Path::new("states").join("n64").join(rom_stem(path));
        std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
        let save_dir = std::fs::canonicalize(save_dir).map_err(|e| e.to_string())?;
        let save_path = save_dir.join("cartridge.srm");
        let mut emulator = n64_core::Emulator::load(rom, &save_dir)?;
        match read_sized_file(&save_path, emulator.persistent_save().len()) {
            Ok(data) => emulator.load_persistent_save(&data)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "Cannot load N64 cartridge/Controller Pak save: {e}"
                ))
            }
        }
        let saved_data = emulator.persistent_save().to_vec();
        Ok(Self {
            emulator,
            rom_path: path.into(),
            save_path,
            saved_data,
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
    pub fn set_button(&mut self, player: u8, button: VirtualButton, pressed: bool) {
        if player != 1 {
            return;
        }
        if let Some(id) = button_id(button) {
            self.emulator.set_button(u32::from(player - 1), id, pressed);
        }
    }
    pub fn set_stick(&mut self, player: u8, x: i16, y: i16) {
        if player == 1 {
            self.emulator.set_stick(0, x, y);
        }
    }
    pub fn memory_regions(&self) -> Vec<MemoryRegion> {
        vec![MemoryRegion {
            id: "rdram",
            label: "N64 RDRAM",
            len: self.emulator.ram().len(),
            writable: true,
        }]
    }
    pub fn read_memory(&self, region_id: &str) -> Option<&[u8]> {
        (region_id == "rdram").then(|| self.emulator.ram())
    }
    pub fn write_memory_byte(&mut self, region_id: &str, offset: usize, value: u8) -> bool {
        region_id == "rdram" && self.emulator.write_ram(offset, value)
    }
    pub fn save_state_to_slot(&mut self, slot: u8) -> Result<()> {
        let data = self.emulator.save_state()?;
        save_state_slot(
            SystemKind::Nintendo64,
            &self.rom_path,
            slot,
            "n64st",
            |path| atomic_write(path, &data),
        )
    }
    pub fn load_state_from_slot(&mut self, slot: u8) -> Result<()> {
        let expected_size = self.emulator.state_size();
        load_state_slot(
            SystemKind::Nintendo64,
            &self.rom_path,
            slot,
            "n64st",
            |path| {
                let data = read_sized_file(path, expected_size).map_err(|e| e.to_string())?;
                self.emulator.load_state(&data)
            },
        )
    }
    pub fn flush_persistent_save(&mut self) -> Result<()> {
        let card = self.emulator.persistent_save();
        if card != self.saved_data || !self.save_path.exists() {
            atomic_write(&self.save_path, card)?;
            self.saved_data.clear();
            self.saved_data.extend_from_slice(card);
        }
        Ok(())
    }
}
// Reject stale/truncated/oversized files before allocating their contents. Limit
// the actual read too, in case the file changes after its metadata was checked.
fn read_sized_file(path: &Path, expected: usize) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() != expected as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid N64 save file size",
        ));
    }
    let mut data = Vec::with_capacity(expected);
    file.take(expected as u64 + 1).read_to_end(&mut data)?;
    if data.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "N64 save file changed while reading",
        ));
    }
    Ok(data)
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
        VirtualButton::A => 0,
        VirtualButton::B => 1,
        VirtualButton::L => 2,
        VirtualButton::Start => 3,
        VirtualButton::Up => 4,
        VirtualButton::Down => 5,
        VirtualButton::Left => 6,
        VirtualButton::Right => 7,
        VirtualButton::CDown => 8,
        VirtualButton::CUp => 9,
        VirtualButton::CLeft => 10,
        VirtualButton::CRight => 11,
        VirtualButton::Z => 12,
        VirtualButton::R => 13,
        _ => return None,
    })
}
