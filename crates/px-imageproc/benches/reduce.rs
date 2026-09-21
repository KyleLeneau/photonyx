//! The reduction kernel, and what reassociating its sums would buy
//! (ADR 007, P12-T2 / D8).
//!
//! The P12-T1 profile put `stack.reduce` at 182 s of thread time in a 95-frame
//! master light — more than every other stage of the pipeline together. This
//! benchmark exists to answer the question D8 poses about such a kernel: is it
//! slow because it is doing a lot of work, or because the compiler cannot
//! vectorize it?
//!
//! The suspicion is the latter, and it is a source-level problem rather than a
//! missing intrinsic. Floating-point addition is not associative, so a single
//! `sum += x` accumulator forces the adds into a chain: each one waits on the
//! last, and an add is several cycles deep. The compiler may not reassociate
//! without being told the result is allowed to change, and it is not.
//!
//! `reassociated_*` below are **not** what ships. They are the experiment: the
//! same arithmetic split across four independent accumulators, which is what
//! lets the adds overlap and what a vectorizer would do if it were permitted.
//! Whether to adopt them is a question about reproducibility — the answer would
//! move in the last bits, and every rejection threshold and parity number in
//! this project is stated against the current association — so the decision is
//! recorded in the ADR, with these numbers, rather than made here.

use criterion::{Criterion, criterion_group, criterion_main};
use px_imageproc::stack::rejection::{Bounds, Rejected, Rejection, Scratch, reject};

/// Frame counts that select each of the project's three rejection defaults
/// (ADR 007, D9): percentile below 7, Winsorized through 30, linear fit above.
const FRAME_COUNTS: [usize; 3] = [5, 25, 95];

struct Xorshift(u64);

impl Xorshift {
    fn next_f32(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// One pixel as a real sequence presents it: a sky level, read noise, a slow
/// trend across the sequence for the linear fit to find, and the occasional
/// cosmic ray or satellite trail that rejection exists to remove.
fn pixel_stack(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = Xorshift(seed | 1);
    (0..n)
        .map(|i| {
            let base = 0.02 + 0.0004 * i as f32 / n as f32;
            let noise = (rng.next_f32() - 0.5) * 0.002;
            if i % 37 == 0 { 0.6 } else { base + noise }
        })
        .collect()
}

/// Enough pixels that the measurement is of the kernel rather than of the
/// timer, and few enough that they stay in cache — the cache behaviour of the
/// band is a separate question, settled by the tiling in `stack/mod.rs`.
const PIXELS: usize = 4096;

fn stacks(n: usize) -> Vec<Vec<f32>> {
    (0..PIXELS).map(|p| pixel_stack(n, p as u64 + 1)).collect()
}

fn rejection_for(n: usize) -> Rejection {
    match n {
        0..=6 => Rejection::Percentile(Bounds::new(0.2, 0.1)),
        7..=30 => Rejection::Winsorized(Bounds::new(3.0, 3.0)),
        _ => Rejection::LinearFit(Bounds::new(5.0, 5.0)),
    }
}

fn kernel(c: &mut Criterion) {
    let mut group = c.benchmark_group("reduce_kernel");

    for n in FRAME_COUNTS {
        let source = stacks(n);
        let rejection = rejection_for(n);
        let mut scratch = Scratch::new(n);
        let mut work = vec![0.0f32; n];

        group.throughput(criterion::Throughput::Elements(PIXELS as u64));
        group.bench_function(format!("reject/{n}_frames"), |b| {
            b.iter(|| {
                let mut counts = Rejected::default();
                for stack in &source {
                    work.copy_from_slice(stack);
                    reject(&mut work, n, rejection, &mut scratch, &mut counts);
                }
                counts
            })
        });
    }

    group.finish();
}

/// The two sums inside the linear-fit path, as they ship and reassociated.
///
/// Isolated from the rest of the kernel so the ratio is a statement about the
/// accumulator chain and not about the sort or the compaction around it.
fn accumulators(c: &mut Criterion) {
    let mut group = c.benchmark_group("accumulator_chain");
    let n = 95;
    let source = stacks(n);

    group.throughput(criterion::Throughput::Elements((PIXELS * n) as u64));

    group.bench_function("serial/residual", |b| {
        b.iter(|| {
            let mut total = 0.0f64;
            for stack in &source {
                total += serial_residual(stack, 0.02, 4e-6);
            }
            total
        })
    });

    group.bench_function("reassociated/residual", |b| {
        b.iter(|| {
            let mut total = 0.0f64;
            for stack in &source {
                total += reassociated_residual(stack, 0.02, 4e-6);
            }
            total
        })
    });

    group.bench_function("serial/cross_moment", |b| {
        b.iter(|| {
            let mut total = 0.0f64;
            for stack in &source {
                total += serial_cross_moment(stack);
            }
            total
        })
    });

    group.bench_function("reassociated/cross_moment", |b| {
        b.iter(|| {
            let mut total = 0.0f64;
            for stack in &source {
                total += reassociated_cross_moment(stack);
            }
            total
        })
    });

    group.finish();
}

/// The mean absolute residual about a fitted line, exactly as `linear_fit`
/// computes it.
fn serial_residual(y: &[f32], intercept: f32, slope: f32) -> f64 {
    let mut residual = 0.0f64;
    for (i, &v) in y.iter().enumerate() {
        residual += (v - (slope * i as f32 + intercept)).abs() as f64;
    }
    residual / y.len() as f64
}

/// The same sum over four independent accumulators.
fn reassociated_residual(y: &[f32], intercept: f32, slope: f32) -> f64 {
    let mut acc = [0.0f64; 4];
    let mut i = 0usize;
    while i + 4 <= y.len() {
        for (lane, slot) in acc.iter_mut().enumerate() {
            let idx = i + lane;
            *slot += (y[idx] - (slope * idx as f32 + intercept)).abs() as f64;
        }
        i += 4;
    }
    let mut tail = 0.0f64;
    while i < y.len() {
        tail += (y[i] - (slope * i as f32 + intercept)).abs() as f64;
        i += 1;
    }
    (acc[0] + acc[1] + acc[2] + acc[3] + tail) / y.len() as f64
}

/// `sum((i - mean_x) * (y - mean_y))`, the surviving loop of the least-squares
/// fit.
fn serial_cross_moment(y: &[f32]) -> f64 {
    let n = y.len();
    let mean_x = (n - 1) as f64 * 0.5;
    let mean_y = y.iter().map(|&v| v as f64).sum::<f64>() / n as f64;

    let mut sxy = 0.0f64;
    for (i, &v) in y.iter().enumerate() {
        sxy += (i as f64 - mean_x) * (v as f64 - mean_y);
    }
    sxy
}

fn reassociated_cross_moment(y: &[f32]) -> f64 {
    let n = y.len();
    let mean_x = (n - 1) as f64 * 0.5;
    let mean_y = y.iter().map(|&v| v as f64).sum::<f64>() / n as f64;

    let mut acc = [0.0f64; 4];
    let mut i = 0usize;
    while i + 4 <= n {
        for (lane, slot) in acc.iter_mut().enumerate() {
            let idx = i + lane;
            *slot += (idx as f64 - mean_x) * (y[idx] as f64 - mean_y);
        }
        i += 4;
    }
    let mut tail = 0.0f64;
    while i < n {
        tail += (i as f64 - mean_x) * (y[i] as f64 - mean_y);
        i += 1;
    }
    acc[0] + acc[1] + acc[2] + acc[3] + tail
}

criterion_group!(benches, kernel, accumulators);
criterion_main!(benches);
