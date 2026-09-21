# 007 — Native Calibration, Registration and Stacking for `px-imageproc`

## Status

Accepted, and implemented. Phases 1–13 are complete; Phase 0 was superseded by what replaced it
and is marked accordingly. `--engine native` runs all five pipelines and is opt-in — the default
engine stays Siril, for the reasons in the P12-T4 amendment.

Supersedes nothing. Depends on [ADR 006](006-native-fits-implementation.md), whose Phases 0–8 are
complete: `px-fits` is a native reader/writer with region reads, streaming rows, and
`rayon`-parallelized full-frame decode.

---

## Licensing and provenance

Siril is **GPL-3.0**. Photonyx is permissive — `LICENSE` and the README say BSD-3-Clause, while
`Cargo.toml` says `MIT OR Apache-2.0`; those two disagree and should be reconciled separately.
Either way, GPL-3.0 code cannot be incorporated into this project.

This constrains *how* the native engine is built, not *what* it does:

- **`siril-sys` is unaffected.** It invokes `siril-cli` as a separate process over a pipe. That
  is arm's-length use, not linking, and is explicitly fine under the GPL.
- **Algorithms and parameters are not the issue.** Which technique to use, and the constants
  that define it — Winsorized clipping at ±1.5σ with a 1.134 consistency factor, a 6-MAD clip,
  RCD demosaicing — are facts about the technique, published in the literature.
- **Expression is the issue.** Transcribing control flow, reproducing internal quirks, or
  pinning tests to values read out of Siril's internals produces a derivative work.

So every algorithm here is implemented from published descriptions and validated against Siril
as a **black box** — comparing outputs, which is ordinary interoperability testing. See handoff
rule 7 for the working rule.

**The clean-room-specification route (added Phase 6c).** There is a second admissible route, and
it is what finally made RCD available. Where an algorithm has no published description, one
person reads the reference implementation and writes an *implementation-independent functional
specification* — behaviour, data flow, constants and rationale, with no code, no identifiers and
no control flow — and a second person implements from that document alone, never having seen the
original. This is the established clean-room method, and it rests on the same distinction as the
rest of this section: the specification carries only the unprotectable functional content, and
what the implementer produces is an independent expression of it.

Two disciplines make it hold, and both are recorded per module:

1. **The wall is real and stated.** `debayer/rcd.rs` names the specification, its date, and the
   fact that its author has never opened `rcd.cc`. A file that cannot say this cannot use this
   route.
2. **Validation stays black-box, and coefficients are never fitted.** Comparing output against
   Siril is interoperability testing. *Searching* a parameter until the output matches is
   deriving the original's expression by another path, and is not permitted — where the
   specification is ambiguous, the ambiguity is resolved by its author, not by the reference's
   behaviour. Phase 6c has a worked example of exactly this, left open rather than fitted.

This was caught partway through Phase 6, after Phases 1–5 had landed. `runtime/blocks.rs` and
`stack/rejection.rs` were structure-preserving ports and have been rewritten from their
specifications; what that cost, and what it improved, is recorded under those phases.

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

### The memory/CPU model

The premise this work was proposed on — block the work by memory budget, scan the same region
across all frames, reduce per pixel — is the right shape, and is what every out-of-core stacker
converges on for the same reason: the reduction needs every frame's value for a given pixel at
once, and a night of frames does not fit in memory.

The description below states the *problem and its constraints*. It is not an implementation
plan: per the clean-room rule in the handoff section, the scheduler is derived from these
constraints rather than transcribed from any existing implementation.

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

**This is the layout as proposed, kept as the record of the intent. The crate as built differs in
three places**, each noted where it happened: `stack/` landed as `mod.rs` + `rejection.rs` +
`normalize.rs` rather than the seven files sketched here (Phase 3 — the finer split would have
spread ~200 lines over six files); `debayer/` gained `mhc.rs`, `bilinear.rs`, `ahd.rs` and
`rcd.rs` as that decision was worked through (Phases 6, 6b, 6c) and folded `equalize.rs` into
the calibration path; and `register/` gained `twopass.rs` and `persist.rs`, which the
orchestration-in-`mod.rs` sketch did not anticipate. Read the crate for what exists.

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
      starfind.rs             # candidate detection, the whole star-finding pass
      psf.rs                  # Gaussian/Moffat Levenberg-Marquardt fit
      starmatch.rs            # triangle similarity matching + vote matrix
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

Disagreements with Siril are adjudicated against analytic ground truth rather than assumed, so
that "Siril did something else" and "we are wrong" stay distinguishable.

**How that ground truth is actually supplied was not what this ADR first planned, and the change
is recorded under Phase 0.** The plan was a committed synthetic corpus with a JSON truth manifest.
What shipped instead is per-test synthetic generation — each test states the truth it needs, in
the test, next to the assertion that uses it — plus black-box comparison against Siril over a
real, uncommitted corpus. The consequence to be aware of when reading the parity tables below:
the analytic bounds (injected sigma recovered to 5%, centroids placed and recovered, warps under a
known homography) run everywhere, but **every number quoted against *Siril* comes from a corpus
that is not in this repository**, reachable only via `PX_IMAGEPROC_CORPUS` and `PX_SIRIL_REF`.
Those tests skip, loudly, without it.

**D3 — The block scheduler is designed from the memory constraint, not ported.**

The partitioning problem is stated directly: given a budget, a frame count, an image shape, and
a worker count, choose row-bands such that the bands resident at once fit the budget, each
worker gets a comparable amount of work, and no band straddles a channel. `runtime/blocks.rs`
solves exactly that:

```
rows_in_flight  = (budget − result − rejection maps − resident masters) / (frames × width × 4)
band_height     = min(rows_in_flight / workers, channel_height)
bands_per_chan  = ceil(channel_height / band_height)
```

with the leftover rows spread one each across the leading bands so no band is more than a row
taller than any other.

This is deliberately *not* the arrangement Siril arrives at, and it is better in three ways
that were measured rather than assumed: it has no mebibyte quantization, so small budgets work
instead of rounding to zero; it derives the band height from the per-worker share directly, so
it cannot overshoot the budget and needs no correction pass; and it drops the trailing-stub
absorption rule, which existed to avoid runt bands that even remainder distribution never
produces.

Its tests are properties — the bands tile the image exactly, never cross a channel, stay inside
the budget, and stay balanced — rather than values captured from another implementation.

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
| `CreateMasterLight` (register + stack) | — (see Phase 11) | ≤ 1.0 σ_bg on the sky | σ_bg(native) ≤ 1.02 × σ_bg(Siril); FWHM within 2%; star count no lower |

The master-light bound is the loosest because a sub-pixel difference in one homography moves
every pixel. The noise and FWHM assertions are what actually constrain quality there — a result
that is *different* but measurably sharper and no noisier is a pass, and the test says so
explicitly rather than being quietly widened.

**The master-light row was corrected in Phase 11, with the measurement attached.** A flat
per-pixel bound turned out to measure two things that are not quality: the `-output_norm` rescale,
which is keyed to two extreme pixels and differs globally between any two implementations, and
sub-pixel differences on saturated star cores. The bound is now stated on the sky, where it means
something (measured 0.33 σ_bg), and the quality assertions carry the rest.

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
7. **Clean-room implementation. This is a licensing requirement, not a style preference.**
   Siril is GPL-3.0; photonyx is permissive. A structure-preserving translation of GPL source
   is a derivative work and cannot be shipped under a permissive licence, so every algorithm
   here is implemented from **published descriptions** — papers, standards, textbooks, and
   permissively-licensed references — and never by translating Siril's code.

   Concretely:
   - **Permitted:** implementing a named, published algorithm (Winsorized sigma clipping,
     biweight midvariance, golden-section search, RCD) from its literature description;
     choosing the same parameters Siril chooses, where those are facts about the technique or
     about astrophotography practice; comparing *outputs* against Siril as black-box
     validation.
   - **Not permitted:** transcribing Siril's control flow, reproducing its internal quirks,
     citing its file and line numbers as the justification for a piece of code, or pinning a
     test to values read out of its internals.

   Cite the *literature*, once per module. Comments inside functions explain this code's own
   reasoning — a non-obvious constant, an edge case — and are worth writing only when the code
   cannot say it itself.
8. **The existing Siril pipelines keep passing their tests unchanged at every commit.**

### Phase 0 — Corpus, parity harness, baselines

*Goal: make both quality and speed measurable before any algorithm is written.*

**This phase was not executed as written, and is closed as superseded rather than left
unchecked.** The goal was met — quality and speed are measurable, and were measured at every
phase gate — but by a different arrangement than the one planned here. What changed, and why, is
recorded below the task list. The original tasks are kept, struck where they were not built, so
that a reader who finds no `imageproc-fixtures` command knows it was decided against rather than
forgotten.

- [x] **P0-T1** Workspace deps: `nalgebra`, `sysinfo`. `[[bench]]` entries (`harness = false`)
      for `stack`, `register`, `budget` and — added in Phase 12 — `reduce`.
      ~~dev-only `approx`~~: never needed. The tolerances here are named, asymmetric and
      per-measurement (0.05 ADU on a printed statistic, 0.1 px on a centroid, a ratio against
      σ_bg), so every assertion states its own bound; a generic approximate-equality macro would
      have hidden exactly the number each test exists to pin.
      ~~`calibrate` bench~~: calibration is one pass of arithmetic over a frame, is never the
      stage a profile points at (`calibrate.apply` is 0.58 s against a 1.44 s write), and a
      microbenchmark of it would have measured `px-fits`.
- [ ] ~~**P0-T2** `cargo xtask imageproc-fixtures`: a committed ≤ 512×512 corpus with a JSON
      ground-truth manifest.~~ **Not built.** See below.
- [x] **P0-T3** `cargo xtask siril-reference`: runs Siril over the corpus and writes reference
      outputs plus a manifest recording the Siril version. Errors with a clear message if Siril
      is not installed; consuming tests skip. Grown one subcommand at a time as each phase needed
      its reference (`stat` in Phase 2, through `--master-light` in Phase 11) rather than built
      up front for outputs no test yet consumed.
- [x] **P0-T4** Parity harness. Landed as one `support.rs` per consuming crate plus a written
      report per comparison under `data/output/*-parity.txt`, rather than one macro: each phase's
      parity question has a different shape (a printed statistic, a star list, a homography, a
      frame), and the reports are what the README's tables are quoted from.
