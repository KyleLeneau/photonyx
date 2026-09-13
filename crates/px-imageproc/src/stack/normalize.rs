//! Per-frame normalization coefficients (ADR 007, P11-T2).
//!
//! Frames of one session are not photometrically identical: the sky brightens,
//! transparency changes, the target moves through different airmass. Stacking
//! them unnormalized makes rejection fire on the *frame* rather than on the
//! outlier — a frame taken under a brighter sky is an outlier at every pixel —
//! and leaves the result weighted by however the sky happened to behave.
//!
//! The fix is a per-frame affine correction onto the reference frame's
//! photometric scale, fitted to robust location and scale rather than to the mean
//! and standard deviation, because the stars and any satellite trails must not
//! influence it (the estimators are in [`crate::stats`]).
//!
//! Each frame gets `value · scale + offset` per channel:
//!
//! ```text
//! scale  = scale_ref / scale_i
//! offset = location_ref − location_i · scale
//! ```
//!
//! so the frame's background lands on the reference's background and its spread
//! on the reference's spread. That is "additive with scaling", the mode Siril's
//! `CreateMasterLight` uses and therefore the one the native path defaults to
//! (ADR 007 D9).

use rayon::prelude::*;

use crate::error::ImageProcError;
use crate::runtime::Runtime;
use crate::sequence::FrameSet;
use crate::stats::{self, ChannelStats};

/// How frames are brought onto a common photometric scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NormMethod {
    /// Leave frames as they are. Correct for calibration masters, where the
    /// frames are already on one scale by construction.
    None,
    /// Match the background level only.
    Additive,
    /// Match the background level by a multiplicative factor, which preserves
    /// zero. Appropriate for flats.
    Multiplicative,
    /// Match background level *and* spread. The default for lights.
    #[default]
    AdditiveScaling,
    /// Multiplicative, with the spread matched too.
    MultiplicativeScaling,
}

/// One frame's correction for one channel: `value · scale + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormCoefficients {
    pub scale: f64,
    pub offset: f64,
}

impl NormCoefficients {
    /// The correction that changes nothing.
    pub const IDENTITY: Self = Self {
        scale: 1.0,
        offset: 0.0,
    };

    pub fn is_identity(&self) -> bool {
        self.scale == 1.0 && self.offset == 0.0
    }

    #[inline]
    pub fn apply(&self, value: f32) -> f32 {
        // A null sample stays null. Normalizing it would turn "no data" into a
        // pedestal, and the stacker would then average invented background into
        // every pixel the warp left outside the footprint.
        if value == 0.0 {
            return 0.0;
        }
        (value as f64 * self.scale + self.offset) as f32
    }
}

/// Normalization for a whole sequence: coefficients per frame, per channel.
#[derive(Debug, Clone, PartialEq)]
pub struct Normalization {
    /// Indexed `[frame][channel]`.
    pub coefficients: Vec<Vec<NormCoefficients>>,
    pub method: NormMethod,
    /// The frame everything was normalized onto.
    pub reference: usize,
}

impl Normalization {
    /// The do-nothing normalization for a sequence of `frames` × `channels`.
    pub fn identity(frames: usize, channels: usize) -> Self {
        Self {
            coefficients: vec![vec![NormCoefficients::IDENTITY; channels]; frames],
            method: NormMethod::None,
            reference: 0,
        }
    }

    pub fn get(&self, frame: usize, channel: usize) -> NormCoefficients {
        self.coefficients
            .get(frame)
            .and_then(|c| c.get(channel))
            .copied()
            .unwrap_or(NormCoefficients::IDENTITY)
    }

    /// Whether every frame's correction is the identity, so the stacker can skip
    /// the pass entirely.
    pub fn is_identity(&self) -> bool {
        self.coefficients
            .iter()
            .flatten()
            .all(NormCoefficients::is_identity)
    }
}

