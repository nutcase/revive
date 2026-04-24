use std::path::Path;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemKind {
    Nes,
    Snes,
    Sg1000,
    MasterSystem,
    MegaDrive,
    Pce,
    GameBoy,
    GameBoyColor,
    GameBoyAdvance,
}

#[derive(Debug, Clone, Copy)]
pub struct SystemInfo {
    pub kind: SystemKind,
    pub label: &'static str,
    pub storage_dir: &'static str,
    pub state_extension: &'static str,
    pub rom_extensions: &'static [&'static str],
    pub dialog_extensions: &'static [&'static str],
    pub frame_rate_hz: f64,
}

const NES_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::Nes,
    label: "NES",
    storage_dir: "nes",
    state_extension: "sav",
    rom_extensions: &["nes"],
    dialog_extensions: &["nes"],
    frame_rate_hz: 60.0988,
};

const SNES_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::Snes,
    label: "SNES",
    storage_dir: "snes",
    state_extension: "sns",
    rom_extensions: &["sfc", "smc"],
    dialog_extensions: &["sfc", "smc"],
    frame_rate_hz: 60.0988,
};

const SG1000_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::Sg1000,
    label: "SG-1000",
    storage_dir: "sg1000",
    state_extension: "sgs",
    rom_extensions: &["sg", "sg1000"],
    dialog_extensions: &["sg", "sg1000"],
    frame_rate_hz: 60.0,
};

const MASTER_SYSTEM_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::MasterSystem,
    label: "Master System",
    storage_dir: "mastersystem",
    state_extension: "smsst",
    rom_extensions: &["sms", "mk3"],
    dialog_extensions: &["sms", "mk3"],
    frame_rate_hz: 59.9227,
};

const MEGA_DRIVE_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::MegaDrive,
    label: "Mega Drive",
    storage_dir: "megadrive",
    state_extension: "mdst",
    rom_extensions: &["md", "gen", "genesis"],
    dialog_extensions: &["md", "gen", "genesis", "bin"],
    frame_rate_hz: 59.9227,
};

const PCE_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::Pce,
    label: "PC Engine",
    storage_dir: "pce",
    state_extension: "pcst",
    rom_extensions: &["pce"],
    dialog_extensions: &["pce"],
    frame_rate_hz: 60.0,
};

const GAME_BOY_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::GameBoy,
    label: "Game Boy",
    storage_dir: "gb",
    state_extension: "",
    rom_extensions: &["gb"],
    dialog_extensions: &["gb"],
    frame_rate_hz: 59.7275,
};

const GAME_BOY_COLOR_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::GameBoyColor,
    label: "Game Boy Color",
    storage_dir: "gbc",
    state_extension: "",
    rom_extensions: &["gbc"],
    dialog_extensions: &["gbc"],
    frame_rate_hz: 59.7275,
};

const GAME_BOY_ADVANCE_INFO: SystemInfo = SystemInfo {
    kind: SystemKind::GameBoyAdvance,
    label: "Game Boy Advance",
    storage_dir: "gba",
    state_extension: "gbas",
    rom_extensions: &["gba"],
    dialog_extensions: &["gba"],
    frame_rate_hz: 59.7275,
};

pub const ALL_SYSTEMS: [SystemKind; 9] = [
    SystemKind::Nes,
    SystemKind::Snes,
    SystemKind::Sg1000,
    SystemKind::MasterSystem,
    SystemKind::MegaDrive,
    SystemKind::Pce,
    SystemKind::GameBoy,
    SystemKind::GameBoyColor,
    SystemKind::GameBoyAdvance,
];

pub const ROM_EXTENSIONS: &[&str] = &[
    "nes", "sfc", "smc", "sg", "sg1000", "sms", "mk3", "md", "gen", "genesis", "pce", "gb", "gbc",
    "gba", "bin",
];

