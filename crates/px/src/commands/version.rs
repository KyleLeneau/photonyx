use std::fmt::Write;

use anyhow::Result;
use owo_colors::OwoColorize;

use px_cli::VersionFormat;

use crate::printer::Printer;
use crate::commands::ExitStatus;

/// Display version information for uv itself (`px self version`)
pub(crate) fn self_version(
    short: bool,
    output_format: VersionFormat,
    printer: Printer,
) -> Result<ExitStatus> {
    let version_info = px_cli::version::px_self_version();
    match output_format {
        VersionFormat::Text => {
            if short {
                writeln!(printer.stdout(), "{}", version_info.cyan())?;
            } else {
                writeln!(printer.stdout(), "px {}", version_info.cyan())?;
            }
        }
        VersionFormat::Json => {
            let string = serde_json::to_string_pretty(&version_info)?;
            writeln!(printer.stdout(), "{string}")?;
        }
    }

    Ok(ExitStatus::Success)
}
