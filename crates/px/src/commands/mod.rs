use std::process::ExitCode;

pub(crate) use inspect::inspect_file;
pub(crate) use masters::create_bias::create_master_bias;
pub(crate) use masters::create_dark::create_master_dark;
pub(crate) use masters::create_flat::create_master_flat;
pub(crate) use masters::find_best::find_best_master;
pub(crate) use masters::list::list_masters;
pub(crate) use observation::calibrate::calibrate_observation;
pub(crate) use observation::list::list_observations;
pub(crate) use observation::preview::preview_observation;
pub(crate) use profile::info::show_profile;
pub(crate) use profile::init::init_profile;
pub(crate) use profile::scan::scan_profile;
pub(crate) use project::init::init_project;
pub(crate) use project::list::list_projects;
pub(crate) use project::sample::create_project_samples;
pub(crate) use project::stack::stack_project_observations;
#[cfg(feature = "self-update")]
pub(crate) use self_update::self_update;
pub(crate) use siril_test::siril_test;
pub(crate) use tui::terminal_ui;
pub(crate) use version::self_version;

mod inspect;
mod masters;
mod observation;
mod profile;
mod project;
#[cfg(feature = "self-update")]
mod self_update;
mod siril_test;
mod tui;
mod version;

#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum ExitStatus {
    /// The command succeeded.
    Success,

    /// The command failed due to an error in the user input.
    Failure,

    /// The command failed with an unexpected error.
    Error,

    /// The command's exit status is propagated from an external command.
    External(u8),
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        match status {
            ExitStatus::Success => Self::from(0),
            ExitStatus::Failure => Self::from(1),
            ExitStatus::Error => Self::from(2),
            ExitStatus::External(code) => Self::from(code),
        }
    }
}
