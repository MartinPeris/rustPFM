//! Optional live independent decoder check, required in the Linux CI job.
use rustpfm::{ByteOrder, ColorType, EncodeOptions, ImageView, encode};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
#[ignore = "requires Netpbm pfmtopam; CI runs explicitly"]
fn netpbm_decodes_rustpfm_output() {
    assert!(
        Command::new("pfmtopam")
            .arg("-version")
            .status()
            .unwrap()
            .success()
    );
    for color in [ColorType::Gray, ColorType::Rgb] {
        let count = 6 * color.channels();
        let original: Vec<f32> = (0..count).map(|v| (v % 17) as f32 / 16.).collect();
        for byte_order in [ByteOrder::Little, ByteOrder::Big] {
            for scale in [0.5, 1., 2.] {
                // Netpbm divides by scale; rustPFM stores caller samples unchanged.
                let stored: Vec<f32> = original.iter().map(|v| v * scale).collect();
                let bytes = encode(
                    ImageView::new(3, 2, color, &stored).unwrap(),
                    EncodeOptions { scale, byte_order },
                )
                .unwrap();
                let mut process = Command::new("pfmtopam")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();
                process.stdin.take().unwrap().write_all(&bytes).unwrap();
                let output = process.wait_with_output().unwrap();
                assert!(output.status.success());
                let split = output
                    .stdout
                    .windows(7)
                    .position(|w| w == b"ENDHDR\n")
                    .unwrap()
                    + 7;
                let header = std::str::from_utf8(&output.stdout[..split]).unwrap();
                assert!(header.starts_with("P7\n"));
                assert!(header.lines().any(|l| l == "WIDTH 3"));
                assert!(header.lines().any(|l| l == "HEIGHT 2"));
                assert!(
                    header
                        .lines()
                        .any(|l| l == format!("DEPTH {}", color.channels()))
                );
                let maxval: u32 = header
                    .lines()
                    .find_map(|l| l.strip_prefix("MAXVAL "))
                    .unwrap()
                    .parse()
                    .unwrap();
                let expected: Vec<u32> = original
                    .iter()
                    .map(|v| (v * maxval as f32 + 0.5).floor() as u32)
                    .collect();
                let payload = &output.stdout[split..];
                let actual: Vec<u32> = if maxval < 256 {
                    payload.iter().map(|v| *v as u32).collect()
                } else {
                    payload
                        .chunks_exact(2)
                        .map(|v| u16::from_be_bytes([v[0], v[1]]) as u32)
                        .collect()
                };
                assert_eq!(actual, expected);
            }
        }
    }
}
