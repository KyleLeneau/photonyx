//! Phase 0 baseline (ADR 006): full-frame read throughput using the *current*
//! `astroimage::ImageConverter::read_raw` path (what `display::decode_preview`
//! calls). Peak-RSS/heap comparisons live in `tests/memory.rs` (criterion
//! measures wall time, not allocation).

use std::hint::black_box;

use astroimage::ImageConverter;
use criterion::{Criterion, criterion_group, criterion_main};

#[path = "support.rs"]
mod support;

fn bench_full_frame(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("full_frame");
    let path = support::write_synthetic_i16_image(&dir, 4096, 4096);

    let mut group = c.benchmark_group("full_frame/4096x4096_i16");
    group.bench_function("astroimage_read_raw_baseline", |b| {
        b.iter(|| {
            let (meta, pixels) = ImageConverter::read_raw(&path).expect("read_raw");
            black_box((meta.width, meta.height));
            black_box(pixels);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_full_frame);
criterion_main!(benches);
