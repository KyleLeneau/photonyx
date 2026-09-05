//! `decode_preview` throughput (ADR 006 Phase 9): the Bayer and mono paths
//! at a representative sensor size, so a future change to debayer/stretch/
//! downscale has a number to check itself against.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use px_imageproc::decode_preview;

#[path = "support.rs"]
mod support;

const W: usize = 4096;
const H: usize = 4096;

fn bench_decode_preview(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("decode_preview");
    let bayer_path = support::write_synthetic_i16_image(&dir, W, H, Some("RGGB"));
    let mono_path = support::write_synthetic_i16_image(&dir, W, H, None);

    let mut group = c.benchmark_group("decode_preview/4096x4096_i16");
    group.throughput(Throughput::Bytes((W * H * 2) as u64));
    group.sample_size(20);

    group.bench_function("bayer_rggb", |b| {
        b.iter(|| {
            let preview = decode_preview(&bayer_path).expect("decode bayer preview");
            black_box(preview.pixels);
        });
    });

    group.bench_function("mono", |b| {
        b.iter(|| {
            let preview = decode_preview(&mono_path).expect("decode mono preview");
            black_box(preview.pixels);
        });
    });

    // Baseline: the astroimage-backed path this crate replaces (dev-dependency
    // only — never a runtime dependency of this crate or px-fits).
    group.bench_function("bayer_rggb_astroimage_baseline", |b| {
        b.iter(|| {
            let (_, _, pixels) = support::astroimage_oracle_decode_preview(&bayer_path);
            black_box(pixels);
        });
    });

    group.bench_function("mono_astroimage_baseline", |b| {
        b.iter(|| {
            let (_, _, pixels) = support::astroimage_oracle_decode_preview(&mono_path);
            black_box(pixels);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_decode_preview);
criterion_main!(benches);
