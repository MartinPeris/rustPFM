# Design and roadmap

## Scope

Build a focused PFM codec with zero third-party dependencies in the default
build. Use Rust's standard library; `no_std` support is not an initial goal.
Do not implement a general numerical array library or image-processing toolkit.
The repository is currently a scaffold, with no public codec API.

## Proposed core

- An image owns a contiguous, top-first `Vec<f32>` with width, height, and a
  grayscale/RGB enum. A validated constructor checks nonzero dimensions, checked
  multiplication, and exact sample count. Keep fields private to preserve those
  invariants. Use native-endian floats in memory.
- Encoding accepts borrowed pixels so callers need not transfer ownership or
  copy their input. Begin with contiguous slices; add strided views through an
  optional adapter when there is a demonstrated need.
- Share codec logic between standard-library reader/writer interfaces and byte
  convenience methods. Specify EOF/trailing-byte policy explicitly: a generic
  stream cannot promise the file reader's size check without additional input.
- Bound header reads, reject invalid dimensions/scales, and check payload sizes
  and pixel limits before allocation. Use fallible allocation where practical.
  Keep I/O and format errors distinguishable; malformed input must not panic.
- Handle byte order explicitly with standard-library float conversions and
  buffered bulk I/O. Start with safe Rust; optimize based on measured bottlenecks.

## Scale and interoperability

Decide and document scale semantics before freezing the API. justPFM multiplies
samples by the header scale magnitude while Netpbm divides by it. Preserve access
to the header magnitude and byte order; consider an explicit raw/scaled decode
option rather than silently claiming the two conventions are equivalent.
Keep defaults and any compatibility mode covered by independent fixtures.

## Files and memory

A bytes API necessarily retains the input or output buffer. Pixel limits must
not be described as a bound on total process memory. Benchmark memory use as
well as throughput.

Atomic file replacement is a separate filesystem convenience feature. Specify
permission and symlink behavior, clean up failed writes, and test supported
platforms before promising it. Atomic replacement alone does not imply durable
storage. Do not sacrifice correctness merely to avoid one carefully chosen
optional dependency.

## Implementation milestones

1. Agree on the image/error types, codec API, scale convention, and minimum
   supported Rust version. Implement validated image construction.
2. Implement header parsing and decoding, then encoding, using shared reader/
   writer logic. Cover grayscale/RGB, endian handling, row order, NaN/infinity,
   scales, truncation, trailing bytes, overflow, and resource limits.
3. Add independent fixtures and interoperability checks against justPFM and
   Netpbm; add property/fuzz tests and a measured coverage gate. Development-only
   dependencies are acceptable and do not need to enter the published core.
4. Add file conveniences and optional `ndarray` adapters. Verify noncontiguous
   views and optional-feature builds without making ndarray a default dependency.
5. Record release-mode benchmarks with raw samples, hardware, workload, cache
   policy, and allocation measurements. Compare equivalent semantics with
   justPFM and existing Rust codecs before claiming any speed advantage.
6. Audit crate contents, document the supported toolchains/platforms and stable
   API, then enable publication and prepare the first release.

## Initial support policy

CI and the local harness target stable Rust on Linux. The edition is 2024; a
minimum supported toolchain has not yet been selected or tested. Native Windows
and macOS validation is deferred. No cross-platform filesystem guarantees or
performance claims are made by this scaffold.
