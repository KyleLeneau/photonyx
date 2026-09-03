//! Phase 0 baseline (ADR 006): region/subset pixel reads. Neither `fitsrs` nor
//! `astroimage` offers a subset-read primitive, so the only baseline available
//! today is "read the full frame, then crop" — exactly the pattern region
//! selection (Phase 4) exists to eliminate. This benchmark is the "before"
//! half of the >= 20x gate in ADR 006.

use std::hint::black_box;

use astroimage::{ImageConverter, PixelData};
use criterion::{Criterion, criterion_group, criterion_main};

#[path = "support.rs"]
mod support;

/// Crops a 512x512 region out of a full row-major `Uint16` frame — the
/// "full read + crop" baseline behaviour region selection replaces.
fn crop_512(pixels: &PixelData, width: usize) -> Vec<u16> {
    let PixelData::Uint16(data) = pixels else {
        panic!("synthetic fixture is always Uint16");
    };
    let (x0, y0, w, h) = (256usize, 256usize, 512usize, 512usize);
    let mut out = Vec::with_capacity(w * h);
    for y in y0..y0 + h {
        let row_start = y * width + x0;
        out.extend_from_slice(&data[row_start..row_start + w]);
    }
    out
}

fn bench_region(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("region");
    let path = support::write_synthetic_i16_image(&dir, 4096, 4096);

    let mut group = c.benchmark_group("region/512x512_from_4096x4096_i16");
    group.bench_function("full_read_plus_crop_baseline", |b| {
        b.iter(|| {
            let (meta, pixels) = ImageConverter::read_raw(&path).expect("read_raw");
            let cropped = crop_512(&pixels, meta.width);
            black_box(cropped);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_region);
criterion_main!(benches);
