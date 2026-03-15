use std::path::{Path, PathBuf};
use std::process::Stdio;

use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::pipe;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::FitsExt;
use crate::message::{SirilError, SirilMessage};

// TODO: Need a working directory to startup in, make it a tmpfs

// TODO: need to be sure to support windows pipes

// TODO: logs from siril should not go to stdout by default (will want for sse streaming)

// FUTURE: Add a Siril setting enum with string backing and associate types (codegen??)

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
    pipe_writer: pipe::Sender,
    msg_rx: mpsc::Receiver<SirilMessage>,
    reader_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    in_pipe_path: PathBuf,
    out_pipe_path: PathBuf,
    _temp_dir: Option<TempDir>,
}

impl Siril {
    /// Spawn a new siril-cli process in pipe mode and wait until it is ready.
    ///
    async fn new<'a>(builder: Builder<'a>) -> Result<Self, SirilError> {
        // Find the right siril-cli for the system
        let siril_exe = find_siril_cli("siril-cli")?;
        tracing::debug!("siril-cli found {:?}", &siril_exe);

        // TODO: AAA Cleanup this now that the builder exists for us to decide where to start (can we drop the lifetime?)

        // Create temp directory to work in
        let temp_dir = TempDir::with_prefix("photonyx-")?;
        let uses_temp_dir = true;
        let dir = temp_dir.path();
        tracing::debug!("starting in directory: {:?}", dir);

        // 1. Generate unique pipe paths
        let id = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let in_pipe_path = PathBuf::from(format!("/tmp/siril_rs_{}.in", id));
        let out_pipe_path = PathBuf::from(format!("/tmp/siril_rs_{}.out", id));

        // 2. Create FIFOs
        use nix::sys::stat::Mode;
        nix::unistd::mkfifo(&in_pipe_path, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|e| SirilError::PipeSetup(format!("mkfifo in: {}", e)))?;
        nix::unistd::mkfifo(&out_pipe_path, Mode::S_IRUSR | Mode::S_IWUSR)
            .map_err(|e| SirilError::PipeSetup(format!("mkfifo out: {}", e)))?;

        // 3. Spawn siril-cli in pipe mode
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

        // 4. Spawn stdout/stderr logging tasks
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

        // 5. Open the output pipe (our read end) FIRST.
        //    On macOS, open_receiver on a FIFO always succeeds immediately.
        let out_pipe_reader = pipe::OpenOptions::new().open_receiver(&out_pipe_path)?;

        // 6. Retry open_sender — macOS returns ENXIO until Siril opens
        //    its read end of the input pipe.
        let in_pipe_writer = loop {
            match pipe::OpenOptions::new().open_sender(&in_pipe_path) {
                Ok(sender) => break sender,
                Err(e) if e.raw_os_error() == Some(nix::libc::ENXIO) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => return Err(SirilError::Io(e)),
            }
        };

        // 7. Background reader: parse pipe-protocol lines → mpsc channel
        let (msg_tx, msg_rx) = mpsc::channel::<SirilMessage>(256);

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

        // 8. Wait for the "ready" message
        let mut siril = Self {
            child,
            pipe_writer: in_pipe_writer,
            msg_rx,
            reader_task,
            stdout_task,
            stderr_task,
            in_pipe_path,
            out_pipe_path,
            _temp_dir: if uses_temp_dir { Some(temp_dir) } else { None },
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
                self.command("set core.mem_mode=1").await?;
                self.command(&format!("set core.mem_amount={}", gb)).await?;
            }
            MemoryLimit::Ratio(percent) => {
                self.command("set core.mem_mode=0").await?;
                self.command(&format!("set core.mem_ratio={:.2}", percent))
                    .await?;
            }
        }

        self.command("capabilities").await?;
        self.command(&format!("set core.extension={}", builder.fits_extension))
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

        let _ = std::fs::remove_file(&self.in_pipe_path);
        let _ = std::fs::remove_file(&self.out_pipe_path);

        Ok(())
    }
}

impl Drop for Siril {
    fn drop(&mut self) {
        self.reader_task.abort();
        self.stdout_task.abort();
        self.stderr_task.abort();
        let _ = self.child.start_kill();
        let _ = std::fs::remove_file(&self.in_pipe_path);
        let _ = std::fs::remove_file(&self.out_pipe_path);
    }
}
