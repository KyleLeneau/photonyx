//! Peak-heap harness (ADR 006 O2). Integration test binaries get their own
//! crate root, so this is the one place in the crate that can set
//! `dhat::Alloc` as the global allocator without affecting the library build
//! or other test binaries.
//!
//! `full_frame_baseline_peak_heap_is_bounded` exercises the *current*
//! `astroimage::ImageConverter::read_raw` path with a deliberately loose
//! bound — it exists to be beaten, and to prove the harness works end to
//! end. The `native_*` tests assert the tight, structural invariants from
//! ADR 006's performance-targets table against `ImageHdu` (Phase 3).
//!
//! `dhat` allows only one live `Profiler` per process and its `HeapStats`
//! are process-global, so every test here takes `DHAT_LOCK` for the whole
//! profiler window — otherwise a second test's `Profiler::new_heap()`
//! panics and concurrent allocations pollute the numbers.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[path = "support.rs"]
mod support;

use std::sync::Mutex;

use px_fits::image::DEFAULT_SCRATCH_BYTES;
use px_fits::reader::FitsReader;
use px_fits::source::{FileSource, SliceSource};

static DHAT_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn full_frame_baseline_peak_heap_is_bounded() {
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = support::scratch_dir().join("memory");
    let path = support::write_synthetic_i16_image(&dir, 512, 512);

    let profiler = dhat::Profiler::new_heap();
    let (meta, pixels) = astroimage::ImageConverter::read_raw(&path).expect("read_raw");
    let stats = dhat::HeapStats::get();
    drop(profiler);

    let pixel_count = meta.width * meta.height;
    match &pixels {
        astroimage::PixelData::Uint16(v) => assert_eq!(v.len(), pixel_count),
        astroimage::PixelData::Float32(v) => assert_eq!(v.len(), pixel_count),
    }

    // Generous bound: current impl is known to allocate more than once (raw
    // bytes, then the typed Vec) and this baseline exists to be beaten, not
    // to gate CI. 4x the theoretical minimum (u16 output) catches only gross
    // regressions like accidentally reading the file twice.
    let theoretical_min = pixel_count * std::mem::size_of::<u16>();
    assert!(
        stats.max_bytes <= theoretical_min * 4,
        "peak heap {} far exceeds 4x theoretical minimum {} for a {}x{} frame; \
         harness bound itself may be broken",
        stats.max_bytes,
        theoretical_min * 4,
        meta.width,
        meta.height,
    );

    eprintln!(
        "[baseline] astroimage::read_raw peak heap = {} bytes for {}x{} u16 frame \
         (theoretical minimum = {} bytes, ratio = {:.2}x)",
        stats.max_bytes,
        meta.width,
        meta.height,
        theoretical_min,
        stats.max_bytes as f64 / theoretical_min as f64,
    );
}

// --- Native `ImageHdu` structural invariants (ADR 006 Phase 3) --------------
//
// dhat's global instrumentation and the test harness allocate a little on
// their own during the profiler window, so "zero allocation" is asserted as
// a small fixed ceiling (`NOISE`) rather than a literal 0. What matters is
// that the ceiling is a *constant* — it does not scale with pixel count, so
// a decode path that accidentally materialized the frame would blow past it.
const NOISE: usize = 16 * 1024;

#[test]
fn native_read_full_into_over_slice_source_does_not_allocate_per_pixel() {
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = support::scratch_dir().join("memory-native");
    let path = support::write_synthetic_i16_image(&dir, 512, 512); // 512 KiB of u16

    let bytes = std::fs::read(&path).expect("read fixture");
    let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
    let img = reader.primary_image().unwrap();
    let mut out = vec![0u16; img.len()]; // allocated before the profiler starts

    let profiler = dhat::Profiler::new_heap();
    img.read_full_into(&mut out).unwrap();
    let stats = dhat::HeapStats::get();
    drop(profiler);

    assert!(
        stats.max_bytes <= NOISE,
        "read_full_into over a slice source allocated {} bytes (> {NOISE}); \
         it must decode straight into the caller buffer",
        stats.max_bytes,
    );
}

#[test]
fn native_read_full_peak_heap_is_output_plus_scratch() {
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = support::scratch_dir().join("memory-native");
    let path = support::write_synthetic_i16_image(&dir, 1024, 1024);

    let reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
    let img = reader.primary_image().unwrap();
    let count = img.len();

    let profiler = dhat::Profiler::new_heap();
    let pixels = img.read_full::<i32>().unwrap();
    let stats = dhat::HeapStats::get();
    drop(profiler);

    assert_eq!(pixels.len(), count);
    let output_bytes = count * std::mem::size_of::<i32>();
    let bound = (output_bytes as f64 * 1.05) as usize + DEFAULT_SCRATCH_BYTES + NOISE;
    assert!(
        stats.max_bytes <= bound,
        "peak heap {} exceeds output ({output_bytes}) * 1.05 + scratch \
         ({DEFAULT_SCRATCH_BYTES}) = {bound}",
        stats.max_bytes,
    );
}

#[test]
fn native_rows_peak_heap_is_one_row_regardless_of_height() {
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = support::scratch_dir().join("memory-native");
    let short = support::write_synthetic_i16_image(&dir, 1024, 8);
    let tall = support::write_synthetic_i16_image(&dir, 1024, 2048);

    let peak = |path: &std::path::Path| {
        let reader = FitsReader::from_source(FileSource::open(path).unwrap()).unwrap();
        let img = reader.primary_image().unwrap();
        let profiler = dhat::Profiler::new_heap();
        let mut rows = img.rows::<i32>();
        let mut sum = 0i64;
        while let Some(row) = rows.next_row() {
            for &v in row.unwrap() {
                sum += v as i64;
            }
        }
        let stats = dhat::HeapStats::get();
        drop(profiler);
        std::hint::black_box(sum);
        stats.max_bytes
    };

    let short_peak = peak(&short);
    let tall_peak = peak(&tall);

    // 1024-wide i16 source -> i32 rows: 2 KiB raw + 4 KiB decoded ~= 6 KiB.
    let one_row_bound = 1024 * (2 + 4) + NOISE;
    assert!(
        tall_peak <= one_row_bound,
        "tall-image rows() peak {tall_peak} exceeds one-row bound {one_row_bound}"
    );
    assert!(
        tall_peak <= short_peak + NOISE && short_peak <= tall_peak + NOISE,
        "rows() peak heap must not scale with image height ({short_peak} vs {tall_peak})"
    );
}