/// The robust location and scale a frame's coefficients are derived from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelLevel {
    pub location: f64,
    pub scale: f64,
}

impl ChannelLevel {
    /// Measures one channel. `lite` uses the median and `1.5 × MAD` instead of
    /// the clipped biweight pair — a few percent less accurate and a great deal
    /// faster, which is the trade Siril exposes and this mirrors.
    pub fn measure(channel: &[f32], width: usize, height: usize, lite: bool) -> Self {
        if lite {
            let median = stats::median(channel);
            let mad = stats::mad(channel, median);
            let (location, scale) = stats::lite_location_scale(median, mad);
            Self { location, scale }
        } else {
            let stats = ChannelStats::compute(channel, width, height);
            Self {
                location: stats.location,
                scale: stats.scale,
            }
        }
    }
}

/// Coefficients taking `frame` onto `reference`'s photometric scale.
pub fn coefficients(
    method: NormMethod,
    reference: ChannelLevel,
    frame: ChannelLevel,
) -> NormCoefficients {
    // A frame with no measurable level or spread cannot be normalized — a
    // synthetic flat field, or a frame that came out empty. Left alone rather
    // than scaled by a ratio with zero in it.
    if frame.location == 0.0 || frame.scale <= 0.0 || reference.scale <= 0.0 {
        return NormCoefficients::IDENTITY;
    }

    match method {
        NormMethod::None => NormCoefficients::IDENTITY,
        NormMethod::Additive => NormCoefficients {
            scale: 1.0,
            offset: reference.location - frame.location,
        },
        NormMethod::Multiplicative => NormCoefficients {
            scale: reference.location / frame.location,
            offset: 0.0,
        },
        NormMethod::AdditiveScaling => {
            let scale = reference.scale / frame.scale;
            NormCoefficients {
                scale,
                offset: reference.location - frame.location * scale,
            }
        }
        NormMethod::MultiplicativeScaling => {
            // Scale matched first, then the level, with no additive term: the
            // product of the two ratios.
            let scale = (reference.scale / frame.scale) * (reference.location / frame.location);
            NormCoefficients { scale, offset: 0.0 }
        }
    }
}

/// Measures every frame of `set` and computes its coefficients relative to
/// `reference`.
///
/// One frame is resident per worker, and the concurrency comes from the memory
/// budget rather than the core count — the same reasoning as star detection: a
/// 61 MP channel is 244 MB and the robust estimators need two scratch buffers
/// the size of the data.
pub fn compute(
    set: &FrameSet,
    runtime: &Runtime,
    method: NormMethod,
    reference: usize,
    lite: bool,
) -> Result<Normalization, ImageProcError> {
    let dims = set.dims();
    if reference >= set.len() {
        return Err(ImageProcError::Processing(format!(
            "normalization reference {reference} is outside a sequence of {}",
            set.len()
        )));
    }
    if method == NormMethod::None {
        return Ok(Normalization::identity(set.len(), dims.channels));
    }

    let levels = measure_all(set, runtime, lite)?;
    let reference_levels = levels[reference]
        .clone()
        .ok_or_else(|| ImageProcError::Registration("the reference frame is unreadable".into()))?;

    let coefficients = levels
        .iter()
        .map(|frame| match frame {
            // An unreadable frame keeps the identity; it will fail on read in
            // the stacker, which is where that belongs.
            None => vec![NormCoefficients::IDENTITY; dims.channels],
            Some(levels) => levels
                .iter()
                .zip(&reference_levels)
                .map(|(frame, reference)| coefficients(method, *reference, *frame))
                .collect(),
        })
        .collect();

    Ok(Normalization {
        coefficients,
        method,
        reference,
    })
}

