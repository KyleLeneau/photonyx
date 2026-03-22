mod commands;
mod logging;

use px_cli::{Cli, Commands};
use logging::init_logging;

/// Only public interace out to the bin/px.rs file, not to be used externally
///
pub async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    init_logging(&cli);

    tracing::info!("Hello from px crate (new)!");

    let file = "'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/NGC 2244/2026-02-02/RAW_O_300.00s/2026-02-02_22-49-25_O_-9.90c_100g_30o_300.00s_d_1x1_0720.fits'";

    match cli.command {
        Commands::Check => commands::stat(file).await?,
        Commands::SirilTest => commands::siril_test().await?,
    }

    Ok(())
}