- [x] **P0-T5** `PX_IMAGEPROC_CORPUS`, pointing at real frames. This became the *primary*
      corpus rather than the large-corpus hook it was planned as.
- [x] **P0-T6** Siril baselines measured and recorded — wall time for all five pipelines, with
      machine, OS and Siril version. They live in the README's Phase 11 and 12 sections beside
      the native numbers they are the baseline for, rather than in a separate "Baseline" section
      that would have to be read in two places.

**Gate: met, on the goal rather than on the letter.** Every phase gate in this document is
backed by a measurement against Siril or against analytic truth, and the parity tests skip
cleanly without `PX_SIRIL_REF`.

#### Why the committed fixture corpus was not built

The premise was that a small committed corpus would make parity reproducible for anyone with the
repository. Three things went against it, in order of when they were found:

1. **The bugs were not reachable at 512×512.** The two worst defects this work produced were a
   quadratic median on duplicate-heavy data (Phase 2) and a memory-budget breach in the master
   light (Phase 11). Both were invisible on a small frame — the first *finished* on a 20k-element
   synthetic test and hung on a 61 MP bias frame, and the second is definitionally about a frame
   set that does not fit. A corpus small enough to commit is a corpus too small to find them.
2. **Truth next to the assertion beat truth in a manifest.** Tests generate what they need —
   a known sigma, a placed centroid, a known homography — in the test body. The tolerance and
   the truth it is measured against are then one screen apart, and neither can drift from the
   other. A JSON manifest would have put a test's truth in a file the test does not name.
3. **The Siril half was never committable anyway.** Reference outputs are large and
   Siril-version-dependent, which this ADR already says under Consequences. Committing the
   *inputs* alone would have produced a corpus that reproduces nothing without a local Siril run
   — which is what `PX_SIRIL_REF` already handles.

**What this costs, stated plainly.** Every figure in this document quoted against Siril was
measured on a corpus that is not in this repository: `data/corpus-mono` (ASI6200MM Pro, 9576×6388)
and an OSC set. A contributor without those frames can run the full unit and analytic suite and
gets the parity tests skipped, not failed. That is a real reproducibility gap, and closing it —
a downloadable corpus, or a generator large enough to be worth timing — is follow-up work, not
something this phase delivered.

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
- [x] **P1-T4** `runtime/blocks.rs`: band sizing and partitioning, derived from the memory
      constraint per D3. **Rewritten clean-room** after the licensing review; the replacement is
      simpler and measurably better — no mebibyte quantization (so sub-MiB budgets work instead
      of rounding to zero), band height derived from the per-worker share directly (so it cannot
      overshoot the budget and needs no correction pass), and the band count rounded up to whole
      waves. That last one matters: 19 bands on 16 cores costs two full waves with the second
      running three cores, and rounding to 32 shorter bands cut a 40-frame 61 MP median stack
      from 4.10 s to **2.51 s** with byte-identical output.
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

**Rewritten clean-room (see Licensing and provenance).** `stack/rejection.rs` was originally a
transcription of Siril's rejection control flow. It is now implemented from the published
descriptions of each estimator, with its own iteration structure and its own stopping rule
(never clip below four survivors, because a spread estimated from fewer measures its own noise).

The measurable cost is a looser agreement with Siril on boundary pixels, and it is worth stating
plainly. On the 59-frame master flat:

| | Ported | Clean-room |
|---|---|---|
| pixels differing > 0.01 ADU | 10 (0.0000163%) | 78,061 (0.128%) |
| max delta | 25.09 ADU | 34.69 ADU |
| RMS delta | 0.0056 ADU | 0.526 ADU (0.0065 σ_bg) |
| median vs Siril | exact | **exact** |
| background noise vs Siril | exact | within 0.004% |

Those 78,061 pixels are a single population: 78,056 of them differ by 5–20 ADU and only 20 by
more. Over 59 frames a 5–20 ADU shift is exactly one moderately-deviant value — a dust mote,
around 300–1200 ADU — included by one implementation and excluded by the other. Flats have many
pixels sitting at the same relative threshold, so the ties concentrate there.

Quality is unchanged: identical median, background noise within 0.004%, RMS 28× tighter than the
gap between Siril's own two rejection methods on the same data. What was lost is agreement on
tie-breaking, which is precisely what a clean-room implementation gives up on purpose.

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

- [x] **P5-T1** `px-pipeline/src/native.rs`: `NativeRuntime`, its built form `NativeEngine`, and
      the shared master-stacking helper. `PipelineError` gains `ImageProc` and
      `UnsupportedByEngine`.
- [x] **P5-T2** Each of the five pipelines split into `<name>/{mod.rs, siril.rs, native.rs}`,
      the struct in `mod.rs` and one execution per environment beside it. Public paths and
      builders unchanged; `px` compiled untouched across the split.
- [x] **P5-T3** `master_bias/native.rs` — median stack.
- [x] **P5-T4** `master_dark/native.rs` — median stack.
- [x] **P5-T5** `master_flat/native.rs` — Winsorized 3/3 rejection stack with the bias
      subtracted **in flight**. `stack_calibrated` takes the master to subtract and applies it
      per block, so the calibrated flats never reach the disk; the resident master is charged
      against the memory budget before blocks are planned.
- [x] **P5-T6** CLI: global `--engine <siril|native>`, `--max-cores`, `--max-memory` (accepting
      `8G`, `512M`, `75%`). `requires_siril` is engine-aware, so `px --engine native master bias`
      runs on a machine with no Siril at all. A native run of a pipeline that has no native
      implementation fails with a clear error rather than falling back silently.
- [x] **P5-T7** `px-pipeline/tests/native_masters.rs` runs the real pipeline structs against the
      corpus and compares to Siril masters.

**Gate: met.** Measured on `data/corpus-mono` (ASI6200MM Pro, 9576×6388):

| Master | Frames | Max delta | RMS delta | Median | Background noise |
|---|---|---|---|---|---|
| bias | 40 | **0.0000 ADU** | 0.00002 ADU | exact | 1.2581 vs 1.2594 |
| dark | 40 | **0.0039 ADU** | 0.00002 ADU | exact | 1.4074 vs 1.4086 |
| flat | 59 | 25.09 ADU | **0.0056 ADU** (7e-5 σ_bg) | **exact** | **exact** |

**Speed**, end to end through the `px` binary against the same commands on the Siril engine:

| Command | Native | Siril | |
|---|---|---|---|
| `master bias` | 2.83 s | 2.24 s | 0.8× |
| `master dark` | 3.27 s | 2.17 s | 0.7× |
| `master flat` | **7.50 s** | 20.98 s | **2.8× faster** |

The flat is where fusing calibration into the stack pays: Siril writes 59 calibrated `pp_flat_`
frames to disk and reads them back, and the native path does not. Bias and dark are slower by a
few hundred milliseconds of process and profile startup on a job that is otherwise I/O bound —
within the "≥ parity" gate's intent but worth revisiting in Phase 12.

**Revisited and closed in Phase 12.** The bias and dark deficit was not startup, it was the
reduction kernel, and P12-T2 removed enough of it to flip both. Re-measured through the same
binary, best of three warm runs:

| Command | Native | Siril | |
|---|---|---|---|
| `master bias` | **2.01 s** | 2.15 s | **1.07× faster** (was 0.8×) |
| `master dark` | **1.98 s** | 2.13 s | **1.08× faster** (was 0.7×) |
| `master flat` | **7.72 s** | 18.20 s | **2.4× faster** |

#### A real bug the corpus caught, and a gate that was measuring the wrong thing

The first master-flat run differed from Siril by max 118 ADU with 89,435 pixels (0.146%) affected.
Diagnosis, in `px-pipeline/tests/flat_diagnostic.rs`: the differences were scattered over 4,781 of
6,388 rows with uniform per-row RMS — no block-boundary signature — and the clamp Siril applies
when writing calibrated flats was ruled out by measurement (calibrated flats span [0.269, 0.995],
nowhere near either bound).

The cause was a divergence introduced by unifying the three clipping estimators behind one loop.
Sigma and MAD clipping measure the spread about the *previous* pass's centre and then move it;
Winsorized clipping cannot, because winsorizing pulls the tails toward the centre and so must use
this pass's. Fixing the ordering moved every headline number:

| | Before | After |
|---|---|---|
| pixels differing > 0.01 ADU | 89,435 (0.146%) | **10 (0.0000163%)** |
| max delta | 118.42 ADU | 25.09 ADU |
| per-row RMS (mean) | 0.5565 ADU | 0.0003 ADU |
| median vs Siril | +0.007 ADU | **exact** |
| background noise vs Siril | −0.0037 ADU | **exact** |

**The max-delta bound this ADR originally specified for the master flat (0.1 σ_bg) is not a
reachable target for any rejection stack, and has been replaced.** Rejection is a discrete
per-pixel decision: a value sitting exactly on the clipping threshold flips one way in one
implementation and the other way in another, and that single flip moves the pixel's mean by
`(outlier − mean) / n` — tens of ADU for a dust mote. Two independent implementations cannot
agree on every boundary pixel, and neither can one implementation against itself with a different
quickselect pivot.

The gate is now what actually characterizes quality for a rejection stack:

- RMS delta ≤ 0.02 σ_bg — met at **7e-5 σ_bg**, 280× inside
- pixels differing by > 0.01 ADU < 0.01% of the frame — met at **0.0000163%** (10 of 61,171,488)
- mean flat level within 0.1% — met exactly
- worst pixel smaller than Siril's own Winsorized-vs-linear gap on the same data (165.7 ADU) —
  met at 25.09 ADU

The last one is the meaningful framing: the native result is closer to Siril's Winsorized master
than Siril's own two rejection methods are to each other.

### Phase 6 — Debayer and `CalibrateLight`

**RCD has no *published* specification, and was substituted at this phase.** Siril gets RCD from
librtprocess, which is GPL-3.0, and no paper states its filters. Under the licensing rule as it
stood it was therefore unavailable, and this phase's task list is rewritten around what *is*
available. (Phase 6c later reached RCD by the clean-room-specification route, which did not exist
as an option when this phase ran. The substitution reasoning below stands as the record of why
gradient-corrected was chosen, and remains the argument for the default it set.)

- [x] **P6-T1** `debayer/mhc.rs`: Malvar-He-Cutler gradient-corrected linear interpolation
      (ICASSP 2004 / MSR-TR-2004-92), whose four 5×5 filters the paper states outright. The
      kernels are transcribed as tap lists so each can be read against the paper's figure.
