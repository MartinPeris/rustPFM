//! Codec errors.
use std::{fmt, io};

/// A malformed image, a resource limit, or an I/O failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Filesystem or stream I/O failed.
    Io(io::Error),
    /// Image dimensions, samples, options, or PFM data are invalid.
    Invalid(&'static str),
    /// Dimensions or buffer sizes overflow the address space.
    SizeOverflow,
    /// The declared image exceeds the configured pixel limit.
    PixelLimit,
    /// A pixel or output buffer could not be allocated.
    Allocation,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Invalid(s) => write!(f, "invalid PFM: {s}"),
            Self::SizeOverflow => f.write_str("image size overflow"),
            Self::PixelLimit => f.write_str("pixel limit exceeded"),
            Self::Allocation => f.write_str("buffer allocation failed"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
/// Result type used by the codec.
pub type Result<T> = std::result::Result<T, Error>;
