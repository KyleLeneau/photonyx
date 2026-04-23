use thiserror::Error;

#[derive(Debug, Clone)]
pub enum SirilMessage {
    Ready,
    Log(String),
    StatusStarting(String),
    StatusSuccess(String),
    StatusError(String),
    StatusExit,
    Progress(u32),
}

impl SirilMessage {
    pub fn parse(line: &str) -> Self {
        let line = line.trim();

        if line == "ready" {
            return Self::Ready;
        }

        if let Some(msg) = line.strip_prefix("log: ") {
            return Self::Log(msg.to_string());
        }

        if let Some(rest) = line.strip_prefix("status: ") {
            if let Some(cmd) = rest.strip_prefix("starting ") {
                return Self::StatusStarting(cmd.to_string());
            }
            if let Some(cmd) = rest.strip_prefix("success ") {
                return Self::StatusSuccess(cmd.to_string());
            }
            if let Some(cmd) = rest.strip_prefix("error ") {
                return Self::StatusError(cmd.to_string());
            }
            if rest == "exit" {
                return Self::StatusExit;
            }
        }

        if let Some(rest) = line.strip_prefix("progress: ")
            && let Some(pct_str) = rest.strip_suffix('%')
            && let Ok(pct) = pct_str.trim().parse::<u32>()
        {
            return Self::Progress(pct);
        }

        // Unknown lines treated as log messages (defensive)
        Self::Log(line.to_string())
    }
}

// ---------------------------------------------------------------------------
// SirilError
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum SirilError {
    #[error("Siril command '{command}' failed: {logs:?}")]
    CommandFailed { command: String, logs: Vec<String> },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Siril process exited unexpectedly")]
    ProcessExited,

    #[error("Timed out waiting for Siril")]
    Timeout,

    #[error("Pipe setup error: {0}")]
    PipeSetup(String),

    #[error("Siril CLI executable not found, please install it first")]
    NotInstalled,

    #[error("Siril {found} is too old; minimum required version is {minimum}")]
    VersionTooOld { found: String, minimum: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Parsing error: {0}")]
    ParseError(String),
}
