# Design and limitations

rustPFM is a focused PFM codec with no third-party dependencies in its default
build. It uses Rust's standard library; `no_std` is not an initial goal.
`ndarray` is an optional adapter rather than the core representation.

## Implemented for 0.1.0

- Validated owned Vec<f32> images and borrowed slices, with native-endian,
  contiguous samples, explicit row order, and grayscale/RGB layout.
- Shared buffered decoding with bounded headers, checked sizes, exact payloads,
  pixel limits, source metadata, and explicit raw/scaled interpretation.
- Direct decoding into initialized pixel storage; native-endian encoding borrows
  pixel bytes for bounded vectored writes, with a 64 KiB conversion fallback.
- Sized file reads, atomic file replacement, and optional file-order storage
  that avoids decoding row reversal.
- Optional ndarray views and copying conversion from noncontiguous arrays.
- Independent Netpbm fixtures, exact-byte expectations, deterministic randomized
  float-bit round trips, malformed-input stress tests, and live interoperability.
- A staged-code pre-commit hook and CI for Rust 1.85.0/stable, optional features,
  documentation, package validation, and a 95% library line-coverage floor.

Public APIs are safe Rust. The crate denies unsafe code except in the private
`src/buffer.rs` module, which owns zeroed allocation, initialized byte views,
and a best-effort Linux huge-page hint. The `hugepages` feature is enabled by
default and can be disabled with `default-features = false`; it adds no crate
dependencies. It changes no system settings, ignores hint failures, and may
leave advice on mappings retained by the allocator after deallocation.
[SAFETY.md](SAFETY.md) documents the invariants and tradeoffs.

Development-only tools and optional adapters may have dependencies; the default
published library has none. See [benchmarks](BENCHMARKS.md) and the
[direct-buffer investigation](benchmarks/FAST_PATH.md) for measurement methods,
results, and limitations.

## Compatibility choices

Minimum supported Rust: **1.85.0**, edition 2024. CI validates that toolchain and
stable on Linux. The local/CI safety harness uses pinned nightly-2026-09-19
for Miri; consumers do not need nightly. Native Windows/macOS filesystem
validation remains deferred.
The core API follows Rust ownership and I/O conventions rather than mechanically
reproducing Python's ndarray layouts. API stability follows Cargo's pre-1.0
semantic-versioning conventions; public changes will be documented.

Scale is a finite float32 magnitude. Default decoding multiplies samples, while
raw decoding retains stored values. Netpbm divides by nonunit magnitude, and
justPFM accepts a broader scale range; see [API details](API.md) before mixing
nonunit-scale files across implementations.

## Follow-up work

- Measure direct-buffer paths across allocators, Linux huge-page policies, and
  platforms; memory-allocation behavior and storage order affect comparisons.
- Extend long-running coverage-guided fuzzing; deterministic randomized tests
  are not an exhaustive fuzz campaign.
- Validate native Windows/macOS permissions, symlinks, rename and cleanup behavior.
- Consider row streaming or borrowed raw-payload views only if real use cases
  justify the additional lifetime, alignment, byte-order and API complexity.
