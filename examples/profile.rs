//! Coarse I/O attribution, not a CPU sampling profiler. Run with --release.
use rustpfm::{ColorType, DecodeOptions, EncodeOptions, Image, decode_reader, encode_writer};
use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Metrics {
    calls: usize,
    bytes: usize,
    elapsed: Duration,
}
struct Meter<T> {
    inner: T,
    metrics: Metrics,
}
impl<T> Meter<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            metrics: Metrics::default(),
        }
    }
}
impl<T: Read> Read for Meter<T> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let start = Instant::now();
        let result = self.inner.read(output);
        self.metrics.elapsed += start.elapsed();
        self.metrics.calls += 1;
        self.metrics.bytes += result.as_ref().copied().unwrap_or(0);
        result
    }
}
impl<T: Write> Write for Meter<T> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let start = Instant::now();
        let result = self.inner.write(input);
        self.metrics.elapsed += start.elapsed();
        self.metrics.calls += 1;
        self.metrics.bytes += result.as_ref().copied().unwrap_or(0);
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
fn report(operation: &str, iteration: usize, total: Duration, metrics: &Metrics) {
    println!(
        "{{\"operation\":\"{operation}\",\"iteration\":{iteration},\"total_seconds\":{},\"io_seconds\":{},\"io_calls\":{},\"bytes\":{}}}",
        total.as_secs_f64(),
        metrics.elapsed.as_secs_f64(),
        metrics.calls,
        metrics.bytes
    );
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: profile OUTPUT_FILE [READ_BUFFER_BYTES]")?;
    let capacity = std::env::args()
        .nth(2)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(64 * 1024);
    if capacity == 0 {
        return Err("read buffer must be positive".into());
    }
    // Create a fresh path so this profiling helper never overwrites an existing file.
    let mut file = Meter::new(
        File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?,
    );
    let size = 2048;
    let image = Image::new(
        size,
        size,
        ColorType::Rgb,
        (0..size * size * 3)
            .map(|i| (i % 257) as f32 - 128.0)
            .collect(),
    )?;
    for iteration in 0..6 {
        use std::io::{Seek, SeekFrom};
        file.inner.seek(SeekFrom::Start(0))?;
        file.metrics = Metrics::default();
        let start = Instant::now();
        encode_writer(&mut file, image.view(), EncodeOptions::default())?;
        report("write", iteration, start.elapsed(), &file.metrics);
    }
    drop(file);
    for iteration in 0..6 {
        let mut reader = BufReader::with_capacity(capacity, Meter::new(File::open(&path)?));
        let start = Instant::now();
        let result = decode_reader(&mut reader, DecodeOptions::default())?;
        report(
            "read",
            iteration,
            start.elapsed(),
            &reader.get_ref().metrics,
        );
        assert_eq!(result.pixels(), image.pixels());
    }
    std::fs::remove_file(path)?;
    Ok(())
}
