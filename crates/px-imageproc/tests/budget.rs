//! Block-scheduler and budget tests (ADR 007, D3).
//!
//! The scheduler is specified by properties, not by a reference
//! implementation's output: the bands must tile the image exactly, never cross
//! a channel, stay inside the memory budget, and stay balanced across workers.
//! Anything that holds for every input is asserted over a sweep; anything that
//! is a deliberate design choice gets its own named test.

use px_imageproc::runtime::{
    Block, BlockPlan, ImageDims, MemoryLimit, ResourceBudget, Runtime, blocks::rows_per_worker,
};

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
const F32: usize = 4;

// ---------------------------------------------------------------------------
// Sizing: how many rows fit
// ---------------------------------------------------------------------------

#[test]
fn rows_per_worker_divides_the_budget_by_what_a_row_costs() {
    // One row of a 1000-wide frame across 10 frames at f32 is 40,000 bytes.
    // Four workers each get a quarter of what is left after the per-worker
    // working stacks (4 x 10 x 4 = 160 bytes) are set aside.
    let working_stacks = 4 * 10 * F32 as u64;
    let rows = rows_per_worker(working_stacks + 4 * 40_000 * 25, 1000, 10, F32, 4);
    assert_eq!(rows, 25);

    // A byte short of that and the last row does not fit.
    let rows = rows_per_worker(working_stacks + 4 * 40_000 * 25 - 1, 1000, 10, F32, 4);
    assert_eq!(rows, 24);
}

#[test]
fn rows_per_worker_is_zero_when_a_single_row_does_not_fit() {
    // Callers must treat zero as "cannot proceed" rather than dividing by it.
    assert_eq!(rows_per_worker(1000, 1000, 10, F32, 4), 0);
    assert_eq!(rows_per_worker(0, 1000, 10, F32, 1), 0);
}

#[test]
fn more_frames_and_more_workers_both_shrink_the_band() {
    let base = rows_per_worker(GIB, 4096, 20, F32, 4);
    assert!(rows_per_worker(GIB, 4096, 40, F32, 4) < base, "frames");
    assert!(rows_per_worker(GIB, 4096, 20, F32, 8) < base, "workers");
}

#[test]
fn a_small_budget_is_usable_rather_than_rounding_to_nothing() {
    // Sizing in bytes rather than whole mebibytes means a modest budget still
    // produces a workable plan instead of collapsing to zero rows.
    let dims = ImageDims::new(256, 256, 1);
    let runtime = Runtime::new(ResourceBudget::new(512 * 1024, 2)).unwrap();
    let plan = runtime
        .plan_blocks(&dims, 8, F32, 0, 0)
        .expect("a 512 KiB budget should plan a 256x256 stack");
    assert_partition_is_sound(&plan, &dims);
}

// ---------------------------------------------------------------------------
// Partition properties
// ---------------------------------------------------------------------------

/// Every partition must satisfy these, for any input.
fn assert_partition_is_sound(plan: &BlockPlan, dims: &ImageDims) {
    assert!(!plan.blocks.is_empty(), "empty plan for {dims:?}");

    for channel in 0..dims.channels {
        let in_channel: Vec<&Block> = plan
            .blocks
            .iter()
            .filter(|b| b.channel == channel)
            .collect();
        assert!(
            !in_channel.is_empty(),
            "channel {channel} got no blocks in {plan:?}"
        );

        let mut expected_start = 0usize;
        for block in &in_channel {
            assert_eq!(
                block.start_row, expected_start,
                "gap or overlap in channel {channel}: {plan:?}"
            );
            assert!(block.end_row >= block.start_row, "inverted block {block:?}");
            expected_start = block.end_row + 1;
        }
        assert_eq!(
            expected_start, dims.height,
            "channel {channel} does not reach the last row: {plan:?}"
        );
    }

    for block in &plan.blocks {
        assert!(
            block.height() <= plan.largest_block_height,
            "{block:?} exceeds largest_block_height {}",
            plan.largest_block_height
        );
        assert!(block.channel < dims.channels);
    }
    assert_eq!(
        plan.largest_block_height,
        plan.blocks.iter().map(|b| b.height()).max().unwrap()
    );
}

