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

pub use blocks::{Block, BlockPlan, ImageDims, rows_per_worker};
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

    /// Plans how to cut `dims` into memory-bounded bands for a reduction
    /// across `nb_frames` frames.
    ///
    /// The result image and any rejection maps are live for the whole run, as
    /// is anything named by `extra_resident_bytes` — a master frame being
    /// subtracted in flight, say. All of it is charged before the bands are
    /// sized, so what the plan promises is what the run actually holds.
    ///
    /// `elem_size` is 4 for the `f32` pipeline (ADR 007 D1).
    pub fn plan_blocks(
        &self,
        dims: &ImageDims,
        nb_frames: usize,
        elem_size: usize,
        nb_rejmaps: usize,
        extra_resident_bytes: u64,
    ) -> Result<BlockPlan, ImageProcError> {
        let pixels = dims.pixel_count() as u64;
        let resident = pixels * elem_size as u64            // the result
            + nb_rejmaps as u64 * pixels * 2                // rejection maps, u16
            + extra_resident_bytes;

        let for_bands = self.budget.max_bytes.saturating_sub(resident);
        let rows = rows_per_worker(
            for_bands,
            dims.width,
            nb_frames,
            elem_size,
            self.budget.cores,
        );

        if rows == 0 {
            return Err(ImageProcError::InsufficientMemory {
                what: format!(
                    "a {}x{}x{} result plus one row of {nb_frames} frames per core, within {}",
                    dims.width, dims.height, dims.channels, self.budget
                ),
            });
        }

        BlockPlan::compute(rows, dims, self.budget.cores)
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("budget", &self.budget)
            .finish_non_exhaustive()
    }
}
