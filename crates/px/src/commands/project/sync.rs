use std::{collections::HashSet, path::PathBuf};

use anyhow::Result;
use px_cli::SyncProjectArgs;
use px_configuration::{Framing, ObservationEntry, ProjectLinearStack, SyncPolicy};
use px_conventions::project::ProjectPath;
use px_index::ProfileIndex;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn sync_project(
    project_path: Option<PathBuf>,
    _args: SyncProjectArgs,
    printer: Printer,
    profile_index: ProfileIndex,
) -> Result<ExitStatus> {
    let project = match ProjectPath::find(project_path) {
        Ok(p) => p,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    let mut config = project.load_config()?;

    if config.sync_policy == SyncPolicy::Manual {
        printer.error(
            "this project has sync_policy: manual — automatic sync is disabled.\n\
             Use `px project edit` to update layers interactively.",
        )?;
        return Ok(ExitStatus::Failure);
    }

    let target = match &config.target {
        Some(t) => t.clone(),
        None => {
            printer.error(
                "no target is set on this project — cannot sync.\n\
                 Add `target: <name>` to px_project.yaml or re-run `px project init --target <name>`.",
            )?;
            return Ok(ExitStatus::Failure);
        }
    };

    let index_obs = profile_index
        .list_observations_by_target(Some(&target))
        .await?;

    if index_obs.is_empty() {
        printer.warn(format!(
            "no observations found in index for target `{target}`"
        ))?;
        return Ok(ExitStatus::Success);
    }

    let mut total_added: usize = 0;

    match &mut config.framing {
        Framing::Single(single) => {
            for stack in &mut single.master_lights {
                let added = sync_stack(stack, &index_obs, &printer)?;
                total_added += added;
            }
        }
        Framing::SpiralMosiac(spiral) => {
            // Match all index observations whose filter equals the framing filter (if set).
            let existing: HashSet<PathBuf> =
                spiral.observations.iter().map(|o| o.path.clone()).collect();

            let mut added = 0usize;
            for obs in &index_obs {
                let matches_filter = spiral
                    .filter
                    .as_deref()
                    .map(|f| f == obs.filter)
                    .unwrap_or(false);

                if !matches_filter {
                    continue;
                }

                let path = PathBuf::from(&obs.raw_path);
                if !existing.contains(&path) {
                    spiral.observations.push(ObservationEntry { path });
                    added += 1;
                }
            }

            if added > 0 {
                printer.info(format!("spiral mosaic: added {added} observation(s)"))?;
            } else {
                printer.info("spiral mosaic: already up to date")?;
            }
            total_added += added;
        }
        Framing::GridMosiac(grid) => {
            for master_light in &mut grid.master_lights {
                for panel in &mut master_light.panels {
                    let added = sync_stack(panel, &index_obs, &printer)?;
                    total_added += added;
                }
            }
        }
    }

    config.save(&project.root)?;

    if total_added > 0 {
        printer.success(format!(
            "synced {total_added} new observation(s) into project"
        ))?;
    } else {
        printer.success("project is already up to date")?;
    }

    Ok(ExitStatus::Success)
}

/// Append any index observations matching `stack.filter` that are not already in `stack.observations`.
/// Returns the number added.
fn sync_stack(
    stack: &mut ProjectLinearStack,
    index_obs: &[px_index::ObservationRecord],
    printer: &Printer,
) -> Result<usize> {
    let existing: HashSet<PathBuf> = stack.observations.iter().map(|o| o.path.clone()).collect();

    let mut added = 0usize;
    for obs in index_obs {
        let matches_filter = stack
            .filter
            .as_deref()
            .map(|f| f == obs.filter)
            .unwrap_or(false);

        if !matches_filter {
            continue;
        }

        let path = PathBuf::from(&obs.raw_path);
        if !existing.contains(&path) {
            stack.observations.push(ObservationEntry { path });
            added += 1;
        }
    }

    if added > 0 {
        printer.info(format!(
            "layer `{}`: added {added} observation(s)",
            stack.name
        ))?;
    }

    Ok(added)
}
