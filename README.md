# rustPFM

A lightweight Rust library for Portable Float Map (PFM) images.

[![Quality](https://github.com/MartinPeris/rustPFM/actions/workflows/quality.yml/badge.svg?branch=main&event=push)](https://github.com/MartinPeris/rustPFM/actions/workflows/quality.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**Status: scaffold.** PFM encoding and decoding are not implemented yet.
The crate is not published on crates.io. The current build has **zero dependencies**.

## Direction

- Grayscale and RGB `f32` pixels, both PFM byte orders.
- Owned pixel buffers and borrowed inputs; standard-library file and memory I/O.
- Bounded parsing, checked dimensions, explicit scale behavior, and pixel limits.
- Optional `ndarray` integration, without imposing it on core users.
- Independent compatibility fixtures and reproducible benchmarks against
  [justPFM](https://github.com/MartinPeris/justPFM).

These are planned capabilities, not current API guarantees. See the
[design and roadmap](DESIGN.md).

## Develop

Install stable Rust with `rustfmt` and `clippy`, then:

```sh
git clone git@github.com:MartinPeris/rustPFM.git
cd rustPFM
git config --local core.hooksPath .githooks
./scripts/check.sh
```

The same checks run locally and in Linux CI: formatting, Clippy, tests, docs,
and package verification. There are no codec tests or coverage claims yet.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow.
