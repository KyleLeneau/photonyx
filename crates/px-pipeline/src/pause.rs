//! Interactive debug breakpoints for pipeline authors.
//!
//! `pause!` is a no-op unless [`px_static::EnvVars::PX_PIPELINE_PAUSE`] is set to a truthy
//! value, so it is safe to leave call sites in place: they never fire in normal use.
//! When enabled, it prints the working directory and call site, then blocks on an interactive
//! prompt so a pipeline can be inspected mid-run before deciding whether to continue or abort.
//!
//! Aborting simply returns [`PipelineError::Aborted`], which unwinds via `?` like any other
//! pipeline error. Cleanup of Siril's scratch directory falls out of that for free: the
//! `Siril` handle (and its `TempDir`) is dropped as the error propagates up the call stack.

use std::path::Path;
use std::sync::OnceLock;

use px_static::EnvVars;

use crate::error::PipelineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseChoice {
    Continue,
    Abort,
}

impl std::fmt::Display for PauseChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PauseChoice::Continue => write!(f, "Continue"),
            PauseChoice::Abort => write!(f, "Abort pipeline"),
        }
    }
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var(EnvVars::PX_PIPELINE_PAUSE)
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

/// Blocks on an interactive prompt if pipeline pausing is enabled, otherwise returns
/// immediately. Prefer the [`crate::pause!`] macro over calling this directly, since it fills
/// in `file` and `line` for you.
pub async fn pause(
    cwd: impl AsRef<Path>,
    label: Option<&str>,
    file: &'static str,
    line: u32,
) -> Result<(), PipelineError> {
    if !enabled() {
        return Ok(());
    }

    let cwd = cwd.as_ref();
    let context = match label {
        Some(label) => format!("{file}:{line} ({label})"),
        None => format!("{file}:{line}"),
    };

    eprintln!();
    eprintln!("⏸  pipeline paused at {context}");
    eprintln!("   cwd: {}", cwd.display());
    eprintln!("   inspect the directory above, then choose how to proceed.");
    eprintln!();

    let choice = tokio::task::spawn_blocking(|| {
        inquire::Select::new(
            "What next?",
            vec![PauseChoice::Continue, PauseChoice::Abort],
        )
        .prompt()
    })
    .await;

    match choice {
        Ok(Ok(PauseChoice::Continue)) => Ok(()),
        Ok(Ok(PauseChoice::Abort)) => Err(PipelineError::Aborted(context)),
        Ok(Err(err)) => {
            eprintln!("pause prompt failed ({err}); continuing automatically");
            Ok(())
        }
        Err(err) => {
            eprintln!("pause prompt task failed ({err}); continuing automatically");
            Ok(())
        }
    }
}

/// Pauses pipeline execution for interactive inspection, gated behind
/// `PX_PIPELINE_PAUSE`. No-op unless that env var is set to a truthy value.
///
/// ```ignore
/// px_pipeline::pause!(siril.initial_directory(), "after RGB composition").await?;
/// ```
#[macro_export]
macro_rules! pause {
    ($cwd:expr) => {
        $crate::pause::pause($cwd, None, file!(), line!())
    };
    ($cwd:expr, $label:expr) => {
        $crate::pause::pause($cwd, Some($label), file!(), line!())
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_op_when_disabled() {
        assert!(
            std::env::var(EnvVars::PX_PIPELINE_PAUSE).is_err(),
            "PX_PIPELINE_PAUSE must be unset for this test to be meaningful"
        );
        assert!(pause("/tmp", Some("test"), file!(), line!()).await.is_ok());
    }
}
