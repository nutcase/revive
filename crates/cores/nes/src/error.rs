use std::fmt;

/// Result type used by the core emulator API.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type used by core emulator operations.
#[derive(Debug)]
pub enum Error {
    /// Filesystem or ROM/save-state I/O failed.
    Io(std::io::Error),
    /// Save-state serialization or deserialization failed.
    SaveStateCodec(Box<bincode::ErrorKind>),
    /// System clock access failed while creating save-state metadata.
    SystemTime(std::time::SystemTimeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(error) => write!(f, "I/O error: {error}"),
            Error::SaveStateCodec(error) => write!(f, "save-state codec error: {error}"),
            Error::SystemTime(error) => write!(f, "system time error: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(error) => Some(error),
            Error::SaveStateCodec(error) => Some(error),
            Error::SystemTime(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<Box<bincode::ErrorKind>> for Error {
    fn from(error: Box<bincode::ErrorKind>) -> Self {
        Error::SaveStateCodec(error)
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Error::SystemTime(error)
    }
}
