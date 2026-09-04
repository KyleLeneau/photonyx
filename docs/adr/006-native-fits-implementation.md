# 006 — Native FITS Implementation for `px-fits`

## Context

`px-fits` today is a thin facade over two third-party libraries:

- **`fitsrs` 0.4.1** — parses headers. `FitsFile::new` constructs a `Fits::from_reader` over a
  `BufReader<File>`, pulls the primary HDU, and exposes keyword access. No pixel data is ever
  read through it.
- **`rustafits`/`astroimage`** — a git dependency (branch `jpg-feature`) that owns *all* pixel
  work in `display.rs`: raw read, BITPIX conversion, Bayer debayer, STF autostretch, downscale,
  and star analysis.

This arrangement has four concrete problems:

1. **No control over I/O volume.** `CalibrationMetadata::from` and `ObservationMetadata::from`
   only need a handful of keywords, but the reader-based API gives no guarantee about how much
   of the file is touched. A profile scan across a night of 300–500 frames pays that cost per
   file.
2. **No control over allocation.** `decode_preview` calls `ImageConverter::read_raw`, which
   materializes the entire frame, then hands it to `process_data`, which allocates again. Peak
   RSS for a 60 MP 16-bit frame is well above the theoretical minimum, and there is no way to
   ask for only the pixels needed.
3. **No region selection.** Neither dependency offers cfitsio's subset-read primitive
   (`fits_read_subset`), which is the natural way to build previews, thumbnails, pixel-peep
   panes, and per-tile statistics without reading a whole frame.
4. **Supply-chain fragility.** `rustafits` is pinned to a personal fork on a feature branch.
   `fitsrs` is a single-maintainer crate whose API has churned across minor versions.

ROADMAP already carries this: *"would like to drop fitsrs for my own fits library with well known
tests, lazy load and performance."*

This ADR records the design and the phased plan to execute it.

---

## Decision

Replace `fitsrs` with a native, dependency-light FITS implementation owned by `px-fits`. The
crate keeps its name and its current public entry points so consumer crates
(`px`, `px-pipeline`, `px-nativeui`) do not churn; the implementation underneath is rewritten.

### Scope

**In scope**

- Read and write FITS files per the FITS Standard 4.0.
- Primary HDU and `IMAGE` extensions; all `BITPIX` values (8, 16, 32, 64, -32, -64), `BSCALE`/
  `BZERO` scaling, `BLANK`, N-dimensional axes.
- `BINTABLE` and `TABLE` (ASCII) extensions — read and write, all standard `TFORM` codes
  including variable-length arrays (`P`/`Q` descriptors).
- Tile-compressed images (the FITS tiled-image convention): `RICE_1`, `GZIP_1`, `GZIP_2`,
  `PLIO_1`.
- cfitsio-style **region selection** — read an arbitrary N-dimensional rectangular subset
  without materializing the full data array.
- Lazy HDU discovery: opening a file reads header blocks only.
- Two byte-source backends, benchmarked against each other and reported in the crate README.

**Out of scope (documented, returns a typed error)**

- `HCOMPRESS_1` tile compression (lossy, substantially more complex; no observed need).
- Random-groups records (a deprecated pre-extension convention).
- World Coordinate System evaluation. WCS keywords are readable as ordinary cards; projecting
  them is not this crate's job.
- Replacing `astroimage`'s debayer/autostretch. Tracked as a deferrable final phase.

### Architecture

```
crates/px-fits/
  Cargo.toml
  README.md                  # includes the mmap-vs-positioned-read benchmark report
  benches/
    header_scan.rs           # metadata-only open across N files
    full_frame.rs            # whole-image read: throughput + peak heap
    region.rs                # subset reads at varying sizes/strides
    write.rs                 # image and table write throughput
  tests/
    conformance.rs           # standard-compliance corpus
    roundtrip.rs             # write -> read -> compare
    memory.rs                # dhat peak-heap assertions
    differential.rs          # phase 1-3 only: diff against fitsrs, then deleted
    fixtures/                # small committed .fits files (generated, deterministic)
  src/
    lib.rs                   # public facade + re-exports; FitsFile compatibility layer
    error.rs                 # FitsError
    source.rs                # ByteSource trait, FileSource, SliceSource, (feature) MmapSource
    block.rs                 # 2880-byte block arithmetic, padding helpers
    card.rs                  # 80-byte card parse/serialize, Value, CONTINUE long strings
    header.rs                # Header, keyword index, typed accessors, HeaderBuilder
    hdu.rs                   # Hdu enum, lazy discovery, byte-offset bookkeeping
    reader.rs                # FitsReader<S>
    writer.rs                # FitsWriter<W>, streaming image/table writers
    image/
      mod.rs                 # ImageHdu
      pixel.rs               # sealed Pixel trait, BitPix, big-endian decode
      scaling.rs             # BSCALE/BZERO/BLANK application
      region.rs              # Region descriptor -> contiguous run planner
    table/
      mod.rs                 # shared column model
      ascii.rs               # TABLE
      binary.rs              # BINTABLE, TFORM/TDIM, heap + P/Q descriptors
    compress/
      mod.rs                 # tiled-image convention dispatch
      rice.rs
      gzip.rs
      plio.rs
    display.rs               # unchanged facade over astroimage until the final phase
```

