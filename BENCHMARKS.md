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

The first release's raw results and interpretation will be recorded after the
integrated codec passes correctness checks, on an otherwise idle machine.
