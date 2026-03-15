mod cli;
mod commands;

use clap::Parser;

use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands, LogFormat};
use crate::commands::{siril_test, stat};

fn init_logging(cli: &Cli) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match cli.verbose {
            0 => "info,siril_sys=debug",
            1 => "debug",
            _ => "trace",
        };
        EnvFilter::new(level)
    });

    let json_log = matches!(cli.log_format, LogFormat::Json);

    let json_layer = json_log.then(|| {
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_current_span(true)
            .with_span_list(true)
    });

    let pretty_layer = (!json_log).then(|| {
        tracing_subscriber::fmt::layer()
            .pretty()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(json_layer)
        .with(pretty_layer)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    init_logging(&cli);

    tracing::info!("Hello from photonyx!");

    let file = "'/Users/kyle/Pictures/Astro/FRA300_6200mc_pro/LIGHT/NGC 2244/2026-02-02/RAW_O_300.00s/2026-02-02_22-49-25_O_-9.90c_100g_30o_300.00s_d_1x1_0720.fits'";

    match cli.command {
        Commands::Check => stat(file).await?,
        Commands::SirilTest => siril_test().await?,
    }

    Ok(())
}
