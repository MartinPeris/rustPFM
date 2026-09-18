#![doc = include_str!("../README.md")]
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

#[cfg(feature = "ndarray")]
mod arrays;
pub use encode::{encode, encode_writer};
pub use files::{read_pfm, write_pfm};
