//! Bounded parsing and owned decoding.
use crate::{
    ByteOrder, ColorType, DecodeOptions, Error, Header, Image, Result, ScaleMode,
    image::sample_count,
};
use std::io::{self, BufRead, Cursor, Read};

const MAX_HEADER_LINE: u64 = 4096;

/// Decode exactly one complete PFM byte buffer.
///
/// Validates its full length before allocating pixels. Rows are returned
/// top-first and contiguous, in native byte order. The default applies the
/// float32 header scale; use [`ScaleMode::Raw`] to preserve stored samples.
pub fn decode(payload: &[u8], options: DecodeOptions) -> Result<Image> {
    decode_reader_sized(Cursor::new(payload), options, Some(payload.len() as u64))
}

/// Decode one PFM image from a buffered reader and require end-of-stream.
///
/// This consumes one extra byte to reject trailing data and can wait for EOF
/// on a live stream. Unlike [`decode`], a generic reader cannot validate its
/// total size before allocation. Set `max_pixels` for untrusted input.
pub fn decode_reader<R: BufRead>(reader: R, options: DecodeOptions) -> Result<Image> {
    decode_reader_sized(reader, options, None)
}

fn line<R: BufRead>(reader: &mut R, consumed: &mut u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(64);
    reader
        .take(MAX_HEADER_LINE + 1)
        .read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_HEADER_LINE as usize {
        return Err(Error::Invalid("header line exceeds 4096 bytes"));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(Error::Invalid("header line is not newline-terminated"));
    }
    *consumed += bytes.len() as u64;
    Ok(bytes)
}
fn text(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes)
        .map(str::trim)
        .map_err(|_| Error::Invalid("header is not UTF-8 text"))
}
fn dimension(token: &str) -> Result<usize> {
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Invalid(
            "dimensions must be positive decimal integers",
        ));
    }
    token.parse().map_err(|_| Error::SizeOverflow)
}

pub(crate) fn decode_reader_sized<R: BufRead>(
    mut reader: R,
    options: DecodeOptions,
    total_len: Option<u64>,
) -> Result<Image> {
    if options.max_pixels == Some(0) {
        return Err(Error::Invalid("pixel limit must be positive"));
    }
    let mut consumed = 0;
    let magic = line(&mut reader, &mut consumed)?;
    let color_type = match text(&magic)? {
        "Pf" => ColorType::Gray,
        "PF" => ColorType::Rgb,
        _ => return Err(Error::Invalid("expected Pf or PF magic")),
    };
    let dims = line(&mut reader, &mut consumed)?;
    let mut parts = text(&dims)?.split_ascii_whitespace();
    let width = dimension(parts.next().ok_or(Error::Invalid("missing width"))?)?;
    let height = dimension(parts.next().ok_or(Error::Invalid("missing height"))?)?;
    if parts.next().is_some() {
        return Err(Error::Invalid("expected exactly two dimensions"));
    }
    let count = sample_count(width, height, color_type)?;
    let pixels_count = width.checked_mul(height).ok_or(Error::SizeOverflow)?;
    if options.max_pixels.is_some_and(|limit| pixels_count > limit) {
        return Err(Error::PixelLimit);
    }
    let scale_line = line(&mut reader, &mut consumed)?;
    let signed_scale: f32 = text(&scale_line)?
        .parse()
        .map_err(|_| Error::Invalid("invalid scale"))?;
    if !signed_scale.is_finite() || signed_scale == 0.0 {
        return Err(Error::Invalid("scale must be finite and nonzero"));
    }
    let byte_order = if signed_scale.is_sign_negative() {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    };
    let scale = signed_scale.abs();
    let payload_bytes = count.checked_mul(4).ok_or(Error::SizeOverflow)?;
    if let Some(length) = total_len {
        if length
            != consumed
                .checked_add(payload_bytes as u64)
                .ok_or(Error::SizeOverflow)?
        {
            return Err(Error::Invalid("payload length does not match dimensions"));
        }
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    pixels.resize(count, 0.0);
    let row_samples = width * color_type.channels();
    let mut buffer = [0u8; 64 * 1024];
    let apply_scale = options.scale_mode == ScaleMode::Apply && scale != 1.0;
    for row in pixels.chunks_exact_mut(row_samples).rev() {
        for chunk in row.chunks_mut(buffer.len() / 4) {
            let bytes = &mut buffer[..chunk.len() * 4];
            if let Err(error) = reader.read_exact(bytes) {
                return if error.kind() == io::ErrorKind::UnexpectedEof {
                    Err(Error::Invalid("truncated pixel payload"))
                } else {
                    Err(error.into())
                };
            }
            for (pixel, encoded) in chunk.iter_mut().zip(bytes.chunks_exact(4)) {
                let bits = [encoded[0], encoded[1], encoded[2], encoded[3]];
                let value = match byte_order {
                    ByteOrder::Little => f32::from_le_bytes(bits),
                    ByteOrder::Big => f32::from_be_bytes(bits),
                };
                *pixel = if apply_scale { value * scale } else { value };
            }
        }
    }
    let mut extra = [0u8; 1];
    if reader.read(&mut extra)? != 0 {
        return Err(Error::Invalid("trailing bytes after pixel payload"));
    }
    Ok(Image::from_decoded(
        Header {
            width,
            height,
            color_type,
            scale,
            byte_order,
        },
        pixels,
    ))
}
