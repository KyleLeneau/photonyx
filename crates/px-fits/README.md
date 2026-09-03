# px-fits

FITS (Flexible Image Transport System) I/O for [Photonyx](../../README.md). This crate is being
rewritten to a native implementation per [ADR 006](../../docs/adr/006-native-fits-implementation.md)
— see that document for the full design and phased implementation plan. This README tracks the
one artifact the ADR asks to live here: the benchmark report comparing implementations and I/O
backends as the rewrite proceeds.

## Status

Rewrite in progress. Phase 0 (harness and baselines) is complete; the public API
(`FitsFile`, `HeaderUtil`, `display::decode_preview`, …) is currently still backed by `fitsrs`
and `astroimage`/`rustafits`. Nothing below reflects the native reader yet — these are the
baselines the native implementation must beat.

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
cp target/criterion/<group>/<function>/new/estimates.json crates/px-fits/benches/baselines/<name>.json
```

Update the table above with the new numbers and machine info in the same commit.