### Key design decisions

**D1 — `ByteSource` abstraction, positioned reads by default.**

```rust
pub trait ByteSource: Send + Sync {
    fn len(&self) -> u64;
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()>;
    /// Zero-copy borrow of the whole file, when the backend can provide one.
    /// `MmapSource` returns `Some`; `FileSource` returns `None`.
    fn as_slice(&self) -> Option<&[u8]> { None }
}
```

The default backend is `FileSource`, built on positioned reads —
`std::os::unix::fs::FileExt::read_at` on Unix and `std::os::windows::fs::FileExt::seek_read`
on Windows, behind one cross-platform shim. Positioned reads take `&self`, so a single open
handle can be shared across rayon workers without locking, and they never rely on an implicit
cursor. (Note for the Windows path: `seek_read` does move the handle's file pointer as a side
effect, but because every call supplies an explicit offset, concurrent calls remain correct.)

`MmapSource` (via `memmap2`) sits behind a non-default `mmap` cargo feature. It is opt-in
because mapping a file that is truncated underneath the process is undefined behaviour, and
network/removable volumes make that a real risk in this domain. When enabled, `as_slice`
returns the mapping and the hot paths skip the intermediate copy entirely.

> **Open question, to be settled by Phase 8's own numbers:** `mmap` may be dropped entirely
> rather than shipped as a feature. If `FileSource` combined with `rayon`-parallelized
> positioned reads (fanning a multi-file scan or a large region read across worker threads)
> proves competitive with `MmapSource` in the Phase 8 report, the simpler all-safe,
> single-backend design wins by default per the "no `unsafe` unless warranted" principle — a
> tie goes to deleting the feature and the `memmap2` dependency, not to keeping it around
> unused. Phase 8's gate (README report + recommendation) already requires this comparison;
> this note just makes explicit that "drop `mmap`" is an acceptable, even expected, outcome of
> that comparison, not a fallback to justify only if mmap loses badly.

**D2 — No `unsafe` in `px-fits` source.** Three sanctioned exceptions, all outside our own
`src/`:

- `memmap2` (feature-gated, contains its own `unsafe`).
- `flate2` for GZIP tile decompression.
- `dhat` as a dev-dependency for peak-heap assertions in tests/benches.

Big-endian decoding uses `i16::from_be_bytes` / `f32::from_be_bytes` over `chunks_exact(N)`.
This autovectorizes; Phase 3 must prove it with a benchmark rather than assume it. If — and
only if — the benchmark shows the safe path is materially slower than the transmute-based
alternative, that is the point at which `unsafe` becomes "warranted", and it must be justified
inline with the benchmark numbers that motivated it.

**D3 — Laziness is structural, not incidental.** Opening a file reads 2880-byte blocks until
`END` and nothing more. Each HDU's data unit size is computed arithmetically from `BITPIX` and
`NAXISn`, so the reader seeks past data to find the next header without ever reading pixels.
`FitsReader::open` on a 120 MB file must touch only a few kilobytes.

**D4 — Single-allocation reads.** `read_full::<T>()` allocates exactly one `Vec<T>` sized to
the output and streams the file into it through a small fixed scratch buffer (default 256 KiB,
tunable), converting endianness and applying `BSCALE`/`BZERO` in place. It never materializes
the raw byte image and then converts — that pattern doubles peak RSS and is the specific thing
this rewrite exists to eliminate. `read_full_into(&mut [T])` lets callers own the allocation and
reuse buffers across frames.

**D5 — Regions as contiguous-run plans.** A `Region` is `{ start: [usize; N], shape: [usize; N] }`
in FITS axis order (fastest-varying axis first). The planner decomposes it into
`shape[1..].product()` contiguous runs of `shape[0]` elements, each satisfied by one positioned
read. For a 2D rectangle this is one read per image row of the subset. The number of bytes read
is therefore bounded and assertable — which is exactly how the tests verify it.

**D6 — Unsigned integers via `BZERO`.** Nearly every astro camera writes `BITPIX = 16` with
`BZERO = 32768` to encode unsigned 16-bit. The `Pixel` conversion layer must handle this as a
first-class case, not as generic float scaling, so that `read_full::<u16>()` on such a file is
an integer add and not a round-trip through `f64`.

