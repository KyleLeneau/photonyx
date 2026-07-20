use anyhow::Result;
use chrono::NaiveDate;
use px_cli::ScanProfileArgs;
use px_conventions::observation::ObservationPath;
use px_fits::{ObservationMetadata, all_fits_files};
use px_index::{CalibrationRecord, MasterKind, MatchCriteria, ObservationRecord, ProfileIndex};

use crate::{ExitStatus, printer::Printer};

// ── Outcome ───────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
struct LinkCounts {
    dark_flat: usize,
    dark_bias: usize,
    flat_only: usize,
    dark_only: usize,
    bias_only: usize,
    no_match: usize,
}

impl LinkCounts {
    fn total_linked(&self) -> usize {
        self.dark_flat + self.dark_bias + self.flat_only + self.dark_only + self.bias_only
    }
}

enum LinkOutcome {
    DarkFlat {
        dark: CalibrationRecord,
        flat: CalibrationRecord,
    },
    DarkBias {
        dark: CalibrationRecord,
        bias: CalibrationRecord,
    },
    FlatOnly(CalibrationRecord),
    DarkOnly(CalibrationRecord),
    BiasOnly(CalibrationRecord),
    NoMatch,
}

impl LinkOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::DarkFlat { .. } => "dark+flat",
            Self::DarkBias { .. } => "dark+bias",
            Self::FlatOnly(_) => "flat",
            Self::DarkOnly(_) => "dark",
            Self::BiasOnly(_) => "bias",
            Self::NoMatch => "no match",
        }
    }

    fn tally(&self, counts: &mut LinkCounts) {
        match self {
            Self::DarkFlat { .. } => counts.dark_flat += 1,
            Self::DarkBias { .. } => counts.dark_bias += 1,
            Self::FlatOnly(_) => counts.flat_only += 1,
            Self::DarkOnly(_) => counts.dark_only += 1,
            Self::BiasOnly(_) => counts.bias_only += 1,
            Self::NoMatch => counts.no_match += 1,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Run calibration matching and return the best `LinkOutcome`.
/// Does not write to the DB — the caller decides when to commit links.
async fn resolve_best_calibration(
    index: &ProfileIndex,
    exposure: f64,
    filter: &str,
    temperature: Option<f64>,
    criteria: &MatchCriteria,
) -> Result<LinkOutcome> {
    let dark = index.find_best_dark(exposure, criteria).await?;
    let flat = index.find_best_flat(filter, criteria).await?;
    // find_best_bias requires temperature; treat absent temperature as no-bias.
    let bias = if temperature.is_some() {
        index.find_best_bias(criteria).await.ok().flatten()
    } else {
        None
    };

    // Priority: dark+flat > dark+bias > flat-only > dark-only > bias-only
    let outcome = match (dark, flat, bias) {
        (Some(d), Some(f), _) => LinkOutcome::DarkFlat { dark: d, flat: f },
        (Some(d), None, Some(b)) => LinkOutcome::DarkBias { dark: d, bias: b },
        (None, Some(f), _) => LinkOutcome::FlatOnly(f),
        (Some(d), None, None) => LinkOutcome::DarkOnly(d),
        (None, None, Some(b)) => LinkOutcome::BiasOnly(b),
        _ => LinkOutcome::NoMatch,
    };

    Ok(outcome)
}

/// Write `calibration_link` rows for the given outcome.
async fn commit_links(index: &ProfileIndex, obs_id: &str, outcome: &LinkOutcome) -> Result<()> {
    match outcome {
        LinkOutcome::DarkFlat { dark, flat } => {
            index
                .link_calibration(obs_id, &dark.id, MasterKind::Dark)
                .await?;
            index
                .link_calibration(obs_id, &flat.id, MasterKind::Flat)
                .await?;
        }
        LinkOutcome::DarkBias { dark, bias } => {
            index
                .link_calibration(obs_id, &dark.id, MasterKind::Dark)
                .await?;
            index
                .link_calibration(obs_id, &bias.id, MasterKind::Bias)
                .await?;
        }
        LinkOutcome::FlatOnly(f) => {
            index
                .link_calibration(obs_id, &f.id, MasterKind::Flat)
                .await?;
        }
        LinkOutcome::DarkOnly(d) => {
            index
                .link_calibration(obs_id, &d.id, MasterKind::Dark)
                .await?;
        }
        LinkOutcome::BiasOnly(b) => {
            index
                .link_calibration(obs_id, &b.id, MasterKind::Bias)
                .await?;
        }
        LinkOutcome::NoMatch => {}
    }
    Ok(())
}

fn criteria_from_record(record: &ObservationRecord) -> MatchCriteria {
    let date = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d").ok();
    MatchCriteria {
        temperature: record.temperature,
        gain: record.gain,
        offset: record.offset,
        binning: record.binning.clone(),
        date,
        date_tolerance_days: Some(720),
        temperature_tolerance: Some(5.0),
    }
}

fn obs_label(target: &str, date: &str, filter: &str, exposure: f64) -> String {
    format!("{target} / {date} / {filter} {exposure}s")
}

// ── Command ───────────────────────────────────────────────────────────────────

pub(crate) async fn scan_profile(
    args: ScanProfileArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    if args.purge {
        purge(&printer, &index).await
    } else if args.relink {
        relink(&printer, &index).await
    } else {
        scan(&printer, &index).await
    }
}

/// Scan LIGHT/ for new observation folders and index them.
async fn scan(printer: &Printer, index: &ProfileIndex) -> Result<ExitStatus> {
    let light_dir = &index.profile.light;

    let observations = match ObservationPath::from(light_dir) {
        Ok(obs) => obs,
        Err(_) => {
            printer.info("No observation folders found in LIGHT/")?;
            return Ok(ExitStatus::Success);
        }
    };

    let mut added: usize = 0;
    let mut skipped: usize = 0;
    let mut warned: usize = 0;
    let mut counts = LinkCounts::default();

    for obs in &observations {
        let raw_path_str = obs.raw_path().to_string_lossy();

        if index.observation_exists(&raw_path_str).await? {
            skipped += 1;
            continue;
        }

        // Read FITS headers from RAW frames — they carry the original capture metadata.
        let raw_files = match all_fits_files(obs.raw_path()) {
            Ok(f) => f,
            Err(e) => {
                printer.warn(format!("Could not list files in {:?}: {e}", obs.raw_path()))?;
                warned += 1;
                continue;
            }
        };

        if raw_files.is_empty() {
            printer.warn(format!("No FITS files in {:?}, skipping", obs.raw_path()))?;
            warned += 1;
            continue;
        }

        let meta = match ObservationMetadata::from(raw_files) {
            Ok(m) => m,
            Err(e) => {
                printer.warn(format!(
                    "Could not read FITS headers from {:?}: {e}, skipping",
                    obs.raw_path()
                ))?;
                warned += 1;
                continue;
            }
        };

        let calibrated_path = if obs.pp_path().is_dir() {
            Some(obs.pp_path().to_string_lossy().into_owned())
        } else {
            None
        };

        let record = ObservationRecord::from_scan(obs, &meta, calibrated_path);
        let criteria = criteria_from_record(&record);
        let exposure = record.exposure;
        let filter = record.filter.clone();
        let date_str = record.date.clone();
        let obs_id = index.register_observation(record).await?;

        let outcome =
            resolve_best_calibration(index, exposure, &filter, meta.temperature, &criteria).await?;

        printer.info(format!(
            "  {}  →  {}",
            obs_label(obs.target_name(), &date_str, &filter, exposure),
            outcome.label()
        ))?;

        commit_links(index, &obs_id, &outcome).await?;
        outcome.tally(&mut counts);
        added += 1;
    }

    printer.success(format!(
        "Scan complete: {added} added, {skipped} already indexed, {warned} skipped with warnings"
    ))?;
    print_link_summary(printer, &counts)?;

    Ok(ExitStatus::Success)
}

/// Re-run calibration linking for all already-indexed observations.
async fn relink(printer: &Printer, index: &ProfileIndex) -> Result<ExitStatus> {
    let observations = index.list_observations().await?;

    if observations.is_empty() {
        printer.info("No observations in index to relink")?;
        return Ok(ExitStatus::Success);
    }

    printer.info(format!("Re-linking {} observations…", observations.len()))?;

    let mut counts = LinkCounts::default();

    for record in &observations {
        let criteria = criteria_from_record(record);
        let outcome = resolve_best_calibration(
            index,
            record.exposure,
            &record.filter,
            record.temperature,
            &criteria,
        )
        .await?;

        printer.info(format!(
            "  {}  →  {}",
            obs_label(
                &record.target_name,
                &record.date,
                &record.filter,
                record.exposure
            ),
            outcome.label()
        ))?;

        index.clear_calibration_links(&record.id).await?;
        commit_links(index, &record.id, &outcome).await?;
        outcome.tally(&mut counts);
    }

    printer.success(format!(
        "Re-link complete: {} processed",
        observations.len()
    ))?;
    print_link_summary(printer, &counts)?;

    Ok(ExitStatus::Success)
}

/// Remove indexed observations whose raw path no longer exists on disk.
async fn purge(printer: &Printer, index: &ProfileIndex) -> Result<ExitStatus> {
    let observations = index.list_observations().await?;

    let missing: Vec<ObservationRecord> = observations
        .into_iter()
        .filter(|record| !std::path::Path::new(&record.raw_path).exists())
        .collect();

    if missing.is_empty() {
        printer.info("No missing observations found — index is clean")?;
        return Ok(ExitStatus::Success);
    }

    printer.info(format!(
        "Found {} observation(s) with missing raw path:",
        missing.len()
    ))?;
    for record in &missing {
        printer.info(format!(
            "  {}  →  {}",
            obs_label(
                &record.target_name,
                &record.date,
                &record.filter,
                record.exposure
            ),
            record.raw_path
        ))?;
    }

    let confirmed = inquire::Confirm::new(&format!(
        "Remove {} observation(s) from the index?",
        missing.len()
    ))
    .with_default(false)
    .prompt()?;

    if !confirmed {
        printer.info("Purge cancelled")?;
        return Ok(ExitStatus::Success);
    }

    for record in &missing {
        index.delete_observation(&record.id).await?;
    }

    printer.success(format!("Purge complete: {} removed", missing.len()))?;

    Ok(ExitStatus::Success)
}

fn print_link_summary(printer: &Printer, counts: &LinkCounts) -> std::fmt::Result {
    let linked = counts.total_linked();
    let mut parts: Vec<String> = Vec::new();

    if counts.dark_flat > 0 {
        parts.push(format!("dark+flat: {}", counts.dark_flat));
    }
    if counts.dark_bias > 0 {
        parts.push(format!("dark+bias: {}", counts.dark_bias));
    }
    if counts.flat_only > 0 {
        parts.push(format!("flat: {}", counts.flat_only));
    }
    if counts.dark_only > 0 {
        parts.push(format!("dark: {}", counts.dark_only));
    }
    if counts.bias_only > 0 {
        parts.push(format!("bias: {}", counts.bias_only));
    }
    if counts.no_match > 0 {
        parts.push(format!("no match: {}", counts.no_match));
    }

    if parts.is_empty() {
        return Ok(());
    }

    printer.info(format!(
        "Calibration links: {linked} linked  |  {}",
        parts.join("  |  ")
    ))
}
