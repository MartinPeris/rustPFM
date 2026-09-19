//! Encoding into bytes or arbitrary writers.
use std::io::{self, IoSlice, Write};

use crate::{ByteOrder, ColorType, EncodeOptions, Error, ImageView, Result};

const CHUNK_BYTES: usize = 64 * 1024;

pub(crate) fn validate_options(options: EncodeOptions) -> Result<()> {
    if !options.scale.is_finite() || options.scale <= 0.0 {
        return Err(Error::Invalid("scale must be finite and positive"));
    }
    Ok(())
}

fn header(view: ImageView<'_>, options: EncodeOptions) -> String {
    let magic = match view.color_type() {
        ColorType::Gray => "Pf",
        ColorType::Rgb => "PF",
    };
    let sign = match options.byte_order {
        ByteOrder::Little => "-",
        ByteOrder::Big => "",
    };
    format!(
        "{magic}\n{} {}\n{sign}{}\n",
        view.width(),
        view.height(),
        options.scale
    )
}

/// Encode a complete PFM image into an owned byte buffer.
///
/// Input pixels remain unchanged; the view specifies their physical row order. The header records the
/// requested scale; encoding does not multiply or divide samples. The returned
/// buffer contains the entire encoded image in addition to the caller's pixels.
pub fn encode(view: ImageView<'_>, options: EncodeOptions) -> Result<Vec<u8>> {
    validate_options(options)?;
    let header = header(view, options);
    let capacity = view
        .pixels()
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(header.len()))
        .ok_or(Error::SizeOverflow)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Error::Allocation)?;
    output.extend_from_slice(header.as_bytes());
    write_pixels(&mut output, view, options)?;
    Ok(output)
}

/// Encode a complete PFM image to a writer using bounded scratch space.
///
/// Options are checked before any bytes are written. I/O failures may leave a
/// partial image in the writer. This function does not flush the writer; callers
/// must flush buffered writers and handle errors themselves. Use [`crate::write_pfm`]
/// for atomic filesystem replacement.
pub fn encode_writer<W: Write>(
    mut writer: W,
    view: ImageView<'_>,
    options: EncodeOptions,
) -> Result<()> {
    validate_options(options)?;
    writer.write_all(header(view, options).as_bytes())?;
    write_pixels(&mut writer, view, options)
}

fn write_pixels<W: Write>(
    writer: &mut W,
    view: ImageView<'_>,
    options: EncodeOptions,
) -> Result<()> {
    let native = if cfg!(target_endian = "little") {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    };
    if options.byte_order == native {
        return write_native(writer, view);
    }
    let mut scratch = [0_u8; CHUNK_BYTES];
    let mut used = 0;
    for row in view.file_rows() {
        let mut remaining = row;
        while !remaining.is_empty() {
            let count = remaining.len().min((CHUNK_BYTES - used) / 4);
            let (chunk, rest) = remaining.split_at(count);
            let end = used + count * 4;
            for (sample, bytes) in chunk.iter().zip(scratch[used..end].chunks_exact_mut(4)) {
                bytes.copy_from_slice(&match options.byte_order {
                    ByteOrder::Little => sample.to_le_bytes(),
                    ByteOrder::Big => sample.to_be_bytes(),
                });
            }
            used = end;
            remaining = rest;
            // Fill across row boundaries instead of issuing one write per row.
            if used == CHUNK_BYTES {
                writer.write_all(&scratch)?;
                used = 0;
            }
        }
    }
    if used != 0 {
        writer.write_all(&scratch[..used])?;
    }
    Ok(())
}

// At most 64 borrowed slices, each <=64 KiB (also below Windows IoSlice limits).
fn write_native<W: Write>(writer: &mut W, view: ImageView<'_>) -> Result<()> {
    let mut chunks = view.file_rows().flat_map(|row| row.chunks(CHUNK_BYTES / 4));
    loop {
        let mut slices = [IoSlice::new(&[]); 64];
        let mut count = 0;
        for slot in &mut slices {
            let Some(chunk) = chunks.next() else {
                break;
            };
            *slot = IoSlice::new(crate::buffer::bytes(chunk));
            count += 1;
        }
        if count == 0 {
            return Ok(());
        }
        let mut pending = &mut slices[..count];
        while !pending.is_empty() {
            match writer.write_vectored(pending) {
                Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero).into()),
                Ok(written) => IoSlice::advance_slices(&mut pending, written),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
