mod commands;
mod logging;
mod printer;

use anyhow::Result;
use logging::init_logging;
use px_cli::{Cli, Commands, SelfCommand, SelfNamespace, SelfUpdateArgs};

pub use crate::commands::ExitStatus;
use crate::printer::Printer;

/// Only public interace out to the bin/px.rs file, not to be used externally
///
pub async fn run(cli: Cli) -> Result<ExitStatus> {
    init_logging(&cli);

    // Configure the `Printer`, which controls user-facing output in the CLI.
    let printer = Printer::Default;

    tracing::info!("Hello from px crate (new)!");

    let file = "'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/NGC 2244/2026-02-02/RAW_O_300.00s/2026-02-02_22-49-25_O_-9.90c_100g_30o_300.00s_d_1x1_0720.fits'";

    match cli.command {
        Commands::Check => commands::stat(file, printer).await,

        Commands::SirilTest => commands::siril_test().await,

        Commands::Self_(SelfNamespace {
            command:
                SelfCommand::Version {
                    short,
                    output_format,
                },
        }) => {
            commands::self_version(short, output_format, printer)
        }

        Commands::Self_(SelfNamespace {
            command:
                SelfCommand::Update(SelfUpdateArgs {
                    target_version: _,
                    token: _,
                    dry_run: _,
                }),
        }) => todo!(),

        Commands::Profile(_profile_namespace) => todo!(),
    }
}
