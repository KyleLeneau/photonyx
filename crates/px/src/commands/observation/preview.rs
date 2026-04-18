use std::{collections::HashSet, path::Path};

use anyhow::Result;
use px_cli::PreviewObservationArgs;
use px_fits::all_fits_files;
use px_nativeui::blink::{self, BlinkAppDelegate};

use crate::{ExitStatus, printer::Printer};

struct NoOpDelegate {
    printer: Printer,
}

impl NoOpDelegate {
    fn new(printer: Printer) -> Self {
        Self { printer }
    }
}

impl BlinkAppDelegate for NoOpDelegate {
    fn load_state(&self, _folder: &Path) -> std::collections::HashSet<String> {
        HashSet::<String>::new()
    }

    fn save_state(&mut self, _folder: &Path, rejected: Vec<String>, total: usize) {
        // Print summary to stdout so it can be piped / logged.
        self.printer
            .info(format!(
                "Blink session complete — {}/{} frames rejected",
                rejected.len(),
                total
            ))
            .ok();

        for name in &rejected {
            self.printer.info(format!("  rejected: {name}")).ok();
        }
    }

    fn frame_rejected_toggle(&mut self, path: String, rejected: bool) {
        self.printer
            .info(format!("frame rejected ({rejected}): {path}"))
            .ok();
    }
}

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

    let delegate = NoOpDelegate::new(printer);

    // eframe must run on the main thread (Cocoa / Wayland requirement).
    // block_in_place lets tokio yield the current thread without blocking the runtime.
    tokio::task::block_in_place(|| {
        blink::launch(paths, folder, args.interval, Box::new(delegate))
    })?;

    Ok(ExitStatus::Success)
}
