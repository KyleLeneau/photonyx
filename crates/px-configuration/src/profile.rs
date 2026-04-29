use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROFILE_CONFIG_FILE: &str = "px_profile.yaml";

#[derive(Debug, Error)]
pub enum ProfileConfigError {
    #[error("profile already exists at `{0}`")]
    AlreadyExists(PathBuf),

    #[error("profile can not be imported at `{0}`")]
    ImportFailed(PathBuf),

    #[error("no profile found at `{0}`")]
    NotFound(PathBuf),

    #[error("failed to read or write profile config: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse profile config: {0}")]
    Parse(#[from] serde_yaml::Error),
}

/// Top-level configuration stored in `px_profile.yaml` at the profile root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Filter names present in this profile (e.g. `["Ha", "OIII", "SII"]`).
    #[serde(default)]
    pub filters: Vec<String>,
}

impl ProfileConfig {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            filters: Vec::new(),
        }
    }

    pub fn config_path(project_dir: &Path) -> PathBuf {
        project_dir.join(PROFILE_CONFIG_FILE)
    }

    pub fn exists(project_dir: &Path) -> bool {
        Self::config_path(project_dir).exists()
    }

    pub fn load(project_dir: &Path) -> Result<Self, ProfileConfigError> {
        let path = Self::config_path(project_dir);
        if !path.exists() {
            return Err(ProfileConfigError::NotFound(project_dir.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, project_dir: &Path) -> Result<(), ProfileConfigError> {
        let path = Self::config_path(project_dir);
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
