use std::{fmt::Write, path::PathBuf};

use anyhow::Result;
use px_cli::BatchCalibrateObservationArgs;
use px_conventions::observation::ObservationPath;
use px_index::{MasterKind, ProfileIndex};
use px_pipeline::calibrate_light::CalibrateLightSetPipeline;
use siril_sys::Builder;

use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter, utils::to_fits_ext};

pub(crate) async fn batch_calibrate_observations(
    args: BatchCalibrateObservationArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    let target = args.target.as_deref();
    let observations = if args.clean {
        index.list_observations_by_target(target).await?
    } else {
        index.list_uncalibrated_observations(target).await?
    };

    if observations.is_empty() {
        let msg = match target {
            Some(t) => format!("No uncalibrated observations for target '{t}'"),
            None => "No uncalibrated observations".to_string(),
        };
        printer.info(msg)?;
        return Ok(ExitStatus::Success);
    }

    printer.info(format!(
        "Batch calibration starting: {} uncalibrated observation(s){}",
        observations.len(),
        target
            .map(|t| format!(" for target '{t}'"))
            .unwrap_or_default()
    ))?;

    let mut calibrated: usize = 0;
    let mut skipped_exists: usize = 0;
    let mut failed: usize = 0;

    for (i, record) in observations.iter().enumerate() {
        if i > 0 {
            writeln!(printer.stdout())?;
        }

        let label = obs_label(
            &record.target_name,
            &record.date,
            &record.filter,
            record.exposure,
        );

        let raw_path = PathBuf::from(&record.raw_path);
        let obs = match ObservationPath::single(&raw_path) {
            Ok(o) => o,
            Err(e) => {
                printer.warn(format!(
                    "  {label}  →  cannot resolve observation path: {e}, skipping"
                ))?;
                failed += 1;
                continue;
            }
        };
        let out_folder = obs.pp_path().to_path_buf();

        let masters = index.get_linked_masters(&record.id).await?;
        if masters.is_empty() {
            printer.warn(format!(
                "  {label}  →  no masters linked, proceeding without"
            ))?;
        }

        let dark = masters
            .iter()
            .find(|m| m.kind == MasterKind::Dark)
            .map(|m| PathBuf::from(&m.master_path));
        let flat = masters
            .iter()
            .find(|m| m.kind == MasterKind::Flat)
            .map(|m| PathBuf::from(&m.master_path));
        let bias = masters
            .iter()
            .find(|m| m.kind == MasterKind::Bias)
            .map(|m| PathBuf::from(&m.master_path));

        if args.clean && out_folder.exists() {
            std::fs::remove_dir_all(&out_folder)?;
        }

        if out_folder.exists() {
            printer.warn(format!(
                "  {label}  →  output folder already exists, use --clean to overwrite"
            ))?;
            skipped_exists += 1;
            continue;
        }

        if args.dry_run {
            let mut parts: Vec<&str> = Vec::new();
            if dark.is_some() {
                parts.push("dark");
            }
            if flat.is_some() {
                parts.push("flat");
            }
            if bias.is_some() {
                parts.push("bias");
            }
            printer.info(format!(
                "  {label}  →  would calibrate [{}]",
                parts.join(", ")
            ))?;
            print_master_paths(printer, &dark, &flat, &bias)?;
            calibrated += 1;
            continue;
        }

        std::fs::create_dir_all(&out_folder)?;
        printer.info(format!("  {label}  →  calibrating…"))?;
        print_master_paths(printer, &dark, &flat, &bias)?;

        let result = CalibrateLightSetPipeline::builder()
            .ext(to_fits_ext(args.ext))
            .siril_builder(Builder::default().output_sink(siril_sys::OutputSink::Discard))
            .raw_folder(raw_path)
            .out_folder(out_folder.clone())
            .maybe_bias(bias)
            .maybe_dark(dark)
            .maybe_flat(flat)
            .build()
            .run(DefaultPipelineReporter::from(printer))
            .await;

        match result {
            Ok(light) => {
                let path_str = out_folder.to_string_lossy().into_owned();
                index.update_calibrated_path(&record.id, &path_str).await?;
                printer.success(format!("  {label}  →  done ({} frames)", light.frame_count))?;
                calibrated += 1;
            }
            Err(e) => {
                printer.error(format!("  {label}  →  failed: {e}"))?;
                failed += 1;
            }
        }
    }

    printer.success(format!(
        "Batch calibration complete: {calibrated} calibrated, \
         {skipped_exists} skipped (output exists), \
         {failed} failed"
    ))?;

    if failed > 0 {
        Ok(ExitStatus::Error)
    } else {
        Ok(ExitStatus::Success)
    }
}

fn print_master_paths(
    printer: Printer,
    dark: &Option<PathBuf>,
    flat: &Option<PathBuf>,
    bias: &Option<PathBuf>,
) -> std::fmt::Result {
    if let Some(p) = dark {
        printer.info(format!("    dark: {}", p.display()))?;
    }
    if let Some(p) = flat {
        printer.info(format!("    flat: {}", p.display()))?;
    }
    if let Some(p) = bias {
        printer.info(format!("    bias: {}", p.display()))?;
    }
    Ok(())
}

fn obs_label(target: &str, date: &str, filter: &str, exposure: f64) -> String {
    format!("{target} / {date} / {filter} {exposure}s")
}
