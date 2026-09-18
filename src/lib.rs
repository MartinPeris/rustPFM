//! Lightweight Portable Float Map images with a dependency-free core.
//! See the repository documentation for release status and conventions.
#![forbid(unsafe_code)]
#![warn(missing_docs)]
mod encode;
mod error;
mod files;
mod image;
pub use error::{Error, Result};
pub use image::{
    ByteOrder, ColorType, DecodeOptions, EncodeOptions, Header, Image, ImageView, ScaleMode,
};

mod decode;
pub use decode::{decode, decode_reader};

pub use encode::{encode, encode_writer};
pub use files::{read_pfm, write_pfm};
#[cfg(feature = "ndarray")]
mod arrays;
