use std::path::{Path, PathBuf};

use px_configuration::{ProjectConfig, ProjectConfigError};
use px_fs::CWD;

#[derive(Debug)]
pub struct ProjectPath {
    /// The root directory for a project
    pub root: PathBuf,
}

impl ProjectPath {
    /// Create a new project rooted at `parent / name`.
    ///
    /// Creates the root directory, all 5 sub-folders, and writes a fresh
    /// `px_profile.yaml`. Returns `AlreadyExists` if the root already exists.
    ///
    pub fn new(root: PathBuf) -> Result<Self, ProjectConfigError> {
        if root.exists() {
            return Err(ProjectConfigError::AlreadyExists(root));
        }

        std::fs::create_dir_all(&root)?;

        let name = root.file_name().unwrap().display().to_string();
        let desc = format!("Photonyx project for: {}", &name);
        let config = ProjectConfig::new(name, Some(desc));
        config.save(&root)?;

        Ok(Self { root })
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

        Ok(Self { root: project_dir })
    }

    /// Lazy load the project config in the path
    ///
    pub fn load_config(&self) -> Result<ProjectConfig, ProjectConfigError> {
        ProjectConfig::load(&self.root)
    }

    /// Save the project config to the root
    ///
    pub fn save_config(&self, config: &ProjectConfig) -> Result<(), ProjectConfigError> {
        config.save(&self.root)
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
