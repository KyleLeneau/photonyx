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

/// Top-level configuration stored in `px_project.yaml` at the project root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The framing and observation content for this project
    pub framing: Framing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLinearStack {
    /// The grouping of hardware_profiles for this stack
    pub profile: PathBuf,

    /// The name that represents this linear stack. Will be used as the output name.
    pub name: String,

    /// A panel identifier if the project is a mosiac
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panel: Option<String>,

    /// A comment for this stack if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,

    /// The filter for this stack if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    #[serde(default)]
    pub extract_background: bool,

    /// Observations registered with this project
    #[serde(default)]
    pub observations: Vec<ObservationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationEntry {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum Framing {
    /// Single target framming where all linear stacks register to minimum framing
    Single(SingleFraming),

    /// A mosiac where the maximum framing needs to be done (smart telescopes like Seestar)
    SpiralMosiac(SpiralMosiacFraming),

    /// An X * Y grid layout mosiac done with multiple identified panels, resulting in a maximum framing
    GridMosiac(GridMosiacFraming),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleFraming {
    /// Create a master_light for every layer
    pub master_lights: Vec<ProjectLinearStack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiralMosiacFraming {
    /// The name that represents this linear stack. Will be used as the output name.
    pub name: String,

    /// Percent of minimum dimension to feather the edges by
    #[serde(default)]
    pub feather_pixels: f32,

    /// The filter for this stack if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// Observations used for this spiral mosiac
    #[serde(default)]
    pub observations: Vec<ObservationEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridMosiacFraming {
    pub master_lights: Vec<GridMosiacMasterLight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridMosiacMasterLight {
    /// The name that represents the panel. Will be used as the output name.
    pub name: String,

    /// The filter for this stack if needed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// Create a master_light for every layer
    pub panels: Vec<ProjectLinearStack>,
}

impl ProjectConfig {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            // filters: Vec::new(),
            // linear_stacks: Vec::new(),
            framing: Framing::Single(SingleFraming {
                master_lights: Vec::new(),
            }),
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
}
