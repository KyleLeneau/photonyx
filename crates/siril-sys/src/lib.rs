mod output;
mod siril;
mod types;

pub mod commands;
pub mod message;
pub mod siril_ext;

pub use output::{OutputLine, OutputSink, OutputStream};
pub use siril::Builder;
pub use siril::Siril;
pub use siril::find_siril_cli;
pub use types::*;

// TODO: Siril macro for easy one off jobs
