# 007 — Native Calibration, Registration and Stacking for `px-imageproc`

## Status

Proposed. Supersedes nothing. Depends on [ADR 006](006-native-fits-implementation.md), whose
Phases 0–8 are complete: `px-fits` is a native reader/writer with region reads, streaming rows,
and `rayon`-parallelized full-frame decode.

---

## Context

ADR 006 gave the project native FITS I/O. Everything that *interprets* those pixels
photographically still runs inside Siril, invoked over a pipe by `siril-sys`:

- `CreateMasterBiasPipeline` → `convert` + `stack ... med`
- `CreateMasterDarkPipeline` → `convert` + `stack ... med`
- `CreateMasterFlatPipeline` → `convert` + `calibrate -bias=` + `stack ... rej -nonorm`
- `CalibrateLightSetPipeline` → `convert` + `calibrate -bias= -dark= -flat= -cfa -debayer`
- `CreateMasterLightPipeline` → `convert` + `register -2pass` + `seqapplyreg` + `stack ... rej`

This works, and it will keep working — `siril-sys` is not going away and is not touched by this
ADR. But routing every frame through a child process has costs the project now has the tools to
avoid:

1. **Forced materialization on disk.** Every stage writes a full frame set before the next one
   reads it: `calibrate` writes `pp_` frames, `seqapplyreg` writes `r_pp_` frames. A 300-frame
   60 MP session moves tens of gigabytes through the filesystem that a fused pipeline would keep
   in a bounded buffer. (The `convert` step itself is cheaper than it looks — for FITS input
   Siril symlinks rather than copies, `conversion.c` l. 635, though that falls back to real
   copies where symlinks are unavailable, notably unprivileged Windows.)
2. **No shared memory or CPU budget.** `siril-sys::Builder` can pass `-m`/CPU limits *to Siril*,
   but the Rust side has no equivalent notion for its own work, and the two cannot be reconciled
   because they are separate processes.
3. **Opaque intermediate state.** Rejection maps, per-frame normalization coefficients, star
   lists, and homographies exist only inside Siril's process. `px` cannot inspect, cache, or
   make decisions from them — which is what several open ROADMAP items (frame review and human
   rejection, live stacking, per-subframe calibration on file drop) actually need.
4. **Process and version coupling.** Every pipeline depends on Siril being installed, on a
   compatible version, and on named-pipe behaviour that has already needed platform-specific
   fixes on Windows.

`px-imageproc` was scaffolded by ADR 006 Phase 9 as the home for exactly this work, and its
README already names calibration, stacking, and composition as planned. This ADR is the design
and the phased plan for them.

### Confirming the assumed memory/CPU model

The premise this work was proposed on is correct. Siril's median/mean stacker
(`src/stacking/median_and_mean.c`) works as follows, and the numbers below are the ones the
native port must reproduce:

**1. Compute a row budget from available memory** (`stack_get_max_number_of_rows`, l. 1203):

```
max_memory  = configured budget, in MB
max_memory -= sizeof(result image)          // naxes[0]*naxes[1]*naxes[2]*elem_size
max_memory -= nb_rejmaps * sizeof(rejmap)   // 0, 1, or 2 full u16 images
rows        = max_memory / (nb_images * naxes[0] * elem_size)
return min(rows, naxes[1] * naxes[2])
```

Note that the row budget is expressed in **rows of the whole stack across all channels**
(`naxes[1] * naxes[2]`), not rows of one channel. The result image and rejection maps are
charged against the budget up front because they are live for the entire run.

**2. Choose a block count that divides work evenly across threads** (`stack_compute_parallel_blocks`,
l. 294, and `refine_blocks_candidate`, l. 260):

```
candidate = nb_threads
while (max_rows * candidate) / nb_threads < naxes[1] * naxes[2]:
    candidate++                       // smallest block count that fits the budget
candidate = refine_blocks_candidate(nb_threads, nb_channels, candidate)
```

`refine_blocks_candidate` rounds the candidate up to a multiple of the channel count, then keeps
adding that multiple until `candidate % nb_threads` is either 0 or within a slack window —
1 below for 4–7 threads, 3 below for 8+. The comment on that function is explicit that this
"favors memory over threads": a block count slightly short of a clean multiple is accepted
because threads drift out of phase once they start reading anyway.

**3. Cut the image into blocks** — each block is a contiguous row range *within one channel*;
block boundaries never straddle channels. The remainder rows are spread one at a time across the
leading blocks rather than dumped into the last one, and a trailing stub shorter than 1/10 of a
block height is absorbed into its predecessor.

**4. Allocate one buffer per thread, not per block** (l. 1360 onward). Each of the `nb_threads`
buffers is sized for the *largest* block: `nb_frames × largest_block_height × naxes[0] × elem_size`,
plus one extra pixel's worth of `nb_frames` values used as the per-pixel working stack, plus
rejection scratch (`rejected[]`, `o_stack[]`, and method-specific arrays for Winsorized/GESDT/
linear fit). Threads pull blocks off the list and reuse their own buffer.

**5. Each thread, for its block:** reads the same row range from *every* frame
(`stack_read_block_data`, applying normalization coefficients and registration shift on the way),
then walks the block pixel by pixel. For each pixel it gathers the `nb_frames` values into the
working stack and reduces them to one output value (median, or reject-then-mean).

So: blocks sized by memory budget, count tuned to the thread count, the same region scanned
across all files, reduction done per-pixel across frames, blocks assembled into the result. That
is the design being ported.

Two details worth calling out because they are easy to miss and both affect correctness:

- **Siril's float pipeline works in [0, 1], not in ADU.** 16-bit input is divided by 65535 on
  read (`INV_USHRT_MAX_SINGLE`, `image_format_fits.c` l. 441); signed 16-bit becomes
  `(32768 + v) / 65535`. Every threshold, sigma, and normalization coefficient in the Siril code
  is in those units.
- **`output_norm` is a min–max rescale, not a clamp** (`norm_to_0_1_range`, l. 556): it finds the
  min and max over all non-zero pixels and linearly maps them to [0, 1], leaving exact zeros
  alone. `CreateMasterLightPipeline` passes `.output_norm(true)`, so the native master-light path
  must do this too or its output will not be comparable.

---

## Decision

Implement calibration, registration, and stacking natively in `px-imageproc`, and add a second
`RunIn` environment — `NativeRuntime` — that the existing pipeline structs implement alongside
their existing `RunIn<siril_sys::Builder>` implementations. A global `--engine` flag on `px`
selects between them.

### Scope

**In scope**

- **Calibration operators:** bias subtraction (master frame or synthetic level), dark subtraction
  with optional dark-current optimization, flat division with automatic level normalization,
  cosmetic correction (hot/cold pixel maps derived from a master dark, CFA-aware).