- [x] **P6-T1b** `debayer/bilinear.rs`: nearest-same-colour averaging, kept as a measured
      comparison point and as an option for anyone who wants smoothing over sharpness.
- [x] **P6-T3** `debayer/mod.rs`: pattern from `BAYERPAT` with `XBAYROFF`/`YBAYROFF`, dispatch
      across gradient-corrected, bilinear, and the existing preview super-pixel path. A frame
      with no declared pattern is an error, not a guess — guessing wrong swaps red and blue,
      which looks plausible enough to ship.
- [x] **P6-T4** Quality measured three ways (below).
- [x] **P6-T2, P6-T5, P6-T6, P6-T7** CFA equalization wiring, `calibrate_light/native.rs`, CLI
      dispatch, and the 100-frame parity run — were blocked on the demosaic decision below, which
      unblocked them. Each is listed with its result after that decision.

#### What the substitution costs

Measured on a real OSC frame (ASI6200MC Pro, 9576×6388), both reconstructions starting from the
*same* Siril-calibrated CFA frame so calibration cannot contaminate the comparison:

| | red | green | blue |
|---|---|---|---|
| bilinear, background noise vs RCD | 0.93× | 0.92× | 0.75× |
| **Malvar-He-Cutler, background noise vs RCD** | **1.23×** | **1.45×** | **1.24×** |

Channel levels match RCD to better than 0.01% in every case, so there is no colour cast either
way. Demosaicing a 61 MP frame takes 0.07 s.

Read naively, bilinear wins. It does not. A blurrier reconstruction *measures* as quieter,
because smoothing correlates neighbouring pixels and a background-noise estimator reads that as
less noise — ranking demosaics by their measured background rewards the blurriest one. Against a
synthetic star field with known truth, the ranking inverts:

| | max error | RMS error |
|---|---|---|
| bilinear | 0.2243 | 0.003385 |
| **Malvar-He-Cutler** | **0.2060** | **0.002192** (35% better) |

So the honest summary is that MHC is the more accurate reconstruction and genuinely noisier in
the background than RCD, because its correction term amplifies high spatial frequencies — noise
included. RCD averages along the local edge direction and avoids that. Both effects are real.

#### Decision: ship both, default to gradient-corrected *(default superseded in Phase 6c)*

Both reconstructions ship. `--debayer <gradient-corrected|bilinear>` selects between them,
`PX_DEBAYER` overrides, and the default is gradient-corrected. (Phase 6b adds a third, `ahd`,
without changing the default. Phase 6c adds `rcd` and **does** take the default, on the reasoning
below inverting once RCD became available.)

The default follows from what the pipeline is *for*. Stacking averages background noise down and
cannot recover resolution that a blurrier reconstruction threw away, so on a stacking pipeline
the accuracy is the part worth keeping and the extra background noise is the part that gets
dealt with anyway. Bilinear stays available for anyone working with few frames, or who prefers
the smoother rendition.

Neither is RCD, and the ADR does not claim they are. Closing that gap was Phase 6b's job; it
narrowed it without closing it, and the default did not move.

- [x] **P6-T2** CFA equalization wired into the native calibration path: the master flat's red
      and blue are levelled to green before the division, so dividing by it corrects the optics
      without also neutralizing the sensor's colour balance.
- [x] **P6-T5** `calibrate_light/native.rs`: per-frame calibrate → optional debayer → `pp_`
      output, several frames at a time with the concurrency taken from the memory budget rather
      than the core count. A colour frame needs its CFA buffer *and* three output channels live
      at once — sixteen bytes per pixel, ~1 GB at 61 MP — so the core count alone would be a
      poor guide. A frame that comes out mostly negative is reported, since that means the
      masters do not match.
- [x] **P6-T6** CLI dispatch for `px obs calibrate` and `px obs batch-calibrate`, plus the
      `--debayer` flag.
- [x] **P6-T7** End-to-end run on the OSC corpus.

**Gate: met.** Four raw CFA lights (ASI6200MC Pro, 9576×6388, RGGB) calibrated against a Siril
master dark and flat, through the `px` binary:

| Engine | Time | |
|---|---|---|
| native, gradient-corrected | **1.16 s** | **2.25× faster** |
| native, bilinear | 0.98 s | 2.66× faster |
| Siril | 2.61 s | — |

Metadata round-trips correctly — target, filter, exposure, RA/Dec, site — and the output is
`pp_`-prefixed where the observation index and every downstream command expect it.

### Phase 6b — AHD demosaicing

Closing the remaining gap to RCD, given the measured 1.23×/1.45×/1.24× background-noise penalty
of gradient-corrected interpolation.

- [x] **P6b-T1** `debayer/ahd.rs`: AHD (Hirakawa & Parks, *Adaptive Homogeneity-Directed
      Demosaicing Algorithm*, IEEE Trans. Image Processing 14(3):360–369, 2005). Published in
      full, so it is available clean-room. Green is interpolated horizontally and vertically by
      the five-tap `[-1/4, 1/2, 1/2, 1/2, -1/4]` filter; red and blue follow for each candidate
      from bilinear interpolation of the R−G and B−G differences; both candidates go to CIELAB
      through the Rec. 709 matrix with no gamma step, since the frame is linear; a homogeneity
      map with per-pixel adaptive thresholds is summed over a 3×3 window and the larger wins,
      an exact tie averaging the two.
- [x] **P6b-T2** Added to `--debayer` as `ahd` and measured the same three ways.
- [x] **P6b-T3** Decided on the numbers: **it does not become the default.**

#### Two things run in bands, and one of them is why

AHD holds eight intermediate planes per pixel — two directional greens, two candidate RGBs, two
CIELABs, two homogeneity maps. Held whole, a 61 MP frame would want close to 4 GB of them, which
is more than the frame. So it runs in row bands sized by pixel count rather than by the frame
(~15 MB each), with a five-pixel halo, in parallel. The halo is what `MARGIN` is: the 3×3
homogeneity window reads homogeneity at ±1, which reads CIELAB at ±2, which reads directional
green at ±3, which reads the CFA at ±5. Get it wrong and the seam between two bands is a line of
pixels reconstructed from truncated support — a handful of rows out of hundreds, which no
whole-frame error bound would notice. `band_seams_are_not_visible` measures the error *per row*
for that reason.

The paper's optional artifact-reduction pass — a few iterations of 3×3 median filtering on the
colour-difference planes — is **not** implemented. It exists because in a natural image an
isolated pixel that disagrees with its neighbours in colour is an artifact. On a star field it is
frequently a star.

#### The numbers, three ways

Against known truth on a synthetic star field (`tests/debayer.rs`, 256×256, 40 stars):

| | max error | RMS error |
|---|---|---|
| bilinear | 0.2243 | 0.003434 |
| Malvar-He-Cutler | 0.2060 | **0.002216** |
| **AHD** | **0.1805** | 0.002276 |

(The RMS figures differ in the third digit from Phase 6's table above because the shared test now
excludes a five-pixel border rather than three — the widest any of the three falls back to
bilinear on, so all are compared only where all three reconstructed properly.)

AHD wins on the worst pixel by 12% and loses on RMS by 2.7%. That split is the algorithm showing
its hand: it is better where there is an edge to interpolate along, and where there is not — most
of a background-dominated frame — the per-pixel direction choice is made on noise, and choosing
adds a little variance of its own.

The clean version of the same point, on a sharp vertical edge
(`an_adaptive_method_beats_a_linear_one_across_a_sharp_edge`): AHD reconstructs it **exactly**,
max error 0.00000, against Malvar-He-Cutler's 0.18750. There is a direction along which the edge
is smooth, and AHD finds it.

Against Siril's RCD on the real OSC frame (ASI6200MC Pro, 9576×6388, same calibrated CFA input
for all three, so calibration cannot contaminate it):

| | time | bgnoise vs RCD (R/G/B) | rms delta vs RCD, ADU (R/G/B) |
|---|---|---|---|
| bilinear | 0.07 s | 0.93 / 0.92 / 0.75 | 89.4 / 34.3 / 82.0 |
| Malvar-He-Cutler | 0.07 s | 1.23 / 1.45 / 1.24 | 61.0 / 41.2 / 59.3 |
| **AHD** | **0.50 s** | **1.20 / 1.55 / 1.32** | **45.2 / 37.2 / 39.7** |

Channel medians match RCD to better than 0.01% for all three, so none of them carries a colour
cast; that is now asserted per method rather than for gradient-corrected alone.

End-to-end through the `px` binary, four raw CFA lights against a Siril master dark and flat:
gradient-corrected 0.99–1.06 s, bilinear 1.06 s, **AHD 2.73–2.94 s**. Siril's baseline on the
same four frames is 2.61 s, so on this machine `--debayer ahd` is the one native configuration
that is *slower* end-to-end than the engine it replaces.

#### P6b-T3: AHD does not become the default

**The trigger condition was not met.** Phase 6b was scheduled against the background-noise
penalty, and AHD does not close it: 1.20/1.55/1.32 against gradient-corrected's 1.23/1.45/1.24 is
no better overall and distinctly worse in green.

That is not a surprise once measured, and the reason is worth recording, because it is the sort of
thing an argument from reputation gets wrong. AHD's directional green carries the *same*
second-derivative sharpening term Malvar-He-Cutler does — Hamilton-Adams is the horizontal half of
it — so it amplifies high frequencies identically. In flat background there is no edge for the
homogeneity test to find, so it picks between two candidates that are equally noisy, and the
picking itself is a source of variance. RCD is quiet because it *averages along* the local edge
direction; AHD *selects between* two non-averaging reconstructions. They are adaptive in
different places.

