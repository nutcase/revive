use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

pub type EmuResult<T> = Result<T, EmuError>;

#[derive(Debug)]
pub enum EmuError {
    Io {
        path: Option<PathBuf>,
        source: std::io::Error,
    },
    InvalidRom(&'static str),
    InvalidState(&'static str),
}

impl EmuError {
    fn io(path: Option<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path, source }
    }
}

impl fmt::Display for EmuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => match path {
                Some(path) => write!(f, "I/O error on {}: {source}", path.display()),
                None => write!(f, "I/O error: {source}"),
            },
            Self::InvalidRom(message) => write!(f, "Invalid ROM: {message}"),
            Self::InvalidState(message) => write!(f, "Invalid emulator state: {message}"),
        }
    }
}

impl Error for EmuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidRom(_) | Self::InvalidState(_) => None,
        }
    }
}

impl From<std::io::Error> for EmuError {
    fn from(source: std::io::Error) -> Self {
        Self::io(None, source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleKind {
    Gb,
    Gbc,
    Gba,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameResult {
    pub cycles: u32,
    pub frame_number: u64,
}

#[derive(Debug, Clone)]
pub struct RomImage {
    bytes: Vec<u8>,
    path: Option<PathBuf>,
}

impl RomImage {
    pub fn from_file(path: impl AsRef<Path>) -> EmuResult<Self> {
        let path = path.as_ref();
        let bytes =
            fs::read(path).map_err(|source| EmuError::io(Some(path.to_path_buf()), source))?;
        Self::from_bytes_with_origin(bytes, Some(path.to_path_buf()))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> EmuResult<Self> {
        Self::from_bytes_with_origin(bytes, None)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn from_bytes_with_origin(bytes: Vec<u8>, path: Option<PathBuf>) -> EmuResult<Self> {
        if bytes.is_empty() {
            return Err(EmuError::InvalidRom("ROM is empty"));
        }

        Ok(Self { bytes, path })
    }
}

pub trait EmulatorCore {
    fn console_kind(&self) -> ConsoleKind;
    fn load_rom(&mut self, rom: RomImage) -> EmuResult<()>;
    fn reset(&mut self);
    fn step_frame(&mut self) -> EmuResult<FrameResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rom_is_rejected() {
        let rom = RomImage::from_bytes(Vec::new());
        assert!(matches!(rom, Err(EmuError::InvalidRom(_))));
    }

    #[test]
    fn valid_rom_from_bytes_is_accepted() {
        let rom = RomImage::from_bytes(vec![0x00, 0xC3, 0x50, 0x01]).expect("ROM should parse");
        assert_eq!(rom.len(), 4);
        assert!(!rom.is_empty());
    }
}
