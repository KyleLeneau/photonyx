use std::fmt::Write;

use anyhow::Result;
use axoupdater::{AxoUpdater, AxoupdateError, UpdateRequest};
use owo_colors::OwoColorize;
use tracing::debug;

use crate::commands::ExitStatus;
use crate::printer::Printer;

/// Attempt to update the px binary.
pub(crate) async fn self_update(
    version: Option<String>,
    token: Option<String>,
    dry_run: bool,
    printer: Printer,
) -> Result<ExitStatus> {
    let mut updater = AxoUpdater::new_for("px");
    updater.disable_installer_output();

    if let Some(ref token) = token {
        updater.set_github_token(token);
    }

    // Load the "install receipt" for the current binary. If the receipt is not found, then px was installed via source.
    let Ok(updater) = updater.load_receipt() else {
        debug!("No receipt found; assuming px was installed via a package manager");
        writeln!(
            printer.stderr(),
            "{}",
            format_args!(
                concat!(
                    "{}{} Self-update is only available for px binaries installed via the standalone installation scripts.",
                ),
                "error".red().bold(),
                ":".bold()
            )
        )?;
        return Ok(ExitStatus::Error);
    };

    // If we know what our version is, ignore whatever the receipt thinks it is!
    // This makes us behave better if someone manually installs a random version of px
    // in a way that doesn't update the receipt.
    if let Ok(version) = env!("CARGO_PKG_VERSION").parse() {
        // This is best-effort, it's fine if it fails (also it can't actually fail)
        let _ = updater.set_current_version(version);
    }

    // Ensure the receipt is for the current binary. If it's not, then the user likely has multiple
    // px binaries installed, and the current binary was _not_ installed via the standalone
    // installation scripts.
    if !updater.check_receipt_is_for_this_executable()? {
        let current_exe = std::env::current_exe()?;
        let receipt_prefix = updater.install_prefix_root()?;

        writeln!(
            printer.stderr(),
            "{}",
            format_args!(
                concat!(
                    "{}{} Self-update is only available for px binaries installed via the standalone installation scripts.",
                    "\n",
                    "\n",
                    "The current executable is at `{}` but the standalone installer was used to install px to `{}`. Are multiple copies of px installed?"
                ),
                "error".red().bold(),
                ":".bold(),
                current_exe.simplified_display().bold().cyan(),
                receipt_prefix.simplified_display().bold().cyan()
            )
        )?;
        return Ok(ExitStatus::Error);
    }

    writeln!(
        printer.stderr(),
        "{}",
        format_args!(
            "{}{} Checking for updates...",
            "info".cyan().bold(),
            ":".bold()
        )
    )?;

    let update_request = if let Some(version) = version {
        UpdateRequest::SpecificTag(version)
    } else {
        UpdateRequest::Latest
    };

    updater.configure_version_specifier(update_request.clone());

    if dry_run {
        // TODO(charlie): `updater.fetch_release` isn't public, so we can't say what the latest
        // version is.
        if updater.is_update_needed().await? {
            let version = match update_request {
                UpdateRequest::Latest | UpdateRequest::LatestMaybePrerelease => {
                    "the latest version".to_string()
                }
                UpdateRequest::SpecificTag(version) | UpdateRequest::SpecificVersion(version) => {
                    format!("v{version}")
                }
            };
            writeln!(
                printer.stderr(),
                "Would update px from {} to {}",
                format!("v{}", env!("CARGO_PKG_VERSION")).bold().white(),
                version.bold().white(),
            )?;
        } else {
            writeln!(
                printer.stderr(),
                "{}",
                format_args!(
                    "You're on the latest version of px ({})",
                    format!("v{}", env!("CARGO_PKG_VERSION")).bold().white()
                )
            )?;
        }
        return Ok(ExitStatus::Success);
    }

    // Run the updater. This involves a network request, since we need to determine the latest
    // available version of px.
    match updater.run().await {
        Ok(Some(result)) => {
            let direction = if result
                .old_version
                .as_ref()
                .is_some_and(|old_version| *old_version > result.new_version)
            {
                "Downgraded"
            } else {
                "Upgraded"
            };

            let version_information = if let Some(old_version) = result.old_version {
                format!(
                    "from {} to {}",
                    format!("v{old_version}").bold().cyan(),
                    format!("v{}", result.new_version).bold().cyan(),
                )
            } else {
                format!("to {}", format!("v{}", result.new_version).bold().cyan())
            };

            writeln!(
                printer.stderr(),
                "{}",
                format_args!(
                    "{}{} {direction} px {}! {}",
                    "success".green().bold(),
                    ":".bold(),
                    version_information,
                    format!(
                        "https://github.com/KyleLeneau/photonyx/releases/tag/{}"
                        result.new_version_tag
                    )
                    .cyan()
                )
            )?;
        }
        Ok(None) => {
            writeln!(
                printer.stderr(),
                "{}",
                format_args!(
                    "{}{} You're on the latest version of px ({})",
                    "success".green().bold(),
                    ":".bold(),
                    format!("v{}", env!("CARGO_PKG_VERSION")).bold().cyan()
                )
            )?;
        }
        Err(err) => {
            return if let AxoupdateError::Reqwest(err) = err {
                if err.status() == Some(http::StatusCode::FORBIDDEN) && token.is_none() {
                    writeln!(
                        printer.stderr(),
                        "{}",
                        format_args!(
                            "{}{} GitHub API rate limit exceeded. Please provide a GitHub token via the {} option.",
                            "error".red().bold(),
                            ":".bold(),
                            "`--token`".green().bold()
                        )
                    )?;
                    Ok(ExitStatus::Error)
                } else {
                    Err(WrappedReqwestError::from(err).into())
                }
            } else {
                Err(err.into())
            };
        }
    }

    Ok(ExitStatus::Success)
}
