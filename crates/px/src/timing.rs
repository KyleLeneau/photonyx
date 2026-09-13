//! Stage timing for the native engine, behind `PX_TIMING` (ADR 007, P12-T1).
//!
//! Setting `PX_TIMING` to anything other than `0` turns on the span recorder in
//! `px-imageproc` and prints its table to stderr when the command finishes —
//! stderr rather than stdout so a machine-readable `--output-format` stays
//! machine-readable.
//!
//! This is what answers "where does the time go" for a real command on real
//! data, rather than for a benchmark on a fixture.

use std::time::Instant;

/// Prints the table when dropped, so a command that returns early still
/// reports what it did before giving up.
pub struct Timing {
    started: Instant,
}

/// Turns recording on when `PX_TIMING` asks for it, and returns the guard that
/// prints the report. `None` means timing is off and nothing is recorded.
pub fn start() -> Option<Timing> {
    match std::env::var("PX_TIMING") {
        Ok(value) if value != "0" && !value.is_empty() => {
            px_imageproc::profile::enable();
            Some(Timing {
                started: Instant::now(),
            })
        }
        _ => None,
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        let report = px_imageproc::profile::render(self.started.elapsed());
        if !report.is_empty() {
            eprint!("\n{report}");
        }
    }
}
