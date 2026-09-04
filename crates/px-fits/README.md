# px-fits

FITS (Flexible Image Transport System) I/O for [Photonyx](../../README.md). This crate is being
rewritten to a native implementation per [ADR 006](../../docs/adr/006-native-fits-implementation.md)
— see that document for the full design and phased implementation plan. This README tracks the
one artifact the ADR asks to live here: the benchmark report comparing implementations and I/O
backends as the rewrite proceeds.

## Status

Rewrite in progress. Phases 0–4 are complete: headers, HDU discovery/navigation,
`ImageHdu::{read_full, read_full_into, rows}` (full-frame reads), and
`ImageHdu::{read_region, read_region_into}` (subset reads) all run on the native reader, with
`fitsrs` fully removed. `display::decode_preview` is still backed by `astroimage`/`rustafits`
(deferrable Phase 9). The "Baseline report" below is the pre-rewrite starting point; the
"Phase N progress" subsections record where the native reader now stands against it.

## Benchmarks

```
cargo bench -p px-fits                 # all benchmarks
cargo bench -p px-fits --bench region  # a single benchmark
```

Fixture generation and standard-compliance validation:

```
cargo xtask fits-fixtures              # (re)generate the committed synthetic corpus
cargo xtask fits-verify                # run fitsverify/astropy over tests/fixtures, if installed
cargo xtask fits-verify --dir <path>   # validate an arbitrary directory of .fits/.fit files
```

Real-world frames can be dropped into an arbitrary directory and pointed at via
`PX_FITS_CORPUS=/path/to/frames` for benchmarks that opt into it (nothing does yet — this lands
with the Phase 3+ benchmarks that need larger, non-synthetic images).

## Baseline report (Phase 0)

Measured against the pre-rewrite implementation: `fitsrs` 0.4.1 for headers, `astroimage`
(`rustafits`, branch `jpg-feature`) for pixel data. Criterion's raw estimates for each run are
committed under [`benches/baselines/`](benches/baselines/) (`estimates.json` per benchmark); the
numbers below are the median-of-100-iterations point estimate from those files.

**Machine:** Apple M4 Max, macOS 26.6.2 (Darwin 25.6.0, arm64), rustc 1.97.0. Warm page cache
(fixtures reused across iterations; first-iteration cold-cache cost is not isolated by these
runs). No Linux/Windows numbers yet — those are needed before Phase 8 draws its mmap-vs-
positioned-read conclusion, since the two platforms are expected to behave differently there.

| Workload | Baseline implementation | Median time | Notes |
|---|---|---|---|
| Header-only scan, 500 files | `FitsFile::new` + `header_rows()` (fitsrs) | 5.79 ms (≈11.6 µs/file) | Baseline has no laziness guarantee; this is the number the native reader's ≥2× gate (ADR 006) is measured against. |
| Full-frame read, 4096×4096 `i16` (33.6 MB) | `astroimage::ImageConverter::read_raw` | 2.25 ms (≈14.9 GB/s) | Warm-cache; reflects batch-processing conditions more than cold single-file reads. |
| Region read, 512×512 from a 4096×4096 `i16` frame | full read + manual crop (no subset-read primitive exists today) | 2.31 ms | Effectively identical to a full-frame read, as expected — cropping after the fact reads and allocates the whole frame regardless of the region size. This is the "before" half of the ADR's ≥20× region-read gate. |
| Write, 4096×4096 `i16`-equivalent buffer | raw `std::fs::write` (no FITS writer exists yet) | 5.08–6.53 ms | Placeholder floor only — `px-fits` cannot write FITS files until Phase 5. |

**Peak heap (dhat, `tests/memory.rs`):** for a 512×512 `i16` frame (524,288 bytes of pixel data
at the theoretical minimum), `astroimage::read_raw` peaks at **1,180,200 bytes — 2.25× the
theoretical minimum**. This is the concrete evidence behind ADR 006's D4 (single-allocation
reads): the current path allocates the raw byte buffer and the typed pixel buffer separately
rather than streaming one into the other.

### Phase 2 progress: header-only scan (honest interim number)

As of the Phase 2 cutover, `FitsFile`/`header_rows()` run entirely on the native reader —
`fitsrs` is a dev-dependency only. Re-running the header-scan benchmark against the native
implementation gives **5.60 ms for the same 500-file corpus (≈11.2 µs/file)**, essentially
parity with the 5.79 ms `fitsrs` baseline above, **not yet the ADR's ≥2× target**.

One real fix landed alongside this measurement: `Card::parse` was allocating a fresh `String`
for every card by remapping each byte through `b as char`, even though the near-universal case
is plain ASCII, where the 80 bytes are already valid UTF-8 and can be borrowed with zero
allocation (`Cow::Borrowed` via `str::from_utf8`, falling back to the byte-remap only for
non-ASCII/malformed cards). That was good for a measured ~5% improvement and is a legitimate
win, but not the dominant cost.

The bulk of the remaining gap is very likely per-card `String` allocation elsewhere in the parse
path (`keyword.to_string()`, comment/value string construction) and `header_rows()` building a
fresh `Vec<(String, String, String)>` on every call — real allocation pressure for headers this
small (single 2880-byte block, ~10 cards), where syscall and parse overhead are comparable in
magnitude rather than I/O-dominated. This is left as an explicit, scoped follow-up rather than
claimed as done: the structural laziness guarantee (exact byte counts via `CountingSource`,
proven in `tests/reader_conformance.rs`) is real and gates cleanly; the *speed* gate does not yet,
and this section will be updated once that follow-up lands rather than silently dropped.

### Phase 3 progress: full-frame reads (native, beats baseline)