**D7 — Public API shape.**

```rust
// Reading
pub struct FitsReader<S: ByteSource> { /* ... */ }
impl FitsReader<FileSource> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FitsError>;
}
impl<S: ByteSource> FitsReader<S> {
    pub fn from_source(source: S) -> Result<Self, FitsError>;
    pub fn primary(&self) -> Result<Hdu<'_, S>, FitsError>;
    pub fn hdu(&self, index: usize) -> Result<Hdu<'_, S>, FitsError>;
    pub fn hdus(&self) -> HduIter<'_, S>;
    pub fn hdu_count(&self) -> Result<usize, FitsError>; // forces a full header scan
}

pub enum Hdu<'a, S> {
    Image(ImageHdu<'a, S>),
    AsciiTable(AsciiTableHdu<'a, S>),
    BinTable(BinTableHdu<'a, S>),
    CompressedImage(CompressedImageHdu<'a, S>),
}

impl<S: ByteSource> ImageHdu<'_, S> {
    pub fn header(&self) -> &Header;
    pub fn shape(&self) -> &[usize];        // NAXIS1..NAXISn, fastest axis first
    pub fn bitpix(&self) -> BitPix;
    pub fn read_full<T: Pixel>(&self) -> Result<Vec<T>, FitsError>;
    pub fn read_full_into<T: Pixel>(&self, out: &mut [T]) -> Result<(), FitsError>;
    pub fn read_region<T: Pixel>(&self, r: &Region) -> Result<Vec<T>, FitsError>;
    pub fn read_region_into<T: Pixel>(&self, r: &Region, out: &mut [T]) -> Result<(), FitsError>;
    pub fn rows<T: Pixel>(&self) -> RowIter<'_, S, T>;  // streaming, one reusable row buffer
}

// Writing
pub struct FitsWriter<W: Write + Seek> { /* ... */ }
impl FitsWriter<BufWriter<File>> {
    pub fn create(path: impl AsRef<Path>) -> Result<Self, FitsError>;
}
impl<W: Write + Seek> FitsWriter<W> {
    pub fn write_image<T: Pixel>(&mut self, h: &HeaderBuilder, data: &[T]) -> Result<(), FitsError>;
    pub fn begin_image(&mut self, h: &HeaderBuilder) -> Result<ImageWriter<'_, W>, FitsError>;
    pub fn write_bintable(&mut self, t: &TableBuilder) -> Result<(), FitsError>;
    pub fn finish(self) -> Result<(), FitsError>;
}

/// In-place header edit when the new header occupies the same block count; otherwise rewrites.
pub fn update_header(path: &Path, hdu: usize, edits: &[CardEdit]) -> Result<(), FitsError>;
```

`HeaderBuilder` enforces the mandatory-keyword ordering the standard requires
(`SIMPLE`/`XTENSION`, `BITPIX`, `NAXIS`, `NAXISn`, …, `END`) so callers cannot emit an invalid
header by accident.

**D8 — Backwards compatibility.** `FitsFile`, `HeaderUtil`, `Binning`, `FitsError`,
`all_fits_files`, `all_color_raw_frames`, and `display::{PreviewImage, decode_preview,
MAX_DISPLAY_DIM}` keep their current signatures and behaviour. `HeaderUtil` becomes inherent
methods on the new `Header` plus a blanket trait impl retained for source compatibility.
`FitsFile::primary_hdu.get_header()` — used directly by `px-pipeline/src/meta.rs` — is preserved
as a method returning the new `&Header`.

### Verification strategy

Cheap, objective oracles matter more than prose here, because the work is being handed off.

**O1 — Instrumented byte source.** `CountingSource<S>` wraps any `ByteSource` and records bytes
read and read-call count. This turns "is it lazy?" and "does the region read only the region?"
into exact assertions:

```rust
assert_eq!(counter.bytes_read(), 2880 * expected_header_blocks);
```

**O2 — Peak heap assertions.** `dhat` in `tests/memory.rs` asserts peak heap for each read mode
against a structural bound (e.g. full-frame read ≤ `output_bytes + scratch + 5%`).

**O3 — Differential testing against `fitsrs`.** For Phases 1–3, `fitsrs` is retained as a
**dev-dependency only** and `tests/differential.rs` asserts that our header parse produces
identical keyword/value/comment triples across the whole corpus. This gives a mechanical
correctness oracle during the riskiest phase. The file and the dev-dependency are deleted at the
end of Phase 3.

