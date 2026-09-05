//! Region/subset pixel reads (ADR 006 Phase 0 baseline + Phase 4 native).
//!
//! Neither `fitsrs` nor `astroimage` offers a subset-read primitive, so the
//! "before" baseline is "read the full frame, then crop" — exactly what
//! region selection exists to eliminate. The native `read_region` path is
//! measured against it here; the ADR gate is >= 20x for a 512x512 window
//! out of a ~60 MP frame (the baseline scales with total pixels, the region
//! read does not, so the ratio grows with frame size).

use std::hint::black_box;

use astroimage::{ImageConverter, PixelData};
use criterion::{Criterion, criterion_group, criterion_main};
use px_fits::Region;
use px_fits::reader::FitsReader;
use px_fits::source::{FileSource, SliceSource};

#[path = "support.rs"]
mod support;

// ~60 MP, matching the frame size the ADR states the >= 20x gate against.
const W: usize = 7744;
const H: usize = 7744;
const X0: usize = 2048;
const Y0: usize = 2048;
const RW: usize = 512;
const RH: usize = 512;

/// Crops a `RW x RH` region out of a full row-major `Uint16` frame — the
/// "full read + crop" baseline behaviour region selection replaces.
fn crop(pixels: &PixelData, width: usize) -> Vec<u16> {
    let PixelData::Uint16(data) = pixels else {
        panic!("synthetic fixture is always Uint16");
    };
    let mut out = Vec::with_capacity(RW * RH);
    for y in Y0..Y0 + RH {
        let row_start = y * width + X0;
        out.extend_from_slice(&data[row_start..row_start + RW]);
    }
    out
}

fn bench_region(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("region");
    let path = support::write_synthetic_i16_image(&dir, W, H);
    let file_bytes = std::fs::read(&path).expect("read synthetic frame");
    let region = Region::rect(X0, Y0, RW, RH);

    let mut group = c.benchmark_group("region/512x512_from_7744x7744_i16");

    group.bench_function("full_read_plus_crop_baseline", |b| {
        b.iter(|| {
            let (meta, pixels) = ImageConverter::read_raw(&path).expect("read_raw");
            black_box(crop(&pixels, meta.width));
        });
    });

    group.bench_function("native_read_region_file_source", |b| {
        b.iter(|| {
            let reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
            let px = reader
                .primary_image()
                .unwrap()
                .read_region::<i16>(black_box(&region))
                .unwrap();
            black_box(px);
        });
    });

    group.bench_function("native_read_region_into_file_source", |b| {
        let reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
        let mut buf = vec![0i16; RW * RH];
        b.iter(|| {
            reader
                .primary_image()
                .unwrap()
                .read_region_into::<i16>(black_box(&region), &mut buf)
                .unwrap();
            black_box(&buf);
        });
    });

    group.bench_function("native_read_region_slice_source", |b| {
        b.iter(|| {
            let reader = FitsReader::from_source(SliceSource::new(file_bytes.clone())).unwrap();
            let px = reader
                .primary_image()
                .unwrap()
                .read_region::<i16>(black_box(&region))
                .unwrap();
            black_box(px);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_region);
criterion_main!(benches);
