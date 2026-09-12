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

### Stacking (ADR 007 Phase 3)

Median masters compared against Siril 1.4.4 over 40 frames of `data/corpus-mono`
(9576×6388, 16-bit), stacked by the same commands `px --engine siril` issues today.

| Master | Max delta | RMS delta | Median | Background noise |
|---|---|---|---|---|
| bias | **0.0000 ADU** | 0.00002 ADU | exact | 1.2581 vs 1.2594 |
| dark | **0.0039 ADU** | 0.00002 ADU | exact | 1.4074 vs 1.4086 |

Roughly 31% of pixels differ, every one by the last bit of an `f32`: with an even frame count
the median is the mean of the two central values, which Siril forms in `f64` before narrowing.

**Speed:** 2.55 s versus Siril's 2.35 s for a 40 × 61 MP median stack (warm cache, M4 Max,
16 cores). Scaling across cores is 5.1× from 1 to 16; an 8× tighter memory budget (16 → 125
blocks) costs 22%.

Reduction cost per method, 20 × 1024² frames:

| Method | Time | Throughput |
|---|---|---|
| sum | 21.9 ms | 959 Melem/s |
| mean | 23.2 ms | 904 Melem/s |
| median | 44.3 ms | 473 Melem/s |
| mean + percentile | 48.1 ms | 436 Melem/s |
| mean + sigma | 50.0 ms | 420 Melem/s |
| mean + linear fit | 55.1 ms | 380 Melem/s |
| mean + Winsorized | 62.1 ms | 338 Melem/s |
| mean + MAD | 68.2 ms | 308 Melem/s |

Outputs land in `data/output/masters/` for side-by-side inspection against the Siril masters in
`data/reference/masters/`.

### Calibration (ADR 007 Phase 4)

Masters are composed the way the workflow intends: bias and dark each stacked from raw frames on
their own, the flat built from bias-calibrated flats, and lights calibrated with the **dark and
flat only** — the raw-stacked dark already carries the bias pedestal.

| Corpus | max abs delta | RMS delta | median (ours / Siril) | background noise |
|---|---|---|---|---|
| mono (ASI6200MM, Hα) | 0.0195 ADU = **0.0011 σ_bg** | 0.00004 ADU | +60.8723 / +60.8725 | 17.7558 / 17.7549 |
| OSC (ASI6200MC, Dual) | 1.6054 ADU = **0.0079 σ_bg** | 0.01571 ADU | +633.1835 / +633.1990 | 203.5618 / 203.5668 |

Bound is 0.05 σ_bg max and 0.01 σ_bg RMS; both are met by 6–45×.

#### What the clamp actually destroys

Calibration keeps values between −1.0 and 0.0 negative — a calibrated sky straddles zero, and
clipping its low half would bias every downstream statistic upward. Only values past ±1.0 are
destroyed, and `OpReport` counts those:

| Corpus | negative | deepest negative | floored to zero | saturated |
|---|---|---|---|---|
| mono | 0.068% | −50,961 ADU (−0.778 unit) | **0** | 193 |
| OSC | ~0.000% | −59,999 ADU (−0.916 unit) | **7** | 249 |

Out of 61,171,488 pixels, with Siril matching to three decimals on the negative fraction. The
deepest negatives are hot pixels in the master dark, which land just inside the floor and which
cosmetic correction patches anyway.

Outputs land in `data/output/calibrated/` for comparison against `data/reference/calibrated/`
and `data/reference-osc/calibrated/`.

### Demosaicing (ADR 007 Phase 6)

Two reconstructions ship. `--debayer` / `PX_DEBAYER` selects; the default is
`gradient-corrected`.

| | vs RCD background noise (R/G/B) | RMS error vs known truth |
|---|---|---|
| `gradient-corrected` (Malvar-He-Cutler) | 1.23× / 1.45× / 1.24× | **0.002192** |
| `bilinear` | 0.93× / 0.92× / 0.75× | 0.003385 |

Read on noise alone, bilinear wins; it does not. A blurrier reconstruction *measures* as
quieter, because smoothing correlates neighbouring pixels and a background-noise estimator reads
that as less noise — ranking demosaics that way rewards the blurriest one. Against a synthetic
star field with known truth the ranking inverts, and gradient-corrected is 35% more accurate.

Channel levels match RCD to better than 0.01% either way, so there is no colour cast. A 61 MP
frame demosaics in 0.07 s.

Neither is RCD, which Siril uses: RCD has no published specification independent of its GPL
implementation, so it cannot be written clean-room. ADR 007 Phase 6b tables AHD as the way to
close the remaining gap.
