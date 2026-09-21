use px_fits::FitsError;
use siril_sys::SirilError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Siril error: {0}")]
    SirilError(#[from] SirilError),

    #[error("fits error: {0}")]
    FitsError(#[from] FitsError),

    #[error("file(s) not found: {0}")]
    FileNotFound(String),

    #[error("Output file(s) not found: {0}")]
    OutputFileNotFound(String),

    #[error("image processing error: {0}")]
    ImageProc(#[from] px_imageproc::ImageProcError),

    /// The selected engine cannot run this pipeline. Reported rather than
    /// silently falling back to the other one — a user who asked for the
    /// native engine and got Siril would have no way to tell.
    #[error("{pipeline} is not supported by the {engine} engine{detail}")]
    UnsupportedByEngine {
        pipeline: String,
        engine: String,
        detail: String,
    },

    #[error("pipeline aborted at pause point: {0}")]
    Aborted(String),
}
