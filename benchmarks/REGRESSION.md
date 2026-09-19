# Performance regression gate

`./scripts/check.sh` ends with `python3 benchmarks/check_regression.py`.
The pre-commit hook runs it against the **staged snapshot**, and CI runs the same
command as part of the existing required Quality gate. No Python packages or
Rust runtime dependencies are added. Linux, Python 3, Git history and the normal
Rust toolchain are required.

## Policy

[regression-policy.json](regression-policy.json) pins the fast implementation at
`fca8bd136aa2b99c8869c09c1a79a8c1478ca659`. Both the baseline and candidate are
built in temporary directories using the current compiler and default features.
Each uses the same validating benchmark driver taken from that pinned revision.
The candidate source hashes identify precisely what ran, including staged code;
the repository HEAD in a hook report is context, not the staged tree's identity.

The gate covers 1024²/2048² grayscale and RGB images: top-first reads at scale 1
and 2, and atomic replacement writes at scale 1. For each case, three paired
trials alternate execution order. Each worker validates every output outside
the timer, warms up twice, and returns seven measured samples.

A case fails only when its candidate median exceeds the baseline by **both 20%
and 0.2 ms in all three paired trials**. Equality at a limit passes. One or two
exceeding trials produce a warning, not a failure. Malformed/missing timings,
missing baseline history, compilation failures and worker failures fail closed.
These are practical confirmation rules, not statistical confidence intervals.
The absolute floor deliberately tolerates small absolute changes even when
their relative percentage is large.

Baseline and candidate run serially on the same host with warm files. Builds
finish before timing begins. Run on an otherwise idle machine: repeated host
contention, thermal drift and noisy shared CI runners can still cause false
alerts. No cross-machine absolute timing threshold is used. The test does not
measure cold storage, durable writes (`fsync`), every byte order/layout, or
performance on other operating systems.

## Reports and failures

Run the gate alone:

```sh
python3 benchmarks/check_regression.py
```

JSON samples, per-trial ratios, source/binary hashes, policy and host details go
to `target/performance/report.json`, with a Markdown table beside it. Hook
reports are retained in the original checkout's `target/performance/`, outside
the disposable staged snapshot. Each run replaces the previous report.
Use `--output /path/report.json` to retain additional runs.

CI adds the table to its job summary, emits error annotations for confirmed
regressions (warnings for unconfirmed changes), and uploads available reports
as the `performance-report` artifact for 30 days, including failed runs.
A failure before the benchmark starts may have no performance artifact.
Notifications follow the user's normal GitHub Actions notification settings;
no email, Slack or other external notification integration is configured.

On an alert, inspect the affected cases and samples, then rerun on an idle host.
Investigate sustained regressions before merging. Do not rerun selectively
until a noisy failure disappears or loosen the policy just to obtain a pass.
The pre-commit hook can be bypassed like any Git hook; CI is the independent
check. `--repository /path/to/clone` supplies baseline history when testing an
exported source snapshot. Shallow clones must fetch the baseline history first
(for example, `git fetch --unshallow origin`); CI checks out full history.

## Updating the baseline

The baseline is fixed, so small slowdowns cannot silently accumulate by always
comparing with the previous commit. After an intentional performance/API change,
review a separate policy change that pins the full SHA of an accepted revision.
Include same-host comparisons and explain any accepted regression or changed
workload. The driver is pinned with the baseline; an incompatible API change
requires an explicit baseline/driver decision, not a skipped measurement.
Thresholds, workload sizes and repetition counts are likewise reviewed source
changes. Do not replace this policy with the absolute timing values in the
historical benchmark reports.

Run the deterministic detector tests with:

```sh
python3 -m unittest discover -s benchmarks -p test_regression.py
```

They cover sustained slowdowns, noisy trials, outlier samples, threshold/floor
behavior, invalid input and nonzero failure reporting. Timing itself is checked
by the real paired gate, not by assertions about how long a unit test should run.
