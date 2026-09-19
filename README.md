# rustPFM

**justPFM → rustPFM:** swap the **j** for an **r**, then swap Python for Rust.
A small Portable Float Map (PFM) image library inspired by
[justPFM](https://github.com/MartinPeris/justPFM).

[![Quality](https://github.com/MartinPeris/rustPFM/actions/workflows/quality.yml/badge.svg?branch=main&event=push)](https://github.com/MartinPeris/rustPFM/actions/workflows/quality.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](https://github.com/MartinPeris/rustPFM/blob/main/LICENSE)

**0.1.0 release candidate — not yet published on crates.io.**

## At a glance

- **Zero default dependencies**, safe public APIs, Rust **1.85+**; validated on Linux.
- Grayscale/RGB float32 images, both byte orders; top-first or file-order storage.
- Bytes, buffered readers, writers, and atomic file replacement.
- Bounded headers, checked dimensions, optional pixel limits, explicit scale modes.
- Optional `ndarray` views and conversion, including strided input.

## Try the candidate

Add the reviewed candidate checkout to your application's `Cargo.toml`:

```toml
[dependencies]
rustpfm = { path = "../rustPFM" }
# Add features = ["ndarray"] if you want the optional array adapter.
```

```rust
use rustpfm::{decode, encode, ColorType, DecodeOptions, EncodeOptions, Image};

fn main() -> rustpfm::Result<()> {
    let image = Image::new(3, 2, ColorType::Gray, vec![1., 2., 3., 4., 5., 6.])?;
    let bytes = encode(image.view(), EncodeOptions::default())?;
    let restored = decode(&bytes, DecodeOptions {
        max_pixels: Some(1_000_000),
        ..Default::default()
    })?;
    assert_eq!(restored.pixels(), image.pixels());
    Ok(())
}
```

| API | Purpose |
| --- | --- |
| `decode` / `encode` | Complete PFM byte buffers. |
| `decode_reader` / `encode_writer` | Standard-library buffered input / generic output. |
| `read_pfm` / `write_pfm` | Files, with size preflight / atomic replacement. |
| `Image::new` / `ImageView::new` | Validated owned / borrowed contiguous pixels. |
| `Image::as_ndarray` / `Image::from_ndarray` | Optional borrowed HWC view / owned array conversion. |

Use scale 1 to preserve sample values. Default decoding multiplies by the header
magnitude, matching justPFM's convention; `ScaleMode::Raw` preserves stored
samples. Netpbm uses a different nonunit scale convention. See the
[API reference](https://github.com/MartinPeris/rustPFM/blob/main/API.md) for scale,
layout, streaming, memory, and filesystem details. For direct file-order decoding,
set `row_order: RowOrder::BottomFirst`; logical `row(y)` and ndarray views still
count from the top. The default `hugepages` feature requests best-effort Linux
huge pages; use `default-features = false` to disable it. See the
[safety and allocation policy](https://github.com/MartinPeris/rustPFM/blob/main/SAFETY.md).

## Further reading

- [Benchmarks against justPFM and zune-ppm](https://github.com/MartinPeris/rustPFM/blob/main/BENCHMARKS.md): reproducible results, including workloads where Rust is slower.
- [Contributing and quality checks](https://github.com/MartinPeris/rustPFM/blob/main/CONTRIBUTING.md): commit hook, CI, 95% line-coverage gate, and interoperability.
- [Design and limitations](https://github.com/MartinPeris/rustPFM/blob/main/DESIGN.md), [changelog](https://github.com/MartinPeris/rustPFM/blob/main/CHANGELOG.md), and [release procedure](https://github.com/MartinPeris/rustPFM/blob/main/RELEASING.md).
