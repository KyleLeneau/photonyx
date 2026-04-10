use std::path::{Path, PathBuf};

use px_configuration::{ProjectConfig, ProjectConfigError};

pub(crate) mod add;
pub(crate) mod align;
pub(crate) mod calibrate;
pub(crate) mod init;
pub(crate) mod list;
pub(crate) mod sample;
pub(crate) mod stack;

/// Walk up from `start` looking for `px_project.yaml`, returning the containing directory.
///
fn find_project_dir(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if ProjectConfig::exists(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Find the project directory and load the config file
///
fn find_and_load_project(
    directory: Option<PathBuf>,
) -> Result<(PathBuf, ProjectConfig), ProjectConfigError> {
    // Resolve project directory
    let project_dir = match directory {
        Some(p) => p,
        None => {
            let cwd = std::env::current_dir()?;
            find_project_dir(&cwd).ok_or(ProjectConfigError::NotFound(cwd))?
        }
    };

    let config = ProjectConfig::load(&project_dir)?;
    Ok((project_dir, config))
}
