use std::fmt::Display;

use strum_macros::Display;
use strum_macros::EnumString;
use strum_macros::FromRepr;

pub trait IntoArgument {
    fn to_argument(&self) -> crate::commands::Argument;
}

impl<T: IntoArgument> IntoArgument for Option<T> {
    fn to_argument(&self) -> crate::commands::Argument {
        self.as_ref()
            .map(|f| f.to_argument())
            .unwrap_or(crate::commands::Argument::None)
    }
}

#[derive(Debug, PartialEq, EnumString, Display, Clone)]
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

#[allow(dead_code)]
#[derive(Debug)]
pub enum RGBImage {
    /// A single RGB image to combine with
    Single(String),
    /// Specify all 3 layers independently
    RGB(String, String, String),
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
            SequenceFilter::Included => crate::commands::Argument::flag("filter-incl"),
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

/// Star catalog used for plate solving.
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum StarCatalog {
    Tycho2,
    Nomad,
    Gaia,
    Ppmxl,
    BrightStars,
    Apass,
}

/// Limit magnitude mode for plate solving.
///
/// - `Default`: automatically computed from field of view (no `-limitmag` flag)
/// - `Offset(f64)`: relative offset from auto magnitude; positive values use `+`, e.g. `-limitmag=+1.5`
/// - `Absolute(f64)`: absolute magnitude limit, e.g. `-limitmag=12.5`
#[derive(Debug, PartialEq, Clone, Default)]
pub enum LimitMag {
    #[default]
    Default,
    Offset(f64),
    Absolute(f64),
}

impl IntoArgument for LimitMag {
    fn to_argument(&self) -> crate::commands::Argument {
        use crate::commands::Argument;
        match &self {
            LimitMag::Default => Argument::None,
            LimitMag::Offset(v) if *v != 0.0 => {
                let s = if *v > 0.0 {
                    format!("+{}", v)
                } else {
                    v.to_string()
                };
                Argument::option("limitmag", Some(s))
            }
            LimitMag::Offset(_) => Argument::None,
            LimitMag::Absolute(v) => Argument::option("limitmag", Some(v.to_string())),
        }
    }
}

#[derive(Debug, PartialEq, FromRepr, Clone, Display, Copy)]
#[repr(u8)]
pub enum RmgreenProtection {
    AverageNeutral = 0,
    MaximumNeutral = 1,
    MaximumMask = 2,
    AdditiveMask = 3,
}

#[derive(Debug, PartialEq, FromRepr, Clone, Display, Copy)]
#[repr(u8)]
pub enum SaturationHueRange {
    PinkOrange = 0,
    OrangeYellow = 1,
    YellowCyan = 2,
    Cyan = 3,
    CyanMagenta = 4,
    MagentaPink = 5,
    ALL = 6,
}

#[derive(Debug, PartialEq, EnumString, Display, Clone, Copy)]
#[strum(serialize_all = "lowercase")]
pub enum StatOption {
    Basic,
    Full,
    Main,
}

pub enum UpdateKeyMethod {
    /// Set key, value and optional comment
    Set(String, String, Option<String>),
    /// Delete by key
    Delete(String),
    /// Rename key to newkey
    Rename(String, String),
    /// Add a comment
    Comment(String),
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

/// Helper to find the best rejection based on number of images
///
#[derive(Debug)]
pub struct BestRejection {
    pub method: StackRejection,
    pub low_threshold: f32,
    pub high_threshold: f32,
}

impl BestRejection {
    pub fn find(image_count: usize) -> Self {
        match image_count {
            0..=6 => Self {
                method: StackRejection::Percentile,
                low_threshold: 0.2,
                high_threshold: 0.1,
            },
            7..=30 => Self {
                method: StackRejection::Winsorized,
                low_threshold: 3.0,
                high_threshold: 3.0,
            },
            _ => Self {
                method: StackRejection::Linear,
                low_threshold: 5.0,
                high_threshold: 5.0,
            },
        }
    }
}

/// Resample during extraction
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum ExtractResample {
    HA,
    OIII,
}

/// Clipping mode for stretches
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum ClipMode {
    Clip,
    ReScale,
    RGBBlend,
    GlobalRescale,
}

/// Clipping channels
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Channels {
    R,
    G,
    B,
    RG,
    RB,
    GB,
}

/// Clipping weight to apply
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum ClipWeight {
    Human,
    Even,
    Independent,
    #[strum(serialize = "sat")]
    Saturation,
}

impl IntoArgument for ClipWeight {
    fn to_argument(&self) -> crate::commands::Argument {
        crate::commands::Argument::flag(self.to_string())
    }
}

/// Common bayer patterns
#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "UPPERCASE")]
pub enum BayerPattern {
    RGGB,
    BGGR,
    GRBG,
    GBRG,
}

#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum LimitOption {
    Clip,
    PosRescale,
    Rescale
}