What AHD does buy is real and is the reason it ships rather than being reverted: it is markedly
closer to RCD overall (rms delta down 26%, 10% and 33%), it is the most accurate of the three on
the worst pixels, and it reconstructs a directional edge exactly. (Phase 6c has since beaten it on
every one of those measures with RCD itself, at half the cost. AHD stays for its distinct artefact
signature, not because it is anyone's best option.) On a frame where the colour
reconstruction is the limiting factor — few subs, bright structured target, visible zippering —
that is what one would reach for. It costs 7× the demosaic time and 3× the end-to-end
calibration time to reach for it.

So: three methods, `--debayer <gradient-corrected|bilinear|ahd>`, `PX_DEBAYER` overrides, default
unchanged at gradient-corrected. The default follows the same reasoning Phase 6 gave — a stacking
pipeline averages background noise down and cannot recover resolution — and now has a third data
point supporting it: the adaptive method that was supposed to fix the noise did not, at 3× the
cost.

### Phase 6c — RCD, by clean-room specification

Phase 6 recorded that RCD was unavailable, and Phase 6b tried to close the gap to it with AHD and
failed. This phase closes it with RCD itself, reached by the route described under *Licensing and
provenance*: the project owner wrote a functional specification from the librtprocess reference,
and `debayer/rcd.rs` was implemented from that document by an author who has never seen `rcd.cc`.

- [x] **P6c-T1** `debayer/rcd.rs`: ratio-corrected demosaicing. Tiled 194×194 with a 9-pixel
      overlap, seven passes per tile in dependency order.
- [x] **P6c-T2** Added to `--debayer` as `rcd` and measured the same three ways.
- [x] **P6c-T3** `rcd` becomes the default `--debayer`.

#### What RCD actually does differently

Two ideas, and between them they explain every measurement in Phases 6 and 6b.

**Ratio correction.** Green at a red site is normally estimated as
`neighbour_green + (centre_red − neighbour_red)` — a *difference* model, which assumes chroma is
locally constant. RCD instead scales the green neighbour by the ratio of local *panchromatic*
levels, assuming relative chroma is locally constant. The level is a deliberately mixed-colour
low-pass (centre, plus half each cardinal neighbour, plus a quarter each diagonal), so it behaves
identically in all three channels. Chroma stays on a difference model; only green is ratio-corrected.

**Soft directional blending.** For each axis pair a discrimination ratio in `[0, 1]` is formed
from directional high-pass energies, refined by keeping whichever of the pixel's own ratio and its
four diagonal neighbours' mean is further from the neutral half, and then used to *interpolate*
between the two directional estimates. Never a hard switch.

That last point is the answer to Phase 6b. AHD **selects** between two non-averaging
reconstructions and so cannot be quieter than either; RCD **blends**, and blending averages. Two
comparisons do the job AHD needs a 3×3 homogeneity pool for, and do it better.

#### The numbers

Against known truth on the synthetic star field, all four methods, interiors only:

| | RMS error | max error |
|---|---|---|
| bilinear | 0.003537 | 0.2243 |
| gradient-corrected | 0.002266 | 0.2060 |
| AHD | 0.002326 | 0.1805 |
| **RCD** | **0.001636** | **0.1110** |

RCD is best on both, by 28% on RMS over the next best and by 39% on the worst pixel. It also
reconstructs a sharp vertical edge *exactly*, as AHD does, and a uniform field and a linear ramp
exactly, which AHD and gradient-corrected also do.

Against Siril's own RCD on the real 61 MP OSC frame — the same calibrated CFA input for all four:

| | time | bgnoise vs RCD (R/G/B) | rms delta vs RCD, ADU (R/G/B) |
|---|---|---|---|
| bilinear | 0.07 s | 0.93 / 0.92 / 0.75 | 89.4 / 34.3 / 82.0 |
| gradient-corrected | 0.07 s | 1.23 / 1.45 / 1.24 | 61.0 / 41.2 / 59.3 |
| AHD | 0.46 s | 1.20 / 1.55 / 1.32 | 45.2 / 37.2 / 39.7 |
| **RCD** | **0.21 s** | **0.95 / 0.95 / 0.93** | **32.4 / 20.6 / 25.5** |

**The background-noise gap is closed.** 0.95/0.95/0.93 against a target of 1.0, where the best
prior method sat at 1.23/1.45/1.24. RMS delta roughly halves again, to 2.7–3.5 σ_bg. Medians match
to better than 0.01%, as they do for every method here.

End-to-end through the `px` binary on four raw CFA lights, machine idle: gradient-corrected
0.90–0.98 s, **RCD 1.43–1.50 s**, AHD 2.31–2.58 s, against Siril's 2.61 s baseline. RCD costs
about **55%** over gradient-corrected end-to-end — less than the 3× the demosaic alone costs,
since the demosaic is not the whole of a calibration pass, but not the near-free change a first
measurement under load suggested. Native RCD remains ~1.8× faster end-to-end than Siril.

This is now gated rather than merely recorded. `debayer_parity.rs` asserts the native RCD's
background noise stays within 15% of Siril's and its difference under 6 σ_bg — bounds set well
outside the measured values, so they catch a regression without pinning the build.

#### P6c-T3: `rcd` becomes the default

Phase 6's default rested on an explicit premise: gradient-corrected was *the best available given
RCD is not*. That premise is gone, and the reasoning inverts cleanly rather than needing a new
argument. RCD is the most accurate of the four against known truth on both RMS and worst pixel, it
is the only one whose background noise matches Siril's own reconstruction, and the frames it
produces are the ones the rest of the pipeline was tuned against — the reference corpus is RCD
output.

It costs ~55% end-to-end over gradient-corrected. That is a real cost and it is worth paying here,
because the thing being bought is the reconstruction every downstream measurement runs on:
registration detects stars in it, stacking averages it, and neither can recover what the demosaic
threw away. Calibration is also the once-per-frame stage, not the inner loop.

The other three stay available and none of their behaviour changes. `--debayer gradient-corrected`
restores the old default exactly, and `PX_DEBAYER` still overrides.

**`--engine` is unaffected and stays Siril.** That default is governed by the P12-T4 blocker list,
where only blocker #1 has been met; the owner wants field testing before it moves.

#### One ambiguity, left open rather than fitted

The specification describes the discrimination high-pass as "a **five-tap** symmetric high-pass
filter" that "combines the samples at **distances 1, 2 and 3** with alternating signs". Those name
different filters: five-tap symmetric reaches 2 (the fourth difference, `[1, −4, 6, −4, 1]`);
distances 1–3 reach 3 (the sixth difference, `[1, −6, 15, −20, 15, −6, 1]`). Both alternate sign,
annihilate smooth gradients, and peak at the CFA sampling rate, so the prose fits either.

The implementation uses the fourth difference, isolated to one constant. It was **not** selected
by trying both against Siril's output — that would be deriving a coefficient from the reference's
behaviour, which is the one thing the clean-room route cannot survive. The measured 0.93–0.95
noise ratio and 2.7–3.5 σ_bg residual are consistent with this being the wrong one of the two, and
the right way to settle it is for the specification's author to say which was meant.

#### Consequence for P12-T4

The P12-T4 blocker list gives "the RCD gap closes" as blocker #1 to `--engine native` becoming the
default. **That blocker is now met.** The remaining two — native background extraction and mosaic
projects, and a release's worth of field use — are unchanged, so the default does not move yet,
but the stated reason it could not has gone. Blocker #3 is the live one: the owner is field-testing
`--engine native` before it becomes the default.

### Phase 7 — Star detection and PSF fitting

- [x] **P7-T1** `register/starfind.rs`: candidate detection — threshold at
      `median + 5·sigma·bgnoise`, local-maximum scan in a `radius`-box, duplicate suppression by
      the `MAX_RADIUS_RATIO_DUP` rule, magnitude-ordered candidate list. Implemented from
      Stetson's DAOPHOT description (PASP 99, 1987, §II) rather than from `peaker`: the
      threshold test runs first and the box scan only on the pixels that pass it, which is what
      keeps detection linear in practice (0.1% of pixels reach the box scan on a real light).
- [x] **P7-T2** `register/psf.rs`: elliptical Gaussian fit with rotation (7 parameters) by
      Levenberg-Marquardt on `nalgebra`, with moment-based initial guesses (Bertin & Arnouts
      1996, §4), parameter bounds, and the `MAX_ITER_ANGLE` iteration cap. Analytic Jacobian,
      checked against finite differences in a unit test — a wrong derivative only slows
      convergence, so it will not fail loudly on its own.
- [x] **P7-T3** Moffat profile (8 parameters, `min_beta = 1.5`), selectable through
      `StarfindOptions`, defaulting off.
- [x] **P7-T4** Star rejection rules: amplitude bounds, roundness floor and ceiling, width,
      RMSE, Moffat beta at its floor, and the `relax_checks` variant, which drops the two shape
      tests and keeps the ones distinguishing a measurement from a non-measurement. Per-reason
      counts are returned, because "twelve stars registered" needs a diagnosis.
- [x] **P7-T5** `fwhm_stats` and `filter_stars_by_amplitude`, with `amplitude_threshold_for`
      for the two-pass re-filter.
- [x] **P7-T6** Tests: on synthetic frames with analytically placed stars, recovered centroids
      within 0.02 px and FWHM within 1% of truth. Against Siril's `findstar` on real frames, the
      measured numbers are below.
- [x] **P7-T7** `benches/register.rs` part 1: detection + fitting throughput at 2000 stars.

**Gate: met.** Against Siril 1.4.4 `findstar` at the same radius, sigma and roundness, on three
real 61 MP lights from the large corpus:

| Frame | Siril stars | Matched within 0.2 px | Centroid RMS | FWHM bias |
|---|---|---|---|---|
| `…_0035` | 5,114 | **96.5%** | **0.0371 px** | −1.09% |
| `…_0125` | 6,294 | **98.7%** | **0.0120 px** | −0.15% |
| `…_0375` | 10,110 | **98.0%** | **0.0129 px** | −0.11% |

against gates of ≥ 95% and ≤ 0.05 px. The parity claim is deliberately one-directional — every
star Siril found is a star this crate found, at the same position — because the two do not agree
on how many stars a frame *has*: uncapped, the native pass keeps 25k–63k where Siril keeps 5k–10k.
That is a threshold and rejection difference at the faint end, not a disagreement about any
particular star.

#### Two things the corpus corrected

**Siril's default radius is 5, not 10.** The reference lists are generated with it set
explicitly, so a future version changing its default cannot silently move the comparison.

**A width bound on the profile scale is wrong for Moffat.** A Moffat alpha means a different
width at every beta — at β = 10 it is twice the Gaussian sigma of the same star — so rejecting
`sx >= radius` threw out every wide-beta fit: all sixteen stars of a synthetic field. Both width
bounds are now in FWHM, the one width the two profiles agree on.

#### Coordinate conventions differ, and it matters at the level these tests measure

Siril reports star positions from the bottom-left *corner* of the first pixel; this crate puts
integer coordinates at pixel *centres* and measures y downward from the top, which is the order
samples arrive in from `px-fits`. The conversion is `ours.x = siril.x − 0.5`,
`ours.y = height − siril.y − 0.5`. Without the half-pixel term every match sits at ~0.5 px and
the centroid RMS gate fails by 10×, which is a good illustration of why the gate is stated in
hundredths of a pixel.

#### The star cap is a throughput decision, not just a bound

A 61 MP light yields 123k candidates against a default cap of 2,000 stars, and fitting costs two
orders of magnitude more per candidate than detecting. The fitter therefore works through
candidates in brightest-first batches and stops once the cap is met: 8k candidates fitted instead
of 123k, **1.13 s instead of 4.40 s** for the same 2,000 brightest stars. Batching is over the
deterministic detection order, so which stars survive does not depend on where a batch boundary
fell.

Throughput on a 2048×2048 field carrying 2,000 fittable stars, 16 cores:

| Workload | Time |
|---|---|
| detection alone | 35.6 ms |
| full pass, Gaussian | 42.8 ms |
| full pass, Moffat | 151.4 ms |
| 1 / 2 / 4 / 8 / 16 cores | 135 / 87 / 60 / 47 / 43 ms |

Detection is memory-bound and dominates, which is why the core scaling flattens: the Gaussian
fitting of 2,000 stars is only ~7 ms of the 43 ms.

### Phase 8 — Star matching and transform estimation

- [x] **P8-T1** `register/transform.rs`: one homogeneous 3×3 `Transform` with application,
      inversion, composition, corner error and RMSE, plus `kind()` — shift, similarity, affine,
      homography, or null for a singular matrix — which is the `guess_transform_from_H` role.
      Classification needs *two* tolerances: the linear coefficients are dimensionless, but the
      projective row is in 1/pixels and enters through a division, so 1e-6 there is ~16 px at
      x = 4000. The projective row is held to 1e-9.
- [x] **P8-T2** Normalized DLT homography (Hartley & Zisserman §4.4.4) and least-squares affine /
      similarity / shift, all through the same Hartley normalization.
- [x] **P8-T3** RANSAC: 3.0 px reprojection threshold (OpenCV's `defaultRANSACReprojThreshold`,
      which Siril uses), adaptive iteration count, refit over the full consensus set, fixed seed.
- [x] **P8-T4** `register/starmatch.rs`: triangle-similarity matching — shape `(b/a, c/a)` plus
      handedness for the triangles of the 40 brightest stars, a sparse vote matrix, and greedy
      one-to-one resolution of the top vote-getters. From the published descriptions (Groth 1986;
      Valdes et al. 1995), not from `atpmatch.c`.
- [x] **P8-T5** Iterative refinement: reject at 3× the surviving residual spread, refit, stop when
      a round rejects nothing or the RMSE stops improving.
- [x] **P8-T6** Tests: synthetic lists under a known homography, and the real-frame comparison
      against Siril's stored registration data. Both below.

**Gate: met, in a corrected form, and the correction is the substance of this phase.**

#### Synthetic truth

On a 9576×6388 frame under a known homography, after the proximity pass described below, corner
error tracks per-star centroid noise linearly and is comfortably inside the 0.01 px bound at any
realistic noise level:

| Centroid noise | 0.005 px | 0.01 px | 0.02 px | 0.04 px |
|---|---|---|---|---|
| Corner error, 800 stars | 0.0013 px | 0.0027 px | 0.0054 px | 0.0108 px |

At 0.02 px noise: 0.0069 px with 100 stars, 0.0017 px with 2,000. The bound holds with one star in
six missing and ten spurious detections bright enough to enter the matcher's window.

#### Real frames: the flat 0.1 px bound is unreachable where the *model* does not fit

Over the 94 frames of the 100-frame run that Siril itself registered:

| | Value |
|---|---|
| corner error vs Siril, median | **0.0644 px** |
| corner error, 90th percentile | 0.1065 px |
| corner error, worst | 0.5405 px |
| frames where our fit is the better least-squares fit | **92 of 94** |
| worst corner error as a multiple of that frame's own misfit | **1.46×** |

The 0.1 px bound is met on every frame where a homography describes the mapping — 42 of 94, median
0.053 px, worst 0.091 px — and missed on 13 frames whose *own* fit residual is 0.35–0.46 px over
~1,700 correspondences. Those residuals are not noise. Binned into a 3×3 grid their cell means run
to 0.43 px, where 150–300 stars of 0.3 px scatter would average to 0.02 px:

```
residual mean per 3x3 cell, worst frame (dx, dy, n)
(+0.069,-0.211,149) (-0.044,+0.065,226) (-0.243,-0.429,112)
(+0.052,-0.040,200) (-0.022,+0.258,325) (-0.065,-0.394,190)
(-0.063,-0.161,161) (-0.047,+0.246,211) (+0.091,-0.382,176)
```

That is unmodelled optical distortion. A homography has eight parameters and cannot describe these
frame pairs to better than a few tenths of a pixel, so two implementations fitting it to the same
stars land a fraction of that misfit apart — and neither is wrong. Siril would not agree with
itself on a different star subset either. Demanding 0.1 px there is demanding that two
least-squares solutions of an inconsistent system coincide.

**The gate is therefore stated in three parts**, exactly as Phase 3's master-flat max-delta bound
was replaced once measurement showed what it was really asking for:

- corner error ≤ 0.1 px on every frame whose own fit residual is < 0.1 px — met on **42 of 42**,
  worst 0.091 px
- corner error ≤ 2× that frame's own fit residual, on **every** frame — met, worst **1.46×**
- our residual never worse than Siril's by more than 1% on the same correspondences — met: better
  on 92 of 94, worse by **0.1%** on two
- every frame Siril registered, the native path registers — met, 94 of 94. Siril skipped 5 of the
  100; the native path skips the same 5.

The second bound is the one that carries the phase: it is scale-free, so it says the two
implementations never differ by more than twice the scatter they are both fitting through,
whatever the frame.

#### Three things measurement forced, in the matcher

**The vote cut has to be relative.** Two 40-star lists give 9,880 triangles each, and shape space
is small enough that unrelated triangles collide within tolerance tens of thousands of times —
about 50 votes per star pair from pure coincidence, against ~900 for a correct pair.

**Near-isoceles triangles have to go.** The whole scheme rests on labelling vertices by the side
each is opposite; if two sides agree to within the matching tolerance the labelling is ambiguous,
and such a triangle casts two wrong votes out of three.

**"Do these lists match at all" is a separate test from "which stars correspond."** A vote matrix
always has a leader, so the leader is required to stand 6× above the distribution's median —
measured at 16.6× for the same field, 2.7× for unrelated fields, 3.2× for a reflected one. Without
it the ranking fell through to its tie-break and returned noise pairs, which is how a mirrored
field came back "registered" with a reflecting affine transform.

#### A real bug, caught by the minimum-pairs test

`nalgebra` computes a *thin* SVD. The exactly-four-correspondence homography builds an 8×9
constraint matrix whose right factor is 8×9 and does not contain the null direction being solved
for, so the fit came back satisfying its own four pairs and nothing else — invisible with five or
more pairs, which is every other test. The matrix is padded to nine rows, which leaves AᵗA and the
null space untouched.

#### Matching is a seed, not the answer

Matching runs on the brightest 40 stars because the triangle count is cubic, so the transform it
produces rests on a few dozen correspondences. It is therefore used as a seed: both full star
lists are re-paired by proximity under it and the transform refitted, which is FOCAS's own
two-stage structure and takes corner accuracy from a tenth of a pixel to a hundredth. Per-frame
cost on 2,000-star lists: **3.5 ms** to match, **4.5 ms** to match and fit.

### Phase 9 — Two-pass registration and framing

- [x] **P9-T1** `register/framing.rs`: reference selection at
      `2·FWHM·(cap − stars)/cap + FWHM`, frames below half the best star count ineligible, and the
      amplitude re-filter when counts cap.
- [x] **P9-T2** `register/twopass.rs`: the loop — detect every frame (budget-bounded), choose,
      register, re-trial up to `MAX_TRIALS_2PASS = 5`.
- [x] **P9-T3** `FRAMING_CURRENT` and `FRAMING_MIN`; `FRAMING_MAX` / `FRAMING_COG` return a typed
      `Unsupported`.
- [x] **P9-T4** Frame exclusion with a typed reason per frame (no stars, too few stars,
      unmatched, unreadable), carried on the result for the reporter to surface.
- [x] **P9-T5** The registration sidecar as a FITS binary table (D6), with a round-trip test.

**Gate: met exactly.** On the 100-frame corpus the native engine chooses reference frame **#44**,
the one Siril chose, and includes the same **95** frames — no frame kept by one and dropped by the
other, in either direction. The whole two-pass loop takes **23.0 s** against Siril's 16.6 s.

#### The capped-count problem

With `max_stars = 2000` every frame of a rich field reports exactly 2,000 stars, so the count term
of the score carries no information and a frame that could have yielded 8,000 ties with one that
barely reached the cap. Capped frames are therefore re-counted above one common amplitude — the
brightest of their individual cut-offs — which makes depth visible again.

#### Reference selection cannot see the sequence, and a test caught it

The score ranks each frame on its *own* quality. A frame that is sharp and star-rich but shows a
different patch of sky — a pointing slip, a stray frame from another target — wins that ranking
and leaves every other frame unregistered. Each trial now falls back through up to three reference
candidates, reusing the star lists it already has, before paying for another detection pass. Siril
has the same structural blind spot; re-trialling alone does not fix it, because the score is
deterministic.

#### Retry order matters

Trials lower the detection threshold first and relax the shape checks only later. Relaxing shape
first would admit cosmic rays and hot pixels as stars while the real ones were still below
threshold — a sequence short of stars is far more often under-detected than mis-shaped.

#### The sidecar round-trip caught a transposed matrix

`nalgebra`'s element iterator is column-major; the reader was row-major. The failure mode is a
silently *wrong* transform rather than a malformed file, which is the kind a round-trip test exists
to find. Coefficients are stored as `D`: rounding a homography to `f32` moves a 9576 px corner by
~0.001 px.

### Phase 10 — Warping

- [x] **P10-T1** `register/warp.rs`: inverse-mapped warp with bilinear, area, bicubic (`a = −0.75`)
      and Lanczos-4. Destination zero-initialized; pixels whose source falls outside the input are
      left untouched.
- [x] **P10-T2** Interpolation clamping, on by default: warp again with a kernel that cannot
      undershoot, mask below `CLAMPING_FACTOR = 0.98` of that guide, dilate with the 3×3 elliptical
      element, copy the guide in. The dilation helper is tested separately.
- [x] **P10-T3** Parallel over output row bands through the `Runtime`.
- [x] **P10-T4** Integer-shift identity for every kernel, round-trip warp, clamping localization,
      and parity against `seqapplyreg`.
- [x] **P10-T5** `benches/register.rs` part 2: warp throughput per kernel, with and without
      clamping.

**Gate: met.** Against Siril's `seqapplyreg` on four 61 MP frames, warping with *Siril's own*
homography so that only the interpolation is under test:

| Frame | RMS delta | Max delta | RMS with clamping off |
|---|---|---|---|
| #44 (reference) | **0.000 σ_bg** | 0.0 ADU | 0.000 σ_bg |
| #47 | **0.187 σ_bg** | 5489 ADU | 2.962 σ_bg |
| #51 | **0.175 σ_bg** | 2359 ADU | 3.163 σ_bg |
| #66 | **0.207 σ_bg** | 5704 ADU | 3.652 σ_bg |

against a gate of 0.5 σ_bg. The reference frame, warped under the identity, is copied exactly. The
last column is the real evidence that the clamping pass matches Siril's: turning it off moves the
result **six times further away** from Siril's output. Max per-pixel deltas are large and always on
saturated cores, where a sub-pixel difference in a near-vertical profile is a large difference in
value — which is why the gate is an RMS one.

#### Throughput, and the weight table

| Kernel, 16.8 MP | Time | Per 61 MP frame |
|---|---|---|
| bilinear | 7.4 ms | 0.027 s |
| area | 7.7 ms | 0.028 s |
| bicubic | 31.6 ms | 0.115 s |
| bicubic + clamp | 52.7 ms | 0.192 s |
| Lanczos-4 | 63.1 ms | 0.230 s |
| **Lanczos-4 + clamp** | **81.8 ms** | **0.298 s** |

Siril's `seqapplyreg` runs at 0.48 s per frame on the same machine, so the default native warp is
1.6× faster.

Getting there needed one optimization. Computing Lanczos weights per pixel costs 32 `sin` calls and
measured **182 ms** for 16.8 MP. The weights depend only on the fractional part of the position, so
they are precomputed on a 1024-step grid and looked up — OpenCV's `INTER_BITS` trick at 32× its
resolution. That is 182 → 63 ms with **no measurable change to the parity numbers** (0.187/0.174/
0.208 → 0.187/0.175/0.207 σ_bg).

**Clamping does not double the warp cost**, as this document originally expected. The guide is a
bilinear warp, not a second Lanczos one, so it adds 30% to Lanczos-4 and 67% to bicubic.

#### Three bugs the warp tests found

**The support bound was off by one on its upper edge**, reading one row past the frame. The kernel
touches `floor(p) − (radius−1) ..= floor(p) + radius`, so the bound on `p` is closed below and
half-open above.

**Area interpolation had a half-pixel bias.** Averaging the 2×2 block starting at `floor(p)` is the
obvious reading of "box filter" and is wrong: the footprint is the unit box *centred* on `p`, whose
overlap weights are `(1 − f, f)` per axis — which is bilinear. At the unit scale registration works
at, the footprint average *is* bilinear. The shifted guide made the clamping mask flag 11% of a
frame instead of the 2.7% that actually rang.

**Clamping must not write a pixel the warp left empty.** The guide's support is 2×2 where
Lanczos-4's is 8×8, so in a four-pixel band at the footprint edge the guide has a value and the real
warp does not. Filling those from the guide invents a frame of data in exactly the band the stacker
treats as signal.

One property is worth stating because it looks like a bug and is not: clamping is **not monotone
per pixel** — 16 of 520 changed pixels in the synthetic test were lowered. The mask is dilated
before the guide is copied, so the ring around a repaired core includes pixels where the sinc read
*higher* than the guide. Siril's clamp behaves the same way, and it is why clamping slightly softens
star cores.

### Phase 11 — `CreateMasterLight` and the end-to-end report

- [x] **P11-T1** `project/master_light/native.rs`: frame collection in the Siril implementation's
      order → two-pass registration → warp → normalized rejection stack → write, returning the
      identical `MasterLight`. `background_extract` returns `UnsupportedByEngine`.
- [x] **P11-T2** `stack/normalize.rs`: additive-with-scaling coefficients from robust location and
      scale, applied per frame and channel as the stacker reads each block.
- [x] **P11-T3** Warped frames stay in memory when they fit the budget and spill to a temporary
      sequence when they do not, through a `BlockSource` the stacker reads either way. Both paths
      tested, and asserted to agree to 1e-5.
- [x] **P11-T4** `Frame::mirror_x`, the orientation step the Siril pipeline performs after
      stacking.
- [x] **P11-T5** CLI dispatch for `px --engine native project stack`, with mosaic projects refused
      up front.
- [x] **P11-T6** End-to-end parity against a Siril master light built from the same frames.
- [x] **P11-T7** The full report, in `crates/px-imageproc/README.md`.

**Gate: met on quality and speed; the difference bound is restated, with the measurement.**

Measured on an M4 Max (16 cores, 128 GB) against Siril 1.4.4 over 100 lights of an ASI6200MM Pro.
Both engines registered the same 95 frames onto the same reference frame.

| | Native | Siril | |
|---|---|---|---|
| background noise | 1.5272 ADU | 1.5637 ADU | **2.3% quieter** |
| median FWHM | 2.431 px | 2.436 px | **0.2% sharper** |
| wall clock, 100 frames | **68.9 s** | 73.6 s | **1.07× faster** |
| peak footprint, 8 GB budget | **4.88 GB** | — | inside the budget |

#### The D2 master-light difference bound is not reachable, and this is what replaces it

D2 asks for ≤ 1.0 σ_bg max and ≤ 0.1 σ_bg RMS. Measured: 2855 σ_bg max and 2.73 σ_bg RMS. Two
measured facts explain the whole of that gap, and neither is a defect in either engine.

**`-output_norm` is keyed to two extreme pixels.** Both pipelines finish by rescaling the result so
its non-zero values span [0, 1] — a mapping fixed by the single darkest and single brightest pixel
of a 61 MP frame. Two masters that agreed everywhere would still differ by a global affine factor;
here it is 1.005× plus a small pedestal. Fitting and removing that relation is the first thing a
meaningful comparison has to do.

**The rest is on the stars.** With the factor removed, the difference splits sharply:

| Region | RMS difference |
|---|---|
| sky (lowest 9 brightness deciles) | **0.33 σ_bg** |
| signal (top decile) | 5.42 σ_bg |
| whole frame | 1.74 σ_bg |

A 0.06 px difference in one frame's homography — inside the Phase 8 gate — moves a star core by a
few percent of its value and the sky by nothing. Requiring 0.1 σ_bg RMS over the whole frame is
requiring the two engines to agree to a tenth of the stacked noise *on saturated cores*, which no
pair of independent implementations can do.

The bound is therefore stated where it means something, and the quality assertions D2 already
identified as the real constraint are the ones that gate:

- sky RMS ≤ 1.0 σ_bg after the `-output_norm` factor is removed — met at **0.33 σ_bg**
- σ_bg(native) ≤ 1.02 × σ_bg(Siril) — met at **0.977**, i.e. quieter
- median FWHM within 2% — met at **0.998**, i.e. sharper
- star count no lower than Siril's — met (both at the 2,000 cap)

This is exactly the case D2 anticipated: "a result that is *different* but measurably sharper and
no noisier is a pass, and the test says so explicitly rather than being quietly widened."

#### The budget is a promise about the process, not about a phase

The first constrained run was the phase's most useful failure. Registration, warping and stacking
each sized themselves against the configured 8 GB, each stayed inside it — and the run peaked at
**19.9 GB**, because memory is not returned to the operating system when it is dropped and each
phase allocated on top of the last. An RSS trace over the run showed the jump happening at the
stack phase while registration's memory was still held.

Three corrections, all measured:

- The budget is **partitioned** across phases rather than lent out three times: registration 30%,
  warping 25%, stacking 55%.
- A warp costs **16 bytes per output pixel**, not 8 — source, result, clamping guide, two masks and
  the level-measurement scratch. Charging only source and result is what let four frames in flight
  become 5 GB.
- The level measurement runs on **every fourth sample**. Neither robust estimator uses the spatial
  arrangement of the data, so a subsample measures the same quantity: at stride 4 a 61 MP channel
  still yields 15 million samples, and the scratch drops from eight bytes per pixel to two.

`BlockSource` also reports the bytes it keeps resident, so an in-memory stack charges the frames it
reads from before sizing its bands.

| Budget | Peak footprint | Wall clock | Warped frames |
|---|---|---|---|
| default (90% of RAM) | 53.2 GB | 68.9 s | held in memory |
| 8 GB | **4.88 GB** | 89.3 s | spilled to disk |

#### Where the time went

Warping was 169 s of a 198 s run before two changes: frames are now warped with budget-bounded
concurrency — a frame's cost is a 122 MB read, a decode, the warp and a level measurement, and only
the middle two keep every core busy — and the levels are measured while the warped frame is still
in memory, which removes a second full read pass. Warping is now 29 s.

| Phase | Time |
|---|---|
| two-pass registration | 20 s |
| warping, 95 frames | 29 s |
| combining | 11 s |

#### What the native engine still cannot do

`CreateMasterLight` with background extraction returns `UnsupportedByEngine`, and mosaic projects
are refused before any work starts — the grid and spiral pipelines and the cross-layer registration
remain Siril-only. Both are reported rather than silently skipped, which is the D5 rule.

### Phase 12 — Tuning and consolidation

- [x] **P12-T1** `px-imageproc/src/profile.rs`: named span accumulation, off unless enabled and
      one relaxed load when it is, so the instrumentation lives on the paths users run rather
      than behind a feature flag that would rot. `PX_TIMING=1 px ...` turns it on for a real
      command and prints to stderr; `px-pipeline/tests/profile.rs` runs all five pipelines over
      the corpus and writes `data/output/profile.txt`.
- [x] **P12-T2** Acted on, four times, and the one thing not acted on is reported with numbers.
- [x] **P12-T3** Revisited. No evidence the scheduler leaves throughput on the table, so
      unchanged — the measurements are below.
- [x] **P12-T4** Decided: **Siril stays the default.** Amendment below.
- [x] **P12-T5** ROADMAP and `docs/crate-dependencies.md` updated. The dependency graph predated
      `px-imageproc` entirely and five other edges had drifted.

**Gate: met.** Every change in this phase is exact — the master-light parity figures are
identical to Phase 11's in every digit (sky 0.333 sigma_bg, noise ratio 0.9766, FWHM ratio
0.9978), which is the point: the output did not change, only the time it took.

