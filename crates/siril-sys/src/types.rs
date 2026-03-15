use std::fmt::Display;

use strum_macros::Display;
use strum_macros::EnumString;

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum FitsExt {
    #[strum(serialize = "fit")]
    FIT,

    #[strum(serialize = "fits")]
    FITS,

    #[strum(serialize = "fts")]
    FTS,
}

/// Represents some of the common Siril settings, use `get -a` to discover more.
///
/// FUTURE: Potentially codegen this all all remainging settings
#[derive(Debug, PartialEq, EnumString, Display)]
pub enum SirilSetting {
    #[strum(serialize = "core.extension")]
    Extension,

    #[strum(serialize = "core.force_16bit")]
    Force16Bit,

    #[strum(serialize = "core.mem_mode")]
    MemoryMode,

    #[strum(serialize = "core.mem_amount")]
    MemoryAmount,

    #[strum(serialize = "core.mem_ratio")]
    MemoryRatio,

    #[strum(to_string = "{0}")]
    Other(String),
}

pub struct Rect {
    pub x: u8,
    pub y: u8,
    pub width: u8,
    pub height: u8,
}
impl Display for Rect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {} {}", self.x, self.y, self.width, self.height)
    }
}

pub struct SigmaRange {
    pub low: f64,
    pub high: f64,
}
impl Display for SigmaRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.low, self.high)
    }
}
