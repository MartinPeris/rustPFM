# Direct-buffer investigation

This revision replaces pixel staging with initialized typed-buffer I/O, adds
bounded native-endian vectored writes, and enables an optional Linux huge-page
hint. Default decoding stays top-first; explicit bottom-first storage skips
row reversal while logical row access remains top-first.

See [SAFETY.md](../SAFETY.md) for the private unsafe boundary, allocator policy,
opt-out, and Miri/native validation limits. Matched default and file-order
measurements against justPFM and zune-ppm will be recorded before PR review.
