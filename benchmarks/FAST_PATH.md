# Direct-buffer performance results

Default 2048² RGB reads now take **8.57–8.69 ms**, compared with
27.87–31.19 ms in the previous optimization and 8.54–8.68 ms in justPFM.
That is about 3.2–3.6× faster than the previous Rust implementation and
essentially tied with justPFM on this machine. These are warm-file measurements,
not a promise about every workload or operating system.

## What changed

The decoder reads directly into fully initialized `f32` storage. Vectored reads
scatter file rows into their final top-first positions, avoiding a separate
row-reversal pass. Native-endian writes borrow the pixel bytes and batch rows
with vectored I/O. Foreign-endian conversion, scaling, strict input validation,
and atomic file replacement remain supported.

Default output remains contiguous and top-first. Explicit `RowOrder::BottomFirst`
uses contiguous file-order storage; `row(y)` and ndarray views still expose
logical top-first rows. The physical order of `pixels()` is explicit. Adding
`DecodeOptions::row_order` changes exhaustive struct literals before 0.1.0.

The public API remains safe, with zero default Rust dependencies and Rust 1.85
support. A private unsafe module handles typed byte views, zeroed allocation,
and an optional Linux huge-page hint. The default `hugepages` feature requests
`MADV_HUGEPAGE` for large allocations; `default-features = false` disables it.
It changes no system settings, ignores advice failures, and does nothing on
other operating systems. Advice can persist in allocator-retained mappings.
See [the safety rationale and validation limits](../SAFETY.md).

## Default top-first comparison

All values below are milliseconds. Each range spans **two trial medians**
(normal and reversed library order), each with five samples after two warmups;
it is not a confidence interval. The previous Rust revision was measured again
for this comparison. The justPFM column comes from the candidate trials.

| Image | Operation | Scale | Previous Rust | Current Rust | justPFM |
|---|---|---:|---:|---:|---:|
| 1024² gray | read | 1 | 0.426–0.509 | 0.526–0.611 | 1.260–1.285 |
| 1024² gray | read | 2 | 0.469–0.591 | 0.574–0.745 | 1.277–1.435 |
| 1024² gray | write | 1 | 1.859–1.946 | 1.417–1.654 | 1.713–1.853 |
| 1024² gray | write | 2 | 1.959–2.187 | 1.396–1.595 | 1.769–1.784 |
| 1024² RGB | read | 1 | 2.422–2.573 | 1.957–1.964 | 2.598–2.632 |
| 1024² RGB | read | 2 | 2.457–2.472 | 2.216–2.324 | 2.833–3.039 |
| 1024² RGB | write | 1 | 6.407–6.685 | 4.153–4.256 | 5.409–5.577 |
| 1024² RGB | write | 2 | 6.507–6.649 | 4.044–4.253 | 5.424–5.686 |
| 2048² gray | read | 1 | 3.452–3.644 | 2.791–2.892 | 3.175–3.248 |
| 2048² gray | read | 2 | 3.461–3.530 | 3.192–3.203 | 3.510–3.523 |
| 2048² gray | write | 1 | 8.760–9.028 | 5.742–5.902 | 6.784–6.802 |
| 2048² gray | write | 2 | 8.744–8.826 | 5.797–5.908 | 6.674–6.891 |
| 2048² RGB | read | 1 | 27.866–31.187 | 8.569–8.691 | 8.540–8.679 |
| 2048² RGB | read | 2 | 27.708–34.586 | 10.198–10.415 | 10.196–10.421 |
| 2048² RGB | write | 1 | 28.073–31.087 | 15.189–23.249 | 16.368–17.920 |
| 2048² RGB | write | 2 | 26.504–28.778 | 15.718–15.729 | 16.255–16.894 |

Small grayscale default reads regress relative to the previous Rust revision,
although they remain faster than justPFM. The large scale-1 RGB write varied
from 15.19 to 23.25 ms; it beat justPFM in one trial and lost in the other.
The measurements do not establish a universal win.

## Optional bottom-first reads

These are separate trials with explicitly different physical output storage.
No Python contiguity conversion is charged: justPFM already exposes reversed
row strides. Bottom-first storage reduces scatter overhead, but is not required
to achieve the default large-image result above.

| Image | Operation | Scale | Rust bottom-first | justPFM |
|---|---|---:|---:|---:|
| 1024² gray | read | 1 | 0.377–0.453 | 1.222–1.251 |
| 1024² gray | read | 2 | 0.496–0.546 | 1.362–1.382 |
| 1024² RGB | read | 1 | 1.706–1.787 | 2.565–2.581 |
| 1024² RGB | read | 2 | 2.168–2.179 | 2.897–2.916 |
| 2048² gray | read | 1 | 2.418–2.661 | 3.166–3.177 |
| 2048² gray | read | 2 | 2.936–3.202 | 3.711–3.993 |
| 2048² RGB | read | 1 | 8.352–8.488 | 8.553–8.613 |
| 2048² RGB | read | 2 | 10.151–10.366 | 10.189–10.510 |

## Other Rust implementation

