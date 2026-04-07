use std::fmt::Write;
// use std::path::Path;
use anyhow::Result;

use siril_sys::Builder;
use siril_sys::FitsExt;
use siril_sys::commands::SetExt;
use siril_sys::siril_ext::*;

use crate::commands::ExitStatus;
use crate::printer::Printer;

pub(crate) async fn siril_test(printer: Printer) -> Result<ExitStatus> {
    tracing::info!("siril_test command called");

    // Startup and wait till process is ready for additional commands
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Inherit)
        // .use_directory(Path::new("/Users/kyle/Development/BortleSpace/photonyx"))
        .build()
        .await?;

    let c = SetExt::builder(FitsExt::FITS).build();
    siril.execute(&c).await?;

    let dir = siril.current_directory().await?;
    writeln!(printer.stdout(), "pwd: {:?}", dir)?;

    Ok(ExitStatus::Success)
}