impl SystemKind {
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_lowercase().as_str() {
            "auto" => None,
            "nes" | "fc" | "famicom" => Some(Self::Nes),
            "snes" | "sfc" | "super-famicom" | "superfamicom" => Some(Self::Snes),
            "sg1000" | "sg-1000" | "sega-sg1000" => Some(Self::Sg1000),
            "sms" | "mastersystem" | "master-system" | "sega-master-system" | "markiii"
            | "mark-iii" => Some(Self::MasterSystem),
            "md" | "genesis" | "megadrive" | "mega-drive" => Some(Self::MegaDrive),
            "pce" | "pcengine" | "pc-engine" | "tg16" | "turbografx" | "turbografx-16" => {
                Some(Self::Pce)
            }
            "gb" | "gameboy" | "game-boy" => Some(Self::GameBoy),
            "gbc" | "gameboycolor" | "game-boy-color" | "gameboy-color" => Some(Self::GameBoyColor),
            "gba" | "gameboyadvance" | "game-boy-advance" | "gameboy-advance" => {
                Some(Self::GameBoyAdvance)
            }
            _ => None,
        }
    }

    pub fn info(self) -> &'static SystemInfo {
        match self {
            Self::Nes => &NES_INFO,
            Self::Snes => &SNES_INFO,
            Self::Sg1000 => &SG1000_INFO,
            Self::MasterSystem => &MASTER_SYSTEM_INFO,
            Self::MegaDrive => &MEGA_DRIVE_INFO,
            Self::Pce => &PCE_INFO,
            Self::GameBoy => &GAME_BOY_INFO,
            Self::GameBoyColor => &GAME_BOY_COLOR_INFO,
            Self::GameBoyAdvance => &GAME_BOY_ADVANCE_INFO,
        }
    }

    pub fn label(self) -> &'static str {
        self.info().label
    }

    pub fn storage_dir(self) -> &'static str {
        self.info().storage_dir
    }

    pub fn state_extension(self) -> &'static str {
        self.info().state_extension
    }

    pub fn frame_rate_hz(self) -> f64 {
        self.info().frame_rate_hz
    }

    pub fn rom_extensions(self) -> &'static [&'static str] {
        self.info().rom_extensions
    }

    pub fn dialog_extensions(self) -> &'static [&'static str] {
        self.info().dialog_extensions
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L,
    R,
    Start,
    Select,
    C,
    Z,
    Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb24,
    Rgba8888,
    Bgra8888,
}

pub struct FrameView<'a> {
    pub width: usize,
    pub height: usize,
    pub format: PixelFormat,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub id: &'static str,
    pub label: &'static str,
    pub len: usize,
    pub writable: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct AudioSpec {
    pub sample_rate_hz: u32,
    pub channels: u8,
}

impl Default for AudioSpec {
    fn default() -> Self {
        Self {
            sample_rate_hz: 44_100,
            channels: 2,
        }
    }
}

pub fn detect_system(path: &Path) -> Result<SystemKind> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if ext == "bin" {
        let data = std::fs::read(path).map_err(|err| err.to_string())?;
        if data.len() >= 0x104 && &data[0x100..0x104] == b"SEGA" {
            return Ok(SystemKind::MegaDrive);
        }
        return Err("ambiguous .bin ROM; pass --system".to_string());
    }

    for system in ALL_SYSTEMS {
        if system.rom_extensions().contains(&ext.as_str()) {
            return Ok(system);
        }
    }

    Err(format!(
        "could not infer system from extension '.{ext}'; pass --system"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_system_from_standard_extensions() {
        assert_eq!(
            detect_system(Path::new("game.nes")).unwrap(),
            SystemKind::Nes
        );
        assert_eq!(
            detect_system(Path::new("game.sfc")).unwrap(),
            SystemKind::Snes
        );
        assert_eq!(
            detect_system(Path::new("game.smc")).unwrap(),
            SystemKind::Snes
        );
        assert_eq!(
            detect_system(Path::new("game.sg")).unwrap(),
            SystemKind::Sg1000
        );
        assert_eq!(
            detect_system(Path::new("game.sg1000")).unwrap(),
            SystemKind::Sg1000
        );
        assert_eq!(
            detect_system(Path::new("game.sms")).unwrap(),
            SystemKind::MasterSystem
        );
        assert_eq!(
            detect_system(Path::new("game.mk3")).unwrap(),
            SystemKind::MasterSystem
        );
        assert_eq!(
            detect_system(Path::new("game.md")).unwrap(),
            SystemKind::MegaDrive
        );
        assert_eq!(
            detect_system(Path::new("game.gen")).unwrap(),
            SystemKind::MegaDrive
        );
        assert_eq!(
            detect_system(Path::new("game.pce")).unwrap(),
            SystemKind::Pce
        );
        assert_eq!(
            detect_system(Path::new("game.gb")).unwrap(),
            SystemKind::GameBoy
        );
        assert_eq!(
            detect_system(Path::new("game.gbc")).unwrap(),
            SystemKind::GameBoyColor
        );
        assert_eq!(
            detect_system(Path::new("game.gba")).unwrap(),
            SystemKind::GameBoyAdvance
        );
    }

    #[test]
    fn detects_megadrive_bin_header() {
        let path = std::env::temp_dir().join(format!("revive-md-{}.bin", std::process::id()));
        let mut rom = vec![0; 0x200];
        rom[0x100..0x104].copy_from_slice(b"SEGA");
        std::fs::write(&path, rom).unwrap();

        let detected = detect_system(&path).unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(detected, SystemKind::MegaDrive);
    }
}
