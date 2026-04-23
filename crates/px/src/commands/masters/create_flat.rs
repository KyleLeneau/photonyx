use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateFlatMasterArgs;
use px_conventions::profile::ProfilePath;
use px_pipeline::master_flat::CreateMasterFlatPipeline;
use siril_sys::Builder;

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

    let master = CreateMasterFlatPipeline::builder()
        .ext(to_fits_ext(args.ext))
        .siril_builder(Builder::default().output_sink(siril_sys::OutputSink::Discard))
        .raw_folder(args.raw_folder)
        .out_folder(out_folder)
        .bias(args.bias)
        .filter(args.filter)
        .build()
        .execute(DefaultPipelineReporter::from(printer))
        .await?;

    // Pretty print the result
    printer.success(format!("Master FLAT stacking completed: {:?}", master))?;

    // TODO: Add this new master to the px_profile.yaml config for later uses

    Ok(ExitStatus::Success)
}
