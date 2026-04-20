mod paths;
mod restore;
mod snapshot;

use crate::{save_state, Nes};

impl Nes {
    /// Save the current emulator snapshot to a numbered slot.
    pub fn save_state(&self, slot: u8, _rom_filename: &str) -> crate::Result<()> {
        let rom_stem = self.rom_stem();
        let save_state = self.build_save_state(&rom_stem)?;
        paths::ensure_save_state_dir(&rom_stem)?;
        let filename = self.save_state_path(slot, &rom_stem);
        save_state.save_to_file(&filename)?;
        Ok(())
    }

    /// Restore an emulator snapshot from a numbered slot.
    pub fn load_state(&mut self, slot: u8) -> crate::Result<()> {
        let rom_stem = self.rom_stem();
        let filename = self.readable_save_state_path(slot, &rom_stem);
        let save_state = save_state::SaveState::load_from_file(&filename)?;
        self.restore_save_state(&save_state)
    }
}
