# px-imageproc

Pixel-space image processing for FITS data, built on top of [`px-fits`](../px-fits).

`px-fits` reads and writes FITS bytes and has no opinion on what the pixels mean
photographically. This crate owns that interpretation: debayering, display
normalization, autostretch, and — planned — calibration, stacking, and composition.

Scaffolded as part of ADR 006 Phase 9. See
[`docs/adr/006-native-fits-implementation.md`](../../docs/adr/006-native-fits-implementation.md)
for the phased plan and rationale for the crate split.

## Parity against Siril

ADR 007 builds native calibration, registration and stacking alongside the Siril-backed
pipelines, and holds them to measured tolerances rather than to "looks right". The comparison
runs against real frames, not synthetic ones — the corpus lives under `data/` and is not
committed.

```
cargo xtask siril-reference                     # drive a local Siril over data/corpus-mono
cargo test --release -p px-imageproc            # run the parity suites
```

Both are optional: the parity tests skip when the corpus or the reference file is absent, so a
checkout without them stays green. Reports are written to `data/output/`.

### Statistics (ADR 007 Phase 2)

Every estimator compared against Siril 1.4.4's `stat main` over 12 frames of `data/corpus-mono`
(ASI6200MM Pro, 9576×6388, 16-bit, Hα 300 s) — 3 each of bias, dark, flat, light.

| Statistic | Agreement | Tolerance |
|---|---|---|
| median, min, max, MAD | ≤ 0.002 ADU | 0.01 ADU |
| mean, sigma, sqrt(BWMV), bgnoise | ≤ 0.049 ADU | 0.06 ADU |

**Worst absolute delta across all 96 comparisons: 0.0494 ADU.** Siril prints `stat` at one
decimal place, so ±0.05 ADU is the resolution floor of this comparison — the two implementations
agree to the last digit Siril will show. The looser band on the accumulated statistics is that
rounding plus the f32 round-trip through the unit range; Siril accumulates over raw 16-bit
integers.

Analytic bounds, from `tests/stats.rs` (no Siril needed): background noise recovers an injected
sigma to within 5% and is unmoved by a smooth gradient or by 1% saturated pixels; robust
location/scale lands within 5% of true sigma with 0.5% of the frame saturated, where classical
sigma is off by 2.5×; every estimator is bit-identical across 1, 2, 3 and 8 cores.
