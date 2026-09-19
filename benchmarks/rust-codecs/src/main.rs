//! Isolated file-read comparison; every result is checked outside the timer.
use std::{error::Error, fs, hint::black_box, io::BufReader, path::Path, time::Instant};
use zune_core::result::DecodingResult;
use zune_ppm::PPMDecoder;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if !(7..=8).contains(&args.len()) {
        return Err("usage: comparison rustpfm|zune-ppm SIZE CHANNELS REPEATS WARMUP FILE".into());
    }
    let size: usize = args[2].parse()?;
    let channels: usize = args[3].parse()?;
    let repeats: usize = args[4].parse()?;
    let warmup: usize = args[5].parse()?;
    if size == 0 || size % 2 != 0 || !matches!(channels, 1 | 3) || repeats == 0 {
        return Err("requires positive even size, 1 or 3 channels, and positive repeats".into());
    }
    let path = Path::new(&args[6]);
    let row_order = match args.get(7).map(String::as_str).unwrap_or("top") {
        "top" => rustpfm::RowOrder::TopFirst,
        "bottom" => rustpfm::RowOrder::BottomFirst,
        _ => return Err("invalid row order".into()),
    };
    let count = size
        .checked_mul(size)
        .and_then(|n| n.checked_mul(channels))
        .ok_or("size overflow")?;
    let mut samples = Vec::new();
    for iteration in 0..1 + warmup + repeats {
        let start = Instant::now();
        let (width, height, actual_channels, pixels) = match args[1].as_str() {
            "rustpfm" => {
                let image = rustpfm::read_pfm(
                    black_box(path),
                    rustpfm::DecodeOptions {
                        row_order,
                        ..Default::default()
                    },
                )?;
                (
                    image.width(),
                    image.height(),
                    image.color_type().channels(),
                    image.into_pixels(),
                )
            }
            "zune-ppm" => {
                let mut decoder = PPMDecoder::new(BufReader::new(fs::File::open(black_box(path))?));
                let result = decoder.decode().map_err(|error| format!("{error:?}"))?;
                let (width, height) = decoder.dimensions().ok_or("missing dimensions")?;
                let channels = decoder
                    .colorspace()
                    .ok_or("missing colorspace")?
                    .num_components();
                let DecodingResult::F32(pixels) = result else {
                    return Err("expected float32 output".into());
                };
                (width, height, channels, pixels)
            }
            _ => return Err("unknown codec".into()),
        };
        black_box(&pixels);
        let elapsed = start.elapsed().as_secs_f64();
        if iteration > warmup {
            samples.push(elapsed);
        }
        assert_eq!((width, height, actual_channels), (size, size, channels));
        assert_eq!(pixels.len(), count);
        for (index, &actual) in pixels.iter().enumerate() {
            let logical_index =
                if args[1] == "rustpfm" && row_order == rustpfm::RowOrder::BottomFirst {
                    let row = size * channels;
                    (size - 1 - index / row) * row + index % row
                } else {
                    index
                };
            assert_eq!(
                actual,
                (logical_index % 257) as f32 - 128.0,
                "sample {index}"
            );
        }
        // Validation and result disposal are outside the timed section.
    }
    let status = fs::read_to_string("/proc/self/status")?;
    let peak = status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))
        .ok_or("missing RSS")?
        .split_whitespace()
        .nth(1)
        .ok_or("invalid RSS")?
        .parse::<usize>()?;
    println!("{{\"samples_seconds\":{samples:?},\"process_peak_rss_kib\":{peak}}}");
    Ok(())
}
