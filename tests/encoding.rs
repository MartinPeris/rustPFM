use rustpfm::{ByteOrder, ColorType, EncodeOptions, Error, ImageView, encode, encode_writer};
use std::io::{self, Write};

#[test]
fn exact_bytes_cover_endianness_channels_row_order_and_scale() {
    for color in [ColorType::Gray, ColorType::Rgb] {
        let samples: Vec<f32> = (0..4 * color.channels()).map(|n| n as f32 + 0.5).collect();
        let view = ImageView::new(2, 2, color, &samples).unwrap();
        for byte_order in [ByteOrder::Little, ByteOrder::Big] {
            let options = EncodeOptions {
                scale: 2.5,
                byte_order,
            };
            let magic = if color == ColorType::Gray { "Pf" } else { "PF" };
            let sign = if byte_order == ByteOrder::Little {
                "-"
            } else {
                ""
            };
            let mut expected = format!("{magic}\n2 2\n{sign}2.5\n").into_bytes();
            let row = 2 * color.channels();
            for index in (row..2 * row).chain(0..row) {
                let bytes = match byte_order {
                    ByteOrder::Little => samples[index].to_bits().to_le_bytes(),
                    ByteOrder::Big => samples[index].to_bits().to_be_bytes(),
                };
                expected.extend_from_slice(&bytes);
            }
            assert_eq!(encode(view, options).unwrap(), expected);
            let mut written = Vec::new();
            encode_writer(&mut written, view, options).unwrap();
            assert_eq!(written, expected);
        }
    }
}

#[test]
fn preserve_all_float_bits_without_scaling() {
    let bits = [
        0,
        0x8000_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_1234,
        0x7f80_0001,
    ];
    let pixels: Vec<f32> = bits.iter().map(|bits| f32::from_bits(*bits)).collect();
    let view = ImageView::new(6, 1, ColorType::Gray, &pixels).unwrap();
    let bytes = encode(view, EncodeOptions::default()).unwrap();
    assert_eq!(&bytes[..10], b"Pf\n6 1\n-1\n");
    for (actual, expected) in bytes[10..].chunks_exact(4).zip(bits) {
        assert_eq!(actual, expected.to_le_bytes());
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max_write: usize,
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.max_write = self.max_write.max(bytes.len());
        // Exercise write_all handling of short writes, too.
        let accepted = bytes.len().min(997);
        self.bytes.extend_from_slice(&bytes[..accepted]);
        Ok(accepted)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("encode_writer must leave flushing to its caller")
    }
}

#[test]
fn wide_rows_use_bounded_scratch_and_handle_short_writes() {
    let pixels: Vec<f32> = (0..50_001 * 3).map(|n| n as f32).collect();
    let view = ImageView::new(50_001, 1, ColorType::Rgb, &pixels).unwrap();
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        max_write: 0,
    };
    encode_writer(&mut writer, view, EncodeOptions::default()).unwrap();
    assert!(writer.max_write <= 64 * 1024);
    assert_eq!(
        writer.bytes,
        encode(view, EncodeOptions::default()).unwrap()
    );
}

