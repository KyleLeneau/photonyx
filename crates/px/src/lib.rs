mod commands;
mod logging;
mod printer;

use anyhow::Result;
use clap::CommandFactory;
use logging::init_logging;
use std::io::stdout;

#[cfg(feature = "self-update")]
use px_cli::SelfUpdateArgs;
use px_cli::{Cli, Commands, SelfCommand, SelfNamespace};

pub use crate::commands::ExitStatus;
use crate::printer::Printer;

/// Only public interace out to the bin/px.rs file, not to be used externally
///
pub async fn run(cli: Cli) -> Result<ExitStatus> {
    init_logging(&cli);

    // Configure the `Printer`, which controls user-facing output in the CLI.
    let printer = Printer::Default;

    let file = "'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/NGC 2244/2026-02-02/RAW_O_300.00s/2026-02-02_22-49-25_O_-9.90c_100g_30o_300.00s_d_1x1_0720.fits'";

    match cli.command {
        Commands::GenerateShellCompletion(args) => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(args.shell, &mut cmd, &bin_name, &mut stdout());
            Ok(ExitStatus::Success)
        }

        Commands::Check => commands::stat(file, printer).await,

        Commands::SirilTest => commands::siril_test(printer).await,

        Commands::Self_(SelfNamespace {
            command:
                SelfCommand::Version {
                    short,
                    output_format,
                },
        }) => commands::self_version(short, output_format, printer),

        #[cfg(feature = "self-update")]
        Commands::Self_(SelfNamespace {
            command:
                SelfCommand::Update(SelfUpdateArgs {
                    target_version,
                    token,
                    dry_run,
                }),
        }) => commands::self_update(target_version, token, dry_run, printer).await,
        #[cfg(not(feature = "self-update"))]
        Commands::Self_(_) => {
            const BASE_MESSAGE: &str =
                "px was not installed with the installer and cannot update itself.";
            anyhow::bail!(BASE_MESSAGE);
        }

        Commands::Profile(_profile_namespace) => todo!(),
    }
}
