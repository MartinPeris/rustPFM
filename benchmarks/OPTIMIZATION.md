# First I/O optimization pass

The optimized codec batches reads and writes across row boundaries using its
existing 64 KiB scratch buffers. It preserves safe Rust, zero default
dependencies, fallible pixel-buffer reservation, exact payload validation,
byte order, scale semantics, contiguous top-first output and atomic file writes.
No public API or minimum Rust version changes.

## What profiling established

CPU sampling through `perf` was unavailable under this host's permissions.
The committed `examples/profile.rs` instead measures time spent in underlying
file Read/Write calls, alongside total stream-codec time. It validates decoded
pixels after timing. It is coarse attribution with instrumentation overhead;
its in-place writes are **not** the atomic-file benchmark below.

For a 2048² RGB image, batching reduces underlying calls from **2,049 to 769
writes** and **2,050 to 770 reads**, including the header and EOF handling.
The helper uses the same 8 KiB BufReader in both versions. No larger extra file
buffer is needed. The first iteration is discarded; all six calls are recorded
in [profile data](results/2026-09-18-io-profile.json).

A separate temporary timer around `pixels.resize(count, 0.0)` measured roughly
18 ms of zero-initialization for the 48 MiB image on the original decoder.
That timer was removed before benchmark measurements; its raw durations are
also in the profile data. To reproduce the probe, wrap that statement with
`Instant::now()` / `elapsed()` and report the duration after measuring it.
This attributes initialization time, not allocator/page-fault/memory-bandwidth
costs individually; no CPU sampling evidence establishes those sub-causes.

Avoiding zero-initialization by appending decoded values and then reversing
rows failed to improve the measured large-image read time, so that experiment
was discarded. A larger BufReader also introduced extra copying without a
large-image benefit. The retained changes only batch the existing scratch I/O;
large RGB initialization remains a profiling target.

## Reproduce

Use the environment and runner instructions in [BENCHMARKS.md](../BENCHMARKS.md).
Both before and after runs use unchanged `compare.py` and `compare_rust.py`.

- Before: `d8bd90842664862e8ce657416446119098f02664`.
- After: `6b38907e3d58a816a1b070b216c2a7df886d19d9`.
- Each commit: run each runner once normally and once with `--reverse-order`.
- Defaults: 1024²/2048², grayscale/RGB, five samples after one validation call
  and two warmups. The justPFM runner covers reads/writes and scales 1/2;
  zune-ppm covers scale-1 reads only, subject to its documented limitations.
- For coarse attribution: `cargo run --locked --release --example profile --
  /tmp/rustpfm-profile.pfm 8192` (put the command on one line). The output path
  must not exist. Copy this helper from the after commit into a separate
  baseline checkout to repeat the before measurement.

Measured 2026-09-18 UTC, Ryzen 9 7940HS, Linux/ext4, Rust 1.98.1, justPFM 1.2.1,
NumPy 2.2.6, pinned zune-ppm/zune-core 0.5.1. Each full trial started from a
clean checkout; no other project builds/tests ran alongside it. The OS was not
isolated. Warm-cache, output-layout, RSS and durability limits in BENCHMARKS.md
apply. These are sequential before/after observations, not confidence intervals
or universal speed guarantees.

## justPFM comparison

Ranges below span the **two trial medians**, in milliseconds (lower is better).
They are not the range of individual samples or statistical error bars.

