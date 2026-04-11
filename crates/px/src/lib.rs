mod commands;
mod logging;
mod printer;
mod resolve;
mod utils;

use anyhow::Result;
use clap::CommandFactory;
use logging::init_logging;
use std::io::stdout;

#[cfg(feature = "self-update")]
use px_cli::SelfUpdateArgs;
use px_cli::{
    Cli, Commands, MasterCommand, MasterNamespace, ObservationCommand, ObservationNamespace,
    ProfileCommand, ProfileNamespace, ProjectCommand, ProjectNamespace, SelfCommand, SelfNamespace,
};

pub use crate::commands::ExitStatus;
use crate::printer::Printer;

/// Only public interace out to the bin/px.rs file, not to be used externally
///
pub async fn run(cli: Cli) -> Result<ExitStatus> {
    init_logging(&cli);

    // Configure the `Printer`, which controls user-facing output in the CLI.
    let printer = Printer::Default;

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
            command: ProfileCommand::Show(args),
        }) => commands::show_profile(args, printer).await,

        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::Init(args),
        }) => commands::init_profile(args, printer).await,

        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::List(args),
        }) => commands::list_profiles(args, printer).await,

        Commands::Profile(ProfileNamespace {
            command: ProfileCommand::Scan(args),
        }) => commands::scan_profile(args, printer).await,

        // Masters
        Commands::Master(MasterNamespace {
            command: MasterCommand::Best(args),
        }) => commands::find_best_master(args, printer).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::List(args),
        }) => commands::list_masters(args, printer).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Bias(args),
        }) => commands::create_master_bias(args, printer).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Dark(args),
        }) => commands::create_master_dark(args, printer).await,

        Commands::Master(MasterNamespace {
            command: MasterCommand::Flat(args),
        }) => commands::create_master_flat(args, printer).await,

        // Observations
        Commands::Observation(ObservationNamespace {
            command: ObservationCommand::List(args),
        }) => commands::list_observations(args, printer).await,

        Commands::Observation(ObservationNamespace {
            command: ObservationCommand::Calibrate(args),
        }) => commands::calibrate_observation(args, printer).await,

        // Project
        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Init(args),
        }) => commands::init_project(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Add(args),
        }) => commands::add_project_observation(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::List(args),
        }) => commands::list_projects(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Calibrate(args),
        }) => commands::calibrate_project(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Stack(args),
        }) => commands::stack_project_observations(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Align(args),
        }) => commands::align_project(args, printer).await,

        Commands::Project(ProjectNamespace {
            command: ProjectCommand::Sample(args),
        }) => commands::create_project_samples(args, printer).await,
    }
}
