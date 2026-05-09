pub mod version;

use core::str;
use std::path::PathBuf;

use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::{Args, Parser, Subcommand};
use clap::{ValueEnum, ValueHint};

use px_static::EnvVars;

fn absolute_path(s: &str) -> Result<PathBuf, String> {
    std::path::absolute(s).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CalibrationImageType {
    Bias,
    Flat,
    Dark,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FitFileExtension {
    Fit,
    Fits,
    Fts,
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
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub top_level: TopLevelArgs,
}

#[derive(Parser)]
#[command(disable_help_flag = true, disable_version_flag = true)]
pub struct TopLevelArgs {
    #[command(flatten)]
    pub global_args: GlobalArgs,

    // /// Path to configuration file
    // #[arg(short, long, default_value = "config.yaml", env = "CONFIG_PATH")]
    // pub config: PathBuf,
    /// Display the concise help for this command.
    #[arg(global = true, short, long, action = clap::ArgAction::HelpShort, help_heading = "Global options")]
    help: Option<bool>,

    /// Display the uv version.
    #[arg(short = 'V', long, action = clap::ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Parser, Debug, Clone)]
#[command(next_help_heading = "Global options", next_display_order = 1000)]
pub struct GlobalArgs {
    /// Log format
    #[arg(short, long, default_value = "pretty", env = "RUST_LOG_FORMAT")]
    pub log_format: LogFormat,

    /// Increase log verbosity (-v = info, -vv = debug, -vvv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Discover a profile in the given directory.
    ///
    /// A `px_profile.yaml` file will be discovered by walking up the directory tree.
    ///
    /// Other command-line arguments (such as relative paths) will be resolved relative
    /// to the current working directory.
    ///
    #[arg(global = true, long, env = EnvVars::PX_PROFILE, value_hint = ValueHint::DirPath)]
    pub profile: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate shell completion
    #[command(alias = "--generate-shell-completion", hide = true)]
    GenerateShellCompletion(GenerateShellCompletionArgs),

    /// Manage the px executable.
    #[command(name = "self")]
    Self_(SelfNamespace),

    /// Test if siril is installed and working
    #[command(hide = true)]
    SirilTest,

    /// Launch a terminal UI poc
    #[command(hide = true)]
    Tui,

    /// Manage hardware profiles
    #[command()]
    Profile(ProfileNamespace),

    /// Inspect a single image
    #[command()]
    Inspect(InspectArgs),

    /// Manage and create master calibration frames
    #[command()]
    Master(MasterNamespace),

    /// Manage and calibrate observation (light) frames
    #[command(alias = "obs")]
    Observation(ObservationNamespace),

    /// Manage and create projects from observations
    #[command()]
    Project(ProjectNamespace),
}

impl Commands {
    /// Only commands that actually invoke Siril return true.
    pub fn requires_siril(&self) -> bool {
        matches!(
            self,
            Commands::SirilTest
                | Commands::Master(MasterNamespace {
                    command: MasterCommand::Bias(_)
                        | MasterCommand::Dark(_)
                        | MasterCommand::Flat(_),
                })
                | Commands::Observation(ObservationNamespace {
                    command: ObservationCommand::Calibrate(_),
                })
                | Commands::Project(ProjectNamespace {
                    command: ProjectCommand::Stack(_) | ProjectCommand::Sample(_),
                })
        )
    }

    /// Everything requires by default and `!` to only specify the ones that don't
    pub fn requires_profile(&self) -> bool {
        !matches!(
            self,
            Commands::GenerateShellCompletion(_)
                | Commands::Self_(_)
                | Commands::SirilTest
                | Commands::Tui
                | Commands::Inspect(_)
                | Commands::Profile(ProfileNamespace {
                    command: ProfileCommand::Init(_),
                })
                | Commands::Observation(ObservationNamespace {
                    command: ObservationCommand::List(_) | ObservationCommand::Preview(_),
                })
                | Commands::Project(ProjectNamespace {
                    command: ProjectCommand::List(_),
                })
        )
    }
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
    /// show details on a profile (current or specified)
    Info(ShowProfileArgs),

    /// make a new profile (dir, layout, config, etc)
    Init(InitProfileArgs),

    /// scan for changes in current or specified profile
    Scan(ScanProfileArgs),
}

#[derive(Args, Debug)]
pub struct ShowProfileArgs {}

#[derive(Args, Debug)]
pub struct InitProfileArgs {
    /// The path to use for the profile (created if it does not exist)
    #[arg(value_hint = ValueHint::DirPath)]
    pub path: PathBuf,
}

#[derive(Args, Debug)]
pub struct ScanProfileArgs {}

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
    #[arg(value_hint = ValueHint::FilePath, value_parser = absolute_path)]
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

    /// Output format to display
    #[arg(short, long, default_value = "pretty")]
    pub output: OutputFormat,
}

#[derive(Args)]
pub struct CreateBiasMasterArgs {
    /// Path to the raw folder of bias frames
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: Option<FitFileExtension>,

    /// Output location for the new master bias
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub out_folder: Option<PathBuf>,
}

#[derive(Args)]
pub struct CreateDarkMasterArgs {
    /// Path to the raw folder of dark frames
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: Option<FitFileExtension>,

    /// Output location for the new master dark
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub out_folder: Option<PathBuf>,
}

#[derive(Args)]
pub struct CreateFlatMasterArgs {
    /// Path to the raw folder of flat frames
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub raw_folder: PathBuf,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: Option<FitFileExtension>,

    /// Output location for the new master flat
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub out_folder: Option<PathBuf>,

    /// Location of the master BIAS (auto-selected from index when omitted)
    #[arg(short, long, value_hint = ValueHint::FilePath, value_parser = absolute_path)]
    pub bias: Option<PathBuf>,

    /// The name of the filter for the master flat
    #[arg(short, long, value_hint = ValueHint::Other)]
    pub filter: String,
}

#[derive(Args)]
pub struct ObservationNamespace {
    #[command(subcommand)]
    pub command: ObservationCommand,
}

#[derive(Subcommand)]
pub enum ObservationCommand {
    /// show all the light frame observations for a profile
    List(ListObservationArgs),

    /// calibration a single raw observation
    #[command(alias = "process")]
    Calibrate(CalibrateObservationArgs),

    /// Preview the observation data to cull frames
    Preview(PreviewObservationArgs),
}

#[derive(Args)]
pub struct ListObservationArgs {}

#[derive(Args, Debug)]
pub struct CalibrateObservationArgs {
    /// Path to the raw folder of light frames
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub raw_folder: PathBuf,

    /// Clean or remove the match calibrated folder from a previous run
    #[arg(long)]
    pub clean: bool,

    /// Output file extension
    #[arg(short, long, default_value = "fit", env = EnvVars::PX_DEFAULT_FIT_EXT)]
    pub ext: Option<FitFileExtension>,

    /// Output location for the calibrated folder (default: peer to input)
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub out_folder: Option<PathBuf>,

    /// Specify a filter for the observation if unable to find
    #[arg(short, long)]
    pub filter: Option<String>,

    // TODO: find best matching masters flag and override the input
    /// Location of the master BIAS
    #[arg(long, value_hint = ValueHint::FilePath, value_parser = absolute_path)]
    pub bias: Option<PathBuf>,

    /// Location of the master DARK
    #[arg(long, value_hint = ValueHint::FilePath, value_parser = absolute_path)]
    pub dark: Option<PathBuf>,

    /// Location of the master FLAT
    #[arg(long, value_hint = ValueHint::FilePath, value_parser = absolute_path)]
    pub flat: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct PreviewObservationArgs {
    /// Path to the folder of light frames to preview
    #[arg(value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub folder: PathBuf,

    /// Autoplay interval in seconds between frames
    #[arg(long, default_value = "0.5")]
    pub interval: f64,
}

#[derive(Args, Debug)]
pub struct ProjectNamespace {
    #[command(subcommand)]
    pub command: ProjectCommand,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// new project setup
    Init(InitProjectArgs),

    /// list all the projects for the profile
    List(ListProjectArgs),

    /// Create linear stacks of the observation + profile + filter combos
    Stack(StackProjectArgs),

    /// Preview color samples of the project
    Sample(SampleProjectArgs),
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FramingType {
    /// Single target; all linear stacks register to minimum framing
    Single,
    /// Spiral mosaic (e.g. Seestar); uses maximum framing
    SpiralMosiac,
    /// Grid (X×Y panel) mosaic; uses maximum framing
    GridMosiac,
}

#[derive(Args, Debug)]
pub struct InitProjectArgs {
    /// Path for the project directory (default: <profile_root>/PROJECTS/<name_snake>)
    #[arg(long, value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub path: Option<PathBuf>,

    /// The name of the project (prompted if omitted; path directory name used in non-interactive mode)
    #[arg(long)]
    pub name: Option<String>,

    /// Short description of the project
    #[arg(long)]
    pub description: Option<String>,

    /// Framing type for the project
    #[arg(long, value_enum)]
    pub framing: Option<FramingType>,

    /// Name for the first stack (master_light name for single; mosaic name for spiral)
    #[arg(long)]
    pub stack_name: Option<String>,

    /// Filter label applied to the stack (e.g. Ha, LRGB, OSC)
    #[arg(long)]
    pub filter: Option<String>,

    /// Edge feather in pixels for spiral-mosiac framing
    #[arg(long)]
    pub feather_pixels: Option<f32>,

    /// Skip all interactive prompts; uses flag values and defaults (fails only if name cannot be derived)
    #[arg(long, short = 'y')]
    pub no_interactive: bool,
}

#[derive(Args, Debug)]
pub struct ListProjectArgs {}

#[derive(Args, Debug)]
pub struct StackProjectArgs {
    /// The path to the project; defaults to searching the current directory and its parents
    #[arg(short, long, value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub project: Option<PathBuf>,

    /// Clean or remove the outputs from a previous run
    #[arg(long)]
    pub clean: bool,
}

#[derive(Args, Debug)]
pub struct SampleProjectArgs {
    /// The path to the project; defaults to searching the current directory and its parents
    #[arg(short, long, value_hint = ValueHint::DirPath, value_parser = absolute_path)]
    pub project: Option<PathBuf>,
}
