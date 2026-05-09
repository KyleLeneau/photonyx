mod commands;
mod logging;
mod printer;
mod reporters;
mod resolve;
mod utils;

use anyhow::Result;
use clap::CommandFactory;
use logging::init_logging;
use px_index::ProfileIndex;
use siril_sys::{SIRIL_MIN_VERSION, SirilError, check_siril_version};
use std::io::stdout;

#[cfg(feature = "self-update")]
use px_cli::SelfUpdateArgs;
use px_cli::{
    Cli, Commands, MasterCommand, MasterNamespace, ObservationCommand, ObservationNamespace,
    ProfileCommand, ProfileNamespace, ProjectCommand, ProjectNamespace, SelfCommand, SelfNamespace,
};

pub use crate::commands::ExitStatus;
use crate::printer::Printer;

fn siril_install_instructions() -> String {
    let platform_note = match std::env::consts::OS {
        "macos" => concat!(
            "  macOS:   Download the .dmg from https://siril.org/download/\n",
            "           or install via Homebrew:  brew install --cask siril",
        ),
        "linux" => concat!(
            "  Linux:   Install via your package manager, e.g.:\n",
            "             sudo apt install siril          # Debian/Ubuntu\n",
            "             sudo dnf install siril          # Fedora\n",
            "           or AppImage/Flatpak from https://siril.org/download/",
        ),
        "windows" => concat!(
            "  Windows: Download the installer from https://siril.org/download/\n",
            "           and ensure siril-cli.exe is on your PATH.",
        ),
        _ => "  Visit https://siril.org/download/ for installation instructions.",
    };

    format!(
        "To install or upgrade Siril:\n{platform_note}\n\n\
         After installation, verify with:  siril-cli --version"
    )
}

/// Only public interace out to the bin/px.rs file, not to be used externally
///
pub async fn run(cli: Cli) -> Result<ExitStatus> {
    init_logging(&cli);

    // TODO: Use cli.top_level.global_args to load the global px config file

    // Configure the `Printer`, which controls user-facing output in the CLI.
    let printer = Printer::Default;

    // Open the profile index (finds profile dir, loads config, opens DB, runs migrations).
    let profile_index = ProfileIndex::find_and_open(cli.top_level.global_args.profile).await;

    // Validate and surface the error only for commands that require a profile.
    if cli.command.requires_profile() {
        match profile_index {
            Ok(ref idx) => {
                printer.info(format!("using profile at: {:?}", idx.profile.root))?;
            }
            Err(ref e) => {
                printer.error(format!("{e} - profile is required for this command"))?;
                return Ok(ExitStatus::Error);
            }
        }
    }

    if cli.command.requires_siril() {
        match check_siril_version("siril-cli", SIRIL_MIN_VERSION) {
            Ok(_) => {}
            Err(SirilError::NotInstalled) => {
                printer.error("Siril is not installed or could not be found.")?;
                printer.error(siril_install_instructions())?;
                return Ok(ExitStatus::Error);
            }
            Err(SirilError::VersionTooOld { found, minimum }) => {
                printer.error(format!(
                    "Siril {found} is installed but version {minimum} or newer is required."
                ))?;
                printer.error(siril_install_instructions())?;
                return Ok(ExitStatus::Error);
            }
            Err(e) => {
                printer.error(format!("Could not verify Siril installation: {e}"))?;
                return Ok(ExitStatus::Error);
            }
        }
    }

    match cli.command {
        Commands::GenerateShellCompletion(args) => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(args.shell, &mut cmd, &bin_name, &mut stdout());
            Ok(ExitStatus::Success)
        }
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
        Commands::Tui => commands::terminal_ui(printer).await,
        Commands::Inspect(args) => commands::inspect_file(args, printer).await,

        // Profile
        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::Info(args),
        }) => commands::show_profile(args, printer).await,

        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::Init(args),
        }) => commands::init_profile(args, printer).await,

        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::Scan(args),
        }) => commands::scan_profile(args, printer).await,

        // Masters
        Commands::Master(MasterNamespace {
            command: MasterCommand::Best(args),
        }) => commands::find_best_master(args, printer, profile_index.unwrap()).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::List(args),
        }) => commands::list_masters(args, printer, profile_index.unwrap()).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Bias(args),
        }) => commands::create_master_bias(args, printer, profile_index.unwrap()).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Dark(args),
        }) => commands::create_master_dark(args, printer, profile_index.unwrap()).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Flat(args),
        }) => commands::create_master_flat(args, printer, profile_index.unwrap()).await,

        // Observations
        Commands::Observation(ObservationNamespace {
            command: ObservationCommand::List(args),
        }) => commands::list_observations(args, printer).await,

        Commands::Observation(ObservationNamespace {
            command: ObservationCommand::Calibrate(args),
        }) => commands::calibrate_observation(args, printer, profile_index.unwrap()).await,

        Commands::Observation(ObservationNamespace {
            command: ObservationCommand::Preview(args),
        }) => commands::preview_observation(args, printer).await,

        // Project
        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Init(args),
        }) => commands::init_project(args, printer, profile_index.unwrap()).await,

        // Commands::Project(ProjectNamespace {
        //     command: ProjectCommand::Add(args),
        // }) => commands::add_project_observation(args, printer).await,
        Commands::Project(ProjectNamespace {
            command: ProjectCommand::List(args),
        }) => commands::list_projects(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Stack(args),
        }) => commands::stack_project_observations(args, printer).await,

        // Commands::Project(ProjectNamespace {
        //     command: ProjectCommand::Align(args),
        // }) => commands::align_project(args, printer).await,
        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Sample(args),
        }) => commands::create_project_samples(args, printer).await,
    }
}
