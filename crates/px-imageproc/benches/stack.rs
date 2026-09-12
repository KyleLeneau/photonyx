//! Stacking throughput by method, frame count, and core budget
//! (ADR 007, P3-T9).
//!
//! Measures the whole path — read, decode, reduce, assemble — on an on-disk
//! sequence, because that is what a pipeline pays. Wall-clock numbers for the
//! full 61 MP corpus are reported by `tests/stack_parity.rs` instead; this
//! benchmark exists to compare methods against each other and to catch a
//! regression in the reduction kernels.

use std::path::{Path, PathBuf};

use criterion::{Criterion, criterion_group, criterion_main};
use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::FrameSet;
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::stack::{Bounds, Method, Rejection, StackOptions, stack};

const GB: u64 = 1 << 30;
const WIDTH: usize = 1024;
const HEIGHT: usize = 1024;

struct Xorshift(u64);

impl Xorshift {
    fn next_u16(&mut self) -> u16 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u16
    }
}

/// Writes a sequence resembling calibration frames: a flat pedestal, read
/// noise, and a sprinkling of hot pixels for the rejection methods to find.
fn build_sequence(dir: &Path, frames: usize) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).expect("create bench dir");
    (0..frames)
        .map(|f| {
            let path = dir.join(format!("f{f:03}.fits"));
            if path.exists() {
                return path;
            }
            let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15 ^ (f as u64 + 1));
            let data: Vec<i16> = (0..WIDTH * HEIGHT)
                .map(|i| {
                    let noise = (rng.next_u16() % 32) as i32 - 16;
                    let value = if i % 5000 == f { 60_000 } else { 1_000 + noise };
                    (value - 32768) as i16
                })
                .collect();
            let header = HeaderBuilder::primary_image(BitPix::I16, &[WIDTH as u64, HEIGHT as u64])
                .expect("header")
                .card("BZERO", px_fits::card::Value::Integer(32768), None)
                .expect("bzero");
            let mut w = FitsWriter::create(&path).expect("create");
            w.write_image(&header, &data).expect("write");
            w.finish().expect("finish");
            path
        })
        .collect()
}

fn methods(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("px-imageproc-bench-stack/20");
    let paths = build_sequence(&dir, 20);
    let set = FrameSet::open(&paths).expect("open");
    let runtime = Runtime::new(ResourceBudget::new(4 * GB, num_cores())).expect("runtime");

    let mut group = c.benchmark_group("stack_method");
    group.throughput(criterion::Throughput::Elements(
        (WIDTH * HEIGHT * paths.len()) as u64,
    ));
    group.sample_size(10);

    for (name, method) in [
        ("median", Method::Median),
        ("mean", Method::Mean(Rejection::None)),
        ("sum", Method::Sum),
        (
            "mean_sigma",
            Method::Mean(Rejection::Sigma(Bounds::symmetric(3.0))),
        ),
        (
            "mean_mad",
            Method::Mean(Rejection::Mad(Bounds::symmetric(3.0))),
        ),
        (
            "mean_winsorized",
            Method::Mean(Rejection::Winsorized(Bounds::symmetric(3.0))),
        ),
        (
            "mean_linearfit",
            Method::Mean(Rejection::LinearFit(Bounds::symmetric(5.0))),
        ),
        (
            "mean_percentile",
            Method::Mean(Rejection::Percentile(Bounds::new(0.2, 0.1))),
        ),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| stack(&set, &runtime, StackOptions::new(method)).expect("stack"))
        });
    }
    group.finish();
}

fn frame_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("stack_frames");
    group.sample_size(10);

    for frames in [5usize, 20, 50] {
        let dir = std::env::temp_dir().join(format!("px-imageproc-bench-stack/{frames}"));
        let paths = build_sequence(&dir, frames);
        let set = FrameSet::open(&paths).expect("open");
        let runtime = Runtime::new(ResourceBudget::new(4 * GB, num_cores())).expect("runtime");

        group.throughput(criterion::Throughput::Elements(
            (WIDTH * HEIGHT * frames) as u64,
        ));
        group.bench_function(format!("median/{frames}_frames"), |b| {
            b.iter(|| stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack"))
        });
    }
    group.finish();
}

/// Does the memory budget actually cost anything? A tighter budget means more,
/// smaller blocks and more read calls.
fn budgets(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("px-imageproc-bench-stack/20");
    let paths = build_sequence(&dir, 20);
    let set = FrameSet::open(&paths).expect("open");

    let mut group = c.benchmark_group("stack_budget");
    group.sample_size(10);

    for (name, bytes) in [
        ("4GiB", 4 * GB),
        ("256MiB", 256 * 1024 * 1024),
        ("64MiB", 64 * 1024 * 1024),
        ("16MiB", 16 * 1024 * 1024),
    ] {
        let runtime = Runtime::new(ResourceBudget::new(bytes, num_cores())).expect("runtime");
        let blocks = stack(&set, &runtime, StackOptions::new(Method::Median))
            .expect("stack")
            .block_count;
        group.bench_function(format!("{name}/{blocks}_blocks"), |b| {
            b.iter(|| stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack"))
        });
    }
    group.finish();
}

fn cores(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("px-imageproc-bench-stack/20");
    let paths = build_sequence(&dir, 20);
    let set = FrameSet::open(&paths).expect("open");

    let mut group = c.benchmark_group("stack_cores");
    group.sample_size(10);

    for cores in [1usize, 2, 4, 8, num_cores()] {
        let runtime = Runtime::new(ResourceBudget::new(4 * GB, cores)).expect("runtime");
        group.bench_function(format!("{cores}_cores"), |b| {
            b.iter(|| stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack"))
        });
    }
    group.finish();
}

fn num_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

criterion_group!(benches, methods, frame_counts, budgets, cores);
criterion_main!(benches);
