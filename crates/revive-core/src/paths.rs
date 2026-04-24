use std::path::{Path, PathBuf};

use crate::system::{Result, SystemKind};

pub(crate) fn rom_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game")
        .to_string()
}

fn state_file_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let stem = rom_stem(rom_path);
    Path::new("states")
        .join(system.storage_dir())
        .join(stem)
        .join(format!("slot{slot}.{ext}"))
}

pub(crate) fn state_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let path = state_file_path(system, rom_path, slot, ext);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    path
}

pub(crate) fn readable_state_path(
    system: SystemKind,
    rom_path: &Path,
    slot: u8,
    ext: &str,
) -> Result<PathBuf> {
    let path = state_file_path(system, rom_path, slot, ext);
    if path.exists() {
        return Ok(path);
    }

    let legacy_path = legacy_state_path(system, rom_path, slot, ext);
    if legacy_path.exists() {
        return Ok(legacy_path);
    }

    Err(missing_state_file_error(&[path, legacy_path]))
}

fn legacy_state_path(system: SystemKind, rom_path: &Path, slot: u8, ext: &str) -> PathBuf {
    let stem = rom_stem(rom_path);
    Path::new("states")
        .join(system.storage_dir())
        .join(format!("{stem}.slot{slot}.{ext}"))
}

pub(crate) fn ensure_readable_nes_state_path(rom_path: &Path, slot: u8) -> Result<()> {
    let path = state_file_path(SystemKind::Nes, rom_path, slot, "sav");
    if path.exists() {
        return Ok(());
    }

    let legacy_path = legacy_nes_state_path(rom_path, slot);
    if legacy_path.exists() {
        return Ok(());
    }

    Err(missing_state_file_error(&[path, legacy_path]))
}

fn legacy_nes_state_path(rom_path: &Path, slot: u8) -> PathBuf {
    let stem = rom_stem(rom_path);
    Path::new("states").join(format!("{stem}.slot{slot}.sav"))
}

fn missing_state_file_error(paths: &[PathBuf]) -> String {
    let looked_for = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(" or ");
    format!("no saved state file found (looked for {looked_for})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_state_file_error_lists_candidate_paths() {
        let err = missing_state_file_error(&[
            PathBuf::from("states/snes/game/slot1.sns"),
            PathBuf::from("states/snes/game.slot1.sns"),
        ]);

        assert_eq!(
            err,
            "no saved state file found (looked for states/snes/game/slot1.sns or states/snes/game.slot1.sns)"
        );
    }

    #[test]
    fn nes_legacy_state_path_matches_vendored_core_layout() {
        let path = legacy_nes_state_path(Path::new("roms/Mario.nes"), 1);

        assert_eq!(path, PathBuf::from("states/Mario.slot1.sav"));
    }
}
