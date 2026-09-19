# Changelog

## 0.1.0 — release candidate

- Add validated owned images and borrowed pixel views with a dependency-free,
  safe Rust core (minimum Rust 1.85).
- Read and write grayscale/RGB PFM files and byte buffers in both byte orders;
  support generic buffered readers/writers and explicit raw/scaled decoding.
- Bound headers, validate sizes, offer pixel limits, and atomically replace files.
- Add optional ndarray HWC views and conversion from strided or reversed arrays.
- Add independent Netpbm fixtures and live interoperability, malformed-input and
  deterministic randomized tests, and a 95% library line-coverage gate in the
  shared local/CI harness.
- Batch serialized rows into 64 KiB writes and use a 64 KiB file-read buffer.
- Record reproducible comparisons with justPFM and zune-ppm, including current performance
  limitations, and explain the justPFM → rustPFM name.