`ImageHdu::read_full` / `read_full_into` / `rows()` now do native full-frame
decoding — big-endian `from_be_bytes` over `chunks_exact`, `BSCALE`/`BZERO`/`BLANK`
applied in the same pass, one output allocation, bounded 256 KiB streaming scratch
(skipped entirely when the source can hand out a whole-file slice). Same machine and
warm-cache conditions as the Phase 0 baseline.

| Path | Median time | Throughput | vs. `astroimage::read_raw` (2.07 ms) |
|---|---|---|---|
| `astroimage::read_raw` (re-measured) | 2.07 ms | 15.1 GiB/s | 1.00× |
| `read_full::<i16>()`, `FileSource` (streaming scratch) | 1.72 ms | 18.2 GiB/s | **1.20× faster** |
| `read_full_into::<i16>()`, `FileSource` (caller buffer reused) | 1.56 ms | 20.1 GiB/s | **1.33× faster** |
| `read_full::<i16>()`, `SliceSource` (whole-file, zero-copy decode) | 1.15 ms | 27.2 GiB/s | **1.80× faster** (also pays a 33 MB buffer clone per iter) |

The relative gate for this workload (ADR 006 performance targets) is "≥ parity in
wall time, materially lower peak RSS than `read_raw`" — met on both counts: faster
*and* the peak-heap invariant below.

**Peak heap (dhat, `tests/memory.rs`):** `read_full_into` over a slice source does
not allocate proportionally to pixel count (asserted ≤ 16 KiB of harness noise for a
512 KiB frame); `read_full` peaks at `output × 1.05 + 256 KiB scratch`; `rows()`
peaks at one row regardless of image height. Contrast the Phase 0 baseline's 2.25×
theoretical-minimum peak.

#### D2: is the safe big-endian decode fast enough? (yes — question closed)

ADR 006 D2 says `unsafe` transmute-based decoding is only "warranted" if the safe
path is *materially* slower. Isolated in-memory decode of 16.7 M big-endian `i16`
samples:

| Decode | Median time | Throughput |
|---|---|---|
| `memcpy` ceiling (same byte volume) | 418 µs | 74.8 GiB/s |
| safe: `i16::from_be_bytes` over `chunks_exact(2)` | 658 µs | 47.5 GiB/s |
| hypothetical `unsafe`: `read_unaligned` + `i16::from_be` | 556 µs | 56.2 GiB/s |

The safe path is ~18 % slower than the `unsafe` alternative *in this microbench*, and
that decode is well under half the cost of a real full-frame read — where the safe
native path already beats the pre-rewrite reader by 20–33 %. That does not clear the
bar for introducing `unsafe` into `src/`. **Decision: keep the safe decode.** Revisit
only if a real-world profile shows decode (not I/O) dominating.

### Phase 4 progress: region selection (≥ 20× gate met)

`ImageHdu::read_region` / `read_region_into` take a `Region { start, shape }`
(FITS axis order; `Region::rect(x, y, w, h)` for the 2D case) and read only the
subset — the plan is one contiguous run per subset row (`shape[1..].product()`
runs in N-D), one positioned read each. `tests/region_conformance.rs` asserts the
exact byte accounting: a 20×12 window is 12 reads of 480 bytes total, nothing more.

Benchmark: a 512×512 window out of a **~60 MP** (7744×7744) `i16` frame. The
baseline ("read the whole frame, then crop" — no subset primitive exists in
`fitsrs`/`astroimage`) scales with total pixels; `read_region` does not, so the
ratio grows with frame size. Same machine/warm-cache as the other reports.

| Path | Median time | vs. full-read-plus-crop (8.13 ms) |
|---|---|---|
| `astroimage::read_raw` + manual crop | 8.13 ms | 1.0× |
| `read_region::<i16>()`, `FileSource` | 164 µs | **49× faster** |
| `read_region_into::<i16>()`, `FileSource` (buffer reused) | 149 µs | **55× faster** |
| `read_region::<i16>()`, `SliceSource` | 1.65 ms | 4.9× (dominated by a 120 MB buffer clone per iter, not the read) |

The ADR's ≥ 20× gate for this workload is met (49–55× on a FileSource). At the
Phase 0 baseline's 4096×4096 (16.7 MP) frame the ratio is ~13–15× — still a large
win, just below 20× because ~512 one-row `pread` syscalls dominate a warm-cache
read at that size; the gap closes as the frame (and thus the baseline's full read)
grows. P4-T7 run-coalescing was not needed: subset rows of a much-wider image are
never adjacent, so there is nothing to coalesce.

### Positioned-read vs. mmap

Not yet measured — the `ByteSource`/`FileSource` positioned-read backend lands in Phase 1 and
`MmapSource` (behind the `mmap` feature) lands in Phase 8. This section will report both
backends across all three prioritized workloads (full-frame throughput/RSS, header-only scan,
region reads), cold and warm page cache, on macOS and Windows, once that phase lands.

## Regenerating fixtures and baselines

The committed fixture corpus (`tests/fixtures/`) is generated by `cargo xtask fits-fixtures` and
must be byte-identical on regeneration — the generator is seeded, not randomized. Do not hand-edit
files under `tests/fixtures/`.

To refresh the committed criterion baselines after a benchmark-relevant change:

```
cargo bench -p px-fits
# single-function groups: one file
cp target/criterion/<group>/new/estimates.json crates/px-fits/benches/baselines/<name>.json
# multi-function groups (e.g. full_frame, decode): one file per function
cp target/criterion/<group>/<function>/new/estimates.json \
   crates/px-fits/benches/baselines/<group>/<function>/estimates.json
```

Update the table above with the new numbers and machine info in the same commit.
