use std::process::ExitCode;

#[cfg(feature = "self-update")]
pub(crate) use self_update::self_update;
pub(crate) use siril_test::siril_test;
pub(crate) use stat::stat;
pub(crate) use version::self_version;

#[cfg(feature = "self-update")]
mod self_update;
mod siril_test;
mod stat;
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
