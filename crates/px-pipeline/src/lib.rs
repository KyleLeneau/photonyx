use std::path::PathBuf;

use crate::error::PipelineError;

pub mod calibrate_light;
pub mod error;
pub mod master_bias;
pub mod master_dark;
pub mod master_flat;
pub mod master_light;
pub mod project;

pub trait PipelineReporter {
    fn step_started(&self, message: &str) -> usize;
    fn step_ended(&self, id: usize, message: &str, success: bool);
}

pub(crate) fn all_paths_exist(paths: Vec<PathBuf>) -> Result<(), PipelineError> {
    let missing: Vec<PathBuf> = paths
        .iter()
        .filter(|p| !p.exists())
        .map(|p| p.to_path_buf())
        .collect();

    if missing.is_empty() {
        Ok(())
    } else {
        Err(PipelineError::FileNotFound(format!(
            "Some paths missing: {:?}",
            missing
        )))
    }
}
