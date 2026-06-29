use std::path::Path;

use anyhow::Result;
use px_cli::CalibrateObservationArgs;
use px_conventions::observation::ObservationPath;
use px_fits::all_fits_files;
use px_index::{MasterKind, ProfileIndex};
use px_pipeline::calibrate_light::CalibrateLightSetPipeline;
use siril_sys::Builder;

use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter, utils::to_fits_ext};

pub(crate) async fn calibrate_observation(
    args: CalibrateObservationArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    printer.info("Observation calibration starting")?;
    printer.info(format!("clean output: {}", args.clean))?;
    printer.info(format!("input folder: {:?}", args.raw_folder))?;
    printer.info(format!("output folder: {:?}", args.out_folder))?;
    printer.info(format!("dark: {:?}", args.dark))?;
    printer.info(format!("flat: {:?}", args.flat))?;
    printer.info(format!("bias: {:?}", args.bias))?;

    // Use convetions to find observation path
    let obs = match ObservationPath::single(&args.raw_folder) {
        Ok(o) => o,
        Err(_) => {
            printer.error("Error finding observation RAW folder")?;
            return Ok(ExitStatus::Error);
        }
    };

    // Check that all files in raw folder are fits
    let raw_files = all_fits_files(&args.raw_folder)?;
    if raw_files.is_empty() {
        printer.error("raw_folder contains no files")?;
        return Ok(ExitStatus::Error);
    }

    // Get the output folder or compute it
    let resolved_out_folder = match args.out_folder.clone() {
        Some(folder) => folder,
        None => obs.pp_path().to_path_buf(),
    };

    // Clean output if specified and exists
    if args.clean && resolved_out_folder.exists() {
        std::fs::remove_dir_all(&resolved_out_folder)?;
    }

    // Validate we got some master's passed in
    // if args.bias.is_none() && args.dark.is_none() && args.flat.is_none() {
    //     printer.error("dark, flat, or bias not specified")?;
    //     return Ok(ExitStatus::Error);
    // }

    if args.bias.is_some() && !args.bias.as_ref().unwrap().exists() {
        printer.error("bias specified does not exist")?;
        return Ok(ExitStatus::Error);
    }

    if args.dark.is_some() && !args.dark.as_ref().unwrap().exists() {
        printer.error("dark specified does not exist")?;
        return Ok(ExitStatus::Error);
    }

    if args.flat.is_some() && !args.flat.as_ref().unwrap().exists() {
        printer.error("bias specified does not exist")?;
        return Ok(ExitStatus::Error);
    }

    // Create the out folder
    if resolved_out_folder.exists() {
        printer.warn("output folder already exists, use --clean")?;
        return Ok(ExitStatus::Error);
    } else {
        std::fs::create_dir_all(&resolved_out_folder)?;
        printer.info(format!("created output folder: {:?}", resolved_out_folder))?;
    }

    // Clone before the pipeline moves them so we can look them up in the index afterward.
    let dark_path = args.dark.clone();
    let flat_path = args.flat.clone();
    let bias_path = args.bias.clone();

    let light = CalibrateLightSetPipeline::builder()
        .ext(to_fits_ext(args.ext))
        .siril_builder(Builder::default().output_sink(siril_sys::OutputSink::Discard))
        .raw_folder(args.raw_folder)
        .out_folder(resolved_out_folder.clone())
        .maybe_bias(args.bias)
        .maybe_dark(args.dark)
        .maybe_flat(args.flat)
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    printer.success(format!(
        "Calibration of light frames completed: {:?}",
        light
    ))?;

    let obs_id = index.register_observation(light).await?;
    printer.info("Observation registered in profile index")?;

    // Link whichever masters were explicitly passed to this run.
    // Masters not in the calibration_set are skipped with a warning — the
    // calibration itself already succeeded, we just can't record the link.
    link_master_if_registered(
        &index,
        &obs_id,
        dark_path.as_deref(),
        MasterKind::Dark,
        &printer,
    )
    .await?;
    link_master_if_registered(
        &index,
        &obs_id,
        flat_path.as_deref(),
        MasterKind::Flat,
        &printer,
    )
    .await?;
    link_master_if_registered(
        &index,
        &obs_id,
        bias_path.as_deref(),
        MasterKind::Bias,
        &printer,
    )
    .await?;

    Ok(ExitStatus::Success)
}

async fn link_master_if_registered(
    index: &ProfileIndex,
    obs_id: &str,
    master_path: Option<&Path>,
    kind: MasterKind,
    printer: &Printer,
) -> Result<()> {
    let Some(path) = master_path else {
        return Ok(());
    };

    let path_str = path.to_string_lossy();
    match index.find_master_by_path(&path_str).await? {
        Some(master) => {
            index.link_calibration(obs_id, &master.id, kind).await?;
            printer.info(format!("Linked {} master: {}", kind.as_str(), path_str))?;
        }
        None => {
            printer.warn(format!(
                "{} master at {} is not in the calibration index — link skipped",
                kind.as_str(),
                path_str
            ))?;
        }
    }

    Ok(())
}
