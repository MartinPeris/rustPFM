# Performance comparison

This opt-in Linux benchmark compares the Rust release build with installed
justPFM 1.2.1. It is a measurement tool, not a CI timing gate. No speed advantage
is assumed: NumPy already implements justPFM's array operations in native code.

```bash
python3 -m venv /tmp/rustpfm-bench-venv
/tmp/rustpfm-bench-venv/bin/pip install justpfm==1.2.1 numpy==2.2.6
/tmp/rustpfm-bench-venv/bin/python benchmarks/compare.py \
  --sizes 1024 2048 --repeats 5 --warmup 2 \
  --output benchmarks/results/local.json
```

Run from an otherwise idle machine with the Rust toolchain available on `PATH`.
The runner builds `cargo build --locked --release --example benchmark` before
starting workers. Each library/case uses a separate process; library order
alternates between cases. Process startup and imports are outside the timers.
The default dataset covers square grayscale and RGB images, reads and atomic
replacement writes, and scale magnitudes 1 and 2. Both libraries receive the
same nonuniform values `(sample_index % 257) - 128`, little-endian files and
contiguous top-first input. File creation uses an independent NumPy fixture
writer; every measured output is validated outside the timed section, including
header, byte order, row order, dimensions and scale semantics.

Files are warm/cache-eligible: fixture creation, a correctness call, and two
warmups precede five measured calls. No eviction or `fsync` is performed. Timings
include library-call allocation and scaling, but exclude validation and disposal
of returned images. Atomic replacement is measured for both implementations;
these are not durable disk throughput measurements.

The output JSON retains every timing sample, medians, payload throughput, source
hashes and Git revision/status, binary hash, compiler, installed Python/NumPy/
justPFM versions, CPU, affinity, filesystem and cache policy. Linux peak RSS is
recorded separately for each worker. It includes runtime, fixture pixels,
validation buffers and all calls; it excludes OS page cache. It is **not a
per-call allocation measurement**, and different runtime/validation costs mean
it should not be interpreted as an exact allocation-efficiency comparison.
Rust's decoder produces contiguous top-first pixels; justPFM returns a writable
array with reversed row strides. No additional contiguity conversion is charged
to Python. Inputs with arbitrary strides and in-memory APIs are outside this
first comparison.

## Recorded results

Measured on 2026-09-18 UTC, AMD Ryzen 9 7940HS, Linux x86-64, local ext4/NVMe,
Rust 1.98.1 default release profile, Python 3.12, NumPy 2.2.6 and justPFM 1.2.1.
The clean measured commit was `1f9d02ba881c3bce1ae1083f8a380b4f84e99337`
(version `0.1.0-dev`), including the integrated codec and EOF retry fix. Other
project builds/tests were paused for both runs; the ordinary OS environment was
not isolated. This is one machine, not a cross-platform performance guarantee.

Raw data: [first trial](benchmarks/results/2026-09-18-linux.json) and
[reversed-order trial](benchmarks/results/2026-09-18-linux-reversed.json).
Reproduce the second trial by adding `--reverse-order` and a separate `--output`.
Each cell below is the median of five calls in milliseconds; smaller is better.

| Size | Channels | Operation | Scale | Rust | Python | Rust reversed | Python reversed |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1024² | 1 | read | 1 | 0.814 | 1.213 | 0.754 | 1.253 |
| 1024² | 1 | read | 2 | 0.791 | 1.297 | 0.807 | 1.256 |
| 1024² | 1 | write | 1 | 4.477 | 1.723 | 4.567 | 1.735 |
| 1024² | 1 | write | 2 | 4.476 | 1.751 | 4.522 | 1.711 |
| 1024² | 3 | read | 1 | 3.147 | 2.519 | 3.094 | 2.528 |
| 1024² | 3 | read | 2 | 3.077 | 2.869 | 3.358 | 3.025 |
| 1024² | 3 | write | 1 | 10.975 | 5.435 | 11.654 | 5.640 |
| 1024² | 3 | write | 2 | 11.130 | 5.390 | 11.591 | 5.431 |
| 2048² | 1 | read | 1 | 4.691 | 3.674 | 4.610 | 3.142 |
| 2048² | 1 | read | 2 | 4.695 | 3.485 | 4.766 | 3.689 |
| 2048² | 1 | write | 1 | 16.727 | 6.773 | 16.468 | 6.812 |
| 2048² | 1 | write | 2 | 16.455 | 6.736 | 18.948 | 8.298 |
| 2048² | 3 | read | 1 | 30.231 | 8.447 | 30.046 | 10.183 |
| 2048² | 3 | read | 2 | 30.151 | 10.086 | 29.652 | 10.187 |
| 2048² | 3 | write | 1 | 37.459 | 17.705 | 41.205 | 18.983 |
| 2048² | 3 | write | 2 | 35.871 | 19.031 | 36.107 | 18.163 |