**O4 — External validation.** `cargo xtask fits-verify` shells out to `fitsverify` and/or
`python -c "from astropy.io import fits; ..."` when available on the machine, and skips with a
clear message when not. Every file the writer produces must pass `fitsverify` with zero errors.
This is advisory in CI (tools may be absent) and mandatory before Phase 5 is marked done.

**O5 — Fixture corpus.**
- *Committed, synthetic:* generated by `cargo xtask fits-fixtures` into
  `crates/px-fits/tests/fixtures/`. Deterministic, each < 100 KB, covering every `BITPIX`, 1–4
  axes, `BSCALE`/`BZERO`/`BLANK`, multi-extension, long strings via `CONTINUE`, both table
  kinds, variable-length columns, and each supported compression type. Regenerating must be
  byte-identical — the xtask is checked by `cargo xtask check`.
- *Malformed:* a `fixtures/invalid/` set (truncated data unit, missing `END`, bad card syntax,
  `NAXIS` overflow, negative `NAXISn`, `BITPIX` = 3) that must each produce a specific
  `FitsError` variant and never panic.
- *Real-world, opt-in:* benchmarks and heavy tests read `PX_FITS_CORPUS=/path/to/frames` when
  set, and fall back to generated large synthetic frames when not, so the suite is green on a
  clean checkout.

**O6 — Fuzzing.** `cargo fuzz` target over the header/card parser and the tile decompressors,
run ad hoc rather than in CI. Any input must yield `Ok` or `Err`, never a panic and never an
unbounded allocation. Allocation guard: reject any declared array length that exceeds the
remaining file length before allocating.

### Performance targets

Absolute throughput numbers are not asserted up front — they are machine-dependent, and Phase 0
exists to establish real baselines. Two kinds of gate apply instead.

**Structural invariants** (exact, machine-independent, enforced by O1/O2):

| Operation | Invariant |
|---|---|
| `FitsReader::open` + read 8 keywords | Bytes read ≤ 2880 × header blocks; peak heap < 64 KiB |
| Full-frame `read_full::<T>` | Peak heap ≤ `len * size_of::<T>()` × 1.05 + scratch |
| `read_full_into` | Zero heap allocation in steady state |
| `read_region`, `w × h` rect | Bytes read ≤ `h × w × bytes_per_px` + slack; read calls = `h` |
| Streaming `rows()` | Peak heap ≤ one row + scratch, independent of image height |
| Write of an `n`-pixel image | Peak heap independent of `n` |

**Relative gates** (vs. the Phase 0 baseline, enforced by criterion):

| Workload | Gate |
|---|---|
| Header-only scan, 500 frames | ≥ 2× faster than the `fitsrs` baseline |
| Full-frame read, 60 MP 16-bit | ≥ parity in wall time, materially lower peak RSS than `read_raw` |
| Region read, 512×512 from a 60 MP frame | ≥ 20× faster than full read + crop |

Any commit that regresses a committed criterion baseline by > 10% must either be reverted or
land with the new baseline and a written justification.

---

## Consequences

- `fitsrs` is removed from `px-fits` dependencies at the end of Phase 3.
- New workspace dependencies: `memmap2` (optional), `flate2`, and dev-only `criterion`, `dhat`,
  `proptest`. All are declared in `[workspace.dependencies]` per repo convention.
- `crates/px-fits/README.md` becomes a real artifact: the benchmark report comparing the
  positioned-read default against the `mmap` feature across all three workloads and both
  platforms the project targets.
- `px-fits` gains write capability the project does not currently have — every FITS file
  produced today comes out of Siril. This unlocks the ROADMAP parking-lot item *"bulk edit or
  ensure a filter name is added to all files in a sequence"* via `update_header`.
- Two `xtask` subcommands are added: `fits-fixtures` and `fits-verify`.
- Consumer crates are untouched through Phase 8, except for the `all_fits_files` ordering fix
  in Phase 2.
- The `rustafits`/`astroimage` git dependency remains until the optional Phase 9 lands. Until
  then `px-fits` has two FITS readers in it: ours, and astroimage's internal one used only by
  `decode_preview`. This is accepted, and is the explicit cost of keeping Phase 9 deferrable.

---

## Implementation plan

### Handoff rules

These apply to every task below and are non-negotiable for delegated work.

1. **One phase per branch, one task per commit.** Conventional commit prefixes (`add:`, `fix:`,
   `chore:`).
2. **Do not modify files outside `crates/px-fits/`, `xtask/`, and the workspace `Cargo.toml`**
   unless the task explicitly says so.
3. **Every task ends green:** `cargo fmt`, `cargo clippy --verbose` (zero warnings),
   `cargo test --verbose`.
4. **Do not weaken a test to make it pass.** If an assertion in this document turns out to be
   wrong, stop and report it rather than editing the threshold.
