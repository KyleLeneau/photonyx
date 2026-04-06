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

pub(crate) fn to_fits_ext(ext: px_cli::FitFileExtension) -> siril_sys::FitsExt {
    match ext {
        px_cli::FitFileExtension::Fit => siril_sys::FitsExt::FIT,
        px_cli::FitFileExtension::Fits => siril_sys::FitsExt::FITS,
        px_cli::FitFileExtension::Fts => siril_sys::FitsExt::FTS,
    }
}
