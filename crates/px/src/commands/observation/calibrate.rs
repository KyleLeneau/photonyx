use anyhow::Result;
use px_cli::CalibrateObservationArgs;
use px_conventions::observation::ObservationPath;
use px_fits::all_fits_files;
use px_index::ProfileIndex;
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

    // Create the out folder
    if resolved_out_folder.exists() {
        printer.warn("output folder already exists, use --clean")?;
        return Ok(ExitStatus::Error);
    } else {
        std::fs::create_dir_all(&resolved_out_folder)?;
        printer.info(format!("created output folder: {:?}", resolved_out_folder))?;
    }

    // Validate we got some master's passed in
    if args.bias.is_none() && args.dark.is_none() && args.flat.is_none() {
        printer.error("dark, flat, or bias not specified")?;
        return Ok(ExitStatus::Error);
    }

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

    index.register_observation(light).await?;
    printer.info("Observation registered in profile index")?;

    Ok(ExitStatus::Success)
}
