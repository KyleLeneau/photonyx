//! Positioned-read backend comparison for the ADR 006 Phase 8 report
//! (P8-T6): `FileSource` serial vs. `rayon`-parallelized, across the three
//! prioritized workloads — full-frame read, header-only scan, region read.
//!
//! The memory-mapped backend that this bench originally also covered was
//! removed in P8-T7 (rayon-parallelized positioned reads won); the report in
//! README.md keeps its numbers. Warm page cache only.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use px_fits::reader::FitsReader;
use px_fits::source::FileSource;
use px_fits::{FitsFile, Region};
use rayon::prelude::*;

#[path = "support.rs"]
mod support;

fn pool(n: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .build()
        .unwrap()
}

fn header_corpus(count: usize) -> Vec<PathBuf> {
    let dir = support::bench_scratch_dir().join(format!("backend/hdr_{count}"));
    std::fs::create_dir_all(&dir).unwrap();
    let src = support::write_synthetic_i16_image(&dir.join("gen"), 8, 8);
    (0..count)
        .map(|i| {
            let p = dir.join(format!("f{i:05}.fits"));
            if !p.exists() {
                std::fs::copy(&src, &p).unwrap();
            }
            p
        })
        .collect()
}

fn bench_full_frame(c: &mut Criterion) {
    let one = pool(1);
    let many = pool(0);

    for &(w, h) in &[(2048usize, 2048usize), (6144, 6144)] {
        let path = support::write_synthetic_i16_image(
            &support::bench_scratch_dir().join("backend/frame"),
            w,
            h,
        );
        let mut g = c.benchmark_group(format!("backend_full_frame/{w}x{h}_i16"));
        g.throughput(Throughput::Bytes((w * h * 2) as u64));

        let read = |p: &rayon::ThreadPool| {
            p.install(|| {
                let r = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
                black_box(r.primary_image().unwrap().read_full::<i16>().unwrap());
            })
        };
        g.bench_function("file_source_serial", |b| b.iter(|| read(&one)));
        g.bench_function("file_source_rayon", |b| b.iter(|| read(&many)));
        g.finish();
    }
}

fn bench_header_scan(c: &mut Criterion) {
    let count = 2000;
    let paths = header_corpus(count);

    let mut g = c.benchmark_group("backend_header_scan/2000_files");
    g.throughput(Throughput::Elements(count as u64));

    g.bench_function("serial", |b| {
        b.iter(|| {
            for p in &paths {
                black_box(
                    FitsFile::new(p.clone())
                        .unwrap()
                        .primary_hdu
                        .get_header()
                        .cards()
                        .len(),
                );
            }
        })
    });
    g.bench_function("rayon", |b| {
        b.iter(|| {
            let n: usize = paths
                .par_iter()
                .map(|p| {
                    FitsFile::new(p.clone())
                        .unwrap()
                        .primary_hdu
                        .get_header()
                        .cards()
                        .len()
                })
                .sum();
            black_box(n);
        })
    });
    g.finish();
}

fn bench_region(c: &mut Criterion) {
    let one = pool(1);
    let many = pool(0);
    let path = support::write_synthetic_i16_image(
        &support::bench_scratch_dir().join("backend/frame"),
        6144,
        6144,
    );
    let region = Region::rect(1024, 1024, 2048, 2048);

    let mut g = c.benchmark_group("backend_region/2048x2048_from_6144x6144_i16");
    g.throughput(Throughput::Bytes((2048 * 2048 * 2) as u64));

    let run = |p: &rayon::ThreadPool, path: &Path| {
        p.install(|| {
            let r = FitsReader::from_source(FileSource::open(path).unwrap()).unwrap();
            black_box(
                r.primary_image()
                    .unwrap()
                    .read_region::<i16>(&region)
                    .unwrap(),
            );
        })
    };
    g.bench_function("file_source_serial", |b| b.iter(|| run(&one, &path)));
    g.bench_function("file_source_rayon", |b| b.iter(|| run(&many, &path)));
    g.finish();
}

criterion_group!(benches, bench_full_frame, bench_header_scan, bench_region);
criterion_main!(benches);
