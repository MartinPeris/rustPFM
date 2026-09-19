use rustpfm::{
    ByteOrder, ColorType, DecodeOptions, Error, Image, ImageView, ScaleMode, decode, decode_reader,
};
use std::io::{self, BufReader, Cursor, Read};

fn fixture(order: ByteOrder, scale: f32) -> Vec<u8> {
    let sign = if order == ByteOrder::Little {
        -scale
    } else {
        scale
    };
    let mut bytes = format!("Pf\n3 2\n{sign}\n").into_bytes();
    for value in [4.0_f32, 5.0, 6.0, 1.0, 2.0, 3.0] {
        bytes.extend_from_slice(&match order {
            ByteOrder::Little => value.to_le_bytes(),
            ByteOrder::Big => value.to_be_bytes(),
        });
    }
    bytes
}
#[test]
fn decodes_both_byte_orders_and_preserves_source_metadata() {
    for order in [ByteOrder::Little, ByteOrder::Big] {
        let bytes = fixture(order, 2.0);
        let mut image = decode(&bytes, DecodeOptions::default()).unwrap();
        assert_eq!(
            (image.width(), image.height(), image.color_type()),
            (3, 2, ColorType::Gray)
        );
        assert_eq!(image.pixels(), &[2.0, 4.0, 6.0, 8.0, 10.0, 12.0]);
        assert_eq!(image.header().byte_order, order);
        assert_eq!(image.header().scale, 2.0);
        image.pixels_mut()[0] = 42.0;
        assert_eq!(
            decode(
                &bytes,
                DecodeOptions {
                    scale_mode: ScaleMode::Raw,
                    max_pixels: Some(6),
                    ..Default::default()
                }
            )
            .unwrap()
            .pixels(),
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        );
        assert_eq!(image.into_pixels()[0], 42.0);
    }
}
#[test]
fn strict_size_limits_and_bad_headers() {
    let bytes = fixture(ByteOrder::Little, 1.0);
    for cut in 0..bytes.len() {
        assert!(decode(&bytes[..cut], DecodeOptions::default()).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(decode(&extra, DecodeOptions::default()).is_err());
    assert!(matches!(
        decode(
            &bytes,
            DecodeOptions {
                max_pixels: Some(5),
                ..Default::default()
            }
        ),
        Err(Error::PixelLimit)
    ));
    assert!(
        decode(
            &bytes,
            DecodeOptions {
                max_pixels: Some(0),
                ..Default::default()
            }
        )
        .is_err()
    );
    for input in [
        "PX\n1 1\n-1\n",
        "Pf\n0 1\n-1\n",
        "Pf\n1 -1\n-1\n",
        "Pf\n1 1 1\n-1\n",
        "Pf\n\n-1\n",
        "Pf\n1\n-1\n",
        "Pf\n1 1\nNaN\n",
        "Pf\n1 1\ninf\n",
        "Pf\n1 1\n-0\n",
        "Pf\n999999999999999999999999999999 1\n-1\n",
    ] {
        assert!(
            decode(input.as_bytes(), DecodeOptions::default()).is_err(),
            "{input}"
        );
    }
    let long = vec![b'P'; 4097];
    assert!(decode(&long, DecodeOptions::default()).is_err());
    assert!(decode(&[0xff, b'\n'], DecodeOptions::default()).is_err());
}
#[test]
fn header_line_boundary_and_crlf() {
    let mut data = b"Pf".to_vec();
    data.resize(4095, b' ');
    data.extend_from_slice(b"\n1 1\n-1\n");
    data.extend_from_slice(&42f32.to_le_bytes());
    assert_eq!(
        decode(&data, DecodeOptions::default()).unwrap().pixels(),
        &[42.]
    );
    data.insert(2, b' ');
    assert!(decode(&data, DecodeOptions::default()).is_err());
    let mut crlf = b"Pf\r\n1 1\r\n-1\r\n".to_vec();
    crlf.extend_from_slice(&42f32.to_le_bytes());
    assert!(decode(&crlf, DecodeOptions::default()).is_ok());
}
#[test]
fn streaming_matches_bytes_and_rejects_extra() {
    let bytes = fixture(ByteOrder::Big, 1.0);
    assert_eq!(
        decode_reader(
            BufReader::with_capacity(1, Cursor::new(&bytes)),
            DecodeOptions::default()
        )
        .unwrap()
        .pixels(),
        &[1., 2., 3., 4., 5., 6.]
    );
    assert!(
        decode_reader(
            Cursor::new(&bytes[..bytes.len() - 1]),
            DecodeOptions::default()
        )
        .is_err()
    );
    let mut extra = bytes;
    extra.push(0);
    assert!(decode_reader(Cursor::new(extra), DecodeOptions::default()).is_err());
}
#[test]
fn image_views_enforce_dimensions_and_lengths() {
    assert!(Image::new(0, 1, ColorType::Gray, vec![]).is_err());
    assert!(ImageView::new(1, 1, ColorType::Rgb, &[1.]).is_err());
    assert!(matches!(
        ImageView::new(usize::MAX, 2, ColorType::Rgb, &[]),
        Err(Error::SizeOverflow)
    ));
    assert!(matches!(
        ImageView::new(isize::MAX as usize, 1, ColorType::Gray, &[]),
        Err(Error::SizeOverflow)
    ));
    let image = Image::new(1, 1, ColorType::Rgb, vec![1., 2., 3.]).unwrap();
    let view = image.view();
    assert_eq!(
        (view.width(), view.height(), view.color_type()),
        (1, 1, ColorType::Rgb)
    );
    assert_eq!(view.pixels(), &[1., 2., 3.]);
}
#[test]
fn io_errors_are_preserved() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("broken"))
        }
    }
    let error = decode_reader(BufReader::new(Broken), DecodeOptions::default()).unwrap_err();
    assert!(matches!(error, Error::Io(_)));
    assert!(std::error::Error::source(&error).is_some());
    assert!(error.to_string().contains("broken"));
}
#[test]
fn special_pixel_bits_are_preserved_without_scaling() {
    for order in [ByteOrder::Little, ByteOrder::Big] {
        let mut bytes = if order == ByteOrder::Little {
            b"Pf\n4 1\n-1\n".to_vec()
        } else {
            b"Pf\n4 1\n1\n".to_vec()
        };
        let bits = [0x80000000u32, 0x7f800000, 0xff800000, 0x7f800001];
        for b in bits {
            bytes.extend_from_slice(&if order == ByteOrder::Little {
                b.to_le_bytes()
            } else {
                b.to_be_bytes()
            });
        }
        assert_eq!(
            decode(&bytes, DecodeOptions::default())
                .unwrap()
                .pixels()
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            bits
        );
    }
}

#[test]
fn retries_interrupted_eof_probe() {
    struct InterruptedEof {
        cursor: Cursor<Vec<u8>>,
        interrupted: bool,
    }
    impl Read for InterruptedEof {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.cursor.position() == self.cursor.get_ref().len() as u64 && !self.interrupted {
                self.interrupted = true;
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.cursor.read(output)
        }
    }
    let reader = InterruptedEof {
        cursor: Cursor::new(fixture(ByteOrder::Little, 1.0)),
        interrupted: false,
    };
    assert_eq!(
        decode_reader(BufReader::new(reader), DecodeOptions::default())
            .unwrap()
            .pixels(),
        &[1., 2., 3., 4., 5., 6.]
    );
}