| Size | Channels | Operation | Scale | Rust before | Rust after | justPFM after |
| --- | --- | --- | --- | ---: | ---: | ---: |
| 1024² | 1 | read | 1 | 0.820–0.825 | 0.480–0.494 | 1.224–1.877 |
| 1024² | 1 | read | 2 | 0.729–0.807 | 0.449–0.517 | 1.333–1.356 |
| 1024² | 1 | write | 1 | 4.402–4.560 | 2.154–2.184 | 1.742–1.755 |
| 1024² | 1 | write | 2 | 4.384–4.584 | 2.196–2.255 | 1.723–1.753 |
| 1024² | 3 | read | 1 | 3.122–3.123 | 2.620–2.632 | 2.552–2.571 |
| 1024² | 3 | read | 2 | 2.946–3.100 | 2.585–2.627 | 2.902–2.932 |
| 1024² | 3 | write | 1 | 10.680–11.051 | 7.159–8.304 | 5.467–5.596 |
| 1024² | 3 | write | 2 | 10.738–10.806 | 7.017–7.410 | 5.558–5.616 |
| 2048² | 1 | read | 1 | 4.641–4.747 | 3.582–3.650 | 3.154–3.179 |
| 2048² | 1 | read | 2 | 4.654–4.783 | 3.555–3.601 | 3.623–3.669 |
| 2048² | 1 | write | 1 | 16.299–16.689 | 9.644–9.656 | 6.817–6.920 |
| 2048² | 1 | write | 2 | 16.134–16.331 | 9.285–9.528 | 6.812–6.827 |
| 2048² | 3 | read | 1 | 29.321–29.327 | 29.237–29.410 | 8.498–9.027 |
| 2048² | 3 | read | 2 | 29.744–32.827 | 29.191–29.303 | 10.374–10.406 |
| 2048² | 3 | write | 1 | 35.001–35.628 | 27.808–27.847 | 16.744–23.543 |
| 2048² | 3 | write | 2 | 35.989–36.530 | 27.815–27.828 | 17.275–18.269 |

Writes improved across all measured cases, with roughly 20–53% lower latency
when matching trial order. For scale-1 2048² RGB, writes improved from 35–36 ms
to about 28 ms. Smaller reads also improved; 2048² RGB scale-1 reads remain
roughly 29 ms. **justPFM still wins most workloads**, especially large RGB
reads. Rust wins the small grayscale reads and is competitive in some nonunit
scale cases; this optimization does not establish general superiority to NumPy.

## zune-ppm comparison

| Size | Channels | Rust before | Rust after | zune-ppm after |
| --- | --- | ---: | ---: | ---: |
| 1024² | 1 | 0.701–0.722 | 0.411–0.452 | 4.928–5.133 |
| 1024² | 3 | 2.693–2.834 | 2.223–2.250 | 15.596–15.755 |
| 2048² | 1 | 4.710–4.743 | 3.710–3.737 | 20.642–20.946 |
| 2048² | 3 | 26.770–27.114 | 26.799–27.239 | 77.316–80.929 |

rustPFM remains faster than the measured zune-ppm buffered file-read path in
all four cases. Large RGB Rust reads did not materially improve in this runner
either. Other zune input paths, writes, odd heights and nonunit scales are not
measured. The two runners are separate experiments, so their absolute timings
must not be treated as one simultaneous three-library trial.

## Raw measurements

- [before](results/2026-09-18-io-before.json)
- [before-reversed](results/2026-09-18-io-before-reversed.json)
- [after](results/2026-09-18-io-after.json)
- [after-reversed](results/2026-09-18-io-after-reversed.json)
- [zune-before](results/2026-09-18-io-zune-before.json)
- [zune-before-reversed](results/2026-09-18-io-zune-before-reversed.json)
- [zune-after](results/2026-09-18-io-zune-after.json)
- [zune-after-reversed](results/2026-09-18-io-zune-after-reversed.json)

## Validation

31 unit/integration tests and a README doctest pass, with 97.39% library line
coverage above the 95% gate. Added independent exact-wire tests cross rows and
64 KiB boundaries, including odd heights, short writes, both byte orders,
nonunit scaling and NaN/signed-zero bit preservation in raw mode. Rust 1.85
all-feature tests and live Netpbm interoperability pass. Existing malformed-input,
I/O-error, pixel-limit and atomic replacement tests remain in the harness.
