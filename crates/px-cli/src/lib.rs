pub mod version;

use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Args, Parser, Subcommand};
use clap::{ValueEnum, ValueHint};

use px_static::EnvVars;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum VersionFormat {
    /// Display the version as plain text.
    Text,
    /// Display the version as JSON.
    Json,
}

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().bold())
    .usage(AnsiColor::Green.on_default().bold())
    .literal(AnsiColor::Cyan.on_default().bold())
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Parser)]
#[command(
    name = "px",
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

    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate shell completion
    #[command(alias = "--generate-shell-completion", hide = true)]
    GenerateShellCompletion(GenerateShellCompletionArgs),

    /// Test if siril is installed and working
    SirilTest,

    /// Manage the px executable.
    #[command(name = "self")]
    Self_(SelfNamespace),

    /// Manage hardware profiles
    #[command()]
    Profile(ProfileNamespace),

    /// Launch a terminal UI poc
    Tui,

    /// Inspect a single image
    #[command()]
    Inspect(InspectArgs),
}

#[derive(ValueEnum, Clone, Debug)]
pub enum LogFormat {
    Pretty,
    Json,
}

#[derive(Args)]
pub struct ProfileNamespace {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    Init,
    List,
    Scan,
}

#[derive(Args)]
pub struct SelfNamespace {
    #[command(subcommand)]
    pub command: SelfCommand,
}

#[derive(Subcommand)]
pub enum SelfCommand {
    /// Update px.
    Update(SelfUpdateArgs),

    /// Display px's version
    Version {
        /// Only print the version
        #[arg(long)]
        short: bool,
        #[arg(long, value_enum, default_value = "text")]
        output_format: VersionFormat,
    },
}

#[derive(Args, Debug)]
pub struct SelfUpdateArgs {
    /// Update to the specified version. If not provided, px will update to the latest version.
    #[arg(value_hint = ValueHint::Other)]
    pub target_version: Option<String>,

    /// A GitHub token for authentication.
    /// A token is not required but can be used to reduce the chance of encountering rate limits.
    #[arg(long, env = EnvVars::PX_GITHUB_TOKEN, value_hint = ValueHint::Other)]
    pub token: Option<String>,

    /// Run without performing the update.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct GenerateShellCompletionArgs {
    pub shell: clap_complete::Shell,
}

#[derive(Args)]
pub struct InspectArgs {
    /// Fits file to inspect
    #[arg(value_hint = ValueHint::FilePath)]
    pub file: PathBuf,
}
