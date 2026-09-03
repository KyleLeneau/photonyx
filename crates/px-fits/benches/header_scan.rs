//! Phase 0 baseline (ADR 006): header-only scan across many files using the
//! *current* `fitsrs`-backed `FitsFile`. This is the workload
//! `all_fits_files` + `CalibrationMetadata::from` exercise across a session's
//! raw frames — the benchmark this crate must beat by >= 2x once the native
//! reader lands (Phase 1-2).

use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use px_fits::FitsFile;

#[path = "support.rs"]
mod support;

/// Builds N copies of a small header-bearing fixture so the scan has a
/// realistic file count without inflating the committed corpus.
fn corpus(n: usize) -> Vec<PathBuf> {
    let dir = support::bench_scratch_dir().join("header_scan");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let src = support::fixtures_dir().join("bitpix16_2d_64x48.fits");
    let bytes = std::fs::read(&src).expect("read seed fixture");

    (0..n)
        .map(|i| {
            let path = dir.join(format!("frame_{i:04}.fits"));
            if !path.exists() {
                std::fs::write(&path, &bytes).expect("write corpus copy");
            }
            path
        })
        .collect()
}

fn bench_header_scan(c: &mut Criterion) {
    let files = corpus(500);

    c.bench_function("header_scan/500_files/fitsrs_baseline", |b| {
        b.iter(|| {
            for path in &files {
                let file = FitsFile::new(path.clone()).expect("open fixture");
                black_box(file.header_rows());
            }
        });
    });
}

criterion_group!(benches, bench_header_scan);
criterion_main!(benches);
