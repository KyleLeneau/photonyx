use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use chrono::NaiveDate;
use regex::Regex;

use crate::error::ConventionsError;

/// Matches the leaf observation folder, e.g. `RAW_300_Ha` or `PP_120_OIII_Narrow`.
///
/// Groups:
///   1. kind    — `RAW` or `PP`
///   2. exposure — integer seconds (e.g. `300`)
///   3. filter  — remainder after exposure, may contain underscores (e.g. `OIII_Narrow`)
static FOLDER_RE: OnceLock<Regex> = OnceLock::new();

fn folder_regex() -> &'static Regex {
    FOLDER_RE.get_or_init(|| Regex::new(r"^(RAW|PP)_(\d+(?:\.\d+)?)_(.+)$").unwrap())
}

#[derive(Debug)]
pub struct ObservationPath {
    /// The raw folder for the observation like "{profile_home}/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra"
    raw: PathBuf,

    /// The pre_processed path for the observation "{profile_home}/LIGHT/NGC_7000_NA_Nebula/2025-06-25/PP_300_Ultra"
    pp: PathBuf,

    /// The filter used parsed from the folder name, e.g. `Ha` or `OIII_Narrow`
    filter: Option<String>,

    /// The exposure time in seconds parsed from the folder name, e.g. `300.0`
    exposure: Option<f64>,

    /// The observation date parsed from the parent directory name, e.g. `2025-06-25`
    date: Option<NaiveDate>,

    /// The target name parsed from the grandparent directory, e.g. `NGC_7000_NA_Nebula`
    target_name: String,
}

// The raw path alone uniquely identifies an observation, so use it as the identity key.
impl PartialEq for ObservationPath {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl Eq for ObservationPath {}
impl Hash for ObservationPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl ObservationPath {
    pub fn raw_path(&self) -> &Path {
        &self.raw
    }

    pub fn pp_path(&self) -> &Path {
        &self.pp
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn exposure(&self) -> Option<f64> {
        self.exposure
    }

    pub fn date(&self) -> Option<NaiveDate> {
        self.date
    }

    pub fn target_name(&self) -> &str {
        &self.target_name
    }

    /// Given a path tries to parse and return a single Observation while parsing parts of the path
    ///
    pub fn single(path: &Path) -> Result<Self, ConventionsError> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(ConventionsError::NotFound)?;

        // Use regex to capture kind, exposure, and filter from the leaf folder name.
        // Filter comes last so it can safely contain underscores (e.g. "OIII_Narrow").
        let caps = folder_regex()
            .captures(name)
            .ok_or(ConventionsError::NotFound)?;

        let is_raw = &caps[1] == "RAW";

        let exposure: f64 = caps[2].parse().map_err(|_| {
            ConventionsError::InvalidFormat(format!(
                "exposure '{}' is not a valid number",
                &caps[2]
            ))
        })?;

        let filter = caps[3].to_string();

        // Walk up the path hierarchy to extract date and target_name.
        //   parent      → date directory  (e.g. 2025-06-25)
        //   grandparent → target directory (e.g. NGC_7000_NA_Nebula)
        let date_dir = path.parent().ok_or(ConventionsError::NotFound)?;
        let target_dir = date_dir.parent().ok_or(ConventionsError::NotFound)?;

        let date = date_dir
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let target_name = target_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(ConventionsError::NotFound)?
            .to_string();

        let (raw, pp) = if is_raw {
            let pp = path.with_file_name(format!("PP_{}_{}", exposure, filter));
            (path.to_path_buf(), pp)
        } else {
            let raw = path.with_file_name(format!("RAW_{}_{}", exposure, filter));
            (raw, path.to_path_buf())
        };

        // Always ensure we have a RAW path
        if !raw.is_dir() {
            return Err(ConventionsError::NotFound);
        }

        Ok(Self {
            raw,
            pp,
            filter: Some(filter),
            exposure: Some(exposure),
            date,
            target_name,
        })
    }

    /// Walk `path` recursively and collect every `RAW_*` / `PP_*` leaf found beneath it.
    ///
    /// Accepts paths at any ancestor depth — e.g. a target directory
    /// (`…/NGC_7000_NA_Nebula`) or a date directory (`…/2025-06-25`).
    /// Directories that match a leaf pattern are collected and not descended into.
    /// Returns `NotFound` if no observations are found.
    pub fn many(path: &Path) -> Result<Vec<Self>, ConventionsError> {
        let mut result = HashSet::<Self>::new();
        Self::walk(path, &mut result)?;
        if result.is_empty() {
            return Err(ConventionsError::NotFound);
        }
        Ok(result.into_iter().collect())
    }

