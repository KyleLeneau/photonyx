//! Tile-compressed image read throughput (ADR 006 P7-T8): `RICE_1`
//! full-frame decode vs. an uncompressed read of the same frame, and a
//! `RICE_1` region read vs. decompressing the whole frame.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use px_fits::header::BitPix;
use px_fits::reader::FitsReader;
use px_fits::{CompAlgo, CompressedImageBuilder, FitsWriter, HeaderBuilder, Region};

#[path = "support.rs"]
mod support;

const W: usize = 2048;
const H: usize = 2048;

fn synthetic_i16() -> Vec<i16> {
    // A smooth gradient (compresses well) plus a little noise.
    let mut v = vec![0i16; W * H];
    let mut s: u64 = 0xBADC0DE;
    for (i, p) in v.iter_mut().enumerate() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        let row = (i / W) as i64;
        *p = ((row * 4 + (i % W) as i64 / 8) % 4000 - 2000 + ((s >> 58) as i64 - 16)) as i16;
    }
    v
}

fn bench_compressed(c: &mut Criterion) {
    let dir = support::bench_scratch_dir().join("compressed");
    std::fs::create_dir_all(&dir).unwrap();
    let pixels = synthetic_i16();

    let comp_path = dir.join("rice.fits");
    {
        let builder = CompressedImageBuilder::new(BitPix::I16, &[W, H])
            .unwrap()
            .algorithm(CompAlgo::Rice1)
            .tile_shape(&[W, 16])
            .unwrap()
            .pixels(&pixels)
            .unwrap();
        let mut w = FitsWriter::create(&comp_path).unwrap();
        w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
            .unwrap();
        w.write_compressed_image(&builder).unwrap();
        w.finish().unwrap();
    }

    let plain_path = dir.join("plain.fits");
    {
        let mut w = FitsWriter::create(&plain_path).unwrap();
        w.write_image::<i16>(
            &HeaderBuilder::primary_image(BitPix::I16, &[W as u64, H as u64]).unwrap(),
            &pixels,
        )
        .unwrap();
        w.finish().unwrap();
    }

    let comp_len = std::fs::metadata(&comp_path).unwrap().len();
    let plain_len = std::fs::metadata(&plain_path).unwrap().len();
    eprintln!(
        "[compressed] RICE_1 file {comp_len} bytes vs plain {plain_len} bytes \
         ({:.2}x smaller)",
        plain_len as f64 / comp_len as f64
    );

    let mut group = c.benchmark_group("compressed/2048x2048_i16");
    group.throughput(Throughput::Bytes((W * H * 2) as u64));

    group.bench_function("rice1_read_full", |b| {
        b.iter(|| {
            let reader = FitsReader::open(&comp_path).unwrap();
            let px = reader
                .compressed_image(1)
                .unwrap()
                .read_full::<i16>()
                .unwrap();
            black_box(px);
        });
    });

    group.bench_function("uncompressed_read_full_reference", |b| {
        b.iter(|| {
            let reader = FitsReader::open(&plain_path).unwrap();
            let px = reader.primary_image().unwrap().read_full::<i16>().unwrap();
            black_box(px);
        });
    });

    let region = Region::rect(768, 768, 512, 512);
    group.bench_function("rice1_read_region_512x512", |b| {
        b.iter(|| {
            let reader = FitsReader::open(&comp_path).unwrap();
            let px = reader
                .compressed_image(1)
                .unwrap()
                .read_region::<i16>(black_box(&region))
                .unwrap();
            black_box(px);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_compressed);
criterion_main!(benches);
