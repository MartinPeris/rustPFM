//! Opt-in release-mode file benchmark; orchestration lives in benchmarks/compare.py.
use rustpfm::{ColorType, DecodeOptions, EncodeOptions, Image, read_pfm, write_pfm};
use std::{error::Error, fs, hint::black_box, path::Path, time::Instant};

fn validate_file(path: &Path, image: &Image, scale: f32) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let mut lines = bytes.splitn(4, |&byte| byte == b'\n');
    assert_eq!(
        lines.next().unwrap(),
        if image.color_type() == ColorType::Gray {
            b"Pf"
        } else {
            b"PF"
        }
    );
    assert_eq!(
        std::str::from_utf8(lines.next().unwrap())?,
        format!("{} {}", image.width(), image.height())
    );
    assert_eq!(
        std::str::from_utf8(lines.next().unwrap())?.parse::<f32>()?,
        -scale
    );
    let payload = lines.next().unwrap();
    assert_eq!(payload.len(), image.pixels().len() * 4);
    let row = image.width() * image.color_type().channels();
    for (index, bytes) in payload.chunks_exact(4).enumerate() {
        let source = (image.height() - 1 - index / row) * row + index % row;
        assert_eq!(
            f32::from_le_bytes(bytes.try_into()?),
            image.pixels()[source]
        );
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 8 {
        return Err("usage: benchmark read|write SIZE CHANNELS SCALE REPEATS WARMUP FILE".into());
    }
    let operation = args[1].as_str();
    if !matches!(operation, "read" | "write") {
        return Err("invalid operation".into());
    }
    let size: usize = args[2].parse()?;
    let channels: usize = args[3].parse()?;
    let color = match channels {
        1 => ColorType::Gray,
        3 => ColorType::Rgb,
        _ => return Err("invalid channels".into()),
    };
    let scale: f32 = args[4].parse()?;
    let repeats: usize = args[5].parse()?;
    let warmup: usize = args[6].parse()?;
    if repeats == 0 {
        return Err("repeats must be positive".into());
    }
    let path = Path::new(&args[7]);
    let count = size
        .checked_mul(size)
        .and_then(|n| n.checked_mul(channels))
        .ok_or("image too large")?;
    let pixels = (0..count).map(|n| (n % 257) as f32 - 128.0).collect();
    let image = Image::new(size, size, color, pixels)?;
    let options = EncodeOptions {
        scale,
        ..EncodeOptions::default()
    };
    let mut samples = Vec::new();
    for iteration in 0..1 + warmup + repeats {
        let start = Instant::now();
        let decoded = if operation == "read" {
            Some(black_box(read_pfm(
                black_box(path),
                DecodeOptions::default(),
            )?))
        } else {
            write_pfm(black_box(path), black_box(image.view()), options)?;
            None
        };
        let elapsed = start.elapsed().as_secs_f64();
        if iteration > warmup {
            samples.push(elapsed);
        }
        // Correctness checks and disposal occur after timing, on every call.
        if let Some(decoded) = decoded {
            assert_eq!(
                (decoded.width(), decoded.height(), decoded.color_type()),
                (size, size, color)
            );
            for (&actual, &expected) in decoded.pixels().iter().zip(image.pixels()) {
                assert_eq!(actual, expected * scale);
            }
        } else {
            validate_file(path, &image, scale)?;
        }
    }
    let status = fs::read_to_string("/proc/self/status")?;
    let peak = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .ok_or("missing Linux RSS metric")?
        .split_whitespace()
        .nth(1)
        .ok_or("invalid RSS metric")?
        .parse::<usize>()?;
    println!("{{\"samples_seconds\":{samples:?},\"process_peak_rss_kib\":{peak}}}");
    Ok(())
}