/// Per-channel levels for every frame, `None` where a frame could not be read.
fn measure_all(
    set: &FrameSet,
    runtime: &Runtime,
    lite: bool,
) -> Result<Vec<Option<Vec<ChannelLevel>>>, ImageProcError> {
    let dims = set.dims();
    let per_frame_bytes = (dims.width * dims.height * 4 * 3) as u64;
    let in_flight = runtime
        .budget()
        .max_frames_in_flight(per_frame_bytes)
        .max(1);

    let measured: Vec<(usize, Option<Vec<ChannelLevel>>)> = runtime.install(|| {
        (0..set.len())
            .collect::<Vec<_>>()
            .par_chunks(set.len().div_ceil(in_flight).max(1))
            .flat_map_iter(|chunk| {
                chunk.iter().map(move |&index| {
                    let Ok(frame) = set.load(index) else {
                        return (index, None);
                    };
                    let levels = (0..frame.channels())
                        .map(|channel| {
                            ChannelLevel::measure(
                                frame.channel(channel),
                                frame.width(),
                                frame.height(),
                                lite,
                            )
                        })
                        .collect();
                    (index, Some(levels))
                })
            })
            .collect()
    });

    let mut levels = vec![None; set.len()];
    for (index, measurement) in measured {
        levels[index] = measurement;
    }
    Ok(levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE: ChannelLevel = ChannelLevel {
        location: 0.10,
        scale: 0.010,
    };

    #[test]
    fn additive_scaling_maps_a_frames_level_and_spread_onto_the_reference() {
        // A frame with a brighter sky and more contrast: half again the spread.
        let frame = ChannelLevel {
            location: 0.16,
            scale: 0.015,
        };
        let c = coefficients(NormMethod::AdditiveScaling, REFERENCE, frame);

        // Its background must land on the reference's background...
        let corrected_location = frame.location * c.scale + c.offset;
        assert!((corrected_location - REFERENCE.location).abs() < 1e-12);
        // ...and its spread on the reference's spread.
        assert!((frame.scale * c.scale - REFERENCE.scale).abs() < 1e-12);
    }

    #[test]
    fn additive_matches_the_level_and_leaves_the_spread() {
        let frame = ChannelLevel {
            location: 0.16,
            scale: 0.015,
        };
        let c = coefficients(NormMethod::Additive, REFERENCE, frame);
        assert_eq!(c.scale, 1.0);
        assert!((frame.location + c.offset - REFERENCE.location).abs() < 1e-12);
    }

    #[test]
    fn multiplicative_preserves_zero() {
        let frame = ChannelLevel {
            location: 0.16,
            scale: 0.015,
        };
        let c = coefficients(NormMethod::Multiplicative, REFERENCE, frame);
        assert_eq!(c.offset, 0.0);
        assert!((frame.location * c.scale - REFERENCE.location).abs() < 1e-12);
        assert_eq!(c.apply(0.0), 0.0);
    }

    #[test]
    fn the_reference_frame_normalizes_to_itself() {
        for method in [
            NormMethod::Additive,
            NormMethod::Multiplicative,
            NormMethod::AdditiveScaling,
            NormMethod::MultiplicativeScaling,
        ] {
            let c = coefficients(method, REFERENCE, REFERENCE);
            assert!(
                (c.scale - 1.0).abs() < 1e-12 && c.offset.abs() < 1e-12,
                "{method:?}: {c:?}"
            );
        }
    }

    #[test]
    fn a_null_sample_stays_null() {
        // Every coefficient pair must leave an unexposed pixel unexposed: the
        // warp's empty border is "no data", and an offset applied there would
        // become a pedestal the stacker averages in as signal.
        let c = coefficients(
            NormMethod::AdditiveScaling,
            REFERENCE,
            ChannelLevel {
                location: 0.16,
                scale: 0.015,
            },
        );
        assert_ne!(
            c.offset, 0.0,
            "this case needs a non-zero offset to be a test"
        );
        assert_eq!(c.apply(0.0), 0.0);
        assert_ne!(c.apply(0.16), 0.0);
    }

    #[test]
    fn a_degenerate_frame_is_left_alone() {
        // No spread to match, or no level: the ratios would be zero or infinite.
        for frame in [
            ChannelLevel {
                location: 0.0,
                scale: 0.01,
            },
            ChannelLevel {
                location: 0.1,
                scale: 0.0,
            },
        ] {
            let c = coefficients(NormMethod::AdditiveScaling, REFERENCE, frame);
            assert!(c.is_identity(), "{frame:?} gave {c:?}");
        }
    }

    #[test]
    fn the_identity_normalization_is_recognized_as_one() {
        let identity = Normalization::identity(5, 3);
        assert!(identity.is_identity());
        assert_eq!(identity.get(2, 1), NormCoefficients::IDENTITY);
        // Out-of-range lookups are the identity too, rather than a panic: a
        // caller mixing up frame counts gets an unnormalized stack, not a crash
        // halfway through one.
        assert_eq!(identity.get(99, 99), NormCoefficients::IDENTITY);
    }

    /// Approximately normal noise, by summing twelve uniforms — the two scale
    /// estimators are only expected to agree on data that is *roughly* normal,
    /// which is what a sky background is and what the consistency factors in
    /// `stats.rs` are derived for.
    fn normal_noise(count: usize, mean: f32, sigma: f32, seed: u64) -> Vec<f32> {
        let mut state = seed;
        let mut next = || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 33) as f64 / (1u64 << 31) as f64) as f32
        };
        (0..count)
            .map(|_| {
                let sum: f32 = (0..12).map(|_| next()).sum();
                // Twelve uniforms on [0, 1) have mean 6 and variance 1.
                mean + sigma * (sum - 6.0)
            })
            .collect()
    }

    #[test]
    fn measured_levels_describe_a_synthetic_frame() {
        // Background at 0.1 with a known sigma, plus bright pixels that must not
        // move either estimate: that is what "robust" has to mean here.
        let width = 300;
        let height = 200;
        let mut data = normal_noise(width * height, 0.1, 0.01, 0xA5A5_1234);
        for i in 0..500 {
            data[i * 97 % (width * height)] = 0.95;
        }

        let full = ChannelLevel::measure(&data, width, height, false);
        let lite = ChannelLevel::measure(&data, width, height, true);

        assert!((full.location - 0.1).abs() < 0.001, "{full:?}");
        assert!((lite.location - 0.1).abs() < 0.001, "{lite:?}");
        assert!((full.scale - 0.01).abs() / 0.01 < 0.05, "{full:?}");
        // The lite pair is documented as accurate to a few percent of the robust
        // one on real data; this pins that claim down.
        assert!(
            (full.scale - lite.scale).abs() / full.scale < 0.05,
            "full {full:?} vs lite {lite:?}"
        );
    }

    #[test]
    fn coefficients_from_two_measured_frames_align_their_backgrounds() {
        // The end-to-end claim: measure two frames that differ in sky level and
        // transparency, and the correction must bring the second onto the first.
        let (width, height) = (300, 200);
        let reference = normal_noise(width * height, 0.10, 0.010, 0xBEEF);
        let brighter: Vec<f32> = normal_noise(width * height, 0.10, 0.010, 0xFEED)
            .iter()
            .map(|v| v * 1.4 + 0.05)
            .collect();

        let reference_level = ChannelLevel::measure(&reference, width, height, false);
        let frame_level = ChannelLevel::measure(&brighter, width, height, false);
        let c = coefficients(NormMethod::AdditiveScaling, reference_level, frame_level);

        let corrected: Vec<f32> = brighter.iter().map(|v| c.apply(*v)).collect();
        let after = ChannelLevel::measure(&corrected, width, height, false);
        assert!(
            (after.location - reference_level.location).abs() < 1e-4,
            "{after:?}"
        );
        assert!((after.scale - reference_level.scale).abs() / reference_level.scale < 0.01);
    }
}
