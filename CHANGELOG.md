# Changelog

## 0.1.0 — release candidate

- Add validated owned images and borrowed pixel views with a dependency-free
  core and safe public APIs (minimum Rust 1.85).
- Read and write grayscale/RGB PFM files and byte buffers in both byte orders;
  support generic buffered readers/writers and explicit raw/scaled decoding.
- Bound headers, validate sizes, offer pixel limits, and atomically replace files.
- Add optional ndarray HWC views and conversion from strided or reversed arrays.
- Add independent Netpbm fixtures and live interoperability, malformed-input and
  deterministic randomized tests, and a 95% library line-coverage gate in the
  shared local/CI harness.
- Decode directly into initialized zeroed pixel storage and use borrowed,
  bounded vectored writes for native-endian encoding; retain a 64 KiB
  conversion buffer for opposite-endian output.
- Add explicit top-first/file-order storage, logical row access, in-place row
  conversion, and top-first ndarray views over either physical layout.
- Enable an optional, dependency-free Linux huge-page hint by default; document
  its allocator tradeoff and opt-out.
- Isolate unsafe internals in one private module and check allocation, byte
  views, direct I/O, and array views with pinned Miri in the quality harness.
- Record reproducible comparisons with justPFM and zune-ppm, including current performance
  limitations, and explain the justPFM → rustPFM name.
