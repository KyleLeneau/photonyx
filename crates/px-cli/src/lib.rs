pub mod version;

use core::str;
use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Args, Parser, Subcommand};
use clap::{ValueEnum, ValueHint};

use px_static::EnvVars;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CalibrationImageType {
    Bias,
    Flat,
    Dark
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FitFileExtension {
    Fit,
    Fits,
    Fts
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum VersionFormat {
    /// Display the version as plain text.
    Text,
    /// Display the version as JSON.
    Json,
}

#[derive(clap::ValueEnum, Clone, Debug, Copy)]
pub enum OutputFormat {
    /// Display output in default format (stdout or terminal UI)
    Pretty,

    /// Display output in json format to stdout
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

    /// Manage and create master calibration frames
    #[command()]
    Master(MasterNamespace),
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

    /// Output format to display
    #[arg(short, long, default_value = "pretty")]
    pub output: OutputFormat,
}

#[derive(Args)]
pub struct MasterNamespace {
    #[command(subcommand)]
    pub command: MasterCommand,
}

#[derive(Subcommand)]
pub enum MasterCommand {
    /// find best master <type> based on query
    Best(FindBestMasterArgs),

    /// show all the master for a profile
    List(ListMasterArgs),

    /// create a new master bias for profile
    Bias(CreateBiasMasterArgs),

    /// create a new master dark for profile
    Dark(CreateDarkMasterArgs),

    /// create a new master flat for profile
    Flat(CreateFlatMasterArgs),
}

#[derive(Args)]
pub struct FindBestMasterArgs {
    /// Image type to look for
    #[arg(short, long)]
    pub image_type: CalibrationImageType,
}

#[derive(Args)]
pub struct ListMasterArgs {
    /// Image types to display (default: none, all)
    #[arg(short, long)]
    pub image_type: Vec<CalibrationImageType>,
}

#[derive(Args)]
pub struct CreateBiasMasterArgs {
    /// Path to the raw folder of bias frames
    #[arg(value_hint = ValueHint::DirPath)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: FitFileExtension,

    /// Output location for the new master bias
    #[arg(value_hint = ValueHint::DirPath)]
    pub out_folder: PathBuf,
}

#[derive(Args)]
pub struct CreateDarkMasterArgs {
    /// Path to the raw folder of dark frames
    #[arg(value_hint = ValueHint::DirPath)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: FitFileExtension,

    /// Output location for the new master dark
    #[arg(value_hint = ValueHint::DirPath)]
    pub out_folder: PathBuf,
}

#[derive(Args)]
pub struct CreateFlatMasterArgs {
    /// Path to the raw folder of flat frames
    #[arg(value_hint = ValueHint::DirPath)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: FitFileExtension,

    /// Output location for the new master flat
    #[arg(value_hint = ValueHint::DirPath)]
    pub out_folder: PathBuf,

    /// Location of the master BIAS
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    pub bias: PathBuf,

    /// The name of the filter for the master flat
    #[arg(short, long, value_hint = ValueHint::Other)]
    pub filter: String
}