5. **No `unsafe`** in `crates/px-fits/src/`. If a benchmark appears to demand it, stop and
   report the numbers; do not add it unilaterally.
6. **Phase gates are hard.** Do not begin phase N+1 until every acceptance criterion of phase N
   is demonstrably met.
7. **Write the spec reference into the code.** Every parsing rule carries a comment citing the
   FITS Standard 4.0 section it implements. Reviewers of this work will not have the spec
   memorized.

### Phase 0 — Harness and baselines

*Goal: make progress measurable before any code changes behaviour. Nothing in this phase alters
runtime behaviour.*

- [x] **P0-T1** Add workspace dependencies: `criterion = "0.5"`, `dhat = "0.3"`,
      `proptest = "1"`, `flate2 = "1"`, `memmap2 = "0.9"` (the first three dev-only at the crate
      level). Wire `[[bench]]` entries with `harness = false` in `crates/px-fits/Cargo.toml`.
- [x] **P0-T2** Add `xtask fits-fixtures`: generates the deterministic synthetic corpus described
      in O5 into `crates/px-fits/tests/fixtures/`. Uses a fixed PRNG seed. Must be idempotent —
      re-running produces byte-identical files. Commit the generated fixtures.
- [x] **P0-T3** Add `xtask fits-verify` (O4): runs `fitsverify` and/or astropy over a directory
      of FITS files; skips with a clear message when neither tool is installed.
- [x] **P0-T4** Add `CountingSource` (O1) and the `dhat` test scaffolding (O2) under
      `#[cfg(test)]` / a `bench-util` module.
- [x] **P0-T5** Write the four benchmark files with the *current* implementation as the subject:
      `header_scan` (fitsrs), `full_frame` (`astroimage::read_raw`), `region` (full read + crop),
      `write` (no-op placeholder that will be filled in Phase 5).
- [x] **P0-T6** Run all benchmarks, commit criterion baselines under
      `crates/px-fits/benches/baselines/`, and record the numbers plus machine/OS in a
      "Baseline" section of `crates/px-fits/README.md`.

**Gate:** `cargo bench -p px-fits` runs clean and the README has real numbers in it.

### Phase 1 — Byte sources, blocks, cards, headers

- [x] **P1-T1** `src/source.rs`: the `ByteSource` trait, `FileSource` (cross-platform positioned
      reads), `SliceSource`. Unit tests for offset/short-read/EOF behaviour on both platforms.
- [x] **P1-T2** `src/block.rs`: 2880-byte block arithmetic — block count for a byte length,
      padding sizes, offset-to-block conversions. Pure functions, exhaustively unit tested
      including the zero-length and exact-multiple edge cases.
- [x] **P1-T3** `src/card.rs`: 80-byte card parsing into `Card { keyword, value, comment }` and
      the `Value` enum (`Integer`, `Float`, `Logical`, `String`, `Complex`, `Undefined`,
      `Invalid(raw)`). Handles fixed and free format, quote escaping (`''`), `COMMENT`/`HISTORY`/
      blank keywords, `HIERARCH`, and the OGIP long-string `CONTINUE` convention. Serialization
      back to 80 bytes is implemented in the same task so a roundtrip property test can be
      written immediately.
- [x] **P1-T4** `proptest` roundtrip: any `Card` we construct serializes to exactly 80 ASCII
      bytes and re-parses to an equal `Card`.
- [x] **P1-T5** `src/header.rs`: `Header` with an ordered `Vec<Card>` plus a keyword → index map
      for O(1) lookup. Typed accessors (`get_string`, `get_f64`, `get_i64`, `get_bool`,
      `get_date_utc`), `naxis()`, `bitpix()`. Reads blocks until `END`; errors if `END` is
      absent before EOF.
- [x] **P1-T6** `src/error.rs`: the full `FitsError` enum. Every malformed fixture from O5 maps
      to a specific variant. Keep the existing variants (`Io`, `MissingPrimaryHDU`, `Processing`)
      for source compatibility; replace `Internal(fitsrs::error::Error)` with native variants.

**Gate:** every `fixtures/invalid/` file returns its designated error and none panics; card
roundtrip property test passes with 10k cases.

### Phase 2 — HDU discovery, lazy navigation, `FitsFile` cutover

- [x] **P2-T1** `src/hdu.rs`: HDU descriptor with header byte range and data-unit byte range,
      computed from `BITPIX` × ∏`NAXISn`. Lazy discovery — HDU *n+1* is located without reading
      HDU *n*'s data. `XTENSION` dispatch to `IMAGE` / `TABLE` / `BINTABLE`.
