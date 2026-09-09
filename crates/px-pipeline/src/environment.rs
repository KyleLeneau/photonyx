use std::future::Future;

use crate::{PipelineReporter, error::PipelineError};

/// A calibration pipeline that can be executed in a given run environment `Env`.
///
/// `Env` is the execution backend (e.g. [`siril_sys::Builder`] for a local Siril
/// process, or a native processing context). Implement this once per
/// pipeline/environment pair.
pub trait RunIn<Env> {
    /// The master frame this pipeline produces.
    type Output;

    fn run(
        &self,
        reporter: impl PipelineReporter,
        env: Env,
    ) -> impl Future<Output = Result<Self::Output, PipelineError>>;
}
