use std::path::{Path, PathBuf};

use px_configuration::{ProjectConfig, ProjectConfigError};
use px_fs::CWD;

#[derive(Debug)]
pub struct ProjectPath {
    /// The root directory for a project
    directory: PathBuf,
}

impl ProjectPath {
    pub fn dir(&self) -> &Path {
        &self.directory
    }

    /// Find the project directory and load the config file
    ///
    pub fn find(directory: Option<PathBuf>) -> Result<Self, ProjectConfigError> {
        // Resolve project directory
        let project_dir = match directory {
            Some(p) => p,
            None => Self::find_project_dir(&CWD)
                .ok_or(ProjectConfigError::NotFound(CWD.to_path_buf()))?,
        };

        Ok(Self {
            directory: project_dir,
        })
    }

    /// Lazy load the project config in the path
    ///
    pub fn load_config(&self) -> Result<ProjectConfig, ProjectConfigError> {
        ProjectConfig::load(&self.directory)
    }

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
}
