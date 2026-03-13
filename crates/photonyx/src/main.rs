mod cli;

use clap::Parser;
use siril_sys::Siril;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::cli::{Cli, LogFormat};

fn init_logging(cli: &Cli) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = match cli.verbose {
            0 => "info,floor_cam_rs=debug",
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

    // Startup and wait till process is ready for additional commands
    let mut siril = Siril::new().await?;
    siril.command("requires 0.99.10").await?;
    siril.command("set core.mem_ratio=0.9").await?;
    siril.command(&format!("load {}", file)).await?;

    let stat_output = siril.command("stat").await;
    for line in &stat_output.unwrap() {
        tracing::info!("stat: {:?}", line);
    }

    siril.close().await?;
    // Siril also cleans up when dropped

    // match cli.command {
    //     Commands::Serve => serve(&cli).await,
    //     Commands::Check => check(&cli),
    // }

    Ok(())
}
