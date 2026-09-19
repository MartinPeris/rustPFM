#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
# Keep the default published library and its build free of third-party crates.
if [ "$(cargo tree --locked --edges normal,build --prefix none | wc -l)" -ne 1 ]; then
    echo "Default runtime/build dependencies must remain empty" >&2
    exit 1
fi
cargo clippy --locked --all-targets --all-features -- -D warnings -D clippy::undocumented_unsafe_blocks
cargo test --locked --no-default-features
cargo test --locked --all-features
cargo llvm-cov --locked --all-features --lib --tests --fail-under-lines 95 --ignore-filename-regex '(^|/)(tests|benches)/'
RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo doc --locked --no-deps --all-features
cargo package --locked --allow-dirty

# Benchmark competitors live in an isolated package; verify correctness, never speed.
cargo fmt --manifest-path benchmarks/rust-codecs/Cargo.toml -- --check
cargo clippy --locked --manifest-path benchmarks/rust-codecs/Cargo.toml -- -D warnings -D clippy::undocumented_unsafe_blocks
benchmark_smoke=$(mktemp)
trap 'rm -f -- "$benchmark_smoke"' EXIT
python3 benchmarks/compare_rust.py --sizes 2 8 --repeats 1 --warmup 1 --output "$benchmark_smoke"
python3 benchmarks/compare_rust.py --sizes 2 8 --repeats 1 --warmup 1 --row-order bottom --output "$benchmark_smoke"

./scripts/check-safety.sh

# Paired timing gate against the reviewed, pinned fast revision.
python3 -m unittest discover -s benchmarks -p test_regression.py
python3 benchmarks/check_regression.py