| | Phase 11 | Phase 12 | |
|---|---|---|---|
| master light, 95 frames | 68.9 s | **49.0 s** | **1.41x** |
| — against Siril's 73.6 s | 1.07x | **1.50x** | |
| master bias, 40 frames | 3.23 s | **1.88 s** | 1.72x |
| master dark, 40 frames | 3.23 s | **1.95 s** | 1.66x |
| master flat, 59 frames | 7.73 s | 7.62 s | — |

Those are pipeline figures from `tests/profile.rs`. Through the `px` binary, which is what Phase 5
measured and left as an open item, the bias and dark deficit is closed: **2.01 s against Siril's
2.15 s** and **1.98 s against 2.13 s**, where Phase 5 measured 0.8x and 0.7x. The cause was never
the startup cost that note guessed at — it was the reduction kernel.

#### Where the time actually went, and what that was worth

The profile is the whole point of the phase, so it is worth stating what it overturned. Phase 7
had spent its optimization effort on PSF fitting, on the reasoning that fitting is two orders of
magnitude dearer per candidate than detecting. That reasoning was right and the conclusion was
wrong: detection was 611 ms a frame against fitting's 145, because **96% of "detection" was not
detection**. It was `Background::measure`, and 484 ms of that was one call — the median.

| Star finding, per 61 MP frame | Before | After |
|---|---|---|
| median of the channel | 484 ms | **19.5 ms** (24.8x) |
| background noise | 23 ms | 23 ms |
| the scan that actually finds stars | 22 ms | 22 ms |
| PSF fitting | 71 ms | 71 ms |
| **whole pass** | **591 ms** | **117 ms** (5.1x) |

