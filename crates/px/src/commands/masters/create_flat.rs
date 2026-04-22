use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateFlatMasterArgs;
use px_conventions::profile::ProfilePath;
use px_fits::{CalibrationMetadata, all_fits_files};
use siril_sys::{
    Builder,
    commands::{Calibrate, Convert, Stack},
    siril_ext::CdExt,
};
use std::path::PathBuf;

pub(crate) async fn create_master_flat(
    args: CreateFlatMasterArgs,
    printer: Printer,
    profile_path: ProfilePath,
) -> Result<ExitStatus> {
    // Guard to make sure the input folder exists first
    if !args.raw_folder.exists() {
        printer.error("Raw flat folder does not exist")?;
        return Ok(ExitStatus::Error);
    }

    // Check that all files in raw folder are fits
    let raw_files = all_fits_files(&args.raw_folder)?;
    if raw_files.is_empty() {
        printer.error("raw_folder contains no files")?;
        return Ok(ExitStatus::Error);
    }

    // Get the output folder from args OR profile convention
    let out_folder = match args.out_folder {
        Some(ref path) if path.exists() => path.clone(),
        Some(_) => {
            printer.error("Output flat folder does not exist")?;
            return Ok(ExitStatus::Error);
        }
        None => profile_path.flat.clone(),
    };

    // TODO: Make bias optional and find the best by default else use specified or error

    // Check if master bias exists
    if !args.bias.exists() {
        printer.error("Missing master bias to calibrate flats with")?;
        return Ok(ExitStatus::Error);
    }

    // Setup the output file
    let name = CalibrationMetadata::from(raw_files.first().unwrap())?.master_flat_name(args.filter);
    let output_file = out_folder.join(name).display().to_string();

    // Setup siril
    let ext = to_fits_ext(args.ext);
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone())
        .build()
        .await?;

    // Move to the raw folder to convert into a sequence
    let sp = printer.spinner("[1/3] Converting flat frames...");
    siril.cd(args.raw_folder).await?;
    siril
        .execute(
            &Convert::builder("flat_")
                .output_dir(siril.initial_directory())
                .build(),
        )
        .await
        .inspect_err(|_| sp.abandon_with_message("✗ Convert failed"))?;

    // Return to working directory
    siril.cd(siril.initial_directory()).await?;
    sp.finish_with_message("[1/3] Converted flat frames");

    // Calibrate the flat frames using the master bias
    let sp = printer.spinner("[2/3] Calibrating flat frames...");
    siril
        .execute(
            &Calibrate::builder("flat_")
                .bias(args.bias.display().to_string())
                .build(),
        )
        .await
        .inspect_err(|_| sp.abandon_with_message("✗ Calibration failed"))?;
    sp.finish_with_message("[2/3] Calibrated flat frames");

    // Stack with defaults
    let sp = printer.spinner("[3/3] Stacking flat frames...");
    siril
        .execute(
            &Stack::builder("pp_flat_")
                .stack_type(siril_sys::StackType::Rej)
                .norm(siril_sys::StackNormFlag::NoNorm)
                .out(&output_file)
                .build(),
        )
        .await
        .inspect_err(|_| sp.abandon_with_message("✗ Stacking failed"))?;
    sp.finish_with_message("[3/3] Stacked flat frames");

    // Confirm the output file exists now
    let result = PathBuf::from(output_file).with_added_extension(ext.to_string());
    if !result.exists() {
        printer.error(format!("Output file is missing: {:?}", result))?;
        return Ok(ExitStatus::Error);
    }

    // Pretty print the result
    printer.success(format!("Master FLAT stacking completed: {:?}", result))?;
    Ok(ExitStatus::Success)
}
