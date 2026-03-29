# 002 — Decoupled Output Routing for Siril stdout/stderr

## Context

The `siril-sys` crate spawns a `siril-cli` process and currently hard-codes where its stdout
and stderr go: `println!("[siril stdout] ...")` and `eprintln!("[siril stderr] ...")` inside
background tasks in `Siril::new()`. This is fine for a CLI but is too rigid for anticipated
future use cases:

- **Silence** — suppress output entirely during tests or batch jobs
- **TUI pane** — redirect lines to a dedicated terminal UI panel
- **SSE / web app** — stream lines as server-sent events to a browser client

The ask is to decouple this so callers who construct a `Siril` instance can specify where
output goes, without changing the existing default behavior or the `build()` return type.

---

## Decision

Introduce an `OutputSink` enum and supporting types in a new `output.rs` module inside
`siril-sys`. Add a single `output_sink` field to `Builder` with a corresponding builder
method. The default behavior (`Inherit`) is unchanged.

### New types — `crates/siril-sys/src/output.rs`

```rust
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
#[derive(Clone, Default)]
pub enum OutputSink {
    /// Forward to the calling process's stdout/stderr (current behavior).
    #[default]
    Inherit,
    /// Silently discard all output.
    Discard,
    /// Send each line to the provided channel sender.
    ///
    /// The caller owns the `Receiver` end. Lines are dropped silently if
    /// the receiver has been dropped.
    Channel(tokio::sync::mpsc::Sender<OutputLine>),
}
```

`mpsc::Sender<T>` is `Clone`, so deriving `Clone` on the whole enum works without `Arc`.
The sink is cloned once per background task (stdout task, stderr task).

### `Builder` changes

Add one field (defaulting via `OutputSink::default()`):

```rust
output_sink: OutputSink,
```

Add one builder method:

```rust
pub fn output_sink(mut self, sink: OutputSink) -> Self {
    self.output_sink = sink;
    self
}
```

### `Siril::new()` changes

Before the tasks are spawned, clone the sink:

```rust
let stdout_sink = builder.output_sink.clone();
let stderr_sink = builder.output_sink; // consume original for the second task
```

Replace hardcoded `println!`/`eprintln!` in each task with a match:

```rust
match sink {
    OutputSink::Inherit  => println!("[siril stdout] {}", line),
    OutputSink::Discard  => {}
    OutputSink::Channel(ref tx) => {
        let _ = tx.send(OutputLine { stream: OutputStream::Stdout, line }).await;
    }
}
```

### `lib.rs` exports

```rust
mod output;
pub use output::{OutputLine, OutputSink, OutputStream};
```

## Known limitation

The `reader_task` (named-pipe protocol messages) also emits `println!` calls for
`SirilMessage::Log`, `Progress`, and other variants. Those prints are **not** covered by
`OutputSink` in this change — structured protocol messages already flow through `msg_rx` and
silencing the reader_task prints is a separate concern to revisit later.

A `// TODO` comment is left at the relevant site in `reader_task` to mark this.