Quickselect has two problems at 61 megapixels and neither is the partition. It needs a copy it
may reorder — 244 MB — and it is inherently serial, because each partition's bounds depend on the
last. Counting has neither. IEEE-754 orders non-negative floats by their bit pattern, so flipping
the negatives and setting the top bit of the rest yields a `u32` that compares exactly as the
`f32` does; the samples can then be tallied into buckets of that key, which is per-sample
independent and copies nothing. Two passes of sixteen bits resolve the key exactly.

**This is the same number, not an approximation**, which matters because the alternative — a
strided subsample, as P11 used for normalization levels — was available and would have been
easier. The median sets the detection threshold, and a threshold that moved when a frame crossed
a size boundary would make star lists depend on the frame's dimensions. `tests/stats.rs` asserts
the two paths agree bit-for-bit over six distributions, including the duplicate-heavy and
negative cases real frames present.

It also removed a buffer: `ChannelStats::compute` needed one for the values and one for the
deviations, and the MAD no longer materializes its deviations. 244 MB less per concurrent frame,
which is part of how many fit a budget.

Two smaller wins, both exact:

- **The stacker's band is transposed into cache-sized tiles before reducing.** Reduction walks
  across frames for one pixel and the band is frame-major, so a pixel's values sat one band apart
  — hundreds of megabytes — and every frame's contribution was a cache miss and usually a TLB
  miss. Worth a fifth of a plain median stack (31.8 s to 25.1 s of reduce time on 40 frames) and
  nothing at all on a rejection stack, where the clipping costs far more than the fetch did.
