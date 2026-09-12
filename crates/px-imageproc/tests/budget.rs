//! Block-scheduler and budget tests (ADR 007, P1-T5).
//!
//! `runtime::blocks` is a port of Siril's stacking block scheduler, and D3
//! says the port is faithful rather than merely equivalent. These tests pin
//! that: the structural invariants a partition must satisfy, the exact
//! arithmetic of the two heuristics, and the row budget.
//!
//! Where a case below is marked "Siril-captured", the expected value came
//! from running the equivalent C function on the same inputs. Do not adjust
//! one of those to make a change pass — a divergence means the port drifted.

use px_imageproc::runtime::{
    Block, BlockPlan, ImageDims, MemoryLimit, ResourceBudget, Runtime, blocks::max_rows_for_budget,
    refine_blocks_candidate,
};

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

// ---------------------------------------------------------------------------
// refine_blocks_candidate
// ---------------------------------------------------------------------------

#[test]
fn refine_below_four_threads_demands_an_exact_multiple() {
    // Under 4 threads Siril insists on a clean multiple; there is no slack
    // window. Mono, so the channel factor is 1 and `factor_of` becomes the
    // thread count itself.
    for threads in 1..4usize {
        for minimum in 1..40usize {
            let got = refine_blocks_candidate(threads, 1, minimum);
            assert_eq!(
                got % threads,
                0,
                "threads={threads} minimum={minimum} gave {got}, not a multiple"
            );
            assert!(got >= minimum);
            assert!(got - minimum < threads, "{got} overshot {minimum}");
        }
    }
}

#[test]
fn refine_rgb_below_four_threads_multiplies_the_factors_when_coprime() {
    // 3 channels, 2 threads: 2 % 3 != 0, so factor_of becomes 6.
    assert_eq!(refine_blocks_candidate(2, 3, 7), 12);
    assert_eq!(refine_blocks_candidate(2, 3, 12), 12);
    assert_eq!(refine_blocks_candidate(2, 3, 13), 18);

    // 3 channels, 3 threads: 3 % 3 == 0, so factor_of collapses to 3.
    assert_eq!(refine_blocks_candidate(3, 3, 7), 9);
    assert_eq!(refine_blocks_candidate(3, 3, 9), 9);
}

#[test]
fn refine_allows_one_short_between_four_and_seven_threads() {
    // 6 threads, mono: a candidate is accepted when rem == 0 or rem >= 5.
    // 11 % 6 == 5, which is within the 1-short window, so 11 stands.
    assert_eq!(refine_blocks_candidate(6, 1, 11), 11);
    // 8 % 6 == 2 -> rejected; 9 % 6 == 3 -> rejected; 10 % 6 == 4 -> rejected;
    // 11 % 6 == 5 -> accepted.
    assert_eq!(refine_blocks_candidate(6, 1, 8), 11);
    // Exact multiples are taken immediately.
    assert_eq!(refine_blocks_candidate(6, 1, 12), 12);
}

#[test]
fn refine_allows_three_short_at_eight_threads_and_above() {
    // 8 threads, mono: accepted when rem == 0 or rem >= 5.
    assert_eq!(refine_blocks_candidate(8, 1, 13), 13); // 13 % 8 == 5
    assert_eq!(refine_blocks_candidate(8, 1, 10), 13); // 10,11,12 -> rem 2,3,4
    assert_eq!(refine_blocks_candidate(8, 1, 16), 16);

    // 16 threads, mono: accepted when rem == 0 or rem >= 13.
    assert_eq!(refine_blocks_candidate(16, 1, 29), 29); // 29 % 16 == 13
    assert_eq!(refine_blocks_candidate(16, 1, 20), 29);
}

