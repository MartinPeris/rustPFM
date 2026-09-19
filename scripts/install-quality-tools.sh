#!/usr/bin/env bash
set -euo pipefail
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-llvm-cov --version 0.6.21 --locked
rustup toolchain install nightly-2026-09-19 --profile minimal --component miri,rust-src
