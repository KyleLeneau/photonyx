use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{ProjectLinearStack, SpiralMosiacFraming};

pub const LOCK_FILE: &str = "px_project.lock";

#[derive(Debug, Error)]
pub enum LockError {
    #[error("failed to read or write lock file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse lock file: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLock {
    pub schema_version: u32,
    pub framing: FramingLock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config", rename_all = "snake_case")]
pub enum FramingLock {
    Single(SingleFramingLock),
    SpiralMosiac(SpiralMosiacLock),
    GridMosiac(GridMosiacLock),
}

/// Lock state for a `SingleFraming` project — one entry per `ProjectLinearStack`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SingleFramingLock {
    pub master_lights: Vec<LayerLock>,
}

/// Lock entry for a single linear stack layer (used in `Single` framing).
/// `registered_master_light` is always set: for multi-layer projects it points to the
/// registered peer; for single-layer projects it mirrors `master_light`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerLock {
    pub name: String,
    pub input_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacked_at: Option<String>,
}

/// Lock state for a `SpiralMosiac` framing — single flat entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiralMosiacLock {
    pub name: String,
    pub input_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacked_at: Option<String>,
}

/// Lock state for a `GridMosiac` framing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GridMosiacLock {
    pub master_lights: Vec<GridLayerLock>,
}

/// Lock entry for one `GridMosiacMasterLight` (a layer in the grid).
/// `master_light` is the `GridMosiacPipeline` output (stitched panels).
/// `registered_master_light` mirrors `master_light` when there is only one layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridLayerLock {
    pub name: String,
    pub panels: Vec<PanelLock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registered_master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacked_at: Option<String>,
}

/// Lock entry for a panel (`ProjectLinearStack`) inside a grid layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelLock {
    pub name: String,
    pub input_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_light: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacked_at: Option<String>,
}

// ── Load / save ──────────────────────────────────────────────────────────────

impl ProjectLock {
    fn lock_path(dir: &Path) -> PathBuf {
        dir.join(LOCK_FILE)
    }

    pub fn load(dir: &Path) -> Result<Option<Self>, LockError> {
        let path = Self::lock_path(dir);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(path)?;
        let lock = serde_yaml::from_str(&content)?;
        Ok(Some(lock))
    }

    pub fn save(&self, dir: &Path) -> Result<(), LockError> {
        let path = Self::lock_path(dir);
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

// ── Hash computation ─────────────────────────────────────────────────────────

/// Produces a deterministic SHA-256 hex fingerprint for a `ProjectLinearStack`.
/// Covers name, filter, extract_background, and the sorted set of observation paths.
pub fn hash_linear_stack(stack: &ProjectLinearStack) -> String {
    let mut sorted_paths: Vec<String> = stack
        .observations
        .iter()
        .map(|o| o.path.to_string_lossy().into_owned())
        .collect();
    sorted_paths.sort();

    let input = format!(
        "{}|{:?}|{}|{}",
        stack.name,
        stack.filter,
        stack.extract_background,
        sorted_paths.join("|")
    );

    sha256_hex(&input)
}

/// Produces a deterministic SHA-256 hex fingerprint for a `SpiralMosiacFraming`.
pub fn hash_spiral_framing(framing: &SpiralMosiacFraming) -> String {
    let mut sorted_paths: Vec<String> = framing
        .observations
        .iter()
        .map(|o| o.path.to_string_lossy().into_owned())
        .collect();
    sorted_paths.sort();

    let input = format!(
        "{}|{:?}|{}|{}",
        framing.name,
        framing.filter,
        framing.feather_pixels,
        sorted_paths.join("|")
    );

    sha256_hex(&input)
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ── Dirty-checking helpers ────────────────────────────────────────────────────

impl LayerLock {
    /// True when the layer must be re-stacked: hash changed or output file missing.
    pub fn is_dirty(&self, hash: &str) -> bool {
        self.input_hash != hash || self.master_light.as_ref().is_none_or(|p| !p.exists())
    }

    /// True when the registered peer is absent or the file no longer exists.
    pub fn is_registration_dirty(&self) -> bool {
        self.registered_master_light
            .as_ref()
            .is_none_or(|p| !p.exists())
    }
}

impl SingleFramingLock {
    pub fn find_layer(&self, name: &str) -> Option<&LayerLock> {
        self.master_lights.iter().find(|l| l.name == name)
    }
}

impl SpiralMosiacLock {
    /// True when the spiral stack must be re-run.
    pub fn is_dirty(&self, hash: &str) -> bool {
        self.input_hash != hash || self.master_light.as_ref().is_none_or(|p| !p.exists())
    }
}

impl PanelLock {
    /// True when this panel must be re-stacked.
    pub fn is_dirty(&self, hash: &str) -> bool {
        self.input_hash != hash || self.master_light.as_ref().is_none_or(|p| !p.exists())
    }
}

impl GridLayerLock {
    pub fn find_panel(&self, name: &str) -> Option<&PanelLock> {
        self.panels.iter().find(|p| p.name == name)
    }

    /// True when the grid-stitched master light must be re-run.
    pub fn is_grid_dirty(&self) -> bool {
        self.master_light.as_ref().is_none_or(|p| !p.exists())
    }

    /// True when the registered peer is absent or missing on disk.
    pub fn is_registration_dirty(&self) -> bool {
        self.registered_master_light
            .as_ref()
            .is_none_or(|p| !p.exists())
    }
}

impl GridMosiacLock {
    pub fn find_layer(&self, name: &str) -> Option<&GridLayerLock> {
        self.master_lights.iter().find(|l| l.name == name)
    }
}