#[test]
fn refine_always_returns_a_multiple_of_the_channel_count() {
    for threads in 1..=16usize {
        for channels in [1usize, 3] {
            for minimum in 1..60usize {
                let got = refine_blocks_candidate(threads, channels, minimum);
                assert_eq!(
                    got % channels,
                    0,
                    "threads={threads} channels={channels} minimum={minimum} -> {got}"
                );
                assert!(got >= minimum);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// max_rows_for_budget
// ---------------------------------------------------------------------------

#[test]
fn row_budget_charges_the_result_image_up_front() {
    // 4096x4096 mono f32 result is exactly 64 MiB. Against a 100 MiB budget
    // that leaves 36 MiB for block buffers, and one row held across 20 frames
    // costs 20*4096*4 = 327680 bytes.
    let dims = ImageDims::new(4096, 4096, 1);
    let rows = max_rows_for_budget(100 * MB, &dims, 20, 4, 0);

    let expected = (100 - 64) * MB / 327_680;
    assert_eq!(rows as u64, expected);
    assert!(
        rows < dims.total_rows(),
        "test is meaningless if the whole image fits"
    );
}

#[test]
fn row_budget_charges_rejection_maps() {
    let dims = ImageDims::new(4096, 4096, 1);
    let without = max_rows_for_budget(200 * MB, &dims, 20, 4, 0);
    let one = max_rows_for_budget(200 * MB, &dims, 20, 4, 1);
    let two = max_rows_for_budget(200 * MB, &dims, 20, 4, 2);

    assert!(two < one && one < without, "{two} < {one} < {without}");
    assert!(without < dims.total_rows());
}

#[test]
fn row_budget_is_capped_at_the_whole_image() {
    // A generous budget must not promise more rows than the image has, or the
    // block planner would be handed a nonsensical candidate count.
    let dims = ImageDims::new(512, 512, 3);
    let rows = max_rows_for_budget(64 * GB, &dims, 4, 4, 0);
    assert_eq!(rows, dims.total_rows());
    assert_eq!(rows, 512 * 3);
}

#[test]
fn row_budget_returns_zero_when_the_result_alone_does_not_fit() {
    // 8000x8000x3 f32 is ~732 MiB; a 256 MiB budget cannot hold it. The
    // budgeter reports 0 rather than a negative or wrapped figure, and the
    // planner turns that into a typed error (asserted below).
    let dims = ImageDims::new(8000, 8000, 3);
    assert_eq!(max_rows_for_budget(256 * MB, &dims, 20, 4, 0), 0);
}

#[test]
fn planning_with_no_room_is_an_error_not_a_panic() {
    let dims = ImageDims::new(8000, 8000, 3);
    let err = BlockPlan::compute(0, &dims, 8).unwrap_err();
    assert!(
        err.to_string().contains("memory budget too small"),
        "unexpected error: {err}"
    );
}

// ---------------------------------------------------------------------------
// BlockPlan structural invariants
// ---------------------------------------------------------------------------

/// Every partition this module produces must satisfy these, for any input.
fn assert_partition_is_sound(plan: &BlockPlan, dims: &ImageDims) {
    assert!(!plan.blocks.is_empty(), "empty plan for {dims:?}");

    // Per channel, blocks must tile [0, height) exactly: contiguous, ordered,
    // no gaps, no overlaps.
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

    // No block exceeds the figure the per-thread buffers are sized from.
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
fn partition_is_sound_across_a_wide_input_sweep() {
    for &(w, h, c) in &[
        (512usize, 512usize, 1usize),
        (512, 512, 3),
        (4096, 4096, 1),
        (4096, 4096, 3),
        (7744, 7744, 1),
        (1920, 1080, 3),
        (100, 37, 1),  // awkward height, not divisible by much
        (100, 101, 3), // prime-ish height with 3 channels
    ] {
        let dims = ImageDims::new(w, h, c);
        for threads in [1usize, 2, 3, 4, 6, 8, 12, 16] {
            for &frames in &[2usize, 5, 20, 100] {
                for &budget in &[128 * MB, 512 * MB, 2 * GB, 16 * GB] {
                    let max_rows = max_rows_for_budget(budget, &dims, frames, 4, 0);
                    if max_rows == 0 {
                        continue; // covered by its own test
                    }
                    let plan = BlockPlan::compute(max_rows, &dims, threads).unwrap_or_else(|e| {
                        panic!("{dims:?} threads={threads} frames={frames} budget={budget}: {e}")
                    });
                    assert_partition_is_sound(&plan, &dims);
                }
            }
        }
    }
}

#[test]
fn blocks_never_straddle_a_channel_boundary() {
    // The reduction reads one channel's rows from each frame, so a block
    // spanning two channels would not be contiguous in any of them. The
    // partition soundness check above proves this per channel; this test
    // states the property directly for an RGB case that has many blocks.
    let dims = ImageDims::new(4096, 4096, 3);
    let max_rows = max_rows_for_budget(512 * MB, &dims, 30, 4, 0);
    let plan = BlockPlan::compute(max_rows, &dims, 8).unwrap();

    assert!(plan.blocks.len() >= 3);
    assert_eq!(
        plan.blocks.len() % 3,
        0,
        "block count must divide by channels"
    );
    for block in &plan.blocks {
        assert!(block.end_row < dims.height);
    }
    assert_partition_is_sound(&plan, &dims);
}

#[test]
fn remainder_rows_go_to_the_leading_blocks_not_the_last_one() {
    // 37 rows over 8 blocks is 4 remainder 5. Siril adds one row each to the
    // first blocks rather than dumping 5 extra on the tail, so heights should
    // be front-loaded and the spread should be at most one row.
    let dims = ImageDims::new(64, 37, 1);
    let plan = BlockPlan::compute(37, &dims, 8).unwrap();
    assert_eq!(plan.blocks.len(), 8);
    assert_partition_is_sound(&plan, &dims);

    let heights: Vec<usize> = plan.blocks.iter().map(|b| b.height()).collect();
    // 37 rows / 8 blocks = 4 remainder 5: the five extra rows go one each to
    // the leading blocks, so no block is more than one row off any other.
    assert_eq!(heights, vec![5, 5, 5, 5, 5, 4, 4, 4], "{heights:?}");
}

#[test]
fn a_single_thread_and_a_huge_budget_gives_one_block_per_channel() {
    for channels in [1usize, 3] {
        let dims = ImageDims::new(256, 256, channels);
        let max_rows = max_rows_for_budget(16 * GB, &dims, 4, 4, 0);
        let plan = BlockPlan::compute(max_rows, &dims, 1).unwrap();

        assert_eq!(plan.blocks.len(), channels);
        assert_eq!(plan.largest_block_height, 256);
        assert_partition_is_sound(&plan, &dims);
    }
}

#[test]
fn a_tighter_budget_produces_more_and_shorter_blocks() {
    let dims = ImageDims::new(4096, 4096, 1);
    let frames = 50;

    let roomy = {
        let rows = max_rows_for_budget(8 * GB, &dims, frames, 4, 0);
        BlockPlan::compute(rows, &dims, 8).unwrap()
    };
    let tight = {
        let rows = max_rows_for_budget(512 * MB, &dims, frames, 4, 0);
        BlockPlan::compute(rows, &dims, 8).unwrap()
    };

    assert!(
        tight.blocks.len() > roomy.blocks.len(),
        "tight={} roomy={}",
        tight.blocks.len(),
        roomy.blocks.len()
    );
    assert!(tight.largest_block_height < roomy.largest_block_height);
    assert_partition_is_sound(&roomy, &dims);
    assert_partition_is_sound(&tight, &dims);
}

#[test]
fn the_faithful_port_can_overshoot_its_own_budget() {
    // Documents the upstream flaw that `Runtime::plan_blocks` exists to fix.
    // Siril's row budget bounds the *average* block height but buffers are
    // sized to the largest block, which is a row taller when the rows do not
    // divide evenly. 4096 rows / 115 blocks on 4 threads: the budget permits
    // 35.75 rows per thread, blocks come out 35 and 36, buffers are sized 36.
    //
    // If this test starts failing because the overshoot is gone, that is good
    // news, but check it is gone for a real reason before deleting it.
    let elem_size = 4usize;
    let dims = ImageDims::new(4096, 4096, 1);
    let (frames, threads, budget) = (200usize, 4usize, 512 * MB);

    let max_rows = max_rows_for_budget(budget, &dims, frames, elem_size, 0);
    let plan = BlockPlan::compute(max_rows, &dims, threads).unwrap();

    let result_bytes = dims.pixel_count() as u64 * elem_size as u64;
    let pool_bytes = threads as u64 * plan.buffer_bytes_per_thread(&dims, frames, elem_size);
    let total = result_bytes + pool_bytes;

    assert!(
        total > budget,
        "expected the port to overshoot; got {total} <= {budget}"
    );
    // Bounded, and small: under 1% of the budget.
    assert!(
        total - budget < budget / 100,
        "overshoot {} is larger than the documented bound",
        total - budget
    );
}

#[test]
fn planned_buffers_stay_inside_the_budget() {
    // The invariant ADR 007 says must not be negotiated: pool buffers plus the
    // result image fit the budget, checked at plan level before a pixel is
    // read. This goes through `Runtime::plan_blocks`, which tightens the
    // faithful port's row budget until the plan really fits.
    let elem_size = 4usize;
    for &(w, h, c) in &[(4096usize, 4096usize, 1usize), (2048, 2048, 3)] {
        let dims = ImageDims::new(w, h, c);
        for &frames in &[10usize, 50, 200] {
            for &budget in &[512 * MB, 2 * GB, 8 * GB] {
                for threads in [1usize, 4, 8, 16] {
                    let runtime = Runtime::new(ResourceBudget::new(budget, threads)).unwrap();
                    let plan = match runtime.plan_blocks(&dims, frames, elem_size, 0, 0) {
                        Ok(p) => p,
                        // A budget that genuinely cannot hold the work is a
                        // typed error, not an overrun. That is the other half
                        // of the invariant.
                        Err(_) => continue,
                    };

                    let result_bytes = dims.pixel_count() as u64 * elem_size as u64;
                    let pool_bytes =
                        threads as u64 * plan.buffer_bytes_per_thread(&dims, frames, elem_size);

                    assert!(
                        result_bytes + pool_bytes <= budget,
                        "{dims:?} frames={frames} threads={threads} budget={budget}: \
                         result {result_bytes} + pool {pool_bytes} overruns",
                    );
                    assert_partition_is_sound(&plan, &dims);
                }
            }
        }
    }
}

#[test]
fn tightening_the_budget_does_not_gut_the_plan() {
    // The tightening loop must shave the row budget, not collapse it: the
    // tightened plan should be within a block or two of the faithful port's.
    let elem_size = 4usize;
    let dims = ImageDims::new(4096, 4096, 1);
    let (frames, threads, budget) = (200usize, 4usize, 512 * MB);

    let faithful = {
        let rows = max_rows_for_budget(budget, &dims, frames, elem_size, 0);
        BlockPlan::compute(rows, &dims, threads).unwrap()
    };
    let runtime = Runtime::new(ResourceBudget::new(budget, threads)).unwrap();
    let tightened = runtime.plan_blocks(&dims, frames, elem_size, 0, 0).unwrap();

    assert!(tightened.blocks.len() >= faithful.blocks.len());
    assert!(
        tightened.blocks.len() <= faithful.blocks.len() + threads * 2,
        "tightened to {} blocks from {}",
        tightened.blocks.len(),
        faithful.blocks.len()
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
        let runtime = Runtime::new(ResourceBudget::new(GB, cores)).unwrap();
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
    let budget = ResourceBudget::resolve(MemoryLimit::Ratio(0.5), Some(4), 8 * GB);
    assert_eq!(budget.max_bytes, 4 * GB);
    assert_eq!(budget.cores, 4);

    let budget = ResourceBudget::resolve(MemoryLimit::FixedGb(2.0), Some(2), 8 * GB);
    assert_eq!(budget.max_bytes, 2 * GB);

    // A zero core count is nonsense; it clamps to one rather than building a
    // pool that can never make progress.
    assert_eq!(ResourceBudget::new(GB, 0).cores, 1);
}

#[test]
fn frames_in_flight_are_bounded_by_memory_then_by_cores() {
    // 8 cores but only room for 3 frames: memory wins.
    let budget = ResourceBudget::new(3 * GB, 8);
    assert_eq!(budget.max_frames_in_flight(GB), 3);

    // Room for 100 frames but only 8 cores: cores win — holding more frames
    // than there are threads to work on them buys nothing.
    assert_eq!(budget.max_frames_in_flight(30 * MB), 8);

    // Too small for even one frame still attempts one, so the caller gets an
    // allocation error naming the real problem instead of a silent no-op.
    assert_eq!(budget.max_frames_in_flight(64 * GB), 1);
}
