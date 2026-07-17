use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROJECT_CONFIG_FILE: &str = "px_project.yaml";

/// Controls whether `px project sync` is allowed to automatically update this project.
///
/// Set to `manual` to opt out — sync will warn and refuse, leaving `px project edit` as the only
/// update path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncPolicy {
    #[default]
    Auto,
    Manual,
}

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

    /// Canonical target name used to query the profile index (e.g. "NGC 1234").
    /// Required for `px project sync` and `px project edit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,

    /// Controls whether `px project sync` is permitted to run automatically.
    /// Defaults to `auto`; set to `manual` to require `px project edit` instead.
    #[serde(default)]
    pub sync_policy: SyncPolicy,

    /// The framing and observation content for this project
    pub framing: Framing,

    /// Settings controlling `px project sample`. Absent means all defaults apply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_sample: Option<ColorSampleConfig>,
}

/// Settings for the `px project sample` command, which auto-detects and produces color
/// composites (RGB, SHO, HOO, LRGB, etc.) from the registered per-filter stacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSampleConfig {
    /// Run Photometric Color Calibration on true-color RGB/LRGB mixes before stretching.
    /// Requires plate solve metadata (target coordinates, pixel size, focal length) on the
    /// composed image, or a prior plate solve.
    #[serde(default)]
    pub enable_pcc: bool,

    /// Output formats to write for each sample.
    #[serde(default = "ColorSampleConfig::default_output_formats")]
    pub output_formats: Vec<SampleOutputFormat>,

    /// Mix names to skip even when the filters needed for them are available
    /// (e.g. "SHO" to skip the Hubble palette). Matches [`ColorMixType`] names, case-insensitive.
    #[serde(default)]
    pub exclude_mixes: Vec<String>,
}

impl ColorSampleConfig {
    fn default_output_formats() -> Vec<SampleOutputFormat> {
        vec![SampleOutputFormat::Png, SampleOutputFormat::Jpg]
    }
}

impl Default for ColorSampleConfig {
    fn default() -> Self {
        Self {
            enable_pcc: false,
            output_formats: Self::default_output_formats(),
            exclude_mixes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleOutputFormat {
    Fit,
    Tiff,
    Png,
    Jpg,
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

impl Framing {
    pub fn kind_str(&self) -> &'static str {
        match self {
            Framing::Single(_) => "single",
            Framing::SpiralMosiac(_) => "spiral_mosaic",
            Framing::GridMosiac(_) => "grid_mosaic",
        }
    }
}

impl fmt::Display for Framing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Framing::Single(s) => write!(f, "{s}"),
            Framing::SpiralMosiac(s) => write!(f, "{s}"),
            Framing::GridMosiac(g) => write!(f, "{g}"),
        }
    }
}

impl fmt::Display for SingleFraming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Single ({} layers)", self.master_lights.len())?;
        for layer in &self.master_lights {
            write!(f, "\n  - {}", layer.name)?;
            if let Some(filter) = &layer.filter {
                write!(f, " [{filter}]")?;
            }
            if let Some(panel) = &layer.panel {
                write!(f, " (panel: {panel})")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for SpiralMosiacFraming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spiral Mosaic: {}", self.name)?;
        if let Some(filter) = &self.filter {
            write!(f, " [{filter}]")?;
        }
        write!(f, " ({} observations)", self.observations.len())
    }
}

impl fmt::Display for GridMosiacFraming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Grid Mosaic ({} panels)", self.master_lights.len())?;
        for panel in &self.master_lights {
            write!(f, "\n  - {}", panel.name)?;
            if let Some(filter) = &panel.filter {
                write!(f, " [{filter}]")?;
            }
            write!(f, " ({} stacks)", panel.panels.len())?;
        }
        Ok(())
    }
}

impl ProjectConfig {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            target: None,
            sync_policy: SyncPolicy::Auto,
            framing: Framing::Single(SingleFraming {
                master_lights: Vec::new(),
            }),
            color_sample: None,
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
