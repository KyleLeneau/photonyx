use std::path::{Path, PathBuf};
use std::process::Stdio;

use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::unix::pipe;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::message::{SirilError, SirilMessage};
use crate::{FitsExt, SirilSetting};

/// Unix FIFOs use `pipe::Sender`; Windows named pipes use `NamedPipeClient`
/// (Siril acts as the server on Windows, we connect as a client).
#[cfg(unix)]
type PipeWriter = pipe::Sender;
#[cfg(windows)]
type PipeWriter = tokio::net::windows::named_pipe::NamedPipeClient;

/// Find the right siril-cli across the system
///
pub fn find_siril_cli(exe: &str) -> Result<PathBuf, SirilError> {
    // Check as-is first (absolute path or already on PATH)
    let p = PathBuf::from(exe);
    if p.exists() {
        return Ok(p);
    }

    let candidates: &[&str] = match std::env::consts::OS {
        "windows" => &[
            "C:/msys64/mingw64/bin/siril-cli.exe",
            "C:/Program Files/SiriL/bin/siril-cli.exe",
        ],
        "macos" => &[
            "/Applications/Siril.app/Contents/MacOS/siril-cli",
            "/Applications/Siril.app/Contents/MacOS/Siril",
        ],
        "linux" => &["/usr/local/bin/siril-cli", "/usr/bin/siril-cli"],
        _ => &[],
    };

    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .ok_or(SirilError::NotInstalled)
}

enum MemoryLimit {
    /// Fixed memory limit in Gigabytes
    FixedGB(u8),

    /// Percent of available memory (0.0 to 1.0)
    Ratio(f64),
}

pub struct Builder<'a> {
    /// Directory to start up in, if None then uses a tempdir
    directory: Option<&'a Path>,

    /// How much CPU to use (in number of cores, default is all)
    cpu_limit: Option<u8>,

    /// How much memory to use (in GB, default is 90% of available memory)
    memory_limit: MemoryLimit,

    /// Default fit extension to use (default: fits)
    fits_extension: FitsExt,
}

impl Default for Builder<'_> {
    fn default() -> Self {
        Self {
            directory: None, // temp dir
            cpu_limit: None, // all cpu's
            memory_limit: MemoryLimit::Ratio(0.9),
            fits_extension: FitsExt::FITS,
        }
    }
}

impl<'a> Builder<'a> {
    pub fn use_cpu_limit(mut self, cores: u8) -> Self {
        self.cpu_limit = Some(cores);
        self
    }

    pub fn use_memory_ratio(mut self, limit: f64) -> Self {
        self.memory_limit = MemoryLimit::Ratio(limit);
        self
    }

    pub fn use_memory_fixed_gb(mut self, limit: u8) -> Self {
        self.memory_limit = MemoryLimit::FixedGB(limit);
        self
    }

    pub fn use_directory(mut self, directory: &'a Path) -> Self {
        self.directory = Some(directory);
        self
    }

    pub fn use_extension(mut self, ext: FitsExt) -> Self {
        self.fits_extension = ext;
        self
    }

    pub async fn build(self) -> Result<Siril, SirilError> {
        if let MemoryLimit::Ratio(r) = self.memory_limit
            && !(0.0..=1.0).contains(&r)
        {
            return Err(SirilError::InvalidConfig(format!(
                "memory ratio must be between 0.0 and 1.0, got {r}"
            )));
        }
        Siril::new(self).await
    }
}

pub struct Siril {
    child: Child,
    pipe_writer: PipeWriter,
    msg_rx: mpsc::Receiver<SirilMessage>,
    reader_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    in_pipe_path: PathBuf,
    out_pipe_path: PathBuf,
    _temp_dir: TempDir,
}

