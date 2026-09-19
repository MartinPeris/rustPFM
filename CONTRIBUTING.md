# Contributing

Install stable Rust and the pinned quality tools (including LLVM coverage):

```sh
rustup toolchain install stable --profile minimal
rustup default stable
./scripts/install-quality-tools.sh
git config --local core.hooksPath .githooks
```

Run `./scripts/check.sh` before opening a pull request. It runs formatting,
a zero-default-dependency check, Clippy with warnings denied, unit/integration/doc tests with default dependencies
disabled and all features enabled, a **95% library line coverage gate**, rustdoc with warnings
denied, `cargo package` with compilation verification, and the Miri safety checks
in `scripts/check-safety.sh`, followed by a paired performance regression gate
against a pinned fast revision. A slowdown above both 20% and 0.2 ms in all
three paired trials fails locally and in CI; partial confirmation warns. See
[the policy and report guide](benchmarks/REGRESSION.md). Cargo.lock is tracked
so local and CI checks resolve the same dependencies. The default build has none. Package verification creates an archive locally; it does not publish it.

The safety script uses pinned `nightly-2026-09-19` with Miri and rust-src, installed
by `scripts/install-quality-tools.sh`. It checks allocation/byte views, direct
I/O, and ndarray views with strict provenance on native and interpreted big-endian Linux targets. This development toolchain does
not change the library's stable Rust 1.85 MSRV. Linux FFI hints are excluded
under Miri; native tests exercise the allocation path. See [SAFETY.md](SAFETY.md)
for that limit and the invariants every unsafe change must preserve.

The pre-commit and pre-merge-commit hooks check a temporary snapshot of the Git index, so partially
staged edits are checked as they would be committed. New files must be staged.
The snapshot and its build outputs are removed afterward. This shell-based hook
requires Bash, Git, tar, and Rust tooling and is validated on Linux. Hooks must
be enabled per clone and can be bypassed; CI supplies the independent check.
GitHub branch protection is not configured for this repository.

Use a feature branch and a focused PR. Add behavior tests with each implemented
capability, including failure paths and independent expected PFM bytes. The integration suite checks independently generated Netpbm fixtures, exact wire
bytes, float bit preservation, truncation, malformed headers, I/O errors, and
deterministic randomized images. These repeatable stress tests are not a claim
of exhaustive fuzzing.

CI tests Rust 1.85.0 (MSRV) and stable with no default features and all features.
The required Quality gate aggregates the complete harness, compatibility matrix,
and independent Netpbm fixture reproduction plus live decoding of rustPFM output.
To run the live check locally, install Netpbm and run
`cargo test --test netpbm -- --ignored`; unlike the regular suite this command
fails when the external converter is absent. Native Windows/macOS testing is
deferred; Linux coverage does not establish native behavior on other systems.

Coverage uses cargo-llvm-cov 0.6.21 and llvm-tools-preview. Only test and benchmark
source is excluded from the library measurement; every library module is counted.
For a browsable report, run `cargo llvm-cov --all-features --html`. The 95% gate
is a regression floor, not proof of correctness. Allocation failure and some
platform-specific I/O failure paths cannot be forced safely in routine tests.

See [RELEASING.md](RELEASING.md) for the first-release checklist.

Keep default runtime dependencies at zero. Optional adapters and development
helpers may add dependencies when justified. Unsafe code is denied outside the
private `src/buffer.rs` boundary. Changes there need explicit safety reasoning,
review, and appropriate Miri/native tests; Clippy requires documented unsafe
blocks. Do not expand that boundary, add publishing credentials, or introduce
automatic releases as part of routine implementation work.
`publish = false` remains in Cargo.toml until the first release is reviewed.