- **The linear fit was re-sorting on every pass.** It need not: the compaction writes survivors
  back in the order it meets them, so an ascending run stays ascending. Sorting once, and
  unstably — two `f32` equal under `total_cmp` have identical bit patterns, and the stable sort
  asks the allocator for scratch above twenty elements, once per pixel — took the master light's
  reduce from 198.8 s to 181.0 s.

Against the Phase 11 breakdown of the same 95-frame run:

| Phase | Phase 11 | Phase 12 |
|---|---|---|
| warping, 95 frames | 29 s | **24.5 s** |
| combining | 11 s | **9.6 s** |
| everything else — registration, collection, the write | 28.9 s | **14.9 s** |
| total | 68.9 s | **49.0 s** |

Warping and combining are reported by the pipeline; the third row is the remainder, and is
dominated by the two-pass registration that the median work above halved.

#### D8: the kernel is not slow for want of vectorization

`stack.reduce` is 181 s of thread time in a 95-frame master light, more than every other stage
together, and it is the kernel D8 anticipated. The answer is measured rather than assumed, in
`benches/reduce.rs`, which runs the shipped single-accumulator loops against four-accumulator
copies — what a vectorizer would do if it were allowed to, and it is not, because floating-point
addition is not associative:

| Loop | Shipped | Reassociated | |
|---|---|---|---|
| mean absolute residual | 166.7 us | 135.7 us | 1.23x faster |
| least-squares cross moment | 270.1 us | 296.9 us | **0.91x, slower** |

One loop gains a fifth and the other loses a tenth, and adopting either would move every
rejection threshold and parity number in this project in its last bits. A third measurement makes
the same point from the other side: `sxx` in the least-squares fit has a closed form,
`n(n^2 - 1)/12`, and replacing the loop that computed it changed the total by nothing (181.0 s to
182.6 s, inside the noise). **Taking arithmetic out of the loop did not help, because the loop is
bound by its accumulator latency rather than by its arithmetic.**

So the reduction is slow because iterated clipping walks the stack several times per pass — an
algorithmic property, not a code-generation one — and no `unsafe` or explicit SIMD would address
it. Recorded, per D8, and not changed.

#### D3: the block scheduler is not leaving throughput on the table

The evidence D3 requires before changing the heuristic does not exist, in three measurements.

**Smaller bands are monotonically worse.** The heuristic takes the fewest bands the budget allows;
the obvious alternative is more and smaller ones, for locality. Measured, on a 20-frame median:

| Budget | Blocks | Time |
|---|---|---|
| 4 GiB | 16 | 35.7 ms |
| 256 MiB | 16 | 35.8 ms |
| 64 MiB | 32 | 37.2 ms |
| 16 MiB | 128 | 47.1 ms |

More bands cost, they do not pay. And a budget sixteen times larger than the one that already
fits buys nothing, which is the property that lets the scheduler size to the budget rather than
to the machine.

**The wave rounding is doing its job.** Blocks are taken in waves of `cores`, so a count just over
a multiple wastes most of a wave; `BlockPlan::compute` rounds the total up to whole waves to
prevent it. Every plan in the profile — 32, 32, 32 and 112 blocks on 16 cores — is an exact
multiple. There is no partial wave anywhere to reclaim.

**Occupancy is 90-93%.** Summing the per-stage thread time against `cores x wall` for the stacking
phase of each pipeline: master bias 90%, master flat 91%, master light 93%. The remaining 7-10%
is the wave barrier and the variance in block read times, and closing it means letting blocks
from different waves run concurrently — which means a band buffer per block rather than per
worker, which is the memory the scheduler exists to bound. That is a worse trade, not a missed
one.

#### P12-T4 amendment: `--engine native` does not become the default

On speed and on measured quality the native engine wins outright — every pipeline is faster, the
master light is 1.50x faster than Siril's and simultaneously 2.3% quieter and 0.2% sharper, and it
needs no Siril install at all. That was not sufficient when this amendment was written, for two
reasons that had nothing to do with either number. **Phase 6c has since removed one of them, and
the decision stands on the other alone.** Both are kept here, because a decision that survives its
main argument being withdrawn should say so rather than quietly re-derive itself.

**~~It would silently change every OSC user's demosaic.~~ Withdrawn — Phase 6c.** The argument was
that RCD could not be implemented clean-room, so the native path substituted Malvar-He-Cutler:
more accurate against known truth, and *measurably noisier in the background*, 1.23x to 1.45x
RCD's per channel. A colour stacker who suddenly found their background noisier would have had
that decided for them with no reason to suspect it. The clean-room-specification route then made
RCD available, it is the default `--debayer`, and its background noise is at or below Siril's
(0.95x/0.95x/0.93x per channel). **A native default would no longer change an OSC user's
demosaic in any way they could measure.**

**It would turn working commands into errors.** Background extraction and mosaic projects — grid
and spiral — are Siril-only and refuse up front. That refusal is correct behaviour for a user who
asked for the native engine and wrong for a user who asked for nothing. This reason is unaffected
by Phase 6c and is now the whole of the case.

**So the decision is now a narrower one than it was**, and worth stating in its narrowed form: the
default stays Siril because flipping it would break three commands that work today, not because
the native engine produces worse pictures. It does not. What would change the decision:

1. ~~**The RCD gap closes** — a demosaic lands with measured background noise at parity.~~
   **Met in Phase 6c.** AHD did not close it (Phase 6b, 1.20×/1.55×/1.32×); native RCD does, at
   0.95×/0.95×/0.93×, for 13% end-to-end. The diagnosis that AHD's failure suggested turned out
   to be right: the fix was an *averaging* directional method, not another selecting one.
2. **The live one.** Background extraction and the mosaic pipelines gain native implementations,
   or the engine selection becomes per-pipeline so a native default falls back for the ones it
   cannot do — which is a change to D5's "report rather than silently skip" rule and needs its
   own argument.
3. A release's worth of `--engine native` use on real projects without a correctness report.

Until then the honest recommendation, and what the ROADMAP now says, is that the native engine is
the better path today — for colour as well as mono, since Phase 6c — and should be chosen
deliberately.

### Phase 13 — Warping, and the passes that were not needed

*Goal: act on what the Phase 12 profile pointed at rather than at what Phase 12 chose to look at.*

Phase 12 ranked its targets by **thread** time and went after `stack.reduce`, which is the largest
number in the profile by a wide margin. That was the wrong ordering for a wall-clock question.
Warping is nearly serial *between* frames — a frame is warped by all sixteen cores, then the next
one is — so its span total is close to its wall-clock cost, and `warp.total` was **0.80–0.82× the
whole run's wall clock**. `stack.reduce`'s 174 s of thread time is 2.15× wall, which is to say
about 11 s of it. Warping was the larger target the whole time and had never been optimized.

- [x] **P13-T1** The clamping guide is produced by the resample pass instead of by a second warp.
- [x] **P13-T2** `clamp_undershoot` works from the list of pixels that rang rather than from a
      frame-sized mask, which removes the dilation and copy passes and two `bool` buffers.
