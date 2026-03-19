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

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum RegistrationTransformation {
    Shift,
    Similarity,
    Affine,
    Homography,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum PixelInterpolation {
    None,
    Nearest,
    Cubic,
    Lanczos4,
    Linear,
    Area,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum DrizzleKernel {
    Point,
    Turbo,
    Square,
    Gaussian,
    Lanczos2,
    Lanczos3,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum SplitOption {
    Hsl,
    Hsv,
    Lab,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum StackType {
    Sum,
    Min,
    Max,
    Med,
    Rej,
}

/// Normalization mode for stacking. Serializes to the full Siril flag string.
#[derive(Debug, PartialEq, EnumString, Display)]
pub enum StackNormFlag {
    #[strum(serialize = "-nonorm")]
    NoNorm,

    #[strum(serialize = "-norm=add")]
    Add,

    #[strum(serialize = "-norm=mul")]
    Mul,

    #[strum(serialize = "-norm=addscale")]
    AddScale,

    #[strum(serialize = "-norm=mulscale")]
    MulScale,
}

#[derive(Debug, PartialEq, EnumString, Display)]
pub enum StackRejection {
    #[strum(serialize = "n")]
    None,

    #[strum(serialize = "p")]
    Percentile,

    #[strum(serialize = "s")]
    Sigma,

    #[strum(serialize = "m")]
    Median,

    #[strum(serialize = "w")]
    Winsorized,

    #[strum(serialize = "l")]
    Linear,

    #[strum(serialize = "g")]
    Generalized,

    #[strum(serialize = "a")]
    Mad,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum StackWeightingFlag {
    #[strum(serialize = "-weight_from_noise")]
    Noise,

    #[strum(serialize = "-weight_from_wfwhm")]
    WFwhm,

    #[strum(serialize = "-weight_from_nbstars")]
    NbStars,

    #[strum(serialize = "-weight_from_nbstack")]
    NbStack,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum StackRejectionMapFlag {
    #[strum(serialize = "-rejmaps")]
    Two,

    #[strum(serialize = "-rejmap")]
    Merged,
}

#[derive(Debug, PartialEq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum SequenceFraming {
    Current,
    Min,
    Max,
    Cog,
}

/// Non-inclusion filter types for sequence filtering.
#[derive(Debug, Clone, PartialEq, EnumString, Display)]
pub enum SequenceFilterType {
    #[strum(serialize = "filter-fwhm")]
    Fwhm,

    #[strum(serialize = "filter-wfwhm")]
    WFwhm,

    #[strum(serialize = "filter-roundness")]
    Roundness,

    #[strum(serialize = "filter-quality")]
    Quality,

    #[strum(serialize = "filter-nbstars")]
    NbStars,

    #[strum(serialize = "filter-bkg")]
    Bkg,

    #[strum(serialize = "filter-nbstack")]
    NbStack,
}

/// A filter for selecting which frames from a sequence to process.
///
/// The `Included` variant selects only manually included frames (`-filter-incl`).
/// `ByValue` and `ByPercent` filter by a quality metric with either an absolute
/// threshold or a percentage of best frames to keep.
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceFilter {
    Included,
    ByValue {
        filter_type: SequenceFilterType,
        value: f64,
    },
    ByPercent {
        filter_type: SequenceFilterType,
        percent: f64,
    },
}

impl SequenceFilter {
    pub fn to_argument(&self) -> crate::commands::Argument {
        match self {
            SequenceFilter::Included => crate::commands::Argument::flag("filter-incl", true),
            SequenceFilter::ByValue { filter_type, value } => {
                crate::commands::Argument::option(filter_type.to_string(), Some(value.to_string()))
            }
            SequenceFilter::ByPercent {
                filter_type,
                percent,
            } => crate::commands::Argument::option(
                filter_type.to_string(),
                Some(format!("{percent}%")),
            ),
        }
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