- [x] **P2-T2** `src/reader.rs`: `FitsReader<S>` with `open`, `from_source`, `primary`, `hdu`,
      `hdus`, `hdu_count`. Internal HDU descriptor cache so repeated access does not re-scan.
- [x] **P2-T3** Laziness test using `CountingSource`: opening a multi-extension fixture and
      reading the primary header touches only the primary header blocks.
- [x] **P2-T4** Rewrite `FitsFile` in `src/lib.rs` on top of `FitsReader`, preserving every
      current signature: `new`, `is_color`, `headers`, `key_values`, `header_rows`, `filter`,
      and the public `primary_hdu` field's `get_header()` access path used by
      `px-pipeline/src/meta.rs`. Port `HeaderUtil` to the new `Header`.
- [x] **P2-T5** Fix `all_fits_files`: currently it walks `read_dir` once per extension, which
      yields an extension-grouped, filesystem-ordered list. Callers such as
      `ObservationMetadata::from` take `paths.first()` and therefore depend on order. Walk the
      directory once, match both extensions case-insensitively, and return a sorted `Vec`.
- [x] **P2-T6** Verify all consumers compile and behave: `cargo build --workspace` and
      `cargo test --workspace` pass unchanged (`px`: 433 tests). The manual `cargo px inspect
      <file>` / `px obs scan` smoke tests were not run — this dev machine has no Siril install,
      and `inspect` requires it to start. Not a gap in `px-fits` coverage: `inspect`'s only
      px-fits calls (`FitsFile::new`, `.header_rows()`) are exercised directly by
      `tests/conformance.rs`. Flagging for whoever has Siril available to confirm end-to-end.

**Gate:** `fitsrs` is a dev-dependency only; the workspace builds; `differential.rs` passes over
the whole corpus.

### Phase 3 — Image data: full-frame reads

- [x] **P3-T1** `src/image/pixel.rs`: sealed `Pixel` trait over `u8, i16, u16, i32, u32, i64,
      f32, f64`; `BitPix` enum; big-endian decode via `from_be_bytes` over `chunks_exact`.
- [x] **P3-T2** `src/image/scaling.rs`: `BSCALE`/`BZERO` application with the `BZERO = 32768`,
      `BITPIX = 16` unsigned case handled as an integer fast path (D6). `BLANK` → `NaN` for
      float targets, passthrough for integer targets.
- [x] **P3-T3** `ImageHdu::read_full` / `read_full_into` — single output allocation, fixed
      scratch buffer, streaming conversion (D4). Scratch size configurable, 256 KiB default.
- [x] **P3-T4** `ImageHdu::rows()` streaming iterator with one reusable row buffer.
- [x] **P3-T5** Memory tests (O2): assert the full-frame and streaming peak-heap invariants from
      the targets table.
- [x] **P3-T6** Benchmark full-frame reads against the Phase 0 `astroimage::read_raw` baseline.
      Include the safe-decode-vs-hypothetical-transmute measurement called for in D2 and record
      the result in the README. If safe decoding is within noise, say so explicitly and close the
      question.
- [x] **P3-T7** Delete `tests/differential.rs` and drop the `fitsrs` dev-dependency.

**Gate:** every `BITPIX` fixture reads to correct values; peak-heap invariants hold; full-frame
throughput is at or above baseline.

### Phase 4 — Region selection

- [x] **P4-T1** `src/image/region.rs`: `Region { start, shape }` in FITS axis order, with
      `Region::rect(x, y, w, h)` for the 2D convenience case. Bounds validation against
      `NAXISn` with a typed out-of-range error.
- [x] **P4-T2** Run planner: decompose an N-D region into contiguous runs; one positioned read
      per run. Unit-test the plan itself (as data) for 1D through 4D cases before testing I/O.
- [x] **P4-T3** `read_region` / `read_region_into`, sharing the conversion path with Phase 3.
- [x] **P4-T4** `CountingSource` assertions: bytes read and read-call count match the Phase 4
      row in the invariants table exactly.
- [x] **P4-T5** Correctness: for every fixture, `read_region(r)` equals the corresponding slice
      of `read_full()`. Cover corners, single-pixel, single-row, single-column, full-extent, and
      out-of-bounds.
- [x] **P4-T6** Benchmark region reads vs. full-read-plus-crop; record the speedup in the README.
- [~] **P4-T7** Optional, only if the benchmark justifies it: coalesce adjacent runs into one
      read when the gap between them is smaller than the syscall cost. *Evaluated and declined:*
      subset rows of a wider image are never adjacent, so there is nothing to coalesce; the
      ≥ 20× gate is met without it (see `crates/px-fits/README.md`).

**Gate:** the ≥ 20× region-read gate is met and the byte-count assertions are exact.

### Phase 5 — Write support

