//! Memory-budgeted block scheduling for whole-sequence reductions (ADR 007,
//! D3 / P1-T4).
//!
//! Median and mean stacking need every frame's value for a given pixel at the
//! same time, and a night of frames does not fit in memory. The strategy —
//! taken from Siril's `src/stacking/median_and_mean.c` — is to cut the output
//! image into horizontal *blocks*, read that same row range from every frame
//! into one buffer, reduce the block pixel by pixel across frames, and move on.
//! Blocks are sized as large as the memory budget allows and counted so the
//! work divides evenly across threads.
//!
//! This module is a faithful port of three Siril functions, and the fidelity
//! is deliberate: `tests/budget.rs` pins the output against values captured
//! from instrumented Siril runs, so a divergence here shows up as a test
//! failure rather than as a subtly different stack.
//!
//! - [`max_rows_for_budget`] ← `stack_get_max_number_of_rows` (l. 1203)
//! - [`refine_blocks_candidate`] ← `refine_blocks_candidate` (l. 260)
//! - [`BlockPlan::compute`] ← `stack_compute_parallel_blocks` (l. 294)

use crate::error::ImageProcError;

/// Siril's `BYTES_IN_A_MB` (`src/core/siril.h` l. 81). Mebibytes, despite the
/// name. The budget arithmetic below truncates to whole units of this exactly
/// where Siril does, because block plans must match.
const BYTES_IN_A_MB: u64 = 1024 * 1024;

/// Dimensions of the image being produced, in Siril's `naxes` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDims {
    /// `naxes[0]` — row length in pixels.
    pub width: usize,
    /// `naxes[1]` — rows per channel.
    pub height: usize,
    /// `naxes[2]` — 1 for mono, 3 for RGB.
    pub channels: usize,
}

impl ImageDims {
    pub fn new(width: usize, height: usize, channels: usize) -> Self {
        Self {
            width,
            height,
            channels,
        }
    }

    /// Pixels in the whole image, across all channels.
    pub fn pixel_count(&self) -> usize {
        self.width * self.height * self.channels
    }

    /// Rows in the whole image, across all channels — Siril's
    /// `naxes[1] * naxes[2]`, which is the unit the row budget is expressed in.
    pub fn total_rows(&self) -> usize {
        self.height * self.channels
    }
}

/// One unit of work: a contiguous run of rows within a single channel.
///
/// Blocks never straddle a channel boundary — the reduction reads one
/// channel's worth of rows from every frame, so a block that spanned two
/// channels would not be a contiguous region in any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub channel: usize,
    /// First row of this block within its channel, inclusive.
    pub start_row: usize,
    /// Last row of this block within its channel, inclusive.
    pub end_row: usize,
}

impl Block {
    pub fn height(&self) -> usize {
        self.end_row - self.start_row + 1
    }

    /// Pixels in this block for one frame.
    pub fn pixel_count(&self, dims: &ImageDims) -> usize {
        self.height() * dims.width
    }
}

/// A complete partition of an image into [`Block`]s, plus the figure the
/// per-thread buffers are sized from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPlan {
    pub blocks: Vec<Block>,
    /// Height of the tallest block. Every thread's buffer is sized for this,
    /// so one buffer can serve any block it is handed.
    pub largest_block_height: usize,
}

/// Rounds `x` up to the next multiple of `factor`. Siril's
/// `round_to_ceiling_multiple` (`src/core/proto.h` l. 298).
fn round_to_ceiling_multiple(x: usize, factor: usize) -> usize {
    debug_assert!(factor > 0);
    let remainder = x % factor;
    if remainder == 0 {
        x
    } else {
        x + factor - remainder
    }
}

/// How many rows of the *whole* image (all channels) can be held in memory at
/// once. Port of `stack_get_max_number_of_rows`.
///
/// The result image and any rejection maps are charged against the budget up
/// front, because they are live for the entire run; what remains is divided by
/// the per-row cost of holding that row from every frame at once.
///
/// `budget_bytes` is the total the run may use. `elem_size` is the in-memory
/// size of one sample (4 for the `f32` pipeline of ADR 007 D1). `nb_rejmaps`
/// is 0, 1, or 2.
///
/// Returns at most [`ImageDims::total_rows`], and may return 0 when the budget
/// cannot even hold the result — callers must treat 0 as "cannot proceed"
/// rather than dividing by it.
pub fn max_rows_for_budget(
    budget_bytes: u64,
    dims: &ImageDims,
    nb_frames: usize,
    elem_size: usize,
    nb_rejmaps: usize,
) -> usize {
    let total_rows = dims.total_rows();

    // Siril tracks this budget in whole MB and lets the subtractions truncate.
    // Reproduced rather than cleaned up so block plans match (D3).
    let mut max_memory_mb = (budget_bytes / BYTES_IN_A_MB) as i64;

    let size_of_result = dims.pixel_count() as u64 * elem_size as u64;
    // Rejection maps are u16 in Siril regardless of the pixel path.
    let size_of_rejmaps = dims.pixel_count() as u64 * 2;

    max_memory_mb -= (size_of_result / BYTES_IN_A_MB) as i64;
    max_memory_mb -= (nb_rejmaps as u64 * size_of_rejmaps / BYTES_IN_A_MB) as i64;
    if max_memory_mb < 0 {
        max_memory_mb = 0;
    }

    let per_row = nb_frames as u64 * dims.width as u64 * elem_size as u64;
    if per_row == 0 {
        return total_rows;
    }
    let rows = (max_memory_mb as u64 * BYTES_IN_A_MB / per_row) as usize;

    rows.min(total_rows)
}