The isolated Rust runner compares zune-ppm 0.5.1 through its `BufReader<File>`
decode path. Default rustPFM is approximately 8–12× faster in these cases.
This does not compare every possible zune API. zune-ppm does not write PFM;
scale-1, even-sized inputs avoid its differing scale and odd-height behavior.
The two benchmark runners have different fixture/validation setup, so their
absolute Rust timings must not be combined as one simultaneous experiment.

| Image | Rust top-first | zune-ppm (top trial) | Rust bottom-first | zune-ppm (bottom trial) |
|---|---:|---:|---:|---:|
| 1024² gray | 0.480–0.536 | 5.380–5.532 | 0.380–0.408 | 5.574–5.808 |
| 1024² RGB | 1.640–1.673 | 16.291–17.999 | 1.622–1.688 | 16.358–17.796 |
| 2048² gray | 2.660–2.811 | 21.907–21.977 | 2.424–2.484 | 22.447–23.032 |
| 2048² RGB | 7.279–7.461 | 83.284–86.291 | 7.032–7.123 | 82.537–85.366 |

## Why the allocation hint matters here

This Linux host uses `always [madvise] never` transparent-huge-page policy.
[NumPy requests huge pages by default](https://numpy.org/doc/stable/reference/global_state.html).
A controlled 2048² RGB scale-1 read probe using the final Rust runtime sources
measured 8.75–8.77 ms with the default feature versus 21.38–22.10 ms without it
(default/off/off/default, five samples and two warmups per trial).
A separate diagnostic during development measured NumPy at 11.67–13.08 ms
with its hint enabled versus 26.32–28.11 ms disabled (on/off/off/on).
These controls isolate an important host-specific effect; they are not the
headline library comparison. All headline Python runs retain normal NumPy
huge-page behavior. No host policy was changed.

The profiling wrapper forwards vectored calls. On 2048² RGB it records 34
underlying read calls and 33 write calls, versus 770/769 in the prior batched
implementation. This is a coarse I/O-call diagnostic; its in-place write is
not the atomic-write benchmark and its timings should not be mixed with it.

## Provenance and reproduction

Measured 2026-09-19 on Linux x86-64, Ryzen 9 7940HS, ext4/NVMe, Rust 1.98.1
release defaults without `target-cpu` overrides, Python 3.12.3, justPFM 1.2.1,
and NumPy 2.2.6. Prior revision: `fa33d6dea2d69f8955dd5d8e4f8bc0e2a9e8892d`.
Candidate runtime and benchmark revision:
`fca8bd136aa2b99c8869c09c1a79a8c1478ca659`.
All ten comparison reports were produced from clean trees. Subsequent changes
add a boundary test and this report; recorded source hashes were checked
against the final runtime and benchmark sources.

Workers run serially in separate processes, with alternating library order;
no other project builds or tests ran concurrently. Every measured result is
validated outside the timer. Files are warm/cache-eligible, without eviction
or `fsync`; allocation and scaling are timed, but startup, validation, and
result disposal are not. OS noise remains possible. Peak RSS includes the
whole worker and is not a per-call memory-efficiency measurement.

```bash
python benchmarks/compare.py --sizes 1024 2048 --repeats 5 --warmup 2 --output /tmp/top.json
python benchmarks/compare.py --sizes 1024 2048 --repeats 5 --warmup 2 --row-order bottom --operations read --output /tmp/bottom.json
python benchmarks/compare_rust.py --sizes 1024 2048 --repeats 5 --warmup 2 --output /tmp/zune-top.json
python benchmarks/compare_rust.py --sizes 1024 2048 --repeats 5 --warmup 2 --row-order bottom --output /tmp/zune-bottom.json
# Repeat each command with --reverse-order and a different output path.
```

See [benchmark setup and limitations](../BENCHMARKS.md). Raw reports retain
samples, source/binary hashes, revision, environment and worker order:

- [before](results/2026-09-19-direct-before.json)
- [before-reversed](results/2026-09-19-direct-before-reversed.json)
- [top](results/2026-09-19-direct-top.json)
- [top-reversed](results/2026-09-19-direct-top-reversed.json)
- [bottom](results/2026-09-19-direct-bottom.json)
- [bottom-reversed](results/2026-09-19-direct-bottom-reversed.json)
- [zune-top](results/2026-09-19-direct-zune-top.json)
- [zune-top-reversed](results/2026-09-19-direct-zune-top-reversed.json)
- [zune-bottom](results/2026-09-19-direct-zune-bottom.json)
- [zune-bottom-reversed](results/2026-09-19-direct-zune-bottom-reversed.json)
- [Rust huge-page control](results/2026-09-19-direct-rust-hugepage-probe.json)
- [NumPy development diagnostic](results/2026-09-19-direct-numpy-hugepage-probe.json)
- [I/O profile](results/2026-09-19-direct-profile.jsonl)

## Correctness checks

The local harness checks formatting, Clippy, zero default dependencies, tests,
95% line coverage, documentation, packaging and benchmark smoke cases.
Strict-provenance Miri checks the private buffer boundary and direct I/O on
native and interpreted big-endian targets. Linux FFI is excluded from Miri;
native tests separately exercise the advised allocation. Tests cover partial
vectored I/O, interruption, errors/panics, initialized buffers, NaN bit patterns,
row/chunk/batch boundaries and storage-order views. Rust 1.85 and independent
Netpbm interoperability checks also pass. See [SAFETY.md](../SAFETY.md).