- [ ] **P5-T1** `HeaderBuilder`: mandatory-keyword ordering enforced by construction (D7);
      rejects invalid `BITPIX`, non-ASCII, over-length keywords, and reserved-keyword misuse.
- [ ] **P5-T2** `src/writer.rs`: `FitsWriter<W>`, `write_image`, block padding with zero bytes
      for data and spaces for headers, per the standard.
- [ ] **P5-T3** `begin_image` streaming writer — write row by row so peak heap is independent of
      image size.
- [ ] **P5-T4** `update_header`: in-place edit when the rewritten header occupies the same block
      count; full-file rewrite via a temp file and atomic rename otherwise. Never leave a
      partially written file at the original path.
- [ ] **P5-T5** Roundtrip tests: write → read → compare for every `BITPIX`, dimensionality, and
      scaling combination.
- [ ] **P5-T6** External validation (O4): every generated file passes `fitsverify` with zero
      errors and opens cleanly in astropy. **This is a mandatory gate, not advisory.**
- [ ] **P5-T7** Fill in `benches/write.rs`; record throughput in the README.

**Gate:** `fitsverify` clean on all written output; roundtrips bit-exact.

### Phase 6 — Tables

- [ ] **P6-T1** `src/table/mod.rs`: shared column model — `ColumnDef { name, format, unit,
      null, scale, zero, dim }` parsed from `TTYPEn`/`TFORMn`/`TUNITn`/`TNULLn`/`TSCALn`/
      `TZEROn`/`TDIMn`.
- [ ] **P6-T2** `src/table/binary.rs`: `BINTABLE` reads. All standard `TFORM` codes
      (`L A X B I J K E D C M`), repeat counts, and `TDIM` reshaping. Row-wise and column-wise
      access; column access must read only that column's byte ranges — assert it with
      `CountingSource`.
- [ ] **P6-T3** Variable-length arrays: `P`/`Q` descriptors and the `PCOUNT` heap, including
      heap offset validation against the declared data-unit length (allocation guard, O6).
- [ ] **P6-T4** `src/table/ascii.rs`: `TABLE` reads — fixed-width `TBCOLn` fields with Fortran
      format codes (`A I F E D`).
- [ ] **P6-T5** Table writing via `TableBuilder`, mirroring Phase 5's structure.
- [ ] **P6-T6** Roundtrip and `fitsverify` validation for both table kinds, including
      variable-length columns.

**Gate:** table fixtures roundtrip; column-selective reads provably touch only their columns.

### Phase 7 — Tile-compressed images

- [ ] **P7-T1** `src/compress/mod.rs`: recognize the tiled-image convention (`ZIMAGE = T`),
      parse `ZBITPIX`, `ZNAXISn`, `ZTILEn`, `ZCMPTYPE`, `ZNAMEn`/`ZVALn`, and present a
      `CompressedImageHdu` with the *logical* image shape, so callers see it as an image.
      Depends on Phase 6 — the compressed data lives in a `BINTABLE`.
- [ ] **P7-T2** `src/compress/rice.rs`: `RICE_1` decoder (`BLOCKSIZE`, `BYTEPIX` parameters).
      Validate against reference files produced by cfitsio's `fpack`.
- [ ] **P7-T3** `src/compress/gzip.rs`: `GZIP_1` and `GZIP_2` (the latter byte-shuffled) via
      `flate2`.
- [ ] **P7-T4** `src/compress/plio.rs`: `PLIO_1` run-length decoder for mask images.
- [ ] **P7-T5** `HCOMPRESS_1` and any unknown `ZCMPTYPE` return
      `FitsError::UnsupportedCompression { .. }` — never a panic, never silent garbage.
- [ ] **P7-T6** **Region reads on compressed images:** decompress only the tiles the region
      intersects. This is the whole reason tiling exists and is the highest-value item in this
      phase. `CountingSource` asserts that untouched tiles are never read.
- [ ] **P7-T7** Compressed-image writing (`RICE_1` and `GZIP_1` only), validated with `funpack`
      or astropy.
- [ ] **P7-T8** Benchmark compressed full-frame and region reads; add to the README.

**Gate:** `fpack`-produced reference files decompress bit-exactly; tile-selective region reads
verified.

### Phase 8 — mmap backend and the README benchmark report

- [ ] **P8-T1** `MmapSource` behind the non-default `mmap` feature; `as_slice` returns the
      mapping so hot paths skip the copy.
- [ ] **P8-T2** Specialize the read paths on `as_slice()` returning `Some` — decode directly
      from the mapped bytes with no intermediate scratch buffer.
- [ ] **P8-T3** Document the safety contract in rustdoc: the caller must not allow the file to
      be truncated or written while mapped; recommend against mmap on network filesystems.
