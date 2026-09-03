//! Phase 0 placeholder (ADR 006 P0-T5): `px-fits` has no write capability yet
//! (every FITS file in the project today comes out of Siril). Write support
//! lands in Phase 5, at which point this file gains a real benchmark of
//! `FitsWriter::write_image` throughput.
//!
//! Until then this measures the floor -- raw `std::fs::write` of an
//! equivalently sized buffer -- so Phase 5 has a "no slower than a bare
//! write(2) call" sanity check to compare against, not just a fresh
//! criterion group with no history.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};

#[path = "support.rs"]
mod support;

fn bench_write_floor(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("write");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let path = dir.join("floor.bin");

    // 4096x4096 16-bit pixels, matching the full_frame/region bench size.
    let buf = vec![0u8; 4096 * 4096 * 2];

    c.bench_function("write/4096x4096_i16_equivalent/raw_fs_write_floor", |b| {
        b.iter(|| {
            std::fs::write(&path, &buf).expect("raw write");
            black_box(&path);
        });
    });
}

criterion_group!(benches, bench_write_floor);
criterion_main!(benches);