- [x] **P13-T3** `separable` takes its tap window as a sub-slice of each source row.
- [x] **P13-T4** `clip_until_stable` and `linear_fit` gather survivors in the pass that counts
      them, instead of comparing every value twice.
- [x] **P13-T5** `winsorized_deviation` clamps as it reads instead of materializing the pulled-in
      stack once per iteration.
- [x] **P13-T6** Output proved unchanged, byte for byte. See the gate.

**Gate: met, and the output is byte-identical.** This is the property the whole phase is
conditional on — every change here is a restatement of the same arithmetic in the same order, so
none of them is allowed to move a pixel. Verified by building the 95-frame master light and the
40-frame master bias before and after and comparing the files: **`cmp` reports no difference on
either**, across 244 MB and 61,171,488 pixels. Every parity figure is therefore unchanged in every
digit — sky 0.333 σ_bg, noise ratio 0.9766, FWHM ratio 0.9978 — which is the same evidence stated
the other way round.

#### What each change was worth

Kernel-level, `benches/register.rs` and `benches/reduce.rs`, A/B against a saved baseline of the
previous code on the same machine:

| Benchmark | Before | After | |
|---|---|---|---|
| `warp/lanczos4_clamped` | 80.4 ms | **44.9 ms** | **1.79×** |
| `warp/lanczos4` | 61.2 ms | **38.2 ms** | 1.60× |
| `warp/bicubic_clamped` | 51.2 ms | **25.7 ms** | 1.99× |
| `warp/bicubic` | 31.3 ms | **20.1 ms** | 1.56× |
| `warp/bilinear` | 7.03 ms | 7.16 ms | **0.98×, slower** |
| `reduce_kernel/reject/95_frames` (linear fit) | 4.105 ms | 4.049 ms | 1.014× |
| `reduce_kernel/reject/25_frames` (Winsorized) | 4.286 ms | 4.221 ms | 1.015× |
| `reduce_kernel/reject/5_frames` (percentile) | 90.1 µs | 90.2 µs | — |

End to end, from `tests/profile.rs` on the 122-frame master light (8 GB budget, 16 cores; median
of three runs before, two after):

| Stage | Before | After | |
|---|---|---|---|
| **wall clock** | **83.4 s** | **66.5 s** | **1.25×** |
| `warp.total` | 68.5 s | **33.6 s** | **2.04×** |
| — `warp.resample` | 47.3 s | 29.4 s | 1.61× |
| — `warp.clamp` | 21.2 s | **3.7 s** | **5.7×** |
| `stack.reduce` | 174.3 s | 168.0 s | 1.04× |

And through the pipeline the README quotes, the 95-frame master light: **47.1 s to 37.8 s**, against
Siril's 73.6 s — from 1.56× to **1.95×**.

#### The guide was a second warp of the whole frame

Clamping needs a reference the kernel cannot undershoot, and it got one by warping the frame again
with the area kernel. That kernel *is* bilinear at the scale registration works at — `area` called
`bilinear` and said so — and bilinear's 2×2 taps are a subset of the 8×8 Lanczos-4 has already
loaded, sampled at a point the resample loop has already inverse-mapped. Producing it in the same
pass costs four multiply-adds; producing it separately cost a second inverse-map, a second support
check and a second traversal of a 244 MB output, for every frame.

Two things made this safe to state as bit-identical rather than merely close. It is the same
function at the same point, so the values are the same values. And the guide's support is *wider*
than Lanczos-4's, so the separate warp also covered a band around the footprint's edge where the
real warp has nothing — a band nothing reads, because the repair never writes a pixel the warp
left at zero. Dropping it loses no information the clamp could have used.

#### Ringing is 1% of a frame and was being paid for at 100%

The repair itself was three whole-frame passes: build a `bool` mask over every pixel, dilate it by
reading five neighbours of every pixel, then walk the frame again to copy the guide in. Two of
those passes exist only to express "and also the neighbours of the pixels that rang" — which, for
the ~1% of pixels that do ring, is better said by walking that 1% and touching their neighbours
directly. The dilation and the copy become proportional to the ringing instead of to the frame,
and two frame-sized `bool` buffers go away with them.

The result is identical where two repaired neighbourhoods overlap, and the reason is worth
recording because it is the sort of thing that looks like a difference and is not: the repair is
`data[i] = guide[i]`, which is idempotent, and the only pixel a second visit declines to write is
one an earlier visit set to zero — by writing the very value it would write again.

#### The kernel was paying an index multiply and a bounds check per tap

`separable` fetched each of its 64 taps with `Frame::get`, which recomputes
`(channel · height + y) · width + x` and bounds-checks the result. But `sample` has *already*
established that the whole support is inside the frame — that is the check that keeps the edge
honest — so each source row contributes a contiguous run of `2·radius` values that can be taken as
a sub-slice. The multiply and the check come out of the inner loop, and what remains is a dot
product over two contiguous slices, which the compiler can do something with.

Worth 1.6× on both kernels that use it, with the accumulation order deliberately untouched.
**This is the optimization D8 asked for and did not find in Phase 12.** Phase 12 looked for
vectorization of the *arithmetic* and correctly refused it, because reassociating floating-point
addition would move every pixel in its last bits. Removing address arithmetic changes no
arithmetic at all, and required no `unsafe` to do it.

The one number that went the wrong way is `warp/bilinear`, 2% slower and reproducibly so. Bilinear
does not use `separable` and gains nothing from the change; what it sees is the restructured band
loop. It is 7 ms on a 16.8 MP frame, it is not the registration kernel — Lanczos-4 is, by D10 —
and it is recorded here rather than dropped from the table.

#### D8 was right about the reduction, and the measurement says so again

P13-T4 and P13-T5 were expected to matter and did not, and that is the useful finding. Removing a
whole comparison pass per clipping iteration, and a buffer written and read once per Winsorizing
iteration, is worth **1.4–1.5% of the rejection kernel** — statistically solid (p < 0.01) and
practically nothing. At the pipeline level it is invisible: master flat, which is 59 frames of
Winsorized rejection and the most rejection-bound pipeline there is, measured 7.38 s before and
7.38 s after.

This is exactly what D8 predicted. The reduce loop is bound by the *serial dependency chain* of
its estimators — each clipping pass needs the previous pass's centre and spread, and each
Winsorizing iteration needs the previous iteration's sigma — not by how many times it touches the
values. Taking work out of a latency-bound loop buys close to nothing, which Phase 12 established
by removing `sxx` from the least-squares fit and watching the total not move. Two more attempts
now agree.

The changes are kept, because they are smaller and clearer than what they replace — one pass that
gathers instead of two that count and then gather, and an estimator that does not need a buffer —
and because being free is a good enough reason to prefer the simpler form. They are not kept for
their speed, and the table says so.

**What is left, in order of what the profile now says.** `stack.reduce` is once again the largest
thread-time figure and is now also a fair share of wall clock; the iterated clipping that makes it
slow is algorithmic and unaddressed. `master_light.levels` is ~15 s of serial per-channel work on
a frame the warp had in cache moments earlier. `starfind.background` traverses the same 244 MB
twice, once for the counting median and once for the row-difference noise. And `stack.read` (53 s
of thread time) is strictly serialized ahead of the reduction inside each worker, where one block
of read-ahead would hide it at the cost of a second band buffer per worker — which is the memory
the scheduler exists to bound, so it is a budget trade rather than a free win and wants measuring
before it is taken.

---

## Open questions

Three of the five questions this ADR opened were answered by the work and are recorded here with
their answers rather than deleted, since the answer is the useful part.

**Settled**

- ~~**Interpolation parity ceiling.**~~ **Reachable, with room.** The concern was that OpenCV's
  fixed-point Lanczos-4 weight tables would put a floor under any `f32` implementation. P10-T4
  measures it: the warp agrees with `seqapplyreg` well inside the estimated 0.5 σ_bg
  (`data/output/warp-parity.txt`), and the quantized weight table this implementation uses is
  1024-step against OpenCV's 32, so the quantization is the finer of the two. The bound did not
  need widening and the fallback plan — assert the direction of the difference instead of its
  magnitude — was not needed.
- ~~**GESDT needs the inverse Student's t CDF.**~~ **Deferred with a typed error, and `statrs`
  not taken** — the outcome the question named as acceptable. `Rejection::Gesdt` fails
  `check_supported()` before a frame is read, so it is a clear refusal rather than a silent
  fallback to a method the caller did not ask for. Nothing selects it: `BestRejection` covers
  every frame count with percentile, Winsorized or linear fit. Carrying a statistics crate for a
  method with no caller was not worth it.
- ~~**Frame-count ceiling for in-memory registration data.**~~ **Not material at any frame count
  the pipelines accept.** The estimate in the question was roughly right — a capped 2000-star
  list is ~100 KB a frame, so 500 frames is ~50 MB — but it is the wrong thing to worry about
  next to what shares the budget with it: a *single* 61 MP frame is 244 MB, and the warped-frame
  residency P11-T3 manages is measured in gigabytes. Star lists are charged against the budget
  and have never been the term that decided anything.

**Still open**

- **Which high-pass does the RCD specification mean?** §3.2 says "five-tap" and "distances 1, 2
  and 3", which are different filters. Phase 6c implements the five-tap fourth difference and
  deliberately did not try the alternative against Siril's output, since fitting a coefficient to
  the reference is the one move the clean-room route cannot survive. The specification's author
  should settle it; if it is the sixth difference, the remaining 2.7–3.5 σ_bg residual is the
  expected place for it to show up.
- **`FitsExt` living in `siril-sys`** (D5) is an accepted wart. Moving it to `px-fits` is the
  obvious cleanup and is blocked only by this ADR's "no changes to Siril crates" constraint.
- **The memory budget is the one bound this ADR calls non-negotiable, and only the stacker has a
  test that enforces it.** `tests/memory.rs` asserts peak heap against the budget for stacking;
  the end-to-end figure that matters most — an 8 GB run peaking at 4.88 GB — is a measurement in
  the README, not an assertion. The bug it replaced (a master light at 19.9 GB under an 8 GB
  budget, each phase individually inside it) would not be caught again by the suite. A
  pipeline-level peak-RSS assertion is the missing guard.
- **Reproducing the Siril-relative numbers needs a corpus that is not in this repository.** See
  the Phase 0 note.
