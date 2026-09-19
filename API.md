# API reference

Run `cargo doc --all-features --open` for signatures and Rustdoc. All core types
and functions are exported at the crate root.

## Images and layout

`Image::new(width, height, ColorType, Vec<f32>)` takes ownership of samples;
`ImageView::new(width, height, ColorType, &[f32])` borrows them. Both require
positive dimensions, checked representable sizes, and exactly width × height ×
channels samples. `ColorType::Gray` has one channel; `Rgb` has three interleaved
red/green/blue channels. Pixels are contiguous, top-first, native-endian `f32`.
PFM payloads are bottom-first; the codec handles row reversal.

Access `width`, `height`, `color_type`, `pixels`, `pixels_mut`, `into_pixels`, and
`view` through Image's methods. Dimensions cannot be mutated independently of
the buffer. `header()` retains the source scale and byte order after decoding.
New images have unit scale and little-endian header metadata; encoding always
uses the explicitly supplied EncodeOptions, not the image's source header.

Decoded images own their samples independently of the input. Pixel NaNs,
infinities, signed zeros and arbitrary float bit patterns are supported. Unit
scale and raw decoding preserve sample bits; arithmetic scaling can change NaN
payloads and overflow/underflow just like other float32 arithmetic.

## Decoding

- `decode(&[u8], DecodeOptions)`: require exactly one complete PFM buffer. Header
  and total payload size are validated before allocating pixels.
- `decode_reader(impl BufRead, DecodeOptions)`: consume a single image and require
  end-of-stream. A final byte probe rejects trailing data; on a live stream it
  may wait for EOF. It cannot preflight the total size before pixel allocation.
- `read_pfm(path, DecodeOptions)`: use a buffered file and preflight its size
  before pixel allocation. Do not modify the file concurrently while reading.

The header has exactly three newline-terminated lines: `Pf`/`PF`, width and
height, and scale. Each line is limited to 4096 bytes including its newline.
CRLF is accepted. Comment lines and multi-image streams are not supported.
Dimensions are positive decimal integers. The scale must parse as a finite,
nonzero `f32`: decimal values are rounded to float32 precision; results that
become zero or nonfinite are rejected.
Negative scale selects little-endian payload; positive selects big-endian.
The payload must contain precisely the declared sample bytes.

`DecodeOptions::default()` applies the scale and sets no pixel limit.
`max_pixels: Some(n)` requires a positive n and bounds width × height before
pixel allocation, regardless of channel count. Set it for untrusted inputs.
A grayscale pixel requires 4 bytes and RGB requires 12, excluding the input
buffer, stack scratch, buffering, and application copies. This is not a total
process memory limit. The codec uses checked arithmetic and fallible pixel
buffer reservation, but operating-system memory overcommit is not controlled.

## Scale conventions

`ScaleMode::Apply` multiplies stored samples by the header magnitude, skipping
arithmetic at exactly 1. `ScaleMode::Raw` returns stored samples unchanged.
The magnitude and multiplication use float32. This follows justPFM's usual
scale convention, but does not promise identical results for scales requiring
float64 precision or outside float32 range. Source metadata remains available
through `Image::header()` in both modes.

Netpbm's `pfmtopam` divides samples by the magnitude. Use scale 1 for normalized
intensity interchange, or deliberately perform the conversion you require.
Independent fixtures test this distinction; see
[fixture provenance](tests/fixtures/netpbm/README.md).

## Encoding

`encode(view, EncodeOptions)` returns a complete `Vec<u8>`.
`encode_writer(impl Write, view, EncodeOptions)` writes without making a complete
payload buffer. Both accept validated ImageViews, store samples unchanged,
and record a positive finite scale in the header. Default EncodeOptions use
scale 1 and little-endian output. `ByteOrder::Big` selects big-endian output.
Input data is never changed.

Writer options are validated before bytes are emitted. A generic writer can
contain partial output after an I/O failure. `encode_writer` does not flush
its writer; the caller must handle flushing and its errors. Serialization uses
64 KiB stack scratch. The bytes convenience API additionally holds the entire
encoded output, and decoding holds the complete owned pixel buffer.

## Filesystem writes

`write_pfm(path, view, EncodeOptions)` creates an exclusive sibling temporary
file, writes and flushes it, closes it, then renames it over the destination.
Failures preserve an existing destination and attempt to remove the temporary
file. Temporary name collisions are retried up to a bounded limit.

On Unix, new files use mode 0600 subject to the process umask. Existing regular
file permission bits are preserved. A destination symlink entry is replaced;
its target is not modified and its permissions are not copied. Ownership, ACLs,
and extended attributes are not preserved. The parent directory must permit
creating and replacing entries. Concurrent destination edits are not coordinated.

This behavior is tested on Linux. Native Windows/macOS behavior is not yet
validated. Flush and atomic rename do not guarantee durability after power loss:
no file or directory fsync is performed. A process crash can leave a temporary
file; cleanup is performed on normal Rust error paths.

## Optional ndarray support

Enable `features = ["ndarray"]` in the dependency declaration.
`image.as_ndarray()` returns a borrowed `(height, width, channels)` view with no
copy. `Image::from_ndarray(array.view())` accepts 2D grayscale or 3D arrays with
one or three channels; it copies logical pixels into an owned Image. Strided,
reversed, and Fortran-order layouts are supported. This conversion allocates a
full pixel buffer; use `ImageView` for already-contiguous borrowed input.

## Errors

Functions return `rustpfm::Result<T>`. `Error::Io` preserves the underlying I/O
error. Other variants distinguish invalid format/options, size overflow,
configured pixel limits, and allocation failure. Error is non-exhaustive: match
with a fallback arm so future error variants remain possible. Error messages
are diagnostic text rather than a stable parsing interface.
