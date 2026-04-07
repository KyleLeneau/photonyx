use std::fmt::Write;
use tokio::io::{self, AsyncBufReadExt};

use crate::printer::Printer;

#[allow(dead_code)]
pub(crate) async fn wait_for_confirm(printer: Printer) {
    let _ = writeln!(printer.stdout(), "Press ENTER to continue...");
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin);
    let mut line = String::new();
    reader.read_line(&mut line).await.ok();
}

/// Map the cli FitExtension to siril_sys type, default to FIT if none
///
pub(crate) fn to_fits_ext(ext: Option<px_cli::FitFileExtension>) -> siril_sys::FitsExt {
    match ext {
        Some(px_cli::FitFileExtension::Fit) => siril_sys::FitsExt::FIT,
        Some(px_cli::FitFileExtension::Fits) => siril_sys::FitsExt::FITS,
        Some(px_cli::FitFileExtension::Fts) => siril_sys::FitsExt::FTS,
        None => siril_sys::FitsExt::FIT,
    }
}