#[test]
fn partitions_are_sound_across_a_wide_input_sweep() {
    for &(w, h, c) in &[
        (512usize, 512usize, 1usize),
        (512, 512, 3),
        (4096, 4096, 1),
        (4096, 4096, 3),
        (7744, 7744, 1),
        (1920, 1080, 3),
        (100, 37, 1),  // awkward height
        (100, 101, 3), // prime-ish height with three channels
        (64, 1, 1),    // single row
        (64, 2, 3),    // fewer rows than workers
    ] {
        let dims = ImageDims::new(w, h, c);
        for workers in [1usize, 2, 3, 4, 6, 8, 12, 16] {
            for &frames in &[2usize, 5, 20, 100] {
                for &budget in &[MIB, 128 * MIB, 512 * MIB, 2 * GIB, 16 * GIB] {
                    let runtime = Runtime::new(ResourceBudget::new(budget, workers)).unwrap();
                    let Ok(plan) = runtime.plan_blocks(&dims, frames, F32, 0, 0) else {
                        continue; // budget genuinely too small; covered elsewhere
                    };
                    assert_partition_is_sound(&plan, &dims);
                }
            }
        }
    }
}

#[test]
fn bands_are_balanced_to_within_one_row() {
    // An uneven split would leave one worker running long after the others.
    for &(h, cap) in &[(37usize, 5usize), (100, 7), (4096, 35), (6388, 100)] {
        let dims = ImageDims::new(64, h, 1);
        let plan = BlockPlan::compute(cap, &dims, 4).unwrap();
        let heights: Vec<usize> = plan.blocks.iter().map(|b| b.height()).collect();
        let (min, max) = (
            *heights.iter().min().unwrap(),
            *heights.iter().max().unwrap(),
        );
        assert!(
            max - min <= 1,
            "heights {heights:?} span more than one row (h={h}, cap={cap})"
        );
        assert!(max <= cap, "band of {max} rows exceeds the {cap}-row cap");
    }
}

#[test]
fn no_block_straddles_a_channel() {
    let dims = ImageDims::new(4096, 4096, 3);
    let runtime = Runtime::new(ResourceBudget::new(512 * MIB, 8)).unwrap();
    let plan = runtime.plan_blocks(&dims, 30, F32, 0, 0).unwrap();

    assert_eq!(
        plan.blocks.len() % 3,
        0,
        "each channel should get the same number of bands"
    );
    for block in &plan.blocks {
        assert!(block.end_row < dims.height);
    }
    assert_partition_is_sound(&plan, &dims);
}

#[test]
fn blocks_are_emitted_in_output_order() {
    // Consumers write bands straight into a contiguous result buffer, so the
    // plan order has to match the output layout.
    let dims = ImageDims::new(128, 256, 3);
    let plan = BlockPlan::compute(40, &dims, 4).unwrap();

    let mut expected_channel = 0usize;
    let mut expected_row = 0usize;
    for block in &plan.blocks {
        assert_eq!(
            (block.channel, block.start_row),
            (expected_channel, expected_row)
        );
        if block.end_row + 1 == dims.height {
            expected_channel += 1;
            expected_row = 0;
        } else {
            expected_row = block.end_row + 1;
        }
    }
    assert_eq!(expected_channel, dims.channels);
}

#[test]
fn the_band_count_fills_whole_waves() {
    // Workers take bands in waves; a count just over a multiple of the worker
    // count wastes most of the last one. The plan rounds up so every wave is
    // full, which costs shorter bands rather than extra work.
    let dims = ImageDims::new(9576, 6388, 1);
    for workers in [4usize, 8, 16] {
        let runtime = Runtime::new(ResourceBudget::new(8 * GIB, workers)).unwrap();
        let plan = runtime.plan_blocks(&dims, 40, F32, 0, 0).unwrap();
        assert_eq!(
            plan.blocks.len() % workers,
            0,
            "{} bands on {workers} workers leaves a partly-idle wave",
            plan.blocks.len()
        );
        assert_partition_is_sound(&plan, &dims);
    }
}