impl Siril {
    /// Spawn a new siril-cli process in pipe mode and wait until it is ready.
    ///
    async fn new<'a>(builder: Builder<'a>) -> Result<Self, SirilError> {
        // Find the right siril-cli for the system
        let siril_exe = find_siril_cli("siril-cli")?;
        tracing::debug!("siril-cli found {:?}", &siril_exe);

        // Always create temp directory to work in but start in builder supplied
        let temp_dir = TempDir::with_prefix("photonyx-")?;
        let dir = if let Some(startup_dir) = builder.directory {
            startup_dir
        } else {
            temp_dir.path()
        };
        tracing::debug!("starting in directory: {:?}", dir);

        // Generate unique pipe identifier
        let id = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );

        // Platform-specific pipe paths
        #[cfg(unix)]
        let (in_pipe_path, out_pipe_path): (PathBuf, PathBuf) = (
            PathBuf::from(format!("/tmp/siril_rs_{}.in", id)),
            PathBuf::from(format!("/tmp/siril_rs_{}.out", id)),
        );
        #[cfg(windows)]
        let (in_pipe_path, out_pipe_path): (PathBuf, PathBuf) = (
            PathBuf::from(format!(r"\\.\pipe\siril_command.in")),
            PathBuf::from(format!(r"\\.\pipe\siril_command.out")),
        );

        // Unix: create FIFOs before spawning (Siril opens them after launch)
        #[cfg(unix)]
        {
            use nix::sys::stat::Mode;
            nix::unistd::mkfifo(&in_pipe_path, Mode::S_IRUSR | Mode::S_IWUSR)
                .map_err(|e| SirilError::PipeSetup(format!("mkfifo in: {}", e)))?;
            nix::unistd::mkfifo(&out_pipe_path, Mode::S_IRUSR | Mode::S_IWUSR)
                .map_err(|e| SirilError::PipeSetup(format!("mkfifo out: {}", e)))?;
        }

        // Spawn siril-cli in pipe mode
        let mut child = Command::new(&siril_exe)
            .arg("-d")
            .arg(dir)
            .arg("-p")
            .arg("-r")
            .arg(&in_pipe_path)
            .arg("-w")
            .arg(&out_pipe_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(SirilError::Io)?;

        // Spawn stdout/stderr logging tasks
        let stdout = child.stdout.take().expect("stdout not captured");
        let stderr = child.stderr.take().expect("stderr not captured");

        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("[siril stdout] {}", line);
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("[siril stderr] {}", line);
            }
        });

        let (msg_tx, msg_rx) = mpsc::channel::<SirilMessage>(256);

        // Unix: open_receiver on the output FIFO first, then retry open_sender
        // until Siril opens its read end of the input FIFO.
        #[cfg(unix)]
        let (in_pipe_writer, reader_task) = {
            let out_pipe_reader = pipe::OpenOptions::new().open_receiver(&out_pipe_path)?;

            let in_pipe_writer = loop {
                match pipe::OpenOptions::new().open_sender(&in_pipe_path) {
                    Ok(sender) => break sender,
                    Err(e) if e.raw_os_error() == Some(nix::libc::ENXIO) => {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    Err(e) => return Err(SirilError::Io(e)),
                }
            };

            let reader_task = tokio::spawn(async move {
                let mut reader = BufReader::new(out_pipe_reader).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let msg = SirilMessage::parse(&line);
                    match &msg {
                        SirilMessage::Log(text) => println!("[siril] {}", text),
                        SirilMessage::Progress(pct) => println!("[siril] progress: {}%", pct),
                        other => println!("[siril] {:?}", other),
                    }
                    if msg_tx.send(msg).await.is_err() {
                        break; // receiver dropped
                    }
                }
            });

            (in_pipe_writer, reader_task)
        };

        // Windows: Siril creates the named pipe servers; we connect as clients.
        // Retry on ERROR_FILE_NOT_FOUND (2) until Siril has created the pipes,
        // and on ERROR_PIPE_BUSY (231) if all instances are temporarily in use.
        #[cfg(windows)]
        let (in_pipe_writer, reader_task) = {
            use tokio::net::windows::named_pipe::ClientOptions;
            const ERROR_FILE_NOT_FOUND: i32 = 2;
            const ERROR_PIPE_BUSY: i32 = 231;

            // Siril creates both pipes in an unspecified order and blocks waiting
            // for a client on each. Connect to both concurrently to avoid deadlock.
            let connect = |path: PathBuf| async move {
                loop {
                    match ClientOptions::new().open(&path) {
                        Ok(client) => return Ok::<_, SirilError>(client),
                        Err(e)
                            if matches!(
                                e.raw_os_error(),
                                Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PIPE_BUSY)
                            ) =>
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        Err(e) => return Err(SirilError::Io(e)),
                    }
                }
            };

            let (in_result, out_result) = tokio::join!(
                connect(in_pipe_path.clone()),
                connect(out_pipe_path.clone()),
            );
            let in_pipe_writer = in_result?;
            let out_pipe_reader = out_result?;

            let reader_task = tokio::spawn(async move {
                let mut reader = BufReader::new(out_pipe_reader).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let msg = SirilMessage::parse(&line);
                    match &msg {
                        SirilMessage::Log(text) => println!("[siril] {}", text),
                        SirilMessage::Progress(pct) => println!("[siril] progress: {}%", pct),
                        other => println!("[siril] {:?}", other),
                    }
                    if msg_tx.send(msg).await.is_err() {
                        break; // receiver dropped
                    }
                }
            });

            (in_pipe_writer, reader_task)
        };

        // Wait for the "ready" message
        let mut siril = Self {
            child,
            pipe_writer: in_pipe_writer,
            msg_rx,
            reader_task,
            stdout_task,
            stderr_task,
            in_pipe_path,
            out_pipe_path,
            _temp_dir: temp_dir,
        };

        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), siril.msg_rx.recv())
                .await
            {
                Ok(Some(SirilMessage::Ready)) => break,
                Ok(Some(_)) => continue, // skip log/progress during startup
                Ok(None) => return Err(SirilError::ProcessExited),
                Err(_) => return Err(SirilError::Timeout),
            }
        }

        // Use the builder to startup once ready
        siril.configure(builder).await?;

        Ok(siril)
    }

    /// Call some helper commands on siril if the user provided a builder to us
    ///
    async fn configure<'a>(&mut self, builder: Builder<'a>) -> Result<(), SirilError> {
        self.command("requires 0.99.10").await?;
        if let Some(cores) = builder.cpu_limit {
            self.command(&format!("setcpu {}", cores)).await?;
        }

        match builder.memory_limit {
            MemoryLimit::FixedGB(gb) => {
                self.set(SirilSetting::MemoryMode, "1").await?;
                self.set(SirilSetting::MemoryAmount, gb).await?;
            }
            MemoryLimit::Ratio(percent) => {
                self.set(SirilSetting::MemoryMode, "0").await?;
                self.set(SirilSetting::MemoryRatio, &format!("{:.2}", percent))
                    .await?;
            }
        }

        self.command("capabilities").await?;
        self.set(SirilSetting::Extension, builder.fits_extension)
            .await?;

        Ok(())
    }

    /// Send a command and wait for it to complete.
    ///
    /// Returns the log lines emitted during command execution.
    /// The lines are also printed to stdout as they arrive.
    pub async fn command(&mut self, cmd: &str) -> Result<Vec<String>, SirilError> {
        // Special handling for exit — Siril may close pipes before responding
        if cmd.trim() == "exit" {
            let _ = self.pipe_writer.write_all(b"exit\n").await;
            return Ok(vec![]);
        }

        // Send the command
        self.pipe_writer
            .write_all(format!("{}\n", cmd).as_bytes())
            .await?;

        // Collect output until status: success/error
        let mut logs = Vec::new();

        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(300), // 5 min for long ops
                self.msg_rx.recv(),
            )
            .await
            {
                Ok(Some(msg)) => match msg {
                    SirilMessage::StatusSuccess(_) => return Ok(logs),
                    SirilMessage::StatusError(_) => {
                        return Err(SirilError::CommandFailed {
                            command: cmd.to_string(),
                            logs,
                        });
                    }
                    SirilMessage::StatusExit => return Err(SirilError::ProcessExited),
                    SirilMessage::Log(text) => logs.push(text),
                    SirilMessage::Progress(_) => {} // logged by background reader
                    SirilMessage::StatusStarting(_) | SirilMessage::Ready => {}
                },
                Ok(None) => return Err(SirilError::ProcessExited),
                Err(_) => return Err(SirilError::Timeout),
            }
        }
    }

    /// Gracefully shut down the Siril process.
    ///
    pub async fn close(mut self) -> Result<(), SirilError> {
        // Send exit command
        let _ = self.command("exit").await;

        // Wait briefly for the process to exit cleanly
        match tokio::time::timeout(std::time::Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(_)) => {}
            _ => {
                let _ = self.child.start_kill();
            }
        }

        self.reader_task.abort();
        self.stdout_task.abort();
        self.stderr_task.abort();

        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.in_pipe_path);
            let _ = std::fs::remove_file(&self.out_pipe_path);
        }

        Ok(())
    }

    /// Sets a Siril setting by key and value (`set {key}={value}`)
    ///
    pub async fn set(
        &mut self,
        setting: SirilSetting,
        value: impl std::fmt::Display,
    ) -> Result<(), SirilError> {
        self.command(&format!("set {}={}", setting, value)).await?;
        Ok(())
    }
}

impl Drop for Siril {
    fn drop(&mut self) {
        self.reader_task.abort();
        self.stdout_task.abort();
        self.stderr_task.abort();
        let _ = self.child.start_kill();
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.in_pipe_path);
            let _ = std::fs::remove_file(&self.out_pipe_path);
        }
    }
}
