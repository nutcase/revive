use super::format::{self, LoadedSaveStateFormat};
use super::types::SaveState;
use crate::Result;

impl SaveState {
    /// Write this snapshot using the current wrapped save-state format.
    pub fn save_to_file(&self, filename: &str) -> Result<()> {
        let data = format::encode_current(self)?;
        std::fs::write(filename, data)?;
        log::info!("Save state written to: {}", filename);
        Ok(())
    }

    /// Load a snapshot, accepting the current wrapper plus legacy raw formats.
    pub fn load_from_file(filename: &str) -> Result<SaveState> {
        let data = std::fs::read(filename)?;
        let (save_state, format) = format::decode_any(&data)?;
        match format {
            LoadedSaveStateFormat::CurrentWrapper(version) => {
                log::info!("Save state loaded from: {} (v{} format)", filename, version);
            }
            LoadedSaveStateFormat::CurrentWrapperU8Mapper(version) => {
                log::info!(
                    "Save state loaded from: {} (v{} format, migrated mapper id)",
                    filename,
                    version
                );
            }
            LoadedSaveStateFormat::CurrentRaw => {
                log::info!("Save state loaded from: {}", filename);
            }
            LoadedSaveStateFormat::CurrentRawU8Mapper => {
                log::info!("Save state loaded from: {} (migrated mapper id)", filename);
            }
            LoadedSaveStateFormat::V2 => {
                log::info!("Save state loaded from: {} (v2 format)", filename);
            }
            LoadedSaveStateFormat::V1 => {
                log::info!("Save state loaded from: {} (v1 format)", filename);
            }
            LoadedSaveStateFormat::Legacy => {
                log::info!("Save state loaded from: {} (legacy format)", filename);
            }
        }
        Ok(save_state)
    }
}
