# Design and limitations

rustPFM is a focused PFM codec with no third-party dependencies in its default
build. It uses Rust's standard library; `no_std` is not an initial goal.
`ndarray` is an optional adapter rather than the core representation.

## Implemented for 0.1.0

- Validated owned Vec<f32> images and borrowed slices, with native-endian,
  contiguous, top-first samples and explicit grayscale/RGB layout.
- Shared buffered decoding with bounded headers, checked sizes, exact payloads,
  pixel limits, source metadata, and explicit raw/scaled interpretation.
- Bytes/writer encoding with a 64 KiB serialization buffer, plus sized file
  reads and atomic file replacement.
- Optional ndarray views and copying conversion from noncontiguous arrays.
- Independent Netpbm fixtures, exact-byte expectations, deterministic randomized
  float-bit round trips, malformed-input stress tests, and live interoperability.
- A staged-code pre-commit hook and CI for Rust 1.85.0/stable, optional features,
  documentation, package validation, and a 95% library line-coverage floor.

Safe Rust is enforced with `forbid(unsafe_code)`. Development-only tools and
optional adapters may have dependencies; the default published library has none.
The straightforward implementation is a measured baseline, not a claim of faster
I/O than NumPy. See [benchmarks](BENCHMARKS.md) for both library orders and raw data.

## Compatibility choices

Minimum supported Rust: **1.85.0**, edition 2024. CI validates that toolchain and
stable on Linux. Native Windows/macOS filesystem validation remains deferred.
The core API follows Rust ownership and I/O conventions rather than mechanically
reproducing Python's ndarray layouts. API stability follows Cargo's pre-1.0
semantic-versioning conventions; public changes will be documented.

Scale is a finite float32 magnitude. Default decoding multiplies samples, while
raw decoding retains stored values. Netpbm divides by nonunit magnitude, and
justPFM accepts a broader scale range; see [API details](API.md) before mixing
nonunit-scale files across implementations.

## Follow-up work

- Profile large-image decoding and writing against the recorded justPFM baseline
  before selecting optimizations, buffer changes, SIMD, or parallelism.
- Extend long-running coverage-guided fuzzing; deterministic randomized tests
  are not an exhaustive fuzz campaign.
- Validate native Windows/macOS permissions, symlinks, rename and cleanup behavior.
- Consider row streaming or borrowed raw-payload views only if real use cases
  justify the additional lifetime, alignment, byte-order and API complexity.
