use std::path::{Path, PathBuf};

use px_configuration::{ProfileConfig, ProfileConfigError};
use px_fs::CWD;

#[derive(Debug)]
pub struct ProfilePath {
    /// The root directory for the profile
    pub root: PathBuf,

    /// The BIAS folder just below the profile root
    pub bias: PathBuf,

    /// The DARK folder just below the profile root
    pub dark: PathBuf,

    /// The FLAT folder just below the profile root
    pub flat: PathBuf,

    /// The LIGHT folder just below the profile root
    pub light: PathBuf,

    /// The PROJECTS folder just below the profile root
    pub projects: PathBuf,
}

impl ProfilePath {
    /// Create a new profile rooted at `parent / name`.
    ///
    /// Creates the root directory, all 5 sub-folders, and writes a fresh
    /// `px_profile.yaml`. Returns `AlreadyExists` if the root already exists.
    ///
    pub fn new(root: PathBuf) -> Result<Self, ProfileConfigError> {
        if root.exists() {
            // Try to import this root if it exists already
            return ProfilePath::import(root);
        }

        let bias = root.join("BIAS");
        let dark = root.join("DARK");
        let flat = root.join("FLAT");
        let light = root.join("LIGHT");
        let projects = root.join("PROJECTS");

        for dir in [&bias, &dark, &flat, &light, &projects] {
            std::fs::create_dir_all(dir)?;
        }

        let name = root.file_name().unwrap().display().to_string();
        let desc = format!("Photonyx profile for: {}", &name);
        let config = ProfileConfig::new(name, Some(desc));
        config.save(&root)?;

        Ok(Self {
            root,
            bias,
            dark,
            flat,
            light,
            projects,
        })
    }

    /// Import an existing directory as a profile.
    ///
    /// Requires `root` to exist plus BIAS/DARK/FLAT/LIGHT subdirs.
    /// Creates PROJECTS if missing, then writes a fresh `px_profile.yaml`.
    /// Returns `AlreadyExists` if a config file is already present.
    pub fn import(root: PathBuf) -> Result<Self, ProfileConfigError> {
        if !root.is_dir() {
            return Err(ProfileConfigError::NotFound(root));
        }
        if ProfileConfig::exists(&root) {
            return Err(ProfileConfigError::AlreadyExists(root));
        }

        let bias = root.join("BIAS");
        let dark = root.join("DARK");
        let flat = root.join("FLAT");
        let light = root.join("LIGHT");
        let projects = root.join("PROJECTS");

        for dir in [&bias, &dark, &flat, &light] {
            if !dir.is_dir() {
                return Err(ProfileConfigError::ImportFailed(dir.clone()));
            }
        }

        std::fs::create_dir_all(&projects)?;

        let name = root.file_name().unwrap().display().to_string();
        let desc = format!("Photonyx profile for: {}", &name);
        let config = ProfileConfig::new(name, Some(desc));
        config.save(&root)?;

        Ok(Self { root, bias, dark, flat, light, projects })
    }

    /// Find the project directory and load the config file
    ///
    pub fn find(directory: Option<PathBuf>) -> Result<Self, ProfileConfigError> {
        // Resolve project directory
        let root = match directory {
            Some(p) => p,
            None => Self::find_profile_dir(&CWD)
                .ok_or(ProfileConfigError::NotFound(CWD.to_path_buf()))?,
        };

        let bias = root.join("BIAS");
        let dark = root.join("DARK");
        let flat = root.join("FLAT");
        let light = root.join("LIGHT");
        let projects = root.join("PROJECTS");

        for dir in [&bias, &dark, &flat, &light, &projects] {
            if !dir.is_dir() {
                return Err(ProfileConfigError::NotFound(dir.clone()));
            }
        }

        Ok(Self {
            root,
            bias,
            dark,
            flat,
            light,
            projects,
        })
    }

    /// Lazy load the profile config in the path
    ///
    pub fn load_config(&self) -> Result<ProfileConfig, ProfileConfigError> {
        ProfileConfig::load(&self.root)
    }

    /// Save the profile config to the root
    ///
    pub fn save_config(&self, config: &ProfileConfig) -> Result<(), ProfileConfigError> {
        config.save(&self.root)
    }

    /// Walk up from `start` looking for `px_profile.yaml`, returning the containing directory.
    ///
    fn find_profile_dir(start: &Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();
        loop {
            if ProfileConfig::exists(&current) {
                return Some(current);
            }
            if !current.pop() {
                return None;
            }
        }
    }
}