- [ ] **P8-T4** Add a `rayon`-parallelized positioned-read path as the buffered contender: for
      the header-scan and multi-tile-region workloads, fan the reads across a `rayon` thread
      pool over shared `FileSource` handles (safe — positioned reads take `&self`, no locking
      needed) rather than reading serially. This is the "buffered+rayon" side of the comparison
      the mmap-drop decision (D1) depends on; without it the mmap-vs-positioned comparison would
      only ever test single-threaded positioned reads, which is not the real alternative.
- [ ] **P8-T5** Run the full CI matrix with `--features mmap` as well as default.
- [ ] **P8-T6** **Write the README benchmark report.** For all three prioritized workloads —
      full-frame throughput and peak RSS, header-only scan across thousands of files, and
      region/subset reads — report positioned-read (serial and `rayon`-parallelized) vs. mmap on
      macOS and Windows, cold and warm page cache, at several file sizes. State a recommendation
      and the reasoning, including where mmap loses (small files, cold cache, network volumes)
      and not only where it wins.
- [ ] **P8-T7** Decide the `mmap` feature's fate from the measured data, per the D1 open
      question: if parallelized positioned reads are competitive, remove `MmapSource`, the
      `mmap` feature, and the `memmap2` dependency rather than keeping an unused-by-default
      backend around. If mmap wins decisively for a specific workload, expose that as an
      explicit per-call opt-in rather than flipping the crate-wide default.

**Gate:** the README report exists, covers both platforms, includes the `rayon`-parallelized
positioned-read comparison, and makes a defensible recommendation — including "drop `mmap`" as
a legitimate outcome, not just "keep it, tuned."

### Phase 9 — `astroimage` removal *(deferrable; do not start without explicit approval)*

*This phase leaves FITS parsing entirely and enters image processing. It is scoped here so the
dependency-removal path is written down, not because it is committed work.*

- [ ] **P9-T1** Bayer pattern detection from `BAYERPAT`/`XBAYROFF`/`YBAYROFF`.
- [ ] **P9-T2** Debayer — bilinear first, correctness-checked against `astroimage` output; VNG
      or better only if quality demands it.
- [ ] **P9-T3** STF autostretch (midtone transfer function) matching current preview output.
- [ ] **P9-T4** Integer downscale honouring `MAX_DISPLAY_DIM`, including the even-factor
      constraint the current Bayer path requires.
- [ ] **P9-T5** Rewire `decode_preview` onto the native pipeline; keep the signature and
      `PreviewImage` shape identical so `px-nativeui` is untouched.
- [ ] **P9-T6** Visual regression: preview output for a corpus of frames compared against
      astroimage's, within a stated per-pixel tolerance.
- [ ] **P9-T7** Remove the `rustafits` git dependency; delete the dead commented-out
      `ImageAnalyzer` block in `display.rs`.
- [ ] **P9-T8** Decide separately whether star analysis (FWHM/eccentricity — currently commented
      out) is reimplemented or dropped. Out of scope for this ADR either way.

---

## Risks

- **Silent numerical drift vs. the current reader.**
  Differential testing against `fitsrs` (O3) through Phase 3; in Phase 4 every region read is
  checked against the corresponding slice of a full read.
- **Real-world files violate the standard in ways the fixtures do not.**
  `PX_FITS_CORPUS` opt-in over actual capture data. Parsing is permissive: a non-conforming card
  becomes `Value::Invalid(raw)` rather than failing the whole file.
- **The compression phase balloons in scope.**
  `HCOMPRESS_1` is excluded up front, and `fpack`-produced files serve as a bit-exact oracle so
  "done" is unambiguous.
- **mmap behaves differently on macOS and Windows.**
  Non-default feature; positioned reads remain the default and the primary tested path.
- **Handoff drift across agents.**
  Hard phase gates, structural invariants instead of prose goals, per-task acceptance criteria.
- **Delegated work weakens a failing assertion to go green.**
  Handoff rule 4: stop and report rather than edit a threshold. Review each phase gate against
  this document, not against the tests as committed.

---

## References

- FITS Standard 4.0 — <https://fits.gsfc.nasa.gov/fits_standard.html>
- Tiled Image Compression Convention — <https://fits.gsfc.nasa.gov/registry/tilecompression.html>
- OGIP long-string `CONTINUE` convention — <https://fits.gsfc.nasa.gov/registry/continue_keyword.html>
- cfitsio, for `fits_read_subset` semantics — <https://heasarc.gsfc.nasa.gov/fitsio/>
- `fitsverify` — <https://heasarc.gsfc.nasa.gov/docs/software/ftools/fitsverify/>
