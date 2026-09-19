# Safety and allocation policy

rustPFM exposes safe public APIs. The crate uses `#![deny(unsafe_code)]` with one
private exception: `src/buffer.rs`. That module provides zeroed `Vec<f32>`
allocation, borrowed byte views, and a Linux allocation hint. The default build
still has zero third-party dependencies; this policy does not claim that all
implementation code is safe Rust.

## Allocation and ownership

The decoder validates dimensions and checked sample/byte counts before requesting
a buffer. The buffer helper independently constructs `Layout::array::<f32>`;
overflow returns `Error::SizeOverflow`. Zero elements return an ordinary empty
vector without an allocation.

For a nonempty buffer, `alloc_zeroed` uses the global allocator with exactly the
alignment and byte size of the requested float32 elements. Null returns
`Error::Allocation`. Every byte of a successful allocation is initialized to
zero, which is a valid `f32` representation. `Vec::from_raw_parts` receives
that allocation exactly once, with length and capacity equal to the element
count. Its eventual deallocation uses the same allocator, size, and alignment;
ordinary vector growth, cloning, and drop remain valid. No `Vec<u8>` allocation
is reinterpreted as a vector with stronger alignment requirements.

Operating-system overcommit can still cause process termination later; fallible
allocation is not a total memory-use guarantee. Applications should set an
appropriate decode pixel limit for untrusted input.

## Borrowed byte views

An existing `f32` slice has aligned, initialized elements and no padding bytes.
Viewing that storage as `u8` requires only alignment 1; its byte length fits
because the original slice is valid. The resulting byte slice borrows the same
allocation for the input borrow's lifetime. Shared views allow no mutation;
mutable views preserve the original exclusive borrow. The helper creates no
independent owner, extends no lifetime, and exposes no pointer publicly.

Every float32 bit pattern is valid, including NaNs, infinities, and signed zero.
Consequently arbitrary byte writes, partial reads, or a reader that inspects the
buffer before writing cannot create invalid float values. Crucially, all bytes
are initialized **before** passing a mutable byte slice to an arbitrary safe
`Read` implementation. On I/O failure or unwinding the vector can be dropped
normally, even when only some bytes have been replaced. Byte-order conversion,
scaling, and row reversal then operate through ordinary safe float slices.

Native-endian encoding borrows initialized byte views for synchronous vectored
writes. Those views cannot outlive or mutate their source image. Opposite-endian
encoding converts into an initialized scratch buffer. Short writes,
interruptions, and writer errors are handled without transferring ownership of
pixel memory.

## Default Linux huge-page hint

The `hugepages` Cargo feature is enabled by default and has no Rust dependencies.
On Linux, the allocator helper makes a best-effort `madvise(MADV_HUGEPAGE)` request
for allocations of at least 2 MiB. It queries the system page size and selects
only complete pages inside the live allocation. Checked range calculations
exclude partial boundary pages; pointer arithmetic remains inside the allocation.
No hint is issued for an empty eligible range or an invalid page size.

This nondestructive hint changes neither the initialized contents nor ownership
of the buffer. The kernel may reject or ignore it; errors are ignored because
correctness does not depend on huge pages. The code changes no sysctl or other
system setting. Other operating systems do not execute the hint.

Huge pages can affect memory use and latency, and are not a universal performance
win. If the allocator retains mappings after the vector is dropped, the advice
can remain on those mappings and affect later allocations. rustPFM does not
attempt to undo it during deallocation. Disable new requests when that tradeoff
is unsuitable:

```toml
rustpfm = { path = "../rustPFM", default-features = false }
```

This leaves the initialized direct-buffer implementation available. Allocator
behavior and host policy still determine allocation performance.

## Validation and its limits

`scripts/check-safety.sh` pins `nightly-2026-09-19` and runs Miri with strict
provenance on buffer helpers, direct I/O, and ndarray views, on native and
interpreted big-endian Linux targets. Both local quality
hooks and CI run this script. Tests exercise unusual float bit patterns, empty
byte views, vector growth, partial/failing I/O, and both row orders. The public
library's minimum toolchain remains stable Rust 1.85.

Miri excludes Linux FFI calls rather than pretending to validate the kernel or
C ABI. Range arithmetic is tested separately, and a native large-allocation
test verifies initialization and use through the hint path. That test does not
prove that the kernel honored the hint. Native Windows/macOS behavior remains
unvalidated. Coverage and Miri check tested executions; neither replaces review
of the unsafe invariants or establishes exhaustive correctness.

## Reference contracts

- [Rust global zeroed allocation](https://doc.rust-lang.org/std/alloc/fn.alloc_zeroed.html)
- [Vec ownership and allocation layout](https://doc.rust-lang.org/std/vec/struct.Vec.html#method.from_raw_parts)
- [Vectored write advancement](https://doc.rust-lang.org/std/io/struct.IoSlice.html#method.advance_slices)
- [Linux memory advice](https://man7.org/linux/man-pages/man2/madvise.2.html)
