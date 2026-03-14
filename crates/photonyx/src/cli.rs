use clap::builder::styling::{AnsiColor, Styles};
use clap::{Parser, Subcommand, ValueEnum};

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser, Debug)]
#[command(
    name = "photonyx",
    version,
    about = "Astrophotography CLI application",
    styles = STYLES,
)]
pub struct Cli {
    // /// Path to configuration file
    // #[arg(short, long, default_value = "config.yaml", env = "CONFIG_PATH")]
    // pub config: PathBuf,
    /// Log format
    #[arg(short, long, default_value = "pretty", env = "RUST_LOG_FORMAT")]
    pub log_format: LogFormat,

    /// Increase log verbosity (-v = debug, -vv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Validate the configuration file and exit
    Check,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LogFormat {
    Pretty,
    Json,
}
