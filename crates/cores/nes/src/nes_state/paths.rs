use std::path::Path;

use crate::Nes;

impl Nes {
    /// Derive a filesystem-safe ROM stem from the loaded ROM path.
    pub(in crate::nes_state) fn rom_stem(&self) -> String {
        self.current_rom_path
            .as_deref()
            .and_then(|p| Path::new(p).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    pub(in crate::nes_state) fn save_state_path(&self, slot: u8, rom_stem: &str) -> String {
        format!("states/nes/{}/slot{}.sav", rom_stem, slot)
    }

    pub(in crate::nes_state) fn readable_save_state_path(
        &self,
        slot: u8,
        rom_stem: &str,
    ) -> String {
        let path = self.save_state_path(slot, rom_stem);
        if Path::new(&path).exists() {
            path
        } else {
            self.legacy_save_state_path(slot, rom_stem)
        }
    }

    fn legacy_save_state_path(&self, slot: u8, rom_stem: &str) -> String {
        format!("states/{}.slot{}.sav", rom_stem, slot)
    }
}

pub(super) fn ensure_save_state_dir(rom_stem: &str) -> crate::Result<()> {
    let dir = Path::new("states").join("nes").join(rom_stem);
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
