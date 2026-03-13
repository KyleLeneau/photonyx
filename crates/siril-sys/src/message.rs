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

#[derive(Debug)]
pub enum SirilError {
    CommandFailed { command: String, logs: Vec<String> },
    Io(std::io::Error),
    ProcessExited,
    Timeout,
    PipeSetup(String),
}

impl From<std::io::Error> for SirilError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for SirilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandFailed { command, logs } => {
                write!(f, "Siril command '{}' failed: {}", command, logs.join("\n"))
            }
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::ProcessExited => write!(f, "Siril process exited unexpectedly"),
            Self::Timeout => write!(f, "Timed out waiting for Siril"),
            Self::PipeSetup(msg) => write!(f, "Pipe setup error: {}", msg),
        }
    }
}

impl std::error::Error for SirilError {}
