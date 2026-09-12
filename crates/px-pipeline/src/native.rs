//! The native run environment (ADR 007, D4).
//!
//! [`NativeRuntime`] is to the native engine what [`siril_sys::Builder`] is to
//! the Siril one: a description of how a run should be resourced, turned into
//! something executable by `build()`. The method names deliberately mirror
//! that builder so a call site can swap one for the other.
//!
//! What it adds over the Siril environment is a budget the Rust side actually
//! honours. `siril_sys::Builder` can pass a memory limit *to Siril*, but the
//! limit applies to another process; here the same numbers bound the block
//! plans and thread pool this process uses.

use std::path::{Path, PathBuf};

use px_imageproc::runtime::{MemoryLimit, ResourceBudget, Runtime, budget};
use siril_sys::FitsExt;

use crate::error::PipelineError;

/// How a native pipeline run is resourced.
#[derive(Debug, Clone)]
pub struct NativeRuntime {
    memory: MemoryLimit,
    cores: Option<usize>,
    ext: FitsExt,
    work_dir: Option<PathBuf>,
}

impl Default for NativeRuntime {
    fn default() -> Self {
        Self {
            // Matches `siril_sys::Builder`'s default, so the two engines
            // behave the same when neither is configured.
            memory: MemoryLimit::Ratio(0.9),
            cores: None,
            ext: FitsExt::FITS,
            work_dir: None,
        }
    }
}

impl NativeRuntime {
    /// Reads container CPU and memory limits, when running under one.
    ///
    /// Without this the budget is resolved against the host's memory, which
    /// inside a container is not the memory this process may use.
    pub fn container_aware_limits(mut self) -> Self {
        self.cores = budget::cgroup_cpu_limit();
        self
    }

    pub fn use_cpu_limit(mut self, cores: usize) -> Self {
        self.cores = Some(cores);
        self
    }

    /// A fraction of available memory, e.g. `0.75`.
    pub fn use_memory_ratio(mut self, ratio: f64) -> Self {
        self.memory = MemoryLimit::Ratio(ratio);
        self
    }

    pub fn use_memory_fixed_gb(mut self, gb: f64) -> Self {
        self.memory = MemoryLimit::FixedGb(gb);
        self
    }

    pub fn use_memory_bytes(mut self, bytes: u64) -> Self {
        self.memory = MemoryLimit::Bytes(bytes);
        self
    }

    pub fn use_extension(mut self, ext: FitsExt) -> Self {
        self.ext = ext;
        self
    }

    pub fn use_directory(mut self, dir: &Path) -> Self {
        self.work_dir = Some(dir.to_path_buf());
        self
    }

    pub fn ext(&self) -> FitsExt {
        self.ext.clone()
    }

    /// Resolves the limits against this machine and builds the thread pool.
    pub fn build(&self) -> Result<NativeEngine, PipelineError> {
        let runtime = Runtime::resolve(self.memory, self.cores)?;
        Ok(NativeEngine {
            runtime,
            ext: self.ext.clone(),
            work_dir: self.work_dir.clone(),
        })
    }
}

/// A built [`NativeRuntime`]: a resolved budget and the pool that enforces it.
#[derive(Debug, Clone)]
pub struct NativeEngine {
    runtime: Runtime,
    ext: FitsExt,
    work_dir: Option<PathBuf>,
}

impl NativeEngine {
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    pub fn budget(&self) -> ResourceBudget {
        self.runtime.budget()
    }

    pub fn ext(&self) -> FitsExt {
        self.ext.clone()
    }

    pub fn work_dir(&self) -> Option<&Path> {
        self.work_dir.as_deref()
    }

    /// Appends the configured FITS extension to a path built without one.
    pub fn with_extension(&self, path: &Path) -> PathBuf {
        let mut name = path.as_os_str().to_os_string();
        name.push(".");
        name.push(self.ext.to_string());
        PathBuf::from(name)
    }
}

// ---------------------------------------------------------------------------
// Shared master-frame stacking
// ---------------------------------------------------------------------------

use px_fits::card::Value;
use px_imageproc::stack::{Method, StackOptions, stack_calibrated};
use px_imageproc::{Frame, FrameSet};

/// One master-frame stacking job.
pub(crate) struct MasterStack {
    pub frames: Vec<PathBuf>,
    pub method: Method,
    /// Subtracted from every frame before combining. Master flats need this;
    /// bias and dark masters do not.
    pub subtract: Option<PathBuf>,
    pub output: PathBuf,
    /// Cards stamped onto the output on top of the ones carried from the first
    /// input frame.
    pub cards: Vec<(String, Value, Option<String>)>,
}

/// What a master-frame stack produced, for reporting.
pub(crate) struct MasterStackOutcome {
    pub path: PathBuf,
    pub frame_count: usize,
    pub blocks: usize,
    pub rejected: u64,
}

/// Runs a master-frame stack to completion, synchronously.
///
/// Callers hand this to `spawn_blocking`: it is minutes of CPU-bound work on a
/// real sequence, and running it on an async worker would stall every other
/// task sharing that thread.
pub(crate) fn run_master_stack(
    engine: &NativeEngine,
    job: MasterStack,
) -> Result<MasterStackOutcome, PipelineError> {
    let set = FrameSet::open(&job.frames)?;

    let subtract = job.subtract.as_deref().map(Frame::open).transpose()?;

    let options = StackOptions::new(job.method);
    let result = stack_calibrated(&set, engine.runtime(), options, subtract.as_ref())?;

    let mut cards: Vec<(&str, Value, Option<&str>)> = vec![(
        "STACKCNT",
        Value::Integer(set.len() as i64),
        Some("frames stacked"),
    )];
    for (keyword, value, comment) in &job.cards {
        cards.push((keyword.as_str(), value.clone(), comment.as_deref()));
    }

    if let Some(parent) = job.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    result.frame.write(&job.output, &cards)?;

    Ok(MasterStackOutcome {
        path: job.output,
        frame_count: set.len(),
        blocks: result.block_count,
        rejected: result.rejected.total() as u64,
    })
}
