use std::path::{Path, PathBuf};

use anyhow::Result;
use px_cli::AddProjectArgs;
use px_configuration::AddObservationOutcome;
use px_fits::{FitsFile, all_fits_files};

use crate::{ExitStatus, printer::Printer};

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

    // Resolve filter — explicit override takes precedence, otherwise read from FITS headers
    let filter = match args.filter {
        Some(f) => f,
        None => match detect_filter_from_fits(&obs_path)? {
            Some(f) => f,
            None => {
                printer.error(
                    "could not determine filter from FITS headers; use --filter to specify it",
                )?;
                return Ok(ExitStatus::Failure);
            }
        },
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
