mod siril;
mod types;

pub mod command;
pub mod message;

pub use siril::Builder;
pub use siril::Siril;
pub use siril::find_siril_cli;
pub use types::*;

// TODO: Siril macro for easy one off jobs
