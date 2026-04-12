use std::path::{Path, PathBuf};

use anyhow::Result;
use inquire::{InquireError, Select, Text};
use px_cli::AddProjectArgs;
use px_configuration::AddObservationOutcome;
use px_fits::{FitsFile, all_fits_files};

use crate::{ExitStatus, printer::Printer, resolve::first_some};

/// Primary command handler for `px project add`
/// Default mode is interactive prompting
///
pub(crate) async fn add_project_observation(
    args: AddProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    // Find the project dir and config to work with
    let (project_dir, mut config) = match super::find_and_load_project(args.project) {
        Ok(tuple) => tuple,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    // Canonicalize so stored paths are absolute and stable
    let obs_path = args.obs_path.canonicalize()?;

    // Derive profile root from the LIGHT/ ancestor in the observation path
    let profile_root = match derive_profile_root(&obs_path) {
        Some(p) => p,
        None => {
            printer.error(format!(
                "could not derive hardware profile from `{}` — \
                 expected a `LIGHT` directory somewhere in the path",
                obs_path.display()
            ))?;
            return Ok(ExitStatus::Failure);
        }
    };

    // Resolve filter: --filter flag → FITS headers → path components → interactive prompt → error
    let filter = match first_some![
        || Ok(args.filter.clone()),
        || detect_filter_from_fits(&obs_path),
        || detect_filter_from_filename(&obs_path),
        prompt_filter,
    ]? {
        Some(f) => f,
        None => {
            printer.error(
                "could not determine filter from FITS headers or path; use --filter to specify it",
            )?;
            return Ok(ExitStatus::Failure);
        }
    };

    let outcome = config.add_observation(
        profile_root.clone(),
        filter.clone(),
        args.panel.clone(),
        obs_path.clone(),
    );

    if outcome == AddObservationOutcome::AlreadyRegistered {
        printer.warn(format!(
            "observation `{}` is already registered in this stack",
            obs_path.display()
        ))?;
        return Ok(ExitStatus::Success);
    }

    config.save(&project_dir)?;

    let profile_name = profile_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    printer.success(format!(
        "added observation to `{}` — filter: {filter}, profile: {profile_name}{}",
        config.name,
        args.panel
            .as_deref()
            .map(|p| format!(", panel: {p}"))
            .unwrap_or_default()
    ))?;

    Ok(ExitStatus::Success)
}

/// Common astrophotography filter names — used for both filename detection and quick-picks.
const KNOWN_FILTERS: &[&str] = &["Ha", "OIII", "SII", "Lum", "Red", "Green", "Blue"];

/// Walk up `obs_path` components to find the directory named `LIGHT/`, then return its parent
/// as the hardware profile root (e.g. `.../PX_Radian75/LIGHT/...` → `.../PX_Radian75`).
fn derive_profile_root(obs_path: &Path) -> Option<PathBuf> {
    let mut current = obs_path;
    loop {
        let parent = current.parent()?;
        if current.file_name().and_then(|n| n.to_str()) == Some("LIGHT") {
            return Some(parent.to_path_buf());
        }
        current = parent;
    }
}

/// Read the `FILTER` header value from the first FITS file found in `obs_path`.
fn detect_filter_from_fits(obs_path: &Path) -> Result<Option<String>> {
    let files = all_fits_files(obs_path)?;
    let Some(first) = files.first() else {
        return Ok(None);
    };
    let filter = FitsFile::new(first.clone())?.filter();
    Ok(filter)
}

/// Scan path components for a known filter name (case-insensitive).
///
/// Matches paths like `.../LIGHT/Ha/`, `.../Ha_LIGHT/`, or `.../NGC1499_Ha_300s/`.
fn detect_filter_from_filename(obs_path: &Path) -> Result<Option<String>> {
    let components: Vec<&str> = obs_path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    for filter in KNOWN_FILTERS {
        let upper = filter.to_uppercase();
        let found = components.iter().any(|segment| {
            let seg_upper = segment.to_uppercase();
            // Match exact component or as a word within a compound name (e.g. "Ha_300s")
            seg_upper == upper
                || seg_upper.starts_with(&format!("{upper}_"))
                || seg_upper.ends_with(&format!("_{upper}"))
                || seg_upper.contains(&format!("_{upper}_"))
        });
        if found {
            return Ok(Some(filter.to_string()));
        }
    }

    Ok(None)
}

/// Prompt the user to pick or type a filter name interactively.
/// Returns `None` if stdin is not a TTY or the user cancels.
fn prompt_filter() -> Result<Option<String>> {
    let mut options: Vec<&str> = KNOWN_FILTERS.to_vec();
    options.push("Other...");

    let choice = match Select::new("Could not detect filter. Select one:", options).prompt() {
        Ok(c) => c,
        Err(InquireError::NotTTY | InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            return Ok(None)
        }
        Err(e) => return Err(e.into()),
    };

    if choice == "Other..." {
        match Text::new("Enter filter name:").prompt() {
            Ok(f) => Ok(Some(f)),
            Err(InquireError::NotTTY | InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    } else {
        Ok(Some(choice.to_string()))
    }
}