struct FailingWriter {
    calls: usize,
}
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        self.calls += 1;
        Err(io::Error::other("injected"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn invalid_options_do_not_write_and_io_failures_are_preserved() {
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    let mut writer = FailingWriter { calls: 0 };
    for scale in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let options = EncodeOptions {
            scale,
            ..EncodeOptions::default()
        };
        assert!(matches!(encode(view, options), Err(Error::Invalid(_))));
        assert!(matches!(
            encode_writer(&mut writer, view, options),
            Err(Error::Invalid(_))
        ));
    }
    assert_eq!(writer.calls, 0);
    match encode_writer(&mut writer, view, EncodeOptions::default()) {
        Err(Error::Io(error)) => assert_eq!(error.to_string(), "injected"),
        other => panic!("unexpected result: {other:?}"),
    }
    assert_eq!(writer.calls, 1);
}

#[test]
fn every_positive_scale_extreme_is_recorded_and_roundtrips_as_metadata() {
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    for scale in [f32::from_bits(1), f32::MIN_POSITIVE, f32::MAX] {
        let encoded = encode(
            view,
            EncodeOptions {
                scale,
                ..EncodeOptions::default()
            },
        )
        .unwrap();
        let decoded = rustpfm::decode(
            &encoded,
            rustpfm::DecodeOptions {
                scale_mode: rustpfm::ScaleMode::Raw,
                ..rustpfm::DecodeOptions::default()
            },
        )
        .unwrap();
        assert_eq!(decoded.header().scale.to_bits(), scale.to_bits());
        assert_eq!(decoded.pixels(), &[1.0]);
    }
}

#[test]
fn payload_write_failure_and_write_zero_are_returned() {
    struct PayloadFailure {
        header_written: bool,
        write_zero: bool,
    }
    impl Write for PayloadFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.header_written {
                self.header_written = true;
                Ok(bytes.len())
            } else if self.write_zero {
                Ok(0)
            } else {
                Err(io::Error::other("payload failure"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.0]).unwrap();
    for write_zero in [false, true] {
        let result = encode_writer(
            PayloadFailure {
                header_written: false,
                write_zero,
            },
            view,
            EncodeOptions::default(),
        );
        match result {
            Err(Error::Io(error)) => assert_eq!(
                error.kind(),
                if write_zero {
                    io::ErrorKind::WriteZero
                } else {
                    io::ErrorKind::Other
                }
            ),
            other => panic!("unexpected result {other:?}"),
        }
    }
}

#[test]
fn row_and_buffer_boundaries_match_independent_wire_bytes() {
    // Include tiny rows, an odd middle row, exact buffer multiples, and rows
    // larger than the scratch buffer. Patterns include noncanonical NaN bits.
    let words = [0, 0x8000_0000, 0x7fc0_1234, 0x7f80_0001, 0x3f80_0000];
    for (width, height) in [(3, 6001), (16384, 2), (16385, 3)] {
        for color in [ColorType::Gray, ColorType::Rgb] {
            let row = width * color.channels();
            let samples: Vec<f32> = (0..row * height)
                .map(|i| f32::from_bits(words[i % words.len()]))
                .collect();
            for byte_order in [ByteOrder::Little, ByteOrder::Big] {
                let magic = if color == ColorType::Gray { "Pf" } else { "PF" };
                let scale = if byte_order == ByteOrder::Little {
                    "-2"
                } else {
                    "2"
                };
                let mut expected = format!("{magic}\n{width} {height}\n{scale}\n").into_bytes();
                for y in (0..height).rev() {
                    for x in 0..row {
                        let bits = words[(y * row + x) % words.len()];
                        expected.extend_from_slice(&match byte_order {
                            ByteOrder::Little => bits.to_le_bytes(),
                            ByteOrder::Big => bits.to_be_bytes(),
                        });
                    }
                }
                let view = ImageView::new(width, height, color, &samples).unwrap();
                let options = EncodeOptions {
                    scale: 2.0,
                    byte_order,
                };
                let mut writer = BoundedWriter {
                    bytes: Vec::new(),
                    max_write: 0,
                };
                encode_writer(&mut writer, view, options).unwrap();
                assert_eq!(writer.bytes, expected);
                assert!(writer.max_write <= 64 * 1024);
                assert_eq!(encode(view, options).unwrap(), expected);
                // Independently generated bytes also exercise large read buffers.
                let decoded = rustpfm::decode(
                    &expected,
                    rustpfm::DecodeOptions {
                        scale_mode: rustpfm::ScaleMode::Raw,
                        ..Default::default()
                    },
                )
                .unwrap();
                let scaled = rustpfm::decode(&expected, rustpfm::DecodeOptions::default()).unwrap();
                for (&actual, &original) in scaled.pixels().iter().zip(&samples) {
                    if original.is_nan() {
                        assert!(actual.is_nan());
                    } else {
                        assert_eq!(actual.to_bits(), (original * 2.0).to_bits());
                    }
                }
                assert_eq!(
                    decoded
                        .pixels()
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                );
            }
        }
    }
}
