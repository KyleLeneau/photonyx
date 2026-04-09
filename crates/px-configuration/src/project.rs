use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROJECT_CONFIG_FILE: &str = "px_project.yaml";

#[derive(Debug, Error)]
pub enum ProjectConfigError {
    #[error("project already exists at `{0}`")]
    AlreadyExists(PathBuf),

    #[error("no project found at `{0}`")]
    NotFound(PathBuf),

    #[error("failed to read or write project config: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse project config: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationEntry {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLinearStack {
    /// The grouping of hardware_profiles for this stack
    pub profile: PathBuf,

    /// The filter that represents this stack
    pub filter: String,

    /// A panel identifier if the project is a mosiac
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,

    /// Observations registered with this project
    #[serde(default)]
    pub observations: Vec<ObservationEntry>,
}

/// Top-level configuration stored in `px_project.yaml` at the project root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Filter names present in this project (e.g. `["Ha", "OIII", "SII"]`).
    #[serde(default)]
    pub filters: Vec<String>,

    /// The linear stacks this project will produce
    #[serde(default)]
    pub linear_stacks: Vec<ProjectLinearStack>,
}

/// Outcome of [`ProjectConfig::add_observation`].
#[derive(Debug, PartialEq, Eq)]
pub enum AddObservationOutcome {
    /// The observation was added to an existing or newly created linear stack.
    Added,
    /// The observation path was already registered in the matching stack; no change was made.
    AlreadyRegistered,
}

impl ProjectConfig {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            filters: Vec::new(),
            linear_stacks: Vec::new(),
        }
    }

    pub fn config_path(project_dir: &Path) -> PathBuf {
        project_dir.join(PROJECT_CONFIG_FILE)
    }

    pub fn exists(project_dir: &Path) -> bool {
        Self::config_path(project_dir).exists()
    }

    pub fn load(project_dir: &Path) -> Result<Self, ProjectConfigError> {
        let path = Self::config_path(project_dir);
        if !path.exists() {
            return Err(ProjectConfigError::NotFound(project_dir.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, project_dir: &Path) -> Result<(), ProjectConfigError> {
        let path = Self::config_path(project_dir);
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Register an observation under the matching `(profile, filter, panel)` linear stack,
    /// creating the stack if one does not yet exist. Also keeps the top-level `filters` list
    /// in sync. Returns [`AddObservationOutcome::AlreadyRegistered`] if the path is already
    /// present so the caller can warn the user without saving.
    pub fn add_observation(
        &mut self,
        profile: PathBuf,
        filter: String,
        panel: Option<String>,
        obs_path: PathBuf,
    ) -> AddObservationOutcome {
        let stack = self
            .linear_stacks
            .iter_mut()
            .find(|s| s.profile == profile && s.filter == filter && s.panel == panel);

        match stack {
            Some(existing) => {
                if existing.observations.iter().any(|o| o.path == obs_path) {
                    return AddObservationOutcome::AlreadyRegistered;
                }
                existing
                    .observations
                    .push(ObservationEntry { path: obs_path });
            }
            None => {
                self.linear_stacks.push(ProjectLinearStack {
                    profile,
                    filter: filter.clone(),
                    panel,
                    comments: None,
                    observations: vec![ObservationEntry { path: obs_path }],
                });
            }
        }

        if !self.filters.contains(&filter) {
            self.filters.push(filter);
        }

        AddObservationOutcome::Added
    }
}
