use rustpfm::{
    ByteOrder, ColorType, DecodeOptions, EncodeOptions, Error, Image, ImageView, RowOrder,
    ScaleMode, decode, decode_reader, encode, encode_writer,
};
use std::io::{self, BufRead, Cursor, IoSlice, Read, Write};

fn native() -> ByteOrder {
    if cfg!(target_endian = "little") {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    }
}

#[test]
fn storage_order_is_explicit_and_logical_rows_are_stable() {
    for height in [1, 2, 3, 129] {
        let expected: Vec<f32> = (0..height * 2).map(|i| i as f32).collect();
        let image = Image::new(2, height, ColorType::Gray, expected.clone()).unwrap();
        for byte_order in [ByteOrder::Little, ByteOrder::Big] {
            let opts = EncodeOptions {
                byte_order,
                scale: 2.0,
            };
            let bytes = encode(image.view(), opts).unwrap();
            for row_order in [RowOrder::TopFirst, RowOrder::BottomFirst] {
                let mut decoded = decode(
                    &bytes,
                    DecodeOptions {
                        row_order,
                        scale_mode: ScaleMode::Raw,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert_eq!(decoded.row_order(), row_order);
                assert_eq!(decoded.row(height), None);
                assert_eq!(decoded.row(usize::MAX), None);
                for y in 0..height {
                    assert_eq!(decoded.row(y).unwrap(), &expected[y * 2..(y + 1) * 2]);
                }
                assert_eq!(encode(decoded.view(), opts).unwrap(), bytes);
                let external = ImageView::with_row_order(
                    2,
                    height,
                    ColorType::Gray,
                    decoded.pixels(),
                    row_order,
                )
                .unwrap();
                assert_eq!(external.row_order(), row_order);
                assert_eq!(encode(external, opts).unwrap(), bytes);
                decoded.set_row_order(RowOrder::TopFirst);
                assert_eq!(decoded.pixels(), expected);
                decoded.set_row_order(RowOrder::BottomFirst);
                decoded.set_row_order(RowOrder::TopFirst);
                assert_eq!(decoded.into_pixels(), expected);
            }
        }
    }
}

struct InspectingReader {
    header: Cursor<&'static [u8]>,
    panic: bool,
}
impl BufRead for InspectingReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.header.fill_buf()
    }
    fn consume(&mut self, count: usize) {
        self.header.consume(count);
    }
}
impl Read for InspectingReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.header.position() < self.header.get_ref().len() as u64 {
            return self.header.read(output);
        }
        // A safe Read is allowed to inspect destination bytes before writing.
        assert!(!output.is_empty());
        assert!(output.iter().all(|&byte| byte == 0));
        output[0] = 0xff; // leave a partial float modified on error/unwind
        assert!(!self.panic, "injected reader panic");
        Err(io::Error::other("injected after partial write"))
    }
}
#[test]
fn direct_read_buffer_is_initialized_even_on_error_and_unwind() {
    let reader = |panic| InspectingReader {
        header: Cursor::new(b"Pf\n2 3\n-1\n"),
        panic,
    };
    assert!(matches!(
        decode_reader(reader(false), DecodeOptions::default()),
        Err(Error::Io(_))
    ));
    assert!(
        std::panic::catch_unwind(|| decode_reader(reader(true), DecodeOptions::default())).is_err()
    );
}

