use crate::engine::EngineConfig;
use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateBiasMasterArgs;
use px_index::ProfileIndex;
use px_pipeline::{RunIn, calibration::master_bias::CreateMasterBiasPipeline};
use std::time::Instant;

pub(crate) async fn create_master_bias(
    args: CreateBiasMasterArgs,
    printer: Printer,
    index: ProfileIndex,
    engine: EngineConfig,
) -> Result<ExitStatus> {
    let start = Instant::now();

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
        None => index.profile.bias.clone(),
    };

    // The same pipeline, run in whichever environment was selected.
    // Both implement `RunIn`, so only the environment differs here.
    let pipeline = CreateMasterBiasPipeline::builder()
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
    printer.success(format!("Master BIAS stacking completed: {:?}", master))?;

    index.register_master(master).await?;

    printer.success(format!(
        "Elapsed time: {:.4}s",
        start.elapsed().as_secs_f64()
    ))?;

    Ok(ExitStatus::Success)
}
