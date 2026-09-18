use rustpfm::{
    ByteOrder, ColorType, DecodeOptions, EncodeOptions, Image, ImageView, ScaleMode, decode,
    decode_reader, encode, encode_writer,
};
use std::io::{self, Cursor, Write};

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn independent_netpbm_fixtures() {
    for (name, color) in [("grayscale", ColorType::Gray), ("rgb", ColorType::Rgb)] {
        // Read independent ASCII PNM values instead of duplicating codec output.
        let ext = if color == ColorType::Gray {
            "pgm"
        } else {
            "ppm"
        };
        let text = std::fs::read_to_string(format!("tests/fixtures/netpbm/{name}.{ext}")).unwrap();
        let numbers: Vec<f32> = text
            .lines()
            .filter(|l| !l.starts_with('#'))
            .flat_map(str::split_whitespace)
            .skip(4)
            .map(|s| s.parse().unwrap())
            .collect();
        for endian in ["little", "big"] {
            for scale in [0.5_f32, 1., 2.] {
                let data = std::fs::read(format!(
                    "tests/fixtures/netpbm/{name}-{endian}-scale-{scale}.pfm"
                ))
                .unwrap();
                for mode in [ScaleMode::Apply, ScaleMode::Raw] {
                    let image = decode(
                        &data,
                        DecodeOptions {
                            scale_mode: mode,
                            max_pixels: Some(6),
                        },
                    )
                    .unwrap();
                    assert_eq!(
                        (image.width(), image.height(), image.color_type()),
                        (3, 2, color)
                    );
                    let factor = if mode == ScaleMode::Apply {
                        scale * scale
                    } else {
                        scale
                    };
                    let expected: Vec<f32> = numbers.iter().map(|v| v / 16. * factor).collect();
                    assert_eq!(image.pixels(), expected);
                    assert_eq!(image.header().scale, scale);
                }
            }
        }
    }
}

#[test]
fn exact_wire_bytes_and_special_float_bits() {
    let words = [
        0x80000000_u32,
        0,
        0x7f800000,
        0xff800000,
        0x7fc01234,
        0x3f800000,
    ];
    let pixels: Vec<f32> = words.iter().map(|v| f32::from_bits(*v)).collect();
    for order in [ByteOrder::Little, ByteOrder::Big] {
        let view = ImageView::new(3, 2, ColorType::Gray, &pixels).unwrap();
        let bytes = encode(
            view,
            EncodeOptions {
                byte_order: order,
                scale: 1.,
            },
        )
        .unwrap();
        let mut expected = if order == ByteOrder::Little {
            b"Pf\n3 2\n-1\n".to_vec()
        } else {
            b"Pf\n3 2\n1\n".to_vec()
        };
        for index in [3, 4, 5, 0, 1, 2] {
            expected.extend_from_slice(&if order == ByteOrder::Little {
                words[index].to_le_bytes()
            } else {
                words[index].to_be_bytes()
            });
        }
        assert_eq!(bytes, expected);
        assert_eq!(
            bits(decode(&bytes, DecodeOptions::default()).unwrap().pixels()),
            words
        );
        assert_eq!(
            bits(
                decode_reader(Cursor::new(&bytes), DecodeOptions::default())
                    .unwrap()
                    .pixels()
            ),
            words
        );
        for length in 0..bytes.len() {
            assert!(decode(&bytes[..length], DecodeOptions::default()).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode(&trailing, DecodeOptions::default()).is_err());
    }
}

#[test]
fn deterministic_randomized_roundtrips_and_malformed_bytes() {
    let mut state = 0x9234_9876_u32;
    for width in 1..12 {
        for height in 1..8 {
            for color in [ColorType::Gray, ColorType::Rgb] {
                let samples: Vec<f32> = (0..width * height * color.channels())
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 17;
                        state ^= state << 5;
                        f32::from_bits(state)
                    })
                    .collect();
                let image = Image::new(width, height, color, samples).unwrap();
                for byte_order in [ByteOrder::Little, ByteOrder::Big] {
                    let encoded = encode(
                        image.view(),
                        EncodeOptions {
                            byte_order,
                            scale: 1.,
                        },
                    )
                    .unwrap();
                    let decoded = decode(&encoded, DecodeOptions::default()).unwrap();
                    assert_eq!(bits(decoded.pixels()), bits(image.pixels()));
                    assert!(
                        decode(
                            &encoded,
                            DecodeOptions {
                                max_pixels: Some(width * height - 1),
                                ..DecodeOptions::default()
                            }
                        )
                        .is_err()
                    );
                }
            }
        }
    }
    for length in 0..512 {
        let noise: Vec<u8> = (0..length)
            .map(|_| {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                (state >> 24) as u8
            })
            .collect();
        assert!(
            decode(
                &noise,
                DecodeOptions {
                    max_pixels: Some(100),
                    ..DecodeOptions::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn invalid_headers_and_options() {
    for header in [
        "",
        "P6\n1 1\n-1\n",
        "Pf\n0 1\n-1\n",
        "Pf\n1 -1\n-1\n",
        "Pf\n1 1 1\n-1\n",
        "Pf\n1 1\n0\n",
        "Pf\n1 1\nNaN\n",
        "Pf\n1 1\ninf\n",
        "Pf\n18446744073709551615 2\n1\n",
    ] {
        let mut data = header.as_bytes().to_vec();
        data.extend_from_slice(&[0; 4]);
        assert!(
            decode(&data, DecodeOptions::default()).is_err(),
            "{header:?}"
        );
    }
    let mut overlong = b"Pf\n".to_vec();
    overlong.extend(std::iter::repeat_n(b' ', 4096));
    overlong.extend_from_slice(b"1 1\n-1\n\0\0\0\0");
    assert!(decode(&overlong, DecodeOptions::default()).is_err());
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.]).unwrap();
    for scale in [0., -1., f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
        assert!(
            encode(
                view,
                EncodeOptions {
                    scale,
                    ..EncodeOptions::default()
                }
            )
            .is_err()
        );
    }
}

struct FailingWriter;
impl Write for FailingWriter {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("injected write failure"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn writer_propagates_io_errors() {
    let view = ImageView::new(1, 1, ColorType::Gray, &[1.]).unwrap();
    assert!(encode_writer(FailingWriter, view, EncodeOptions::default()).is_err());
}
