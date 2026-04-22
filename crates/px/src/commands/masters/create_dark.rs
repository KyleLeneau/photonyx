use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateDarkMasterArgs;
use px_conventions::profile::ProfilePath;
use px_fits::{CalibrationMetadata, all_fits_files};
use siril_sys::{
    Builder,
    commands::{Convert, Stack},
    siril_ext::CdExt,
};
use std::path::PathBuf;

pub(crate) async fn create_master_dark(
    args: CreateDarkMasterArgs,
    printer: Printer,
    profile_path: ProfilePath,
) -> Result<ExitStatus> {
    // Guard to make sure the input folder exists first
    if !args.raw_folder.exists() {
        printer.error("Raw dark folder does not exist")?;
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
            printer.error("Output dark folder does not exist")?;
            return Ok(ExitStatus::Error);
        }
        None => profile_path.dark.clone(),
    };

    // Setup the output file
    let name = CalibrationMetadata::from(raw_files.first().unwrap())?.master_dark_name();
    let output_file = out_folder.join(name).display().to_string();

    // Setup siril
    let ext = to_fits_ext(args.ext);
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone())
        .build()
        .await?;

    // Move to the raw folder to convert into a sequence
    siril.cd(args.raw_folder).await?;
    siril
        .execute(
            &Convert::builder("dark_")
                .output_dir(siril.initial_directory())
                .build(),
        )
        .await?;

    // Return to working directory
    siril.cd(siril.initial_directory()).await?;

    // Stack with defaults
    siril
        .execute(
            &Stack::builder("dark_")
                .stack_type(siril_sys::StackType::Med)
                .out(&output_file)
                .build(),
        )
        .await?;

    // Confirm the output file exists now
    let result = PathBuf::from(output_file).with_added_extension(ext.to_string());
    if !result.exists() {
        printer.error(format!("Output file is missing: {:?}", result))?;
        return Ok(ExitStatus::Error);
    }

    // Pretty print the result
    printer.success(format!("Master DARK stacking completed: {:?}", result))?;
    Ok(ExitStatus::Success)
}
