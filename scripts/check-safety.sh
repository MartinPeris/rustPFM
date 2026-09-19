#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Pin the interpreter; the public library still supports stable Rust 1.85.
export MIRIFLAGS="${MIRIFLAGS:-} -Zmiri-strict-provenance"
cargo +nightly-2026-09-19 miri test --locked --all-features --lib buffer::
cargo +nightly-2026-09-19 miri test --locked --all-features --test direct_io --test arrays
# Exercise native/opposite byte-order paths on an interpreted big-endian target.
cargo +nightly-2026-09-19 miri test --locked --target s390x-unknown-linux-gnu --all-features --lib buffer::
cargo +nightly-2026-09-19 miri test --locked --target s390x-unknown-linux-gnu --all-features --test direct_io --test arrays
