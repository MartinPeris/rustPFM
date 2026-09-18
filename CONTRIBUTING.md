# Contributing

Install stable Rust and the `rustfmt` and `clippy` components:

```sh
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
rustup default stable
git config --local core.hooksPath .githooks
```

Run `./scripts/check.sh` before opening a pull request. It runs formatting,
Clippy with warnings denied, unit/integration/doc tests, rustdoc with warnings
denied, and `cargo package` with compilation verification. Cargo.lock is tracked
so local and CI checks resolve the same dependencies. The core currently has
none. Package verification creates an archive locally; it does not publish it.

The pre-commit hook checks a temporary snapshot of the Git index, so partially
staged edits are checked as they would be committed. New files must be staged.
The snapshot and its build outputs are removed afterward. This shell-based hook
requires Bash, Git, tar, and Rust tooling and is validated on Linux. Hooks must
be enabled per clone and can be bypassed; CI supplies the independent check.
GitHub branch protection is not configured by this scaffold.

Use a feature branch and a focused PR. Add behavior tests with each implemented
capability, including failure paths and independent expected PFM bytes. The
current crate has no implementation and consequently no codec tests; a passing
scaffold build is not evidence of codec correctness or coverage. Add coverage
and fuzzing alongside implementation, as described in DESIGN.md.

Keep default runtime dependencies at zero. Optional adapters and development
helpers may add dependencies when justified. Do not add unsafe code, publishing
credentials, or automatic releases as part of routine implementation work.
`publish = false` remains in Cargo.toml until the first release is reviewed.
