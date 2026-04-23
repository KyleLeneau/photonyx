use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateBiasMasterArgs;
use px_conventions::profile::ProfilePath;
use px_pipeline::master_bias::CreateMasterBiasPipeline;
use siril_sys::Builder;

pub(crate) async fn create_master_bias(
    args: CreateBiasMasterArgs,
    printer: Printer,
    profile_path: ProfilePath,
) -> Result<ExitStatus> {
    // Guard to make sure the input folder exists first
    if !args.raw_folder.exists() {
        printer.error("Raw bias folder does not exist")?;
        return Ok(ExitStatus::Error);
    }

    // Get the output folder from args OR profile convention
    let out_folder = match args.out_folder {
        Some(ref path) if path.exists() => path.clone(),
        Some(_) => {
            printer.error("Output bias folder does not exist")?;
            return Ok(ExitStatus::Error);
        }
        None => profile_path.bias.clone(),
    };

    let master = CreateMasterBiasPipeline::builder()
        .ext(to_fits_ext(args.ext))
        .siril_builder(Builder::default().output_sink(siril_sys::OutputSink::Discard))
        .raw_folder(args.raw_folder)
        .out_folder(out_folder)
        .build()
        .execute()
        .await?;

    // Pretty print the result
    printer.success(format!("Master BIAS stacking completed: {:?}", master))?;

    // TODO: Add this new master to the px_profile.yaml config for later uses

    Ok(ExitStatus::Success)
}
