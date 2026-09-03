//! Phase 0 peak-heap harness (ADR 006 O2). Integration test binaries get
//! their own crate root, so this is the one place in the crate that can set
//! `dhat::Alloc` as the global allocator without affecting the library build
//! or other test binaries.
//!
//! The assertion here is deliberately loose: it exercises the *current*
//! `astroimage::ImageConverter::read_raw` path (what `display::decode_preview`
//! calls today) and only proves the dhat harness itself works end to end.
//! The tight, structural invariants from ADR 006 ("peak heap <=
//! output_bytes * 1.05 + scratch") apply to the native reader starting in
//! Phase 3 and belong in this same file once `ImageHdu::read_full` exists.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[path = "support.rs"]
mod support;

#[test]
fn full_frame_baseline_peak_heap_is_bounded() {
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
