//! Full-frame read throughput (ADR 006 Phase 0 baseline + Phase 3 native).
//!
//! Two groups:
//! - `full_frame/4096x4096_i16`: the Phase 0 `astroimage::read_raw` baseline
//!   next to the native `ImageHdu::read_full` / `read_full_into` paths over a
//!   `FileSource` (streaming scratch) and a `SliceSource` (whole-file
//!   zero-copy decode).
//! - `decode/16Mpx_i16`: the ADR 006 D2 question — is the safe
//!   `from_be_bytes`-over-`chunks_exact` decode materially slower than a
//!   transmute + byte-swap? Measured against a `memcpy` ceiling. The
//!   `unsafe` contender lives here, in a bench, never in `src/`.
//!
//! Peak-RSS/heap comparisons live in `tests/memory.rs` (criterion measures
//! wall time, not allocation).

use std::hint::black_box;

use astroimage::ImageConverter;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use px_fits::reader::FitsReader;
use px_fits::source::{FileSource, SliceSource};

#[path = "support.rs"]
mod support;

const W: usize = 4096;
const H: usize = 4096;

fn bench_full_frame(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("full_frame");
    let path = support::write_synthetic_i16_image(&dir, W, H);
    let file_bytes = std::fs::read(&path).expect("read synthetic frame");

    let mut group = c.benchmark_group("full_frame/4096x4096_i16");
    group.throughput(Throughput::Bytes((W * H * 2) as u64));

    group.bench_function("astroimage_read_raw_baseline", |b| {
        b.iter(|| {
            let (meta, pixels) = ImageConverter::read_raw(&path).expect("read_raw");
            black_box((meta.width, meta.height));
            black_box(pixels);
        });
    });

    group.bench_function("native_read_full_i16_file_source", |b| {
        b.iter(|| {
            let reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
            let pixels = reader.primary_image().unwrap().read_full::<i16>().unwrap();
            black_box(pixels);
        });
    });

    group.bench_function("native_read_full_into_i16_file_source", |b| {
        let reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
        let count = reader.primary_image().unwrap().len();
        let mut buf = vec![0i16; count];
        b.iter(|| {
            reader
                .primary_image()
                .unwrap()
                .read_full_into(black_box(&mut buf))
                .unwrap();
            black_box(&buf);
        });
    });

    group.bench_function("native_read_full_i16_slice_source", |b| {
        b.iter(|| {
            let reader = FitsReader::from_source(SliceSource::new(file_bytes.clone())).unwrap();
            let pixels = reader.primary_image().unwrap().read_full::<i16>().unwrap();
            black_box(pixels);
        });
    });

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    // 16 Mi big-endian i16 samples in memory — no I/O in this group.
    let n = W * H;
    let mut raw = Vec::with_capacity(n * 2);
    let mut x: u32 = 0x1234_5678;
    for _ in 0..n {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        raw.extend_from_slice(&(x as i16).to_be_bytes());
    }

    let mut group = c.benchmark_group("decode/16Mpx_i16");
    group.throughput(Throughput::Bytes((n * 2) as u64));

    // Ceiling: a straight byte copy of the same volume.
    group.bench_function("memcpy_ceiling", |b| {
        let mut dst = vec![0u8; raw.len()];
        b.iter(|| {
            dst.copy_from_slice(black_box(&raw));
            black_box(&dst);
        });
    });

    // What `px_fits::image::pixel::decode` actually does for BITPIX=16.
    group.bench_function("safe_from_be_bytes_chunks_exact", |b| {
        let mut dst = vec![0i16; n];
        b.iter(|| {
            for (d, ch) in dst.iter_mut().zip(black_box(&raw).chunks_exact(2)) {
                *d = i16::from_be_bytes([ch[0], ch[1]]);
            }
            black_box(&dst);
        });
    });

    // Hypothetical `unsafe` alternative (D2): reinterpret the buffer as
    // `&[i16]` and byte-swap in place. Bench-only; this pattern is exactly
    // what the "no unsafe unless a benchmark demands it" rule is weighed
    // against.
    group.bench_function("unsafe_transmute_then_swap_bytes", |b| {
        let mut dst = vec![0i16; n];
        b.iter(|| {
            let src = black_box(&raw);
            let base = src.as_ptr();
            for (i, d) in dst.iter_mut().enumerate() {
                // SAFETY (bench only): `i*2 + 1 < src.len()`, `i16` has no
                // invalid bit patterns, `read_unaligned` needs no alignment.
                let s = unsafe { (base.add(i * 2) as *const i16).read_unaligned() };
                *d = i16::from_be(s);
            }
            black_box(&dst);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_full_frame, bench_decode);
criterion_main!(benches);