**The initial Rust implementation is not generally faster than justPFM.** It wins
1024² grayscale reads here, but justPFM wins larger reads and every measured
write. For the 48 MiB RGB case at scale 1, Rust reads took about 30 ms versus
8–10 ms for justPFM; Rust writes took 37–41 ms versus 18–19 ms. Reversing worker
order preserved the direction of those differences. These are observed medians,
not statistical confidence intervals or universal speed ratios.

The 48 MiB RGB workers recorded roughly 98 MiB Rust and 313 MiB Python peak RSS.
These totals include different validation allocations and runtime overhead, so
they **do not establish a per-call codec memory advantage**. The JSON retains
all case-specific measurements and the limitations above apply.

The dependency-free core, safe Rust and bounded streaming scratch are useful
properties independently of speed. Follow-up profiling should investigate sample
conversion, output initialization, row traversal and file I/O granularity before
choosing optimizations. None of these candidate explanations has been established
as the bottleneck by this benchmark. Preserve these results as the initial
baseline and rerun both trial orders after any optimization.

## Rust comparison: zune-ppm

A separate runner compares file reads with
[zune-ppm 0.5.1](https://docs.rs/zune-ppm/0.5.1/zune_ppm/), using pinned
zune-core 0.5.1 with its `std` feature. These dependencies live in a separate
benchmark package and do not enter rustPFM's library dependency graph.

```bash
python3 benchmarks/compare_rust.py --sizes 1024 2048 --repeats 5 --warmup 2 \
  --output benchmarks/results/rust-local.json
python3 benchmarks/compare_rust.py --sizes 1024 2048 --repeats 5 --warmup 2 \
  --reverse-order --output benchmarks/results/rust-local-reversed.json
```

Only the Python standard library and Rust toolchain are needed. Both codecs
are compiled together with the default release profile and measured in separate,
serial workers. File opening, buffered decoding, allocation, output extraction
and file closing are timed; independent fixture generation, startup, validation
and result disposal are excluded. Every returned sample, dimension and channel
count is checked against the nonuniform input. One validation call and two
warmups precede five samples; order alternates and the second trial reverses it.
The local/CI quality harness runs small correctness cases without timing gates.

This comparison covers little-endian, scale-1 grayscale/RGB files with even
square dimensions. zune-ppm does not encode PFM, ignores scale magnitude, and
its 0.5.1 row-flip implementation does not handle odd heights correctly, so
writes, nonunit scales and odd heights are deliberately excluded. Its
[decoder source](https://docs.rs/crate/zune-ppm/0.5.1/source/src/decoder.rs)
documents the implementation used. Both return contiguous top-first float32
pixels, but their validation guarantees differ: rustPFM also checks the exact
payload length and rejects trailing data. zune-ppm is used with a standard
`BufReader<File>`; rustPFM uses its public `read_pfm` API. This measures those
file-read paths, not every possible buffering or in-memory configuration.

Raw JSON records pinned dependency versions, source/binary hashes, compiler,
Git state, host/storage information, all samples and process peak RSS. The
warm-cache and RSS limitations described above apply; these results should not
be combined with older justPFM trials as if all libraries ran simultaneously.
