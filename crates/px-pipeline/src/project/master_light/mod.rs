//! Creating a linear stack (master light) from a set of calibrated light
//! frames.
//!
//! The pipeline itself lives here; its execution lives in the sibling
//! modules, one per run environment (ADR 007 D5).

pub mod native;
pub mod siril;

use siril_sys::FitsExt;
use std::path::{Path, PathBuf};

#[derive(bon::Builder)]
pub struct CreateMasterLightPipeline {
    pub ext: FitsExt,
    pub light_folders: Vec<PathBuf>,
    pub name: String,
    pub filter: Option<String>,
    #[builder(default = false)]
    pub background_extract: bool,
    pub out_folder: PathBuf,
}

pub fn master_light_path(folder: &Path, name: &String) -> PathBuf {
    folder.join(format!("{}_LIGHT_master", name))
}

pub fn registered_master_light_path(folder: &Path, name: &String) -> PathBuf {
    folder.join(format!("r_{}_LIGHT_master", name))
}