#[test]
fn every_worker_gets_a_band_even_on_a_short_image() {
    // A budget that could hold the whole image in one band would otherwise
    // leave all but one core idle.
    for workers in [2usize, 4, 8, 16] {
        let dims = ImageDims::new(256, 256, 1);
        let runtime = Runtime::new(ResourceBudget::new(16 * GIB, workers)).unwrap();
        let plan = runtime.plan_blocks(&dims, 4, F32, 0, 0).unwrap();
        assert!(
            plan.blocks.len() >= workers,
            "{} bands for {workers} workers leaves cores idle",
            plan.blocks.len()
        );
        assert_partition_is_sound(&plan, &dims);
    }
}

#[test]
fn a_tighter_budget_produces_more_and_shorter_bands() {
    let dims = ImageDims::new(4096, 4096, 1);
    let frames = 50;

    let roomy = Runtime::new(ResourceBudget::new(8 * GIB, 8))
        .unwrap()
        .plan_blocks(&dims, frames, F32, 0, 0)
        .unwrap();
    let tight = Runtime::new(ResourceBudget::new(256 * MIB, 8))
        .unwrap()
        .plan_blocks(&dims, frames, F32, 0, 0)
        .unwrap();

    assert!(
        tight.blocks.len() > roomy.blocks.len(),
        "tight={} roomy={}",
        tight.blocks.len(),
        roomy.blocks.len()
    );
    assert!(tight.largest_block_height < roomy.largest_block_height);
}

// ---------------------------------------------------------------------------
// The budget bound
// ---------------------------------------------------------------------------

