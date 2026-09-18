#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --no-default-features
cargo test --locked --all-features
cargo llvm-cov --locked --all-features --lib --tests --fail-under-lines 95 --ignore-filename-regex '(^|/)(tests|benches)/'
RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo doc --locked --no-deps --all-features
cargo package --locked --allow-dirty
