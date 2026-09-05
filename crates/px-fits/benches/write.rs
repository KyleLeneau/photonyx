//! Image-write throughput (ADR 006 Phase 5). `FitsWriter::write_image` and
//! the row-streaming `begin_image` path are measured against the Phase 0
//! floor — a bare `std::fs::write` of the same byte volume — so "no slower
//! than write(2) plus endianness encoding" is a checkable claim.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use px_fits::header::BitPix;
use px_fits::{FitsWriter, HeaderBuilder};

#[path = "support.rs"]
mod support;

const W: usize = 4096;
const H: usize = 4096;

fn bench_write(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("write");
    std::fs::create_dir_all(&dir).expect("create scratch dir");

    let pixels: Vec<i16> = (0..(W * H) as i64).map(|i| i as i16).collect();
    let raw_bytes = vec![0u8; W * H * 2];

    let mut group = c.benchmark_group("write/4096x4096_i16");
    group.throughput(Throughput::Bytes((W * H * 2) as u64));

    let floor_path = dir.join("floor.bin");
    group.bench_function("raw_fs_write_floor", |b| {
        b.iter(|| {
            std::fs::write(&floor_path, black_box(&raw_bytes)).expect("raw write");
            black_box(&floor_path);
        });
    });

    let bulk_path = dir.join("bulk.fits");
    group.bench_function("fits_writer_write_image", |b| {
        let header = HeaderBuilder::primary_image(BitPix::I16, &[W as u64, H as u64]).unwrap();
        b.iter(|| {
            let mut w = FitsWriter::create(&bulk_path).unwrap();
            w.write_image::<i16>(&header, black_box(&pixels)).unwrap();
            w.finish().unwrap();
        });
    });

    let stream_path = dir.join("stream.fits");
    group.bench_function("fits_writer_begin_image_row_streaming", |b| {
        let header = HeaderBuilder::primary_image(BitPix::I16, &[W as u64, H as u64]).unwrap();
        b.iter(|| {
            let mut w = FitsWriter::create(&stream_path).unwrap();
            {
                let mut iw = w.begin_image::<i16>(&header).unwrap();
                for row in pixels.chunks(W) {
                    iw.write_row(black_box(row)).unwrap();
                }
                iw.finish().unwrap();
            }
            w.finish().unwrap();
        });
    });

    group.finish();
}

/// Write throughput straight to memory (no filesystem), isolating the
/// endianness-encoding + block-framing cost from disk I/O.
fn bench_write_in_memory(c: &mut Criterion) {
    let pixels: Vec<i16> = (0..(W * H) as i64).map(|i| i as i16).collect();
    let mut group = c.benchmark_group("write/4096x4096_i16_in_memory");
    group.throughput(Throughput::Bytes((W * H * 2) as u64));

    group.bench_function("fits_writer_write_image_to_vec", |b| {
        let header = HeaderBuilder::primary_image(BitPix::I16, &[W as u64, H as u64]).unwrap();
        b.iter(|| {
            let mut buf: Vec<u8> = Vec::with_capacity(W * H * 2 + 2880);
            {
                let mut w = FitsWriter::new(&mut buf);
                w.write_image::<i16>(&header, black_box(&pixels)).unwrap();
                let _ = w.finish().unwrap();
            }
            black_box(buf.len());
        });
    });

    group.bench_function("memcpy_plus_byteswap_ceiling", |b| {
        b.iter(|| {
            let mut buf: Vec<u8> = Vec::with_capacity(W * H * 2);
            for &v in black_box(&pixels) {
                buf.extend_from_slice(&v.to_be_bytes());
            }
            black_box(buf.len());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_write, bench_write_in_memory);
criterion_main!(benches);