#[test]
fn planned_buffers_stay_inside_the_budget() {
    // The invariant ADR 007 says must not be negotiated: the result, the
    // rejection maps, and every worker's band together fit the budget.
    for &(w, h, c) in &[(4096usize, 4096usize, 1usize), (2048, 2048, 3)] {
        let dims = ImageDims::new(w, h, c);
        for &frames in &[10usize, 50, 200] {
            for &budget in &[256 * MIB, 512 * MIB, 2 * GIB, 8 * GIB] {
                for workers in [1usize, 4, 8, 16] {
                    for maps in [0usize, 2] {
                        let runtime = Runtime::new(ResourceBudget::new(budget, workers)).unwrap();
                        let Ok(plan) = runtime.plan_blocks(&dims, frames, F32, maps, 0) else {
                            continue;
                        };

                        let pixels = dims.pixel_count() as u64;
                        let resident = pixels * F32 as u64 + maps as u64 * pixels * 2;
                        let bands =
                            workers as u64 * plan.buffer_bytes_per_worker(&dims, frames, F32);

                        assert!(
                            resident + bands <= budget,
                            "{dims:?} frames={frames} workers={workers} maps={maps} \
                             budget={budget}: resident {resident} + bands {bands} overruns",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a_resident_master_is_charged_against_the_budget() {
    // Subtracting a master in flight keeps it live for the whole run, so it
    // must shrink the bands rather than being paid for out of thin air.
    let dims = ImageDims::new(2048, 2048, 1);
    let runtime = Runtime::new(ResourceBudget::new(512 * MIB, 4)).unwrap();

    let without = runtime.plan_blocks(&dims, 40, F32, 0, 0).unwrap();
    let master_bytes = dims.pixel_count() as u64 * F32 as u64;
    let with = runtime
        .plan_blocks(&dims, 40, F32, 0, master_bytes)
        .unwrap();

    // Charging the master can only shrink the bands, never grow them. It may
    // leave them unchanged — band counts are whole numbers, so a small charge
    // often lands inside the same split.
    assert!(with.largest_block_height <= without.largest_block_height);

    // The invariant that matters: the plan fits with the master counted in.
    let resident = dims.pixel_count() as u64 * F32 as u64 + master_bytes;
    let bands = 4 * with.buffer_bytes_per_worker(&dims, 40, F32);
    assert!(
        resident + bands <= 512 * MIB,
        "resident {resident} + bands {bands} overruns a 512 MiB budget"
    );

    // And with a budget tight enough for the rounding not to absorb it, the
    // shrink is visible.
    let tight = Runtime::new(ResourceBudget::new(64 * MIB, 4)).unwrap();
    let a = tight.plan_blocks(&dims, 40, F32, 0, 0).unwrap();
    let b = tight.plan_blocks(&dims, 40, F32, 0, master_bytes).unwrap();
    assert!(
        b.largest_block_height < a.largest_block_height,
        "a {master_bytes}-byte master should shrink bands at a 64 MiB budget: {} vs {}",
        b.largest_block_height,
        a.largest_block_height
    );
}

#[test]
fn a_budget_too_small_for_the_result_is_reported_not_overrun() {
    let dims = ImageDims::new(8000, 8000, 3);
    let runtime = Runtime::new(ResourceBudget::new(256 * MIB, 8)).unwrap();
    let err = runtime.plan_blocks(&dims, 20, F32, 0, 0).unwrap_err();
    assert!(
        err.to_string().contains("memory budget too small"),
        "unexpected error: {err}"
    );
}

#[test]
fn planning_with_no_rows_is_an_error_not_a_panic() {
    let dims = ImageDims::new(8000, 8000, 3);
    let err = BlockPlan::compute(0, &dims, 8).unwrap_err();
    assert!(
        err.to_string().contains("memory budget too small"),
        "unexpected error: {err}"
    );
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

#[test]
fn runtime_confines_work_to_its_core_count() {
    use std::collections::HashSet;
    use std::sync::Mutex;

    use rayon::prelude::*;

    for cores in [1usize, 2, 4] {
        let runtime = Runtime::new(ResourceBudget::new(GIB, cores)).unwrap();
        assert_eq!(runtime.cores(), cores);

        let seen: Mutex<HashSet<usize>> = Mutex::new(HashSet::new());
        runtime.install(|| {
            (0..10_000u32).into_par_iter().for_each(|_| {
                if let Some(i) = rayon::current_thread_index() {
                    seen.lock().unwrap().insert(i);
                }
            });
        });

        let seen = seen.into_inner().unwrap();
        assert!(
            seen.len() <= cores,
            "work ran on {} threads with a {cores}-core budget",
            seen.len()
        );
    }
}

#[test]
fn a_budget_ratio_resolves_against_the_probe() {
    let budget = ResourceBudget::resolve(MemoryLimit::Ratio(0.5), Some(4), 8 * GIB);
    assert_eq!(budget.max_bytes, 4 * GIB);
    assert_eq!(budget.cores, 4);

    let budget = ResourceBudget::resolve(MemoryLimit::FixedGb(2.0), Some(2), 8 * GIB);
    assert_eq!(budget.max_bytes, 2 * GIB);

    // A zero core count is nonsense; it clamps to one rather than building a
    // pool that can never make progress.
    assert_eq!(ResourceBudget::new(GIB, 0).cores, 1);
}

#[test]
fn frames_in_flight_are_bounded_by_memory_then_by_cores() {
    // 8 cores but only room for 3 frames: memory wins.
    let budget = ResourceBudget::new(3 * GIB, 8);
    assert_eq!(budget.max_frames_in_flight(GIB), 3);

    // Room for 100 frames but only 8 cores: cores win — holding more frames
    // than there are threads to work on them buys nothing.
    assert_eq!(budget.max_frames_in_flight(30 * MIB), 8);

    // Too small for even one frame still attempts one, so the caller gets an
    // allocation error naming the real problem instead of a silent no-op.
    assert_eq!(budget.max_frames_in_flight(64 * GIB), 1);
}
