use std::fs::{create_dir_all, File};
use std::io::{Read, Result, Write};
use std::path::{Path, PathBuf};

pub(crate) fn get_save_file_path(rom_path: &str) -> PathBuf {
    let path = Path::new(rom_path);
    let mut save_path = path.to_path_buf();
    save_path.set_extension("sav");
    save_path
}

pub(crate) fn load_sram(rom_path: &str) -> Result<Option<Vec<u8>>> {
    let save_path = get_save_file_path(rom_path);

    if !save_path.exists() {
        return Ok(None);
    }

    let mut file = File::open(&save_path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;

    Ok(Some(data))
}

pub(crate) fn save_sram(rom_path: &str, data: &[u8]) -> Result<()> {
    let save_path = get_save_file_path(rom_path);

    // Create directory if it doesn't exist
    if let Some(parent) = save_path.parent() {
        create_dir_all(parent)?;
    }

    let mut file = File::create(&save_path)?;
    file.write_all(data)?;
    file.sync_all()?;

    Ok(())
}
