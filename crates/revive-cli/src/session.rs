use std::error::Error;
use std::path::{Path, PathBuf};

use revive_cheat::CheatManager;
use revive_core::{CoreInstance, SystemKind};

use crate::state::state_key_help;

pub(crate) struct CheatPaths {
    active: PathBuf,
    legacy_fallback: Option<PathBuf>,
}

impl CheatPaths {
    pub(crate) fn resolve(cli_path: Option<&Path>, system: SystemKind, rom_path: &Path) -> Self {
        match cli_path {
            Some(path) => Self {
                active: path.to_path_buf(),
                legacy_fallback: None,
            },
            None => Self {
                active: default_cheat_path(system, rom_path),
                legacy_fallback: Some(legacy_cheat_path(rom_path)),
            },
        }
    }

    pub(crate) fn active(&self) -> &Path {
        &self.active
    }

    pub(crate) fn load(&self, required: bool) -> Result<CheatManager, Box<dyn Error>> {
        if self.active.exists() {
            let manager = CheatManager::load_from_file(&self.active)?;
            println!("Loaded cheats: {}", manager.entries.len());
            return Ok(manager);
        }
        if let Some(legacy_path) = self
            .legacy_fallback
            .as_deref()
            .filter(|legacy_path| legacy_path.exists())
        {
            let manager = CheatManager::load_from_file(legacy_path)?;
            println!(
                "Loaded legacy cheats: {} ({})",
                manager.entries.len(),
                legacy_path.display()
            );
            return Ok(manager);
        }
        if required {
            return Err(format!("cheat file does not exist: {}", self.active.display()).into());
        }
        Ok(CheatManager::new())
    }
}

pub(crate) fn print_session_banner(core: &CoreInstance, rom_path: &Path, cheat_path: &Path) {
    println!("Loaded      : {}", rom_path.display());
    println!("System      : {}", core.system().label());
    println!("Title       : {}", core.title());
    println!("Cheats      : {}", cheat_path.display());
    for region in core.memory_regions() {
        println!(
            "Memory      : {} ({}, {} bytes)",
            region.id, region.label, region.len
        );
    }
    println!("State keys  : {}", state_key_help());
    println!("Controls    : arrows move, Enter start, Shift/Backspace select");
    println!("Cheat panel : Tab toggle");
}

fn default_cheat_path(system: SystemKind, rom_path: &Path) -> PathBuf {
    PathBuf::from("cheats")
        .join(system.storage_dir())
        .join(rom_file_stem(rom_path))
        .join("cheats.json")
}

fn legacy_cheat_path(rom_path: &Path) -> PathBuf {
    PathBuf::from("cheats").join(format!("{}.json", rom_file_stem(rom_path)))
}

fn rom_file_stem(rom_path: &Path) -> String {
    rom_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("game")
        .to_string()
}
