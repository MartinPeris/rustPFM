# Independent Netpbm fixtures

The twelve PFM files were produced by the actual `pamtopfm` executable, not
rustPFM or justPFM. They were copied from
[justPFM's interoperability fixtures](https://github.com/MartinPeris/justPFM/tree/v1.2.1/tests/fixtures/netpbm).
The original 3 × 2 ASCII PGM/PPM images have maxval 16 and exactly representable
normalized float32 samples. Each has both byte orders and scales 0.5, 1 and 2.

Generated on Linux x86-64 with Ubuntu 24.04 `netpbm` and `libnetpbm11t64`, version
`2:11.05.02-1.1build1` (Netpbm 11.5.2). The images, fixtures and generator are
original justPFM test data under its [MIT license](LICENSE.justPFM). No Netpbm
binaries or source code are vendored.

Install Netpbm, then regenerate or verify from the repository root:

```sh
python3 tests/fixtures/netpbm/generate.py
python3 tests/fixtures/netpbm/generate.py --check
cargo test --test codec independent_netpbm_fixtures
cargo test --test netpbm -- --ignored
```

The generator requires the real external converter and byte-for-byte equality
with checked-in fixtures. The last command independently decodes rustPFM output
with `pfmtopam`; it fails if the converter is absent. Linux CI requires both.

## Scale convention

Given normalized input `n` and positive scale `s`, Netpbm's `pamtopfm` stores
`n * s`; its reader divides by `s`. rustPFM, like justPFM, stores caller samples
unchanged and by default **multiplies** on decoding. Thus these fixture samples
decode to `n * s * s` with `ScaleMode::Apply`, and `n * s` with `ScaleMode::Raw`.
Unit scale gives matching interpretation across both libraries. Live writer
tests supply `n * s` and verify that Netpbm returns `n`, with expected quantization
calculated independently from the PAM output maxval.

Format references: [PFM](https://netpbm.sourceforge.net/doc/pfm.html),
[pamtopfm](https://netpbm.sourceforge.net/doc/pamtopfm.html),
[pfmtopam](https://netpbm.sourceforge.net/doc/pfmtopam.html).