struct Vectored {
    bytes: Vec<u8>,
    calls: usize,
    zero: bool,
}
impl Write for Vectored {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn write_vectored(&mut self, input: &[IoSlice<'_>]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.zero {
            return Ok(0);
        }
        let mut remaining = 11; // cross 8-byte row boundaries, ending mid-float
        let mut written = 0;
        for slice in input {
            let take = slice.len().min(remaining);
            self.bytes.extend_from_slice(&slice[..take]);
            written += take;
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("codec must not flush");
    }
}
#[test]
fn native_vectored_writes_retry_and_advance_across_slices() {
    let image = Image::new(
        2,
        129,
        ColorType::Gray,
        (0..258).map(|i| i as f32).collect(),
    )
    .unwrap();
    let options = EncodeOptions {
        byte_order: native(),
        scale: 1.0,
    };
    let mut writer = Vectored {
        bytes: vec![],
        calls: 0,
        zero: false,
    };
    encode_writer(&mut writer, image.view(), options).unwrap();
    assert!(writer.calls > 3);
    let mut expected = format!(
        "Pf\n2 129\n{}1\n",
        if native() == ByteOrder::Little {
            "-"
        } else {
            ""
        }
    )
    .into_bytes();
    for y in (0..129).rev() {
        for x in 0..2 {
            expected.extend_from_slice(&((y * 2 + x) as f32).to_ne_bytes());
        }
    }
    assert_eq!(writer.bytes, expected);
    let mut writer = Vectored {
        bytes: vec![],
        calls: 0,
        zero: true,
    };
    assert!(
        matches!(encode_writer(&mut writer, image.view(), options), Err(Error::Io(error)) if error.kind() == io::ErrorKind::WriteZero)
    );
}

#[test]
fn direct_io_preserves_special_bits_in_both_storage_and_byte_orders() {
    let words: [u32; 6] = [
        0x8000_0000,
        0x7f80_0001,
        0x7fc0_1234,
        0xff80_0000,
        0,
        0xffff_ffff,
    ];
    let image = Image::new(
        2,
        3,
        ColorType::Gray,
        words.iter().map(|&v| f32::from_bits(v)).collect(),
    )
    .unwrap();
    for byte_order in [ByteOrder::Little, ByteOrder::Big] {
        let mut expected = format!(
            "Pf\n2 3\n{}1\n",
            if byte_order == ByteOrder::Little {
                "-"
            } else {
                ""
            }
        )
        .into_bytes();
        for index in [4, 5, 2, 3, 0, 1] {
            expected.extend_from_slice(&match byte_order {
                ByteOrder::Little => words[index].to_le_bytes(),
                ByteOrder::Big => words[index].to_be_bytes(),
            });
        }
        assert_eq!(
            encode(
                image.view(),
                EncodeOptions {
                    byte_order,
                    scale: 1.0
                }
            )
            .unwrap(),
            expected
        );
        for row_order in [RowOrder::TopFirst, RowOrder::BottomFirst] {
            let decoded = decode(
                &expected,
                DecodeOptions {
                    row_order,
                    scale_mode: ScaleMode::Raw,
                    ..Default::default()
                },
            )
            .unwrap();
            for y in 0..3 {
                assert_eq!(
                    decoded
                        .row(y)
                        .unwrap()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    words[y * 2..(y + 1) * 2]
                );
            }
        }
    }
}

struct VectoredReader {
    header: Cursor<Vec<u8>>,
    payload: Cursor<Vec<u8>>,
    calls: usize,
    eof: bool,
}
impl BufRead for VectoredReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.header.fill_buf()
    }
    fn consume(&mut self, count: usize) {
        self.header.consume(count);
    }
}
impl Read for VectoredReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.payload.read(output)
    }
    fn read_vectored(&mut self, outputs: &mut [std::io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.eof {
            return Ok(0);
        }
        let mut remaining = 11;
        let mut read = 0;
        for output in outputs {
            let take = output.len().min(remaining);
            // Decoder storage remains initialized even across partial float reads.
            assert!(output[..take].iter().all(|&byte| byte == 0));
            let actual = self.payload.read(&mut output[..take])?;
            read += actual;
            remaining -= actual;
            if remaining == 0 || actual == 0 {
                break;
            }
        }
        Ok(read)
    }
}
#[test]
fn vectored_reads_retry_and_advance_across_row_and_batch_boundaries() {
    let header = format!(
        "Pf\n2 65\n{}2\n",
        if native() == ByteOrder::Little {
            "-"
        } else {
            ""
        }
    );
    let mut payload = Vec::new();
    for y in (0..65).rev() {
        for x in 0..2 {
            payload.extend_from_slice(&((y * 2 + x) as f32).to_ne_bytes());
        }
    }
    let reader = |eof| VectoredReader {
        header: Cursor::new(header.as_bytes().to_vec()),
        payload: Cursor::new(payload.clone()),
        calls: 0,
        eof,
    };
    let mut successful = reader(false);
    let image = decode_reader(&mut successful, DecodeOptions::default()).unwrap();
    assert!(successful.calls > 3);
    assert_eq!(
        image.pixels(),
        &(0..130).map(|i| (i * 2) as f32).collect::<Vec<_>>()
    );
    assert!(matches!(
        decode_reader(reader(true), DecodeOptions::default()),
        Err(Error::Invalid(_))
    ));
}
