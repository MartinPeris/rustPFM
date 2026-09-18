//! A lightweight Portable Float Map (PFM) codec for Rust.
//!
//! This crate is an initial scaffold. Encoding, decoding, and image types are
//! not implemented yet. It has no dependencies and is not published on crates.io.
//!
//! The planned core uses owned `Vec<f32>` pixels, borrowed slices for encoding,
//! and standard-library I/O. Array-library integrations will remain optional.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
