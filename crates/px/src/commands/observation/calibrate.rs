use anyhow::Result;
use px_cli::CalibrateObservationArgs;
use px_conventions::observation::ObservationPath;
use px_fits::{all_color_raw_frames, all_fits_files};
use px_fs::OptionPath;
use siril_sys::{
    Builder, ConversionFile,
    commands::{Calibrate, Convert},
    siril_ext::CdExt,
};

use crate::{ExitStatus, printer::Printer, utils::to_fits_ext};

pub(crate) async fn calibrate_observation(
    args: CalibrateObservationArgs,
    printer: Printer,
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

    let all_color = all_color_raw_frames(&raw_files)?;
    printer.info(format!("Raw images are OSC: {all_color}"))?;

    let sp = printer.spinner("[1/3] Converting light frames...");

    // Setup siril
    let ext = to_fits_ext(args.ext);
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone())
        .build()
        .await?;

    // Move to the raw folder to convert into a sequence
    siril.cd(&args.raw_folder).await?;
    siril
        .execute(
            &Convert::builder("light_")
                .output_dir(siril.initial_directory())
                .build(),
        )
        .await
        .inspect_err(|_| sp.abandon_with_message("✗ Convert failed"))?;

    // Return to working directory
    siril.cd(&siril.initial_directory()).await?;
    sp.finish_with_message("[1/3] Converted flat frames");

    let sp = printer.spinner("[2/3] Calibrating light frames...");

    // Run calibration
    siril
        .execute(
            &Calibrate::builder("light_")
                .maybe_bias(args.bias.some_string())
                .maybe_dark(args.dark.some_string())
                .maybe_flat(args.flat.some_string())
                .cfa(all_color)
                .debayer(all_color)
                .equalize_cfa(all_color)
                .build(),
        )
        .await
        .inspect_err(|_| sp.abandon_with_message("✗ Calibration failed"))?;
    sp.finish_with_message("[2/3] Calibrated light frames");

    let sp = printer.spinner("[3/3] Moving calibrated light frames...");

    // Load the conversion file and move final files to output
    let conversion_file = siril.initial_directory().join("light_conversion.txt");
    let conversion = ConversionFile::new(conversion_file)?;
    conversion.move_converted_files(&resolved_out_folder, "pp_")?;

    sp.finish_with_message("[3/3] Calibrated light frames moved");
    printer.success(format!(
        "Calibration of light frames completed: {:?}",
        resolved_out_folder
    ))?;
    Ok(ExitStatus::Success)
}
