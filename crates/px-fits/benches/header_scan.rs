//! Header-only scan across many files, via the public `FitsFile` API. This
//! is the workload `all_fits_files` + `CalibrationMetadata::from` exercise
//! across a session's raw frames.
//!
//! The Phase 0 baseline (`fitsrs`-backed `FitsFile`, benchmark name
//! `header_scan/500_files/fitsrs_baseline`) is frozen in
//! `benches/baselines/header_scan.json` and the README's Baseline report —
//! that number doesn't change. As of the Phase 2 cutover this benchmark
//! itself measures the *native* `FitsFile`, so the function name below no
//! longer says `fitsrs`; comparing a fresh run's numbers against the
//! committed baseline JSON is exactly how the ADR 006 ">= 2x" gate is
//! checked.

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

    c.bench_function("header_scan/500_files/fits_file_open_and_scan", |b| {
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
