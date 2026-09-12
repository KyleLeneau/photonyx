//! Block planning must stay negligible against the work it schedules
//! (ADR 007, P1-T7).
//!
//! The block-assignment loop appends one block per iteration, so it should be
//! linear in the block count. This benchmark exists to catch a regression to
//! quadratic — planning a 60 MP stack is done once and must not show up in a
//! profile at all.

use criterion::{Criterion, criterion_group, criterion_main};
use px_imageproc::runtime::{
    BlockPlan, ImageDims, ResourceBudget, Runtime, blocks::max_rows_for_budget,
};

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

fn plan(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_plan");

    // A 60 MP mono frame over 200 subs with a tight budget — the most blocks
    // a realistic run produces.
    let dims = ImageDims::new(7744, 7744, 1);
    let rows = max_rows_for_budget(2 * GB, &dims, 200, 4, 0);
    let plan = BlockPlan::compute(rows, &dims, 16).unwrap();
    group.throughput(criterion::Throughput::Elements(plan.blocks.len() as u64));

    group.bench_function("compute/60mp_200frames_16threads", |b| {
        b.iter(|| BlockPlan::compute(std::hint::black_box(rows), &dims, 16).unwrap())
    });

    // The tightening loop in `plan_blocks` re-plans until the plan fits, so it
    // is the figure that actually matters at a call site.
    let runtime = Runtime::new(ResourceBudget::new(2 * GB, 16)).unwrap();
    group.bench_function("plan_blocks/60mp_200frames_16threads", |b| {
        b.iter(|| runtime.plan_blocks(&dims, 200, 4, 0, 0).unwrap())
    });

    group.finish();
}

/// Doubling the block count should roughly double the time, not quadruple it.
fn scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_plan_scaling");
    let dims = ImageDims::new(4096, 4096, 1);

    for &frames in &[25usize, 50, 100, 200] {
        let rows = max_rows_for_budget(512 * MB, &dims, frames, 4, 0);
        if rows == 0 {
            continue;
        }
        let blocks = BlockPlan::compute(rows, &dims, 8).unwrap().blocks.len();
        group.bench_function(format!("{blocks}_blocks"), |b| {
            b.iter(|| BlockPlan::compute(std::hint::black_box(rows), &dims, 8).unwrap())
        });
    }

    group.finish();
}

criterion_group!(benches, plan, scaling);
criterion_main!(benches);
