use std::path::{Path, PathBuf};

use px_fits::{MasterBias, MasterDark, MasterFlat};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROFILE_CONFIG_FILE: &str = "px_profile.yaml";

#[derive(Debug, Error)]
pub enum ProfileConfigError {
    #[error("profile already exists at `{0}`")]
    AlreadyExists(PathBuf),

    #[error("no profile found at `{0}`")]
    NotFound(PathBuf),

    #[error("failed to read or write profile config: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse profile config: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum CalibrationType {
    FLAT,
    BIAS,
    DARK,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationMaster {
    pub kind: CalibrationType,
    pub source: PathBuf,
    pub master: PathBuf,
    pub active: bool,
    pub temperature: f64,
    pub gain: i64,
    pub offset: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f64>,
    // pub binning: Binning,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

impl PartialEq for CalibrationMaster {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.source == other.source && self.master == other.master
    }
}

impl From<MasterBias> for CalibrationMaster {
    fn from(value: MasterBias) -> Self {
        Self {
            kind: CalibrationType::BIAS,
            source: value.source,
            master: value.path,
            active: true,
            temperature: value.temperature,
            gain: value.gain,
            offset: value.offset,
            exposure: None,
            filter: None,
        }
    }
}

impl From<MasterDark> for CalibrationMaster {
    fn from(value: MasterDark) -> Self {
        Self {
            kind: CalibrationType::DARK,
            source: value.source,
            master: value.path,
            active: true,
            temperature: value.temperature,
            gain: value.gain,
            offset: value.offset,
            exposure: Some(value.exposure),
            filter: None,
        }
    }
}

impl From<MasterFlat> for CalibrationMaster {
    fn from(value: MasterFlat) -> Self {
        Self {
            kind: CalibrationType::FLAT,
            source: value.source,
            master: value.path,
            active: true,
            temperature: value.temperature,
            gain: value.gain,
            offset: value.offset,
            exposure: None,
            filter: Some(value.filter),
        }
    }
}

/// Top-level configuration stored in `px_project.yaml` at the project root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Filter names present in this profile (e.g. `["Ha", "OIII", "SII"]`).
    #[serde(default)]
    pub filters: Vec<String>,

    /// Stored/created calibration masters on this profile
    #[serde[default]]
    pub calibration_master: Vec<CalibrationMaster>,
}

impl ProfileConfig {
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            filters: Vec::new(),
            calibration_master: Vec::new(),
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

    pub fn add_master(&mut self, master: CalibrationMaster) -> Result<Self, ProfileConfigError> {
        // TODO: Validation or do we need a date?
        if !self.calibration_master.contains(&master) {
            self.calibration_master.push(master);
        }
        Ok(self.clone())
    }
}
