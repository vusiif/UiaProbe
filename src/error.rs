use std::fmt;

/// Typed errors returned by UiaProbe operations.
#[derive(Debug)]
pub enum ProbeError {
    /// Failed to initialize COM or create the UIA automation instance.
    ComInit(String),
    /// Target window handle is invalid, closed, or unavailable.
    InvalidWindow(String),
    /// UIA traversal or property access failed.
    UiaFailure(String),
    /// Invalid CLI arguments or configuration.
    InvalidArgs(String),
    /// JSON serialization failed.
    Serialization(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComInit(msg) => write!(f, "COM initialization error: {msg}"),
            Self::InvalidWindow(msg) => write!(f, "Invalid window: {msg}"),
            Self::UiaFailure(msg) => write!(f, "UIA error: {msg}"),
            Self::InvalidArgs(msg) => write!(f, "Invalid arguments: {msg}"),
            Self::Serialization(msg) => write!(f, "Serialization error: {msg}"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// Convenience result alias for UiaProbe.
pub type Result<T> = std::result::Result<T, ProbeError>;