/// Adjusts a candidate block count so the blocks divide evenly across channels
/// and reasonably evenly across threads. Port of `refine_blocks_candidate`.
///
/// The count is always rounded up to a multiple of `nb_channels`. Beyond that,
/// Siril accepts a count that is *slightly short* of a clean multiple of the
/// thread count — 1 short at 4–7 threads, 3 short at 8+ — on the reasoning
/// that threads drift out of phase once they start reading anyway, so it is
/// better to use all the memory than to hold out for perfect load balance.
/// Below 4 threads it insists on an exact multiple.
pub fn refine_blocks_candidate(
    nb_threads: usize,
    nb_channels: usize,
    minimum_blocks: usize,
) -> usize {
    debug_assert!(nb_threads >= 1 && nb_channels >= 1);

    let mut factor_of = nb_channels;
    if nb_threads < 4 {
        if factor_of != 1 && nb_threads.is_multiple_of(factor_of) {
            factor_of = nb_threads;
        } else {
            factor_of *= nb_threads;
        }
        return round_to_ceiling_multiple(minimum_blocks, factor_of);
    }

    let minus_factor_allowed = if nb_threads < 8 { 1 } else { 3 };
    let mut candidate = round_to_ceiling_multiple(minimum_blocks, factor_of);
    loop {
        let rem = candidate % nb_threads;
        if rem == 0 || rem >= nb_threads - minus_factor_allowed {
            return candidate;
        }
        candidate += factor_of;
    }
}

impl BlockPlan {
    /// Partitions `dims` into blocks, given how many rows fit in memory at once
    /// and how many threads will process them. Port of
    /// `stack_compute_parallel_blocks`.
    ///
    /// `max_rows` comes from [`max_rows_for_budget`] and must be at least 1 —
    /// a budget too small to hold a single row of every frame cannot be
    /// satisfied by any partition, and is reported rather than silently
    /// overrun.
    pub fn compute(
        max_rows: usize,
        dims: &ImageDims,
        nb_threads: usize,
    ) -> Result<Self, ImageProcError> {
        if nb_threads < 1 {
            return Err(ImageProcError::Processing(
                "block planning needs at least one thread".to_string(),
            ));
        }
        if max_rows < 1 {
            return Err(ImageProcError::InsufficientMemory {
                what: "one row of every frame".to_string(),
            });
        }
        if dims.height == 0 || dims.width == 0 || dims.channels == 0 {
            return Err(ImageProcError::Processing(format!(
                "cannot plan blocks for an empty image: {dims:?}"
            )));
        }

        let total_rows = dims.total_rows();

        // Smallest block count whose blocks fit the row budget when spread
        // over the thread pool.
        let mut candidate = nb_threads;
        while (max_rows * candidate) / nb_threads < total_rows {
            candidate += 1;
        }
        let nb_blocks = refine_blocks_candidate(nb_threads, dims.channels, candidate);

        let height_of_blocks = total_rows / nb_blocks;
        if height_of_blocks == 0 {
            // More blocks than rows: one row per block at best. Siril does not
            // guard this and walks off the end of its array instead.
            return Err(ImageProcError::Processing(format!(
                "block planning produced {nb_blocks} blocks for {total_rows} rows"
            )));
        }
        let blocks_per_channel = nb_blocks / dims.channels;
        let mut remainder = dims.height % blocks_per_channel;

        let mut blocks = Vec::with_capacity(nb_blocks);
        let mut largest_block_height = 0usize;
        let mut channel = 0usize;
        let mut row = 0usize;

        while channel < dims.channels {
            if blocks.len() >= nb_blocks {
                // Siril logs "A bug has been found" here. Same condition, but
                // surfaced as an error instead of a printf.
                return Err(ImageProcError::Processing(format!(
                    "block planning overflowed its {nb_blocks}-block budget for {dims:?}"
                )));
            }

            // Captured before the channel can advance below, matching Siril's
            // assignment order.
            let block_channel = channel;
            let start_row = row;
            let mut end = start_row + height_of_blocks - 1;
            // Spread the leftover rows one at a time over the leading blocks
            // rather than piling them all into the last one.
            if remainder > 0 {
                end += 1;
                remainder -= 1;
            }

            // Absorb a trailing stub shorter than a tenth of a block into this
            // one, instead of emitting a runt block.
            if end + 1 >= dims.height || dims.height - end < height_of_blocks / 10 {
                end = dims.height - 1;
                row = 0;
                channel += 1;
                remainder = dims.height - blocks_per_channel * height_of_blocks;
            } else {
                row = end + 1;
            }

            let block = Block {
                channel: block_channel,
                start_row,
                end_row: end,
            };
            largest_block_height = largest_block_height.max(block.height());
            blocks.push(block);
        }

        Ok(Self {
            blocks,
            largest_block_height,
        })
    }

    /// Bytes one thread's buffer needs: the largest block, held for every
    /// frame, plus one extra frame-sized run used as the per-pixel working
    /// stack (Siril's `+ 1ul` pixel, `median_and_mean.c` l. 1367).
    pub fn buffer_bytes_per_thread(
        &self,
        dims: &ImageDims,
        nb_frames: usize,
        elem_size: usize,
    ) -> u64 {
        let pixels_in_block = self.largest_block_height as u64 * dims.width as u64;
        nb_frames as u64 * (pixels_in_block + 1) * elem_size as u64
    }
}
