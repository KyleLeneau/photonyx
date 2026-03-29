use tokio::sync::mpsc;

/// Which OS-level stream a line originated from.
#[derive(Clone)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

/// A single line of output from the siril-cli process.
#[derive(Clone)]
pub struct OutputLine {
    pub stream: OutputStream,
    pub line: String,
}

/// Controls where siril-cli stdout/stderr lines are routed.
///
/// Set on [`crate::Builder`] via [`crate::Builder::output_sink`].
#[derive(Clone, Default)]
pub enum OutputSink {
    /// Forward to the calling process's stdout/stderr (default).
    #[default]
    Inherit,
    /// Silently discard all output.
    Discard,
    /// Send each line to the provided channel sender.
    ///
    /// The caller owns the `Receiver` end. Lines are dropped silently if the
    /// receiver has been dropped.
    Channel(mpsc::Sender<OutputLine>),
}
