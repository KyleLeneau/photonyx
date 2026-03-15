mod siril;

pub mod command;
pub mod message;

pub use siril::Builder;
pub use siril::Siril;
pub use siril::find_siril_cli;
use strum_macros::Display;
use strum_macros::EnumString;

// TODO: Siril macro for easy one off jobs

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum FitsExt {
    #[strum(serialize = "fit")]
    FIT,

    #[strum(serialize = "fits")]
    FITS,

    #[strum(serialize = "fts")]
    FTS,
}
