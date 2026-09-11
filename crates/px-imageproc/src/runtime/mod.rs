//! The bounded execution context every `px-imageproc` pipeline stage runs in
//! (ADR 007, D4 / P1-T6).
//!
//! A [`Runtime`] pairs a resolved [`ResourceBudget`] with a thread pool sized
//! to it. The pool is *dedicated*, not `rayon`'s global one: a run configured
//! for two cores must not be widened because some other part of the process
//! already initialized the global pool at sixteen, and conversely constraining
//! this work must not constrain the rest of `px`.

pub mod blocks;
pub mod budget;

pub use blocks::{Block, BlockPlan, ImageDims, max_rows_for_budget, refine_blocks_candidate};
pub use budget::{MemoryLimit, ResourceBudget};

use std::sync::Arc;

use crate::error::ImageProcError;

/// A budget plus the pool that enforces its core count.
#[derive(Clone)]
pub struct Runtime {
    budget: ResourceBudget,
    pool: Arc<rayon::ThreadPool>,
}

impl Runtime {
    /// Builds a runtime for an already-resolved budget.
    pub fn new(budget: ResourceBudget) -> Result<Self, ImageProcError> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(budget.cores)
            .thread_name(|i| format!("px-imageproc-{i}"))
            .build()
            .map_err(|e| ImageProcError::Processing(format!("thread pool: {e}")))?;
        Ok(Self {
            budget,
            pool: Arc::new(pool),
        })
    }

    /// Resolves `limit` and `cores` against the current machine (honouring
    /// cgroup limits) and builds the runtime.
    pub fn resolve(limit: MemoryLimit, cores: Option<usize>) -> Result<Self, ImageProcError> {
        let cores = cores.or_else(budget::cgroup_cpu_limit);
        let budget = ResourceBudget::resolve(limit, cores, budget::available_memory());
        Self::new(budget)
    }

    pub fn budget(&self) -> ResourceBudget {
        self.budget
    }

    pub fn cores(&self) -> usize {
        self.budget.cores
    }

    /// Runs `op` inside this runtime's pool, so every `rayon` call it makes
    /// is confined to the budgeted cores.
    pub fn install<T: Send>(&self, op: impl FnOnce() -> T + Send) -> T {
        self.pool.install(op)
    }

    /// Plans how to cut `dims` into memory-bounded blocks for a reduction
    /// across `nb_frames` frames, charging `nb_rejmaps` rejection maps and the
    /// result image against the budget first.
    ///
    /// `elem_size` is 4 for the `f32` pipeline (ADR 007 D1).
    ///
    /// # Why this is not just `BlockPlan::compute`
    ///
    /// [`BlockPlan::compute`] is a faithful port of Siril's scheduler (D3), and
    /// that scheduler can overshoot its own budget by a small margin. Its row
    /// budget guarantees `height_of_blocks <= max_rows / nb_threads`, but
    /// `height_of_blocks` is a *floor* while the per-thread buffers are sized
    /// to the **largest** block, which is one row taller whenever the rows do
    /// not divide evenly. With 4096 rows over 115 blocks on 4 threads the
    /// budget permits 35.75 rows per thread, blocks come out 35 and 36 rows,
    /// and buffers are sized for 36 — overrunning a 512 MiB budget by 2.1 MB
    /// (0.39%). The `+1` working-stack pixel per frame adds a little more.
    ///
    /// 0.39% is not much, but ADR 007 says the budget bound is the one that
    /// must not be negotiated, so this method tightens the row budget until the
    /// plan actually fits. That is a deliberate, measured deviation from the
    /// port; `BlockPlan::compute` itself stays faithful so the fidelity tests
    /// keep testing the real thing.
    pub fn plan_blocks(
        &self,
        dims: &ImageDims,
        nb_frames: usize,
        elem_size: usize,
        nb_rejmaps: usize,
    ) -> Result<BlockPlan, ImageProcError> {
        let insufficient = || ImageProcError::InsufficientMemory {
            what: format!(
                "a {}x{}x{} result plus one row of {nb_frames} frames within {}",
                dims.width, dims.height, dims.channels, self.budget
            ),
        };

        // Live for the whole run, so charged before any block buffer.
        let fixed_bytes = dims.pixel_count() as u64 * elem_size as u64
            + nb_rejmaps as u64 * dims.pixel_count() as u64 * 2;
        let for_blocks = self.budget.max_bytes.saturating_sub(fixed_bytes);
        if for_blocks == 0 {
            return Err(insufficient());
        }

        let mut rows = max_rows_for_budget(
            self.budget.max_bytes,
            dims,
            nb_frames,
            elem_size,
            nb_rejmaps,
        );
        if rows == 0 {
            return Err(insufficient());
        }

        loop {
            let plan = BlockPlan::compute(rows, dims, self.budget.cores)?;
            let pool_bytes =
                self.budget.cores as u64 * plan.buffer_bytes_per_thread(dims, nb_frames, elem_size);
            if pool_bytes <= for_blocks {
                return Ok(plan);
            }

            // Scale the row budget down by however much we overshot, and always
            // make progress so this terminates.
            let scaled = (rows as u64 * for_blocks / pool_bytes) as usize;
            let next = scaled.min(rows - 1);
            if next == 0 {
                return Err(insufficient());
            }
            rows = next;
        }
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}
