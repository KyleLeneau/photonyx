use std::fmt::Write;

use anyhow::Result;
use owo_colors::OwoColorize;
use px_cli::InspectArgs;
use siril_sys::{
    Builder,
    commands::{Dumpheader, Stat},
    siril_ext::*,
};

use crate::{commands::ExitStatus, printer::Printer};

pub(crate) async fn inspect_file(args: InspectArgs, printer: Printer) -> Result<ExitStatus> {
    // writeln!(printer.stdout(), "File: {:?}", args.file)?;

    if !args.file.exists() {
        writeln!(
            printer.stderr(),
            "{}",
            format_args!(
                concat!("{}{} File does not exist to inspect",),
                "error".red().bold(),
                ":".bold()
            )
        )?;
        return Ok(ExitStatus::Error);
    }

    // Startup and wait till process is ready for additional commands
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .build()
        .await?;

    siril.load_path(args.file.clone()).await?;

    let stat_output = siril.execute(&Stat::builder().build()).await;
    for line in &stat_output.unwrap() {
        writeln!(printer.stdout(), "stat: {:?}", line)?;
    }

    let header_output = siril.execute(&Dumpheader::builder().build()).await;
    for line in &header_output.unwrap() {
        writeln!(printer.stdout(), "header: {:?}", line)?;
    }

    // This dumps header and image stats to a JSON file alongside the file passed in...
    // siril.command(&format!("jsonmetadata {:?}", args.file)).await?;

    Ok(ExitStatus::Success)
}