    fn walk(path: &Path, out: &mut HashSet<Self>) -> Result<(), ConventionsError> {
        for entry in std::fs::read_dir(path)? {
            let entry_path = entry?.path();
            if !entry_path.is_dir() {
                continue;
            }
            match Self::single(&entry_path) {
                Ok(obs) => {
                    out.insert(obs);
                }
                Err(ConventionsError::NotFound) => Self::walk(&entry_path, out)?,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Return all observations reachable from `path`.
    ///
    /// - If `path` is itself a leaf (`RAW_*` / `PP_*`), it is included directly.
    /// - If `path` is a parent (target or date dir), every leaf beneath it is collected.
    /// - Both sources are merged and deduplicated by `raw` path.
    /// - Returns `NotFound` if neither source yields any result.
    pub fn from(path: &Path) -> Result<Vec<Self>, ConventionsError> {
        let mut result = HashSet::<Self>::new();

        if let Ok(obs) = ObservationPath::single(path) {
            result.insert(obs);
        }

        match ObservationPath::many(path) {
            Ok(multi) => result.extend(multi),
            Err(ConventionsError::NotFound) => {}
            Err(e) => return Err(e),
        }

        if result.is_empty() {
            return Err(ConventionsError::NotFound);
        }

        Ok(result.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_raw_folder() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ha");
        fs::create_dir_all(&raw).unwrap();

        let obs = ObservationPath::single(&raw).unwrap();

        assert_eq!(obs.raw_path(), &raw);
        assert_eq!(
            obs.pp_path(),
            dir.path().join("NGC_7000_NA_Nebula/2025-06-25/PP_300_Ha")
        );
        assert_eq!(obs.target_name(), "NGC_7000_NA_Nebula");
        assert_eq!(obs.date(), NaiveDate::from_ymd_opt(2025, 6, 25));
        assert_eq!(obs.exposure(), Some(300.0));
        assert_eq!(obs.filter(), Some("Ha"));
    }

    #[test]
    fn parses_pp_folder_with_underscored_filter() {
        let dir = tempfile::tempdir().unwrap();
        // Only the PP folder needs to exist; RAW is derived but not checked
        let pp = dir.path().join("NGC_7000/2025-06-25/PP_120_OIII_Narrow");
        let raw = dir.path().join("NGC_7000/2025-06-25/RAW_120_OIII_Narrow");
        fs::create_dir_all(&pp).unwrap();
        fs::create_dir_all(&raw).unwrap();

        let obs = ObservationPath::single(&pp).unwrap();

        assert_eq!(obs.pp_path(), &pp);
        assert_eq!(obs.raw_path(), &raw);
        assert_eq!(obs.filter(), Some("OIII_Narrow"));
    }

    #[test]
    fn returns_not_found_for_unrecognised_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("NGC_7000/2025-06-25/Misc_300_Ha");
        fs::create_dir_all(&path).unwrap();

        assert!(matches!(
            ObservationPath::single(&path),
            Err(ConventionsError::NotFound)
        ));
    }

    #[test]
    fn returns_not_found_when_raw_folder_missing() {
        let dir = tempfile::tempdir().unwrap();
        // Create only the PP folder; RAW counterpart does not exist on disk
        let pp = dir.path().join("NGC_7000/2025-06-25/PP_300_Ha");
        fs::create_dir_all(&pp).unwrap();

        assert!(matches!(
            ObservationPath::single(&pp),
            Err(ConventionsError::NotFound)
        ));
    }

    #[test]
    fn date_is_none_for_non_date_parent() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("NGC_7000/not-a-date/RAW_300_Ha");
        fs::create_dir_all(&raw).unwrap();

        let obs = ObservationPath::single(&raw).unwrap();
        assert!(obs.date().is_none());
    }

    // Helpers — build a temp directory tree and return the root.
    //
    //   root/
    //     NGC_7000/
    //       2025-06-25/
    //         RAW_300_Ha/
    //         RAW_120_OIII/
    //       2025-06-26/
    //         RAW_300_Ha/   ← RAW must exist; PP is the entry point but RAW is checked
    //         PP_300_Ha/
    fn build_target_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for leaf in &[
            "NGC_7000/2025-06-25/RAW_300_Ha",
            "NGC_7000/2025-06-25/RAW_120_OIII",
            "NGC_7000/2025-06-26/RAW_300_Ha",
            "NGC_7000/2025-06-26/PP_300_Ha",
        ] {
            fs::create_dir_all(root.join(leaf)).unwrap();
        }
        dir
    }

    #[test]
    fn many_from_target_dir_finds_all_leaves() {
        let dir = build_target_tree();
        let target = dir.path().join("NGC_7000");

        let mut results = ObservationPath::many(&target).unwrap();
        results.sort_by(|a, b| a.raw.cmp(&b.raw));

        assert_eq!(results.len(), 3);
        // sorted by raw path: RAW_120_OIII < RAW_300_Ha < (PP_300_Ha → raw = RAW_300_Ha, same key — deduplicated)
        assert_eq!(results[0].filter, Some("OIII".into()));
        assert_eq!(results[0].exposure, Some(120.0));
        assert_eq!(results[1].filter, Some("Ha".into()));
        assert_eq!(results[1].exposure, Some(300.0));
        assert_eq!(
            results[2].pp,
            dir.path().join("NGC_7000/2025-06-26/PP_300_Ha")
        );
    }

    #[test]
    fn many_from_date_dir_finds_leaves_under_it() {
        let dir = build_target_tree();
        let date_dir = dir.path().join("NGC_7000/2025-06-25");

        let mut results = ObservationPath::many(&date_dir).unwrap();
        results.sort_by(|a, b| a.raw.cmp(&b.raw));

        assert_eq!(results.len(), 2);
        // sorted by raw path: RAW_120_OIII < RAW_300_Ha
        assert_eq!(results[0].filter, Some("OIII".into()));
        assert_eq!(results[1].filter, Some("Ha".into()));
    }

    #[test]
    fn many_returns_not_found_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            ObservationPath::many(dir.path()),
            Err(ConventionsError::NotFound)
        ));
    }

    // from() tests

    #[test]
    fn from_on_a_leaf_returns_that_single_observation() {
        let dir = build_target_tree();
        let leaf = dir.path().join("NGC_7000/2025-06-25/RAW_300_Ha");

        let results = ObservationPath::from(&leaf).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].raw, leaf);
        assert_eq!(results[0].filter, Some("Ha".into()));
        assert_eq!(results[0].exposure, Some(300.0));
    }

    #[test]
    fn from_on_date_dir_returns_all_leaves_under_it() {
        let dir = build_target_tree();
        let date_dir = dir.path().join("NGC_7000/2025-06-25");

        let mut results = ObservationPath::from(&date_dir).unwrap();
        results.sort_by(|a, b| a.raw.cmp(&b.raw));

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].filter, Some("OIII".into()));
        assert_eq!(results[1].filter, Some("Ha".into()));
    }

    #[test]
    fn from_on_target_dir_returns_all_leaves_across_dates() {
        let dir = build_target_tree();
        let target = dir.path().join("NGC_7000");

        let mut results = ObservationPath::from(&target).unwrap();
        results.sort_by(|a, b| a.raw.cmp(&b.raw));

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].filter, Some("OIII".into()));
        assert_eq!(results[1].filter, Some("Ha".into()));
        assert_eq!(results[2].date, NaiveDate::from_ymd_opt(2025, 6, 26));
    }

    #[test]
    fn from_deduplicates_matching_raw_and_pp_leaves() {
        // Build a tree where both RAW_ and PP_ exist for the same observation.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("NGC_7000/2025-06-25/RAW_300_Ha")).unwrap();
        fs::create_dir_all(root.join("NGC_7000/2025-06-25/PP_300_Ha")).unwrap();

        let target = root.join("NGC_7000");
        let results = ObservationPath::from(&target).unwrap();

        // Both resolve to the same raw path → deduplicated to 1
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filter, Some("Ha".into()));
    }

    #[test]
    fn from_returns_not_found_for_dir_with_no_observations() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            ObservationPath::from(dir.path()),
            Err(ConventionsError::NotFound)
        ));
    }
}
