//! Turning the `--engine`, `--max-cores` and `--max-memory` flags into a run
//! environment for a pipeline (ADR 007, D11).
//!
//! The two environments are interchangeable at the call site — every pipeline
//! implements `RunIn` for both — so this exists only to decide which one to
//! hand over, and to apply the same resource limits to either.

use px_cli::{DebayerMethod as CliDebayer, Engine, GlobalArgs, MemoryLimitArg};
use px_pipeline::NativeRuntime;
use siril_sys::Builder;

/// The engine selection plus the limits that apply to it.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub engine: Engine,
    cores: Option<usize>,
    memory: Option<MemoryLimitArg>,
    debayer: CliDebayer,
}

impl EngineConfig {
    pub fn from_globals(args: &GlobalArgs) -> Self {
        Self {
            engine: args.engine,
            cores: args.max_cores,
            memory: args.max_memory,
            debayer: args.debayer,
        }
    }

    /// A configuration pinned to Siril, for the paths that have no native
    /// implementation. Explicit rather than implicit: a call site that cannot
    /// run natively should say so where it is, not depend on a flag it ignores.
    pub fn siril_only() -> Self {
        Self {
            engine: Engine::Siril,
            cores: None,
            memory: None,
            debayer: CliDebayer::default(),
        }
    }

    pub fn is_native(&self) -> bool {
        self.engine == Engine::Native
    }

    /// A Siril builder carrying the same limits.
    ///
    /// Siril takes its memory ceiling as a ratio or a whole number of
    /// gibibytes, so a byte-precise `--max-memory` is rounded to GB here.
    /// That imprecision is Siril's; the native engine uses the exact figure.
    pub fn siril(&self) -> Builder {
        let mut builder = Builder::default().container_aware_limits();
        if let Some(cores) = self.cores {
            builder = builder.use_cpu_limit(cores.min(u8::MAX as usize) as u8);
        }
        match self.memory {
            Some(MemoryLimitArg::Ratio(r)) => builder = builder.use_memory_ratio(r),
            Some(MemoryLimitArg::Bytes(b)) => {
                builder = builder.use_memory_fixed_gb(b as f64 / (1024.0 * 1024.0 * 1024.0));
            }
            None => {}
        }
        builder
    }

    /// A native runtime carrying the same limits.
    pub fn native(&self) -> NativeRuntime {
        let mut runtime = NativeRuntime::default().container_aware_limits();
        if let Some(cores) = self.cores {
            runtime = runtime.use_cpu_limit(cores);
        }
        match self.memory {
            Some(MemoryLimitArg::Ratio(r)) => runtime = runtime.use_memory_ratio(r),
            Some(MemoryLimitArg::Bytes(b)) => runtime = runtime.use_memory_bytes(b),
            None => {}
        }
        runtime.use_debayer(match self.debayer {
            CliDebayer::GradientCorrected => px_imageproc::debayer::Method::GradientCorrected,
            CliDebayer::Bilinear => px_imageproc::debayer::Method::Bilinear,
            CliDebayer::Ahd => px_imageproc::debayer::Method::Ahd,
            CliDebayer::Rcd => px_imageproc::debayer::Method::Rcd,
        })
    }
}
