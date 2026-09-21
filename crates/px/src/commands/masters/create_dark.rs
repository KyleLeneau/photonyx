use crate::engine::EngineConfig;
use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateDarkMasterArgs;
use px_index::ProfileIndex;
use px_pipeline::{RunIn, calibration::master_dark::CreateMasterDarkPipeline};

pub(crate) async fn create_master_dark(
    args: CreateDarkMasterArgs,
    printer: Printer,
    index: ProfileIndex,
    engine: EngineConfig,
) -> Result<ExitStatus> {
    // Guard to make sure the input folder exists first
    if !args.raw_folder.exists() {
        printer.error("Raw dark folder does not exist")?;
        return Ok(ExitStatus::Error);
    }

    // Get the output folder from args OR profile convention
    let out_folder = match args.out_folder {
        Some(ref path) if path.exists() => path.clone(),
        Some(_) => {
            printer.error("Output dark folder does not exist")?;
            return Ok(ExitStatus::Error);
        }
        None => index.profile.dark.clone(),
    };

    // The same pipeline, run in whichever environment was selected.
    // Both implement `RunIn`, so only the environment differs here.
    let pipeline = CreateMasterDarkPipeline::builder()
        .ext(to_fits_ext(args.ext))
        .raw_folder(args.raw_folder)
        .out_folder(out_folder)
        .build();

    let master = if engine.is_native() {
        pipeline.run(printer.reporter(), engine.native()).await?
    } else {
        pipeline.run(printer.reporter(), engine.siril()).await?
    };

    // Pretty print the result
    printer.success(format!("Master DARK stacking completed: {:?}", master))?;

    index.register_master(master).await?;

    Ok(ExitStatus::Success)
}