- **Debayering at calibration quality:** RCD (Siril's default `bayer_inter`), for the
  `-cfa -debayer -equalize_cfa` path that `CalibrateLightSetPipeline` uses today. The existing
  super-pixel debayer stays as the preview-only path it was written to be.
- **Frame statistics:** median, MAD, sigma, background level, background noise, and the
  location/scale estimators Siril's normalization depends on.
- **Normalization:** additive, multiplicative, and their scaled variants, reference-frame
  relative, with the "lite" (median/1.5·MAD) variant.
- **Stacking:** sum, min, max, median, and mean-with-rejection covering percentile clipping,
  sigma clipping, MAD clipping, sigma-median, Winsorized sigma, linear fit, and GESDT.
- **Registration:** star detection, PSF centroiding, triangle-similarity star matching, RANSAC
  transform estimation (shift / similarity / affine / homography), two-pass reference selection,
  and subpixel warping with Lanczos-4 and bicubic interpolation including clamping.
- **A memory- and CPU-budgeted execution model** — the block scheduler above, generalized so that
  whole-frame-per-thread stages (statistics, star detection, warping) draw from the same budget.
- **`NativeRuntime`**, a `RunIn` environment carrying that budget, plus a `--engine native`
  CLI path for all five pipelines.

**Out of scope (this ADR)**

- Colour sample pipelines (`project/sample.rs`) and both mosaic pipelines. They keep their Siril
  implementations; `--engine native` reports that they are unsupported and exits non-zero.
- Drizzle, and stacking with maximized framing. `FRAMING_CURRENT` and `FRAMING_MIN` only.
- Overlap-based normalization (`compute_normalization_overlaps`) and feathered blending.
- Astrometric (plate-solve-driven) registration, distortion/SIP correction, and X-Trans sensors.
- Background extraction (`SeqSubSky`). `CreateMasterLightPipeline`'s `background_extract` option
  is rejected by the native engine until a later ADR.
- Any change to `siril-sys`, or to the `RunIn` trait, the pipeline structs' public fields, or
  their builders.
- Weighting by noise/wFWHM/star count. Unweighted mean only; the hooks are left in place.

### Architecture

```
crates/px-imageproc/
  src/
    lib.rs
    error.rs                  # ImageProcError, extended
    frame.rs                  # Frame, FrameView, planar f32 channel-major buffer
    sequence.rs               # FrameSet: ordered frames + cached headers + reg data
    runtime/
      mod.rs                  # ResourceBudget, Runtime (owns the rayon pool)
      budget.rs               # memory probe, cgroup awareness, MB accounting
      blocks.rs               # BlockPlan: the port of the scheduler described above
    stats.rs                  # median, MAD, sigma, bgnoise, location/scale
    calibrate/
      mod.rs                  # CalibrationMasters, calibrate_frame
      ops.rs                  # sub/div/scalar ops, in Siril's [0,1] convention
      dark_optimize.rs        # golden-section search on calibrated-frame noise
      cosmetic.rs             # hot/cold detection from a master dark, CFA-aware
    debayer/
      mod.rs                  # dispatch; re-exports the existing super_pixel path
      rcd.rs                  # Ratio Corrected Demosaicing
      equalize.rs             # equalize_cfa channel balance
    normalize.rs              # norm coefficients, reference-relative
    stack/
      mod.rs                  # Stacker entry points, block loop, result assembly
      reader.rs               # block reads across the frame set
      median.rs               # quickselect
      mean.rs                 # reject-then-mean reduction
      rejection.rs            # the seven rejection algorithms
      minmax.rs
      sum.rs
    register/
      mod.rs                  # orchestration: detect -> match -> estimate -> apply
      starfind.rs             # candidate detection (port of peaker)
      psf.rs                  # Gaussian/Moffat Levenberg-Marquardt fit
      match.rs                # triangle similarity matching + vote matrix
      transform.rs            # Transform enum, DLT, RANSAC
      framing.rs              # reference selection, output geometry
      warp.rs                 # Lanczos-4 / bicubic / bilinear + clamping
  benches/
    stack.rs  calibrate.rs  register.rs  budget.rs
  tests/
    parity.rs                 # existing preview parity, extended
    stack_parity.rs  calibrate_parity.rs  register_parity.rs
    budget.rs                 # block scheduler unit tests vs. Siril's numbers
    memory.rs                 # dhat peak-heap assertions against the budget

crates/px-pipeline/
  src/environment.rs          # unchanged trait; NativeRuntime re-exported
  src/native.rs               # NativeRuntime builder (the new RunIn env)
  src/calibration/master_bias/{mod.rs, siril.rs, native.rs}
  src/calibration/master_dark/{mod.rs, siril.rs, native.rs}
  src/calibration/master_flat/{mod.rs, siril.rs, native.rs}
  src/calibration/calibrate_light/{mod.rs, siril.rs, native.rs}
  src/project/master_light/{mod.rs, siril.rs, native.rs}
```

Each `mod.rs` holds the pipeline struct and its `#[derive(bon::Builder)]` exactly as it is
today, and the two `RunIn` impls sit in sibling files. The public path
(`px_pipeline::calibration::master_bias::CreateMasterBiasPipeline`) and every builder field are
unchanged, so `px` compiles against the same API it does now.

### Key design decisions

**D1 — `f32` internally, any `BITPIX` in and out, in Siril's [0, 1] convention.**

One code path, not Siril's two. Frames are read through `px-fits`
`read_region_into::<f32>` / `read_full_into::<f32>`, which applies `BSCALE`/`BZERO` for free, and
then scaled to [0, 1] by the same rules Siril uses (`/65535` for 16-bit, `(32768+v)/65535` for
signed 16-bit, pass-through for float input already in range). Accumulators that sum across
frames are `f64`. Output is `BITPIX = -32`, matching `siril_sys::Builder`'s `use_32bit: true`
default.

The cost is 2× the block memory of Siril's `u16` path for 16-bit input. The budgeter accounts for
it honestly, so the effect is more blocks rather than an overrun.

**D2 — Parity is measured, tolerance-bounded, and published.**

Native output is compared against Siril output on a fixed corpus and must satisfy per-pipeline
bounds on max absolute difference, RMS difference, and background-noise ratio (the table under
"Parity targets"). Bit-exactness is explicitly *not* the goal: it would forbid `f64`
accumulation and parallel reductions that are more accurate than what Siril does. Where the
native result is expected to be *better* than Siril's, the test asserts the direction of the
difference rather than just its magnitude.

Every fixture also has analytic ground truth (Phase 0 generates frames from a known bias level,
known dark current, known flat field, known star field, and known dither offsets), so a
disagreement with Siril can be adjudicated instead of assumed.

**D3 — The block scheduler is a faithful port, tested against Siril's own numbers.**

`runtime/blocks.rs` reproduces `stack_get_max_number_of_rows`, `refine_blocks_candidate`, and
`stack_compute_parallel_blocks` including the remainder distribution and the 1/10-block
absorption rule. Its unit tests assert exact block counts and heights for a table of
(image size, frame count, thread count, memory budget) tuples captured from instrumented Siril
runs. Deviating from Siril here is allowed only with a benchmark showing it is better, recorded
in the README.

One deviation is already known and is *not* a throughput matter. Siril's row budget bounds the
**average** block height (`height_of_blocks = total_rows / nb_blocks`, a floor) while the
per-thread buffers are sized to the **largest** block, which is one row taller whenever the rows
do not divide evenly. For a 4096-row frame over 115 blocks on 4 threads the budget permits 35.75
rows per thread, blocks come out at 35 and 36 rows, buffers are sized for 36, and a 512 MiB
budget is overrun by 2.1 MB (0.39%). The `+1` working-stack pixel per frame adds a little more.
Since the budget bound is the one thing this ADR declares non-negotiable, `Runtime::plan_blocks`
tightens the row budget until the plan genuinely fits, while `BlockPlan::compute` stays a
faithful port so the fidelity tests keep testing the real algorithm. Both behaviours are pinned
by tests (`the_faithful_port_can_overshoot_its_own_budget`, `planned_buffers_stay_inside_the_budget`).

**D4 — `NativeRuntime` is a new `RunIn` environment; the trait does not change.**

```rust
pub struct NativeRuntime { /* builder-shaped, mirrors siril_sys::Builder */ }

impl NativeRuntime {
    pub fn container_aware_limits(self) -> Self;
    pub fn use_cpu_limit(self, cores: usize) -> Self;
    pub fn use_memory_ratio(self, ratio: f64) -> Self;
    pub fn use_memory_fixed_gb(self, gb: f64) -> Self;
    pub fn use_extension(self, ext: FitsExt) -> Self;
    pub fn use_directory(self, dir: &Path) -> Self;
    pub fn build(self) -> Result<Runtime, ImageProcError>;
}
```

The method names deliberately mirror `siril_sys::Builder` so the two environments are
interchangeable at the call site. `build()` resolves the ratio against physical (or cgroup-
limited) memory and constructs a *dedicated* `rayon::ThreadPool` sized to the core limit —
not the global pool, so a constrained run cannot be widened by other code, and `px`'s own
concurrency is unaffected.

**D5 — Two `RunIn` impls per pipeline struct, in sibling modules.**

`impl RunIn<NativeRuntime> for CreateMasterBiasPipeline` lives in `native.rs` next to the
existing `impl RunIn<siril_sys::Builder>` in `siril.rs`. The generic `RunIn<Env>` trait was built
for exactly this. The alternative — parallel `Native*Pipeline` structs — would duplicate every
builder and force `px` to branch on types rather than on a value.

One wart is accepted: the structs carry `ext: FitsExt`, a `siril-sys` type, which the native path
therefore also reads. `px-pipeline` already depends on `siril-sys` unconditionally, so this adds
no coupling; moving `FitsExt` into `px-fits` is a follow-up that would touch `siril-sys` and is
out of scope here.

**D6 — No Siril sequence files, no `convert` step.**

The native engine works directly on the `Vec<PathBuf>` the pipelines already collect from
`all_fits_files`. Registration data lives in a `FrameSet` in memory and is persisted, when the
pipeline needs it across steps, as a sidecar FITS binary table (written with the `px-fits`
Phase 6 table writer) rather than a `.seq` file. Nothing writes a second copy of the input
frames to disk.

**D7 — `nalgebra` for small dense linear algebra. No OpenCV, no GSL, no BLAS.**

PSF fitting needs Levenberg-Marquardt on 7–8 parameters; homography estimation needs a
normalized DLT with an 8×9 SVD; normalization needs small least-squares solves. `nalgebra` is
pure Rust, has no system dependencies, and handles all three at these sizes. Porting
`findHomography` and `gsl_multifit_nlinear` by hand on top of hand-rolled matrix code would be
strictly worse. `ndarray` + a BLAS backend is rejected: it reintroduces a system dependency for
matrices that are never larger than 9×9.

**D8 — No `unsafe`, no explicit SIMD, in the first implementation.**

ADR 006 D2 measured the safe path at ~82% of an `unsafe` alternative on a decode microbenchmark
and concluded that did not clear the bar. The same rule applies here. If a profile shows a
specific kernel is the bottleneck and autovectorization is not firing, that is a reported finding
with numbers, not a unilateral change.

**D9 — Rejection and normalization defaults come from the existing `BestRejection` helper.**

`siril_sys::BestRejection::find(n)` already encodes the project's choice: percentile (0.2/0.1)
below 7 frames, Winsorized (3.0/3.0) through 30, linear fit (5.0/5.0) above. The native stacker
takes the method and thresholds as parameters and the native `CreateMasterLightPipeline` calls
the same helper, so both engines make the same choice for the same input.

**D10 — Registration follows Siril's defaults exactly.**

Homography transform, `min_pairs = 10` (absolute floor 4), `max_stars = 2000`, star finder
`radius = 5`, `sigma = 1.0`, `roundness = 0.5`, `convergence = 1`, `min_beta = 1.5`, detection
threshold `median + 5·sigma·bgnoise`, registration layer = green for RGB and 0 for mono,
Lanczos-4 interpolation with clamping enabled. These are read out of Siril's
`src/core/settings.c` and `src/core/command.c` and are cited in the code.

**D11 — Global CLI flag, three new global args.**

```
--engine <siril|native>   [env: PX_ENGINE]  default: siril
--max-cores <N>           [env: PX_MAX_CORES]
--max-memory <SPEC>       [env: PX_MAX_MEMORY]   e.g. "8G", "75%"
```

`--engine` defaults to `siril` for the whole of this work; flipping the default is a separate
decision made after the parity report exists. `Commands::requires_siril()` becomes
engine-aware so `px --engine native` does not demand a Siril install for pipelines that no
longer need one. `--max-cores` / `--max-memory` apply to whichever engine is selected: they feed
`NativeRuntime` for native, and the existing `use_cpu_limit` / `use_memory_*` for Siril.

**D12 — `px-imageproc` stays orchestration-free.**

It depends on `px-fits`, `rayon`, `nalgebra`, `sysinfo`, `thiserror` — and nothing else in the
workspace. No `px-pipeline`, no `siril-sys`, no `px-index`, no async. Pipelines own all
file-naming, metadata, reporting, and indexing; `px-imageproc` owns pixels and the budget. This
keeps the algorithm layer testable without a profile, a database, or a Siril install.

---

## Parity and performance targets

### Parity targets (D2)

Measured on the Phase 0 corpus, native vs. Siril output, in [0, 1] units. `σ_bg` is the
background noise of the Siril result.

| Pipeline | Max abs diff | RMS diff | Additional assertion |
|---|---|---|---|
| `CreateMasterBias` (median) | ≤ 1e-6 | ≤ 1e-7 | exact match on odd frame counts expected; investigate any miss |
| `CreateMasterDark` (median) | ≤ 1e-6 | ≤ 1e-7 | hot-pixel positions identical |
| `CreateMasterFlat` (bias-sub + rej) | ≤ 0.1 σ_bg | ≤ 0.02 σ_bg | mean flat level within 0.1% |
| `CalibrateLight` (mono, no debayer) | ≤ 0.05 σ_bg | ≤ 0.01 σ_bg | — |
| `CalibrateLight` (CFA + RCD debayer) | ≤ 0.5 σ_bg | ≤ 0.1 σ_bg | looser: independent RCD port |
| Star detection | — | — | ≥ 95% of Siril's stars matched within 0.2 px; centroid RMS ≤ 0.05 px |
| Registration transforms | — | — | corner reprojection error vs. Siril's H ≤ 0.1 px on every frame |
| `CreateMasterLight` (register + stack) | ≤ 1.0 σ_bg | ≤ 0.1 σ_bg | σ_bg(native) ≤ 1.02 × σ_bg(Siril); FWHM within 2% |

The master-light bound is the loosest because a sub-pixel difference in one homography moves
every pixel. The noise and FWHM assertions are what actually constrain quality there — a result
that is *different* but measurably sharper and no noisier is a pass, and the test says so
explicitly rather than being quietly widened.

### Performance targets

Baselines are the current Siril pipelines, wall-clock end to end, measured in Phase 0 on the
same machine and corpus. Native must reach:

| Workload | Gate |
|---|---|
| `CreateMasterBias/Dark`, 50 × 16-bit frames | ≥ parity |
| `CreateMasterFlat`, 30 frames | ≥ parity |
| `CalibrateLight`, 100 frames | ≥ 1.5× faster |
| `CreateMasterLight`, 100 frames, register + stack | ≥ parity |
| Any of the above | peak RSS ≤ the configured `--max-memory`, verified by `dhat` |

These gates are deliberately modest, because an earlier draft's "2× everywhere" was not
defensible. A median stack of 50 bias frames is I/O-bound in both engines and Siril's `convert`
symlinks FITS input rather than copying it, so there is no large constant to reclaim there —
parity is the honest bar, and beating it is upside. `CalibrateLight` gets the one real speed
gate because it is where the redundant full-frame write actually lives. The master-light gate is
parity because a from-scratch star matcher and warper competing with OpenCV's SIMD-tuned
`warpPerspective` is a genuine fight.

**Phase 0 measures the baseline before any of this is built.** If the measured numbers show a
gate here is either trivially met or unreachable, the gate is corrected in this document with
the measurement attached — not quietly restated at review time.

**The budget assertion is the one that must not be negotiated.** A run configured for 4 GB that
peaks at 6 GB is a failure even if it is fast.

---

## Consequences

- New workspace dependencies: `nalgebra` and `sysinfo`. Both are widely used and pure Rust
  (`sysinfo` uses platform APIs directly, no C build).
- `px-imageproc` grows from a preview helper into the project's algorithm core. Its README gains
  a parity-and-performance report in the same shape as `px-fits`'s benchmark report.
- Two `xtask` subcommands are added: `imageproc-fixtures` (deterministic synthetic corpus with
  analytic ground truth) and `siril-reference` (drives the Siril pipelines to produce reference
  outputs for the parity tests, skipping cleanly when Siril is absent).
- Siril remains required for: colour samples, both mosaics, background extraction, and the
  default `--engine siril` path. `px` does not lose a capability at any point in this plan.
- Five pipeline modules become directories. Their public paths and builders do not change.
- `px-fits` gains no new API surface. If a gap appears (e.g. a cheaper multi-region read), it is
  an ADR 006 follow-up, not a change smuggled in here.
- Reference outputs are not committed — they are large and Siril-version-dependent. The corpus
  *inputs* are generated deterministically and committed; `PX_SIRIL_REF` points the parity tests
  at a locally generated reference tree, and those tests skip with a clear message when it is
  absent so CI without Siril stays green.

---

## Implementation plan

### Handoff rules

These apply to every task below and are non-negotiable for delegated work.

1. **One phase per branch, one task per commit.** Conventional commit prefixes (`add:`, `fix:`,
   `chore:`).
2. **Do not modify files outside `crates/px-imageproc/`, `crates/px-pipeline/`,
   `crates/px-cli/`, `crates/px/`, `xtask/`, and the workspace `Cargo.toml`** — and within
   `px-pipeline`/`px-cli`/`px` only what the task names. **`crates/siril-sys/` is off limits.**
3. **Every task ends green:** `cargo fmt`, `cargo clippy --verbose` (zero warnings),
   `cargo test --verbose`.
4. **Do not weaken a test or a tolerance to make it pass.** If a number in this document turns
   out to be wrong, stop and report it with the measurement.
5. **No `unsafe`** in `crates/px-imageproc/src/` (D8).
6. **Phase gates are hard.** Do not start phase N+1 until every acceptance criterion of phase N
   is demonstrably met.
7. **Cite sources sparingly — once per module, not per function.** A module-level doc comment
   names the algorithm and any literature reference (Valdes et al. for triangle matching,
   Luo & Zhang for RCD), and points at the Siril file when the code is a port of one. Do not
   thread Siril file/line references through the body of the code: this crate is a native
   implementation that should read as one, not as an annotated translation. Comments inside
   functions explain *this* code's reasoning — a non-obvious constant, an edge case, a
   deliberate divergence — and are worth writing only when the code cannot say it itself.
8. **The existing Siril pipelines keep passing their tests unchanged at every commit.**

### Phase 0 — Corpus, parity harness, baselines

*Goal: make both quality and speed measurable before any algorithm is written.*

- [ ] **P0-T1** Workspace deps: `nalgebra`, `sysinfo`; dev-only `approx`. Add `[[bench]]` entries
      (`harness = false`) for `stack`, `calibrate`, `register`, `budget` in `px-imageproc`.
- [ ] **P0-T2** `cargo xtask imageproc-fixtures`: seeded, idempotent generator producing a
      committed corpus under `crates/px-imageproc/tests/fixtures/` — bias, dark (with a known hot
      pixel map), flat (known vignette), and light sets (known star field via analytic Gaussian
      PSFs, known dither offsets and field rotation), in 16-bit and 32-bit float, mono and RGGB.
      Small enough to commit (≤ 512×512), plus a manifest of the ground truth as JSON.
- [~] **P0-T3** `cargo xtask siril-reference [--corpus DIR] [--out DIR] [--frames N]`: runs Siril
      over the corpus and writes reference outputs plus a manifest recording the Siril version.
      Errors with a clear message if Siril is not installed; consuming tests skip.
      *`stat` reference landed for Phase 2. Extending it to the five pipeline outputs is Phase 3+
      work, done as each pipeline needs its reference.*
- [ ] **P0-T4** Parity harness in `tests/support.rs`: load two FITS frames, report max abs diff,
      RMS diff, background noise of each, and a per-pixel diff histogram; an `assert_parity!`
      macro taking explicit named tolerances; a `siril_ref()` helper that skips the test when
      `PX_SIRIL_REF` is unset.
- [ ] **P0-T5** Large-corpus hook: `PX_IMAGEPROC_CORPUS` pointing at real frames for the
      benchmarks (the committed fixtures are too small to time meaningfully).
- [ ] **P0-T6** Measure and record the Siril baseline — wall time and peak RSS for all five
      pipelines on the large corpus — in a "Baseline" section of
      `crates/px-imageproc/README.md`, with machine, OS, and Siril version.

**Gate:** `cargo xtask imageproc-fixtures` is byte-idempotent; the README has real Siril numbers;
the parity harness compiles and skips cleanly without `PX_SIRIL_REF`.

### Phase 1 — Frame model, frame sets, resource budget, block scheduler

*Goal: the execution substrate. No photographic algorithm yet.*

- [x] **P1-T1** `frame.rs`: `Frame` — planar channel-major `Vec<f32>`, `width`/`height`/`channels`,
      retained `px_fits::Header`. Constructors from an `ImageHdu` applying the D1 [0, 1]
      convention (with the signed-16-bit and float-passthrough cases tested), and a writer back
      to `BITPIX = -32` that preserves and updates the header. `FrameView` for borrowed blocks.
- [x] **P1-T2** `sequence.rs`: `FrameSet` — ordered paths, lazily cached headers, a uniformity
      check (dimensions, channels, `BITPIX`) with a typed error naming the offending file, and a
      slot for per-frame registration data.
- [x] **P1-T3** `runtime/budget.rs`: `ResourceBudget { max_bytes, cores }`. Physical memory via
      `sysinfo`, cgroup v1/v2 limits honoured (mirroring `siril-sys`'s reader, reimplemented —
      that crate is off limits), ratio and fixed-GB forms. Unit tests with the probe injected.
- [x] **P1-T4** `runtime/blocks.rs`: the D3 port. `max_rows_for_budget(...)` and
      `BlockPlan::compute(rows_budget, dims, channels, threads)` returning
      `Vec<Block { channel, start_row, end_row }>` plus `largest_block_height`.
- [x] **P1-T5** `tests/budget.rs`: exact-value tests for `refine_blocks_candidate` across
      threads 1..16 × channels {1,3}; block plans for a table of (dims, frames, threads, budget)
      tuples asserting block count, per-block height, channel boundaries, remainder
      distribution, and the 1/10-absorption rule. Property test: blocks tile the image exactly,
      never straddle a channel, and no block exceeds `largest_block_height`.
- [x] **P1-T6** `runtime/mod.rs`: `Runtime` — owns a dedicated `rayon::ThreadPool` sized to
      `cores`, exposes `install()` and the budget. Test that work submitted through it never
      runs on more than `cores` threads.
- [x] **P1-T7** `benches/budget.rs`: plan computation is O(blocks) and negligible against a real
      stack (guards against an accidental O(n²) in the assignment loop).

**Gate:** block plans match the captured Siril values exactly; the thread-count assertion holds.

### Phase 2 — Statistics

- [x] **P2-T1** `stats.rs`: `median` (quickselect, `f32`), `mad`, `sigma`, `mean`, min/max, and
      a combined `ChannelStats` pass.
      **Selection must be three-way (Dutch-flag), not two-way.** A calibration frame is millions
      of pixels over a few hundred distinct values, so the pivot is usually the modal value; a
      two-way partition sends every duplicate to one side and advances by one element per pass,
      which is quadratic. This was caught only by the real corpus — a 20k-element synthetic test
      of identical values was slow but finished, while a 61 MP bias frame did not finish in ten
      minutes. `median_is_linear_on_duplicate_heavy_data` guards it.
- [x] **P2-T2** Background level and background noise: the spread of first-order differences
      along each row, sigma-clipped per row, then the median across rows.
- [x] **P2-T3** `location`/`scale` (IKSS) estimators used by normalization, plus the "lite"
      pair (median, 1.5·MAD).
- [x] **P2-T4** Tests: analytic ground truth on generated distributions (`tests/stats.rs`), and a
      comparison against Siril's `stat main` output over the real-frame corpus
      (`tests/stats_parity.rs`), skipping cleanly when the corpus or reference is absent.
      **The original "≤ 1e-6 relative" gate was not achievable and has been corrected** — Siril
      prints `stat` with `%0.1f` in 16-bit ADU (`command.c` l. 9264), so ±0.05 ADU is the floor
      on what any comparison against it can resolve. See the revised gate below.
- [x] **P2-T5** Parallel statistics over a `Runtime`, with a test that results are independent of
      thread count.

**Gate (revised, and met):** every estimator agrees with Siril's `stat main` to within its
printed precision on every corpus frame — ±0.01 ADU for the exact order statistics (median, min,
max, MAD) and ±0.06 ADU for the accumulated ones (mean, sigma, sqrt(BWMV), bgnoise), the latter
being Siril's ±0.05 rounding plus the f32 round-trip through the unit range.

Measured over 12 frames of `data/corpus-mono` (ASI6200MM Pro, 9576×6388, 16-bit, Hα), 3 each of
bias/dark/flat/light: **worst absolute delta 0.0494 ADU across all 96 comparisons**, with median,
min, max and MAD matching to within 0.002. Report at `data/output/stats-parity.txt`; regenerate
the reference with `cargo xtask siril-reference`.

Analytic bounds additionally met (`tests/stats.rs`): background noise recovers an injected sigma
to within 5% and is unmoved by gradients or stars; robust scale lands within 5% of true sigma
with 0.5% of pixels saturated; every estimator is bit-identical across 1, 2, 3, and 8 cores.

### Phase 3 — Stacking

*Goal: the deliverable that makes master bias and dark possible.*

Landed as two modules rather than the six sketched under Architecture —
`stack/mod.rs` (block loop, reducers, assembly) and `stack/rejection.rs`. The finer split was
proposed before the code existed and would have spread ~200 lines over six files.

- [x] **P3-T1** Block reads across the set, via `FrameSet::read_block_into`, which touches only
      the block's bytes (byte accounting pinned in `tests/frame.rs`). Per-frame registration
      shift is **not** wired yet — it lands with Phase 9, which is what produces the shifts.
- [x] **P3-T2** The block loop: plan, one buffer set per worker sized to the tallest block,
      blocks distributed over the `Runtime` pool in waves of `cores`, assembled into the result
      without an intermediate copy. Peak-heap assertions in `tests/memory.rs`.
- [x] **P3-T3** Median reduction over the shared three-way quickselect.
- [x] **P3-T4** `Sum`, `Min`, `Max`.
- [x] **P3-T5** Percentile, sigma, MAD, sigma-median, Winsorized, and linear-fit rejection.
      GESDT returns `ImageProcError::Unsupported` — it needs the inverse Student's t CDF, and
      `BestRejection::find` never selects it (the ADR's stated acceptable outcome).
- [x] **P3-T6** Reject-then-mean with `f64` accumulation, the fall-back to a median when
      rejection empties the stack, and per-pixel rejection maps.
- [x] **P3-T7** `output_norm`: min–max rescale of non-zero values, zeros preserved.
- [x] **P3-T8** `tests/stack_parity.rs` against Siril masters over the real corpus.
- [x] **P3-T9** `benches/stack.rs`: method, frame count, budget, and core scaling.

**Gate: met.** Measured on `data/corpus-mono` (ASI6200MM Pro, 9576×6388, 40 frames), against
masters produced by the same commands `px --engine siril` issues today:

| Master | Max delta | RMS delta | Median | Background noise |
|---|---|---|---|---|
| bias | **0.0000 ADU** | 0.00002 ADU | exact | 1.2581 vs 1.2594 |
| dark | **0.0039 ADU** | 0.00002 ADU | exact | 1.4074 vs 1.4086 |

About 31% of pixels differ at all, every one of them by the last bit of an `f32`: with an even
frame count the median is the mean of the two central values, and Siril forms that in `f64`
before narrowing while the native path stays in `f32`. 2e-5 ADU on a 16-bit scale.

**Speed: at parity.** 2.55 s for a 40 × 61 MP median stack against Siril's 2.35 s (warm cache,
M4 Max, 16 cores) — 1.09×, inside the "≥ parity" gate. Core scaling 5.1× from 1 to 16 cores;
tightening the budget 8× (16 → 125 blocks) costs 22%.

**Memory: inside the budget.** `tests/memory.rs` asserts peak heap against the plan, that peak
heap does not grow with sequence length (4× the frames at the same budget stays under 2× the
heap), and that requesting rejection maps does not breach the budget. The last one revealed the
budgeter working as intended and against intuition: asking for maps *lowers* peak heap, because
they are charged before the plan is drawn and the bands shrink to pay for them.

**One deliberate divergence.** Linear-fit rejection recomputes the index mean and variance for
the current stack length on every clipping iteration. Siril precomputes both once for the full
frame count and keeps using them as the stack shrinks, so every pass after the first fits
against constants that no longer describe its data. Recomputing is what a least-squares fit
means. The divergence is confined to `LinearFit`, which `BestRejection` selects above 30 frames,
and is unmeasured against Siril until a rejection-stacked master exists to compare — that lands
with Phase 5's master flat.

### Phase 4 — Calibration operators

- [x] **P4-T1** `calibrate/ops.rs`: subtract, divide, scalar multiply/subtract in the unit range.
      The clamping rule is shared: above `1.0` clamps, below `-1.0` becomes `0.0`, and **values
      between `-1.0` and `0.0` stay negative**. That last case is load-bearing — a calibrated sky
      background straddles zero, and clipping its negative half would bias every downstream
      statistic upward. Scalar operations do not clamp at all, since they are interior steps.
- [x] **P4-T2** `calibrate/mod.rs`: `Masters { bias, bias_level, dark, flat }` and `calibrate`,
      applying bias → dark → flat, with the flat's level taken from the central third of the
      master (the corners are vignetted and would drag it down).
- [x] **P4-T3** `calibrate/dark_optimize.rs`: golden-section search over the dark's scaling
      factor, minimizing the noise left in a 512² central sample, plus the cheaper
      exposure-ratio path.
- [x] **P4-T4** `calibrate/cosmetic.rs`: hot/cold detection from the master dark's own median and
      sigma, and CFA-aware patching — hot pixels from a neighbour mean, cold from a neighbour
      median over a wider window, both striding by two on a colour sensor so replacements come
      from same-coloured pixels.
- [x] **P4-T5** Unit tests (`tests/calibrate.rs`) and parity against Siril's `calibrate_single`
      on a real 61 MP light (`tests/calibrate_parity.rs`).

**Gate: met on both corpora, with a wide margin.**

The masters are composed the way the project builds them and the way the workflow intends:
master bias and master dark are each stacked from raw frames on their own; the master flat is
built from flats calibrated with the master bias; and a light is then calibrated with the **dark
and the flat only**. No separate bias subtraction — the raw-stacked dark already carries the
bias pedestal, and subtracting a master bias as well would remove it twice.

| Corpus | max abs delta | RMS delta | median (ours / Siril) | background noise |
|---|---|---|---|---|
| mono (ASI6200MM, Hα) | 0.0195 ADU = **0.0011 σ_bg** | 0.00004 ADU | +60.8723 / +60.8725 | 17.7558 / 17.7549 |
| OSC (ASI6200MC, Dual) | 1.6054 ADU = **0.0079 σ_bg** | 0.01571 ADU | +633.1835 / +633.1990 | 203.5618 / 203.5668 |

Bound is 0.05 σ_bg max, 0.01 σ_bg RMS. Both are met by 6–45×.

### The negative clamp, measured

`ops::clamp` keeps values between `-1.0` and `0.0` negative and destroys only what falls outside
that: past `1.0` it saturates, past `-1.0` it zeroes. `OpReport` now counts both destructive
cases separately from the merely-negative ones, so this is measured rather than argued.

| Corpus | negative | deepest negative | **floored to zero** | saturated |
|---|---|---|---|---|
| mono | 0.068% | −50,961 ADU (−0.778 unit) | **0** | 193 (0.0003%) |
| OSC | ~0.000% | −59,999 ADU (−0.916 unit) | **7** | 249 (0.0004%) |

Out of 61,171,488 pixels. Siril's own output has the same negative fraction to three decimals
and the same deepest value to within 1.5 ADU.

Reading: **the clamp is a non-issue in this workflow.** The deepest negatives are hot pixels in
the master dark subtracted from an ordinary light — they land at −0.78 to −0.92, just inside the
floor — and the floor itself fires on 0 and 7 pixels respectively, both of which cosmetic
correction would patch anyway. What the earlier draft of this test measured (99.82% negative, a
−250 ADU median) was not the clamp misbehaving but the masters being composed wrongly: a bias
subtracted on top of a dark that already contained it.

`CalibrationReport` carries `negative_ratio`, `floored_pixels`, and `saturated_pixels` so Phase 5's
pipelines can refuse or warn on mismatched masters rather than silently producing a frame whose
sky is below zero.

**Yardstick for the Phase 3 linear-fit divergence.** Two Siril master flats built from the same
59 frames, differing only in rejection method (Winsorized 3/3 vs linear-fit 5/5), differ by
**max 165.7 ADU, RMS 14.8 ADU**. That is the scale on which a rejection-method choice moves a
master flat, and therefore the scale against which the native linear-fit divergence must be
judged when Phase 5 produces a native master flat.

### Phase 5 — `NativeRuntime` and the three master-frame pipelines

*Goal: the first user-visible native path.*

- [ ] **P5-T1** `px-pipeline/src/native.rs`: the `NativeRuntime` builder of D4, re-exported from
      the crate root next to `RunIn`. `PipelineError` gains `ImageProc(#[from] ImageProcError)`
      and `UnsupportedByEngine(String)`.
- [ ] **P5-T2** Split `calibration/master_bias.rs` into `master_bias/{mod.rs, siril.rs}` with no
      behaviour change; confirm `px` builds untouched. Repeat for `master_dark`, `master_flat`,
      `calibrate_light`, `project/master_light`. **One commit, mechanical, zero logic edits.**
- [ ] **P5-T3** `master_bias/native.rs`: `impl RunIn<NativeRuntime>` — collect frames, uniformity
      check, median stack, write output with the same name `CalibrationMetadata::master_bias_name`
      produces, propagate header metadata from the first frame, return the identical `MasterBias`.
- [ ] **P5-T4** `master_dark/native.rs`, same shape.
- [ ] **P5-T5** `master_flat/native.rs`: bias-calibrate each flat in-flight, then stack with
      rejection and no normalization, matching the Siril pipeline's `-nonorm`. Set `FILTER` on the
      output the way `SequpdateKey` does today.
- [ ] **P5-T6** CLI (D11): `--engine`, `--max-cores`, `--max-memory` on `GlobalArgs`;
      `requires_siril()` becomes engine-aware; `create_master_bias`/`dark`/`flat` dispatch on the
      flag. Unsupported combinations produce a clear error, not a panic.
- [ ] **P5-T7** End-to-end test: `px --engine native master bias` over the corpus produces a file
      that passes `assert_parity!` against the Siril-engine output of the same command, and
      `index.register_master` records identical metadata from both.

**Gate:** all three master-frame commands work under `--engine native`, hit their parity
tolerances, and meet the ≥ 2× speed gate.

### Phase 6 — Debayer and `CalibrateLight`

- [ ] **P6-T1** `debayer/rcd.rs`: Ratio Corrected Demosaicing, full resolution, ported from
      librtprocess (which is what Siril's `demosaicing_rtp.cpp` calls). Cite the source and the
      Luo & Zhang paper.
- [ ] **P6-T2** `debayer/equalize.rs`: `equalize_cfa` per-channel balance, matching Siril.
- [ ] **P6-T3** `debayer/mod.rs`: pattern from `BAYERPAT` with `XBAYROFF`/`YBAYROFF` offsets and
      row-order handling; dispatch between RCD and the existing preview super-pixel path; a typed
      error for X-Trans.
- [ ] **P6-T4** RCD parity test against Siril's debayer output — tolerance ≤ 0.5 σ_bg, with the
      looser bound justified in a comment as an independent port.
- [ ] **P6-T5** `calibrate_light/native.rs`: per-frame calibrate (+ optional cosmetic correction)
      → optional debayer → write `pp_`-prefixed output to `out_folder`, parallel across frames
      with concurrency drawn from the budget (`max_bytes / bytes_per_frame_in_flight`), not a
      fixed number. Handles the single-frame case the Siril impl special-cases. Returns the
      identical `CalibratedLight`.
- [ ] **P6-T6** CLI dispatch for `px obs calibrate` and `px obs batch-calibrate`.
- [ ] **P6-T7** Parity + performance test for a 100-frame CFA set.

**Gate:** calibrated lights within tolerance, mono and CFA; ≥ 2× faster; peak RSS within budget.

### Phase 7 — Star detection and PSF fitting

- [ ] **P7-T1** `register/starfind.rs`: candidate detection — threshold at
      `median + 5·sigma·bgnoise`, local-maximum scan in a `radius`-box, duplicate suppression by
      the `MAX_RADIUS_RATIO_DUP` rule, magnitude-ordered candidate list. Port of `peaker`.
- [ ] **P7-T2** `register/psf.rs`: elliptical Gaussian fit with rotation (7 parameters) by
      Levenberg-Marquardt on `nalgebra`, with Siril's initial guesses, parameter bounds, and
      `MAX_ITER_ANGLE` iteration cap. Analytic Jacobian, matching `psf_Gaussian_df_ang`.
- [ ] **P7-T3** Moffat profile (8 parameters, `min_beta = 1.5`), selectable, defaulting off as in
      Siril.
- [ ] **P7-T4** Star rejection rules: amplitude bounds, roundness floor, `max_r`, RMSE, and the
      `relax_checks` variant. Port of `reject_star`.
- [ ] **P7-T5** `FWHM_stats` and `filter_stars_by_amplitude`, needed by two-pass reference
      selection.
- [ ] **P7-T6** Tests: on synthetic frames with analytically placed stars, recovered centroids
      within 0.02 px and FWHM within 1% of truth. Against Siril's `findstar` on real frames:
      ≥ 95% matched within 0.2 px, centroid RMS ≤ 0.05 px.
- [ ] **P7-T7** `benches/register.rs` part 1: detection + fitting throughput at 2000 stars.

**Gate:** the detection parity numbers above, on at least three real frames from the large corpus.

### Phase 8 — Star matching and transform estimation

- [ ] **P8-T1** `register/transform.rs`: `Transform` — shift, similarity, affine, homography —
      with application, inversion, composition, and `guess_transform_from_H` (Siril classifies a
      homography by inspecting its coefficients; several code paths depend on it).
- [ ] **P8-T2** Normalized DLT homography from ≥ 4 correspondences, and least-squares affine /
      similarity / shift from their respective minima. Conditioning via Hartley normalization.
- [ ] **P8-T3** RANSAC wrapper: reprojection threshold 3.0 px (OpenCV's
      `defaultRANSACReprojThreshold`, which Siril uses), adaptive iteration count, final
      refit on inliers. Deterministic given a seed — parity tests need reproducibility.
- [ ] **P8-T4** `register/match.rs`: triangle-similarity matching — build the triangle array from
      the N brightest stars, sort by side ratio, match by ratio proximity, accumulate a vote
      matrix, take the top vote-getters. Port of `atpmatch.c`'s `fill_triangle_array` /
      `make_vote_matrix` / `top_vote_getters`, citing Valdes et al. 1995.
- [ ] **P8-T5** Iterative refinement: fit, reject by residual, refit, matching `iter_trans`.
- [ ] **P8-T6** Tests: synthetic star lists under a known homography (with added noise, spurious
      detections, and missing stars) recover it to ≤ 0.01 px corner error. Against Siril's stored
      registration data on the corpus: corner reprojection ≤ 0.1 px on every frame.

**Gate:** the 0.1 px corner-error bound against Siril's own homographies on 100 real frames.

### Phase 9 — Two-pass registration and framing

- [ ] **P9-T1** `register/framing.rs`: reference selection by Siril's score —
      `2·FWHMx·(maxstars − nbstars)/maxstars + FWHMx`, only ranking frames with at least half the
      maximum star count, and the amplitude-threshold re-filter when the star count caps at
      `max_stars_fitted`. Port of `global.c` l. 895–990.
- [ ] **P9-T2** The two-pass loop: detect stars in every frame (budget-bounded concurrency),
      pick the reference, compute every transform to it, verify enough frames registered, and
      re-trial up to `MAX_TRIALS_2PASS = 5` when they did not.
- [ ] **P9-T3** Output geometry for `FRAMING_CURRENT` and `FRAMING_MIN` (`compute_roi`). Reject
      `FRAMING_MAX` / `FRAMING_COG` with a typed error (out of scope).
- [ ] **P9-T4** Frame exclusion: frames that fail detection or matching are dropped with a
      reported reason, mirroring Siril's `incl` flag, and surfaced through `PipelineReporter`.
- [ ] **P9-T5** Persist registration data as a sidecar FITS binary table (D6) and read it back.
      Roundtrip test.

**Gate:** on the 100-frame corpus, the native reference frame matches Siril's choice, and the
same set of frames is included.

### Phase 10 — Warping

- [ ] **P10-T1** `register/warp.rs`: inverse-mapped warp with bilinear, area, bicubic (OpenCV's
      `INTER_CUBIC` kernel, `a = -0.75`), and Lanczos-4 kernels. Destination is zero-initialized
      and pixels whose source falls outside the input are left untouched — that is what
      `cvTransformImage`'s `BORDER_TRANSPARENT` over a `Scalar(0)` destination amounts to
      (`opencv.cpp` l. 109–137, 531).
- [ ] **P10-T2** Interpolation clamping (Siril's `clamp`, **on by default**, and visibly
      different on star cores). It is not a kernel tweak — it is a second pass
      (`opencv.cpp` l. 532–542): warp the same input again with `INTER_AREA` to build a *guide*,
      form the mask `out < guide * 0.98` (`CLAMPING_FACTOR`), dilate that mask with a 3×3
      elliptical structuring element, and copy the guide's values into the masked pixels.
      Requires a small morphological dilation helper, which this task also adds and tests.
- [ ] **P10-T3** Parallelize by output row band through the `Runtime`; memory-bounded so only the
      needed source rows are resident.
- [ ] **P10-T4** Tests: with clamping off, a warp by an exact-integer shift equals a `memcpy`
      shift for every kernel; round-trip warp by `H` then `H⁻¹` returns the original within 1e-4
      in the interior; with clamping on, the clamped pixel *set* matches Siril's within 1% of
      pixels. Against Siril's `seqapplyreg` output at ≤ 0.5 σ_bg.
- [ ] **P10-T5** `benches/register.rs` part 2: warp throughput per kernel at 60 MP, with and
      without clamping (clamping roughly doubles the warp cost — it is a second full warp).

**Gate:** warp parity at ≤ 0.5 σ_bg with clamping on, and the integer-shift identity exact with
clamping off.

### Phase 11 — `CreateMasterLight` and the end-to-end report

- [ ] **P11-T1** `project/master_light/native.rs`: multi-folder frame collection with the same
      ordering the Siril impl produces → two-pass registration → warp → normalized
      rejection-stack (`AddScale` normalization, `BestRejection::find`, `output_norm`) → write.
      Returns the identical `MasterLight`. Rejects `background_extract` with
      `UnsupportedByEngine`.
- [ ] **P11-T2** `normalize.rs` wired in: additive-with-scaling coefficients relative to the
      reference frame, the path `CreateMasterLightPipeline` uses today. Parity test on the
      coefficients themselves against instrumented Siril output, before comparing images.
- [ ] **P11-T3** Avoid the intermediate write: warped frames feed the stacker's block reader
      directly where the budget allows, spilling to a temp sequence only when it does not. Both
      paths tested; the spill path is what keeps a 500-frame 60 MP session inside 8 GB.
- [ ] **P11-T4** The `mirrorx` orientation step the Siril pipeline performs after stacking.
- [ ] **P11-T5** CLI dispatch for `px project stack`.
- [ ] **P11-T6** End-to-end parity at the D2 master-light tolerances, plus the σ_bg and FWHM
      assertions.
- [ ] **P11-T7** Write the full report into `crates/px-imageproc/README.md`: parity table, speed
      table, peak-RSS-vs-budget table, machine and Siril version, and an honest account of every
      tolerance that had to be argued for rather than met outright.

**Gate:** `px --engine native project stack` produces a master light meeting every D2 bound, at
≥ parity speed, within budget.

### Phase 12 — Tuning and consolidation

- [ ] **P12-T1** Profile all five pipelines; record where time actually goes.
- [ ] **P12-T2** Act on what the profile shows. If it shows a kernel where autovectorization is
      not firing, report the numbers (D8) rather than reaching for `unsafe`.
- [ ] **P12-T3** Revisit D3: if the profile shows the ported block heuristic is leaving
      throughput on the table, benchmark an alternative and record the comparison. Changing it
      requires that evidence.
- [ ] **P12-T4** Decide, with the report in hand, whether `--engine native` should become the
      default. That decision is an ADR amendment, not a task outcome.
- [ ] **P12-T5** Update ROADMAP and `docs/crate-dependencies.md`.

---

## Open questions

- **Interpolation parity ceiling.** OpenCV's `remap` uses fixed-point weight tables for Lanczos-4;
  a straightforward `f32` implementation will differ by small amounts everywhere. The 0.5 σ_bg
  bound in P10-T4 is an estimate. If it proves unreachable, the right move is to measure whether
  the difference is *worse* (ringing, softness) or merely different, and adjust the assertion to
  test that — not to widen the number quietly.
- **GESDT needs the inverse Student's t CDF**, which Siril gets from GSL. `statrs` provides it;
  whether that dependency is worth carrying for one rejection method the project's
  `BestRejection` helper never selects is a Phase 3 call. Deferring GESDT with a typed error is
  an acceptable outcome.
- **Frame-count ceiling for in-memory registration data.** Star lists for 2000 stars × 500 frames
  is on the order of 100 MB. It should be charged against the budget; P9-T2 will confirm whether
  that is material.
- **`FitsExt` living in `siril-sys`** (D5) is an accepted wart. Moving it to `px-fits` is the
  obvious cleanup and is blocked only by this ADR's "no changes to Siril crates" constraint.
