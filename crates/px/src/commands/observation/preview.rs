use anyhow::Result;
use px_cli::PreviewObservationArgs;
use px_fits::all_fits_files;
use px_nativeui::blink;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn preview_observation(
    args: PreviewObservationArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    let folder = args.folder.canonicalize().unwrap_or(args.folder.clone());

    if !folder.is_dir() {
        printer.error(format!("Not a directory: {}", folder.display()))?;
        return Ok(ExitStatus::Failure);
    }

    let mut paths = all_fits_files(&folder)?;
    if paths.is_empty() {
        printer.error(format!("No FITS files found in {}", folder.display()))?;
        return Ok(ExitStatus::Failure);
    }

    // Sort for a consistent blink order.
    paths.sort();

    printer.info(format!(
        "Opening {} frames from {} …",
        paths.len(),
        folder.display()
    ))?;

    // eframe must run on the main thread (Cocoa / Wayland requirement).
    // block_in_place lets tokio yield the current thread without blocking the runtime.
    tokio::task::block_in_place(|| blink::launch(paths, folder, args.interval))?;

    Ok(ExitStatus::Success)
}
