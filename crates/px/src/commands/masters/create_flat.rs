use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter, utils::to_fits_ext};
use anyhow::Result;
use px_cli::CreateFlatMasterArgs;
use px_fits::{CalibrationMetadata, all_fits_files};
use px_index::{MatchCriteria, ProfileIndex};
use px_pipeline::master_flat::CreateMasterFlatPipeline;
use siril_sys::Builder;

pub(crate) async fn create_master_flat(
    args: CreateFlatMasterArgs,
    printer: Printer,
    index: ProfileIndex,
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
        None => index.profile.flat.clone(),
    };

    // Read one raw flat to extract metadata for bias matching
    let raw_files = all_fits_files(&args.raw_folder)?;
    if raw_files.is_empty() {
        printer.error("Raw flat folder contains no files")?;
        return Ok(ExitStatus::Error);
    }
    let meta = CalibrationMetadata::from(raw_files.first().unwrap())?;

    // Resolve the master bias: prefer the explicit arg, otherwise find the best from the index
    let bias = match args.bias {
        Some(ref path) if path.exists() => path.clone(),
        Some(_) => {
            printer.error("Specified master bias does not exist")?;
            return Ok(ExitStatus::Error);
        }
        None => {
            // TODO: revisit if this criteria should come from someplace else
            let criteria = MatchCriteria {
                temperature: meta.temperature,
                gain: meta.gain,
                offset: meta.offset,
                binning: Some(meta.binning.to_string()),
                date: meta.capture_date().map(|dt| dt.date()),
                date_tolerance_days: Some(2),
                ..Default::default()
            };
            match index.find_best_bias(&criteria).await? {
                Some(record) => record.master_path.into(),
                None => {
                    printer.error("No matching master bias found in index")?;
                    return Ok(ExitStatus::Error);
                }
            }
        }
    };

    printer.info(format!("using master bias: {:?}", bias))?;

    let master = CreateMasterFlatPipeline::builder()
        .ext(to_fits_ext(args.ext))
        .siril_builder(Builder::default().output_sink(siril_sys::OutputSink::Discard))
        .raw_folder(args.raw_folder)
        .out_folder(out_folder)
        .bias(bias)
        .filter(args.filter)
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    // Pretty print the result
    printer.success(format!("Master FLAT stacking completed: {:?}", master))?;

    index.register_master(master).await?;

    Ok(ExitStatus::Success)
}
