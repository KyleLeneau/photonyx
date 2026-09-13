//! The two-pass registration loop (ADR 007, P9-T2 / P9-T4).
//!
//! One pass over the sequence detecting stars, then the reference is chosen from
//! what that measured, then a second pass computing every frame's transform onto
//! it. The alternative — register each frame to the first as you go — is one
//! pass cheaper and picks the reference before knowing anything about the
//! sequence, which is the wrong trade when the first frame is the one the guider
//! was still settling on.
//!
//! Concurrency here is bounded by *memory*, not by core count. A 61 MP frame is
//! 244 MB as unit-range `f32` and star detection needs the whole channel
//! resident, so sixteen cores would want 4 GB of frames in flight; the budget
//! decides how many frames are open at once and the pool fills the rest of the
//! cores with work inside each frame.

use std::path::PathBuf;

use rayon::prelude::*;

use super::framing::{
    FrameQuality, Framing, OutputGeometry, frame_qualities, output_geometry, select_reference,
};
use super::psf::{PsfStar, fwhm_stats};
use super::starfind::{StarfindOptions, find_stars_reported};
use super::starmatch::{MatchOptions, register_pair};
use super::transform::{RansacOptions, Transform, TransformKind};
use crate::error::ImageProcError;
use crate::runtime::{ImageDims, Runtime};
use crate::sequence::FrameSet;

/// Passes of the whole loop before giving up.
///
/// A trial that registers too few frames is retried with the star finder opened
/// up — a lower threshold, then relaxed shape checks — because the usual cause
/// is a sequence whose stars sit just under the detection threshold, not one
/// that cannot be registered. Five is Siril's figure and more than enough: the
/// schedule below has exhausted everything useful by the fourth.
pub const MAX_TRIALS_2PASS: usize = 5;

/// Reference candidates a trial tries before re-detecting with opened-up
/// parameters.
///
/// Reference selection ranks each frame on its *own* quality and cannot see
/// whether the rest of the sequence agrees with it, so a frame that is sharp and
/// star-rich but shows a different patch of sky — a pointing slip, a frame from
/// another target that landed in the folder — can win the ranking and leave
/// every other frame unregistered. Trying the next-best candidate costs one more
/// matching pass over already-detected star lists, which is far cheaper than
/// re-running detection, and it is the difference between stacking one frame and
/// stacking the sequence.
const MAX_REFERENCE_CANDIDATES: usize = 3;

/// Fraction of frames that must register before a trial is accepted.
///
/// Not 100%: a real sequence contains frames ruined by cloud or a satellite, and
/// re-running detection on the whole set because one frame in fifty is
/// unusable would be pure cost. Below this, though, the *parameters* are the
/// likely problem rather than the frames.
const MIN_REGISTERED_FRACTION: f64 = 0.9;

/// Why a frame is not in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Excluded {
    /// Star detection found nothing fittable.
    NoStars,
    /// Fewer stars than matching needs.
    TooFewStars,
    /// Stars found, but no consistent correspondence with the reference.
    MatchFailed,
    /// The frame could not be read.
    Unreadable,
}

impl std::fmt::Display for Excluded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Excluded::NoStars => "no stars detected",
            Excluded::TooFewStars => "too few stars to match",
            Excluded::MatchFailed => "could not be matched to the reference",
            Excluded::Unreadable => "could not be read",
        };
        f.write_str(text)
    }
}

/// One frame's registration outcome.
#[derive(Debug, Clone)]
pub struct FrameRegistration {
    pub index: usize,
    pub path: PathBuf,
    /// Maps this frame's pixels onto the reference frame. `None` for an excluded
    /// frame.
    pub transform: Option<Transform>,
    /// Why it is excluded, or `None` if it is not.
    pub excluded: Option<Excluded>,
    pub stars: usize,
    /// Mean major-axis FWHM, pixels.
    pub fwhm: f64,
    pub roundness: f64,
    /// Correspondences supporting the transform, and their RMS residual.
    pub inliers: usize,
    pub rmse: f64,
}

impl FrameRegistration {
    pub fn included(&self) -> bool {
        self.transform.is_some() && self.excluded.is_none()
    }
}

/// What a registration run produced.
#[derive(Debug, Clone)]
pub struct Registration {
    /// Index of the frame everything else is registered to.
    pub reference: usize,
    /// One entry per input frame, in sequence order.
    pub frames: Vec<FrameRegistration>,
    pub geometry: OutputGeometry,
    /// Trials the loop needed. More than one means the first parameters found
    /// too few frames.
    pub trials: usize,
    /// Star-finder options the accepted trial used, which may not be the ones
    /// passed in.
    pub starfind: StarfindOptions,
}

impl Registration {
    pub fn included(&self) -> impl Iterator<Item = &FrameRegistration> {
        self.frames.iter().filter(|f| f.included())
    }

    pub fn included_count(&self) -> usize {
        self.included().count()
    }

    /// Mean FWHM over the included frames — the sequence's seeing.
    pub fn mean_fwhm(&self) -> f64 {
        let included: Vec<f64> = self.included().map(|f| f.fwhm).collect();
        if included.is_empty() {
            return 0.0;
        }
        included.iter().sum::<f64>() / included.len() as f64
    }

    /// The transform to apply when warping frame `index`, including the shift
    /// that places the chosen output region at the output's origin.
    pub fn output_transform(&self, index: usize) -> Option<Transform> {
        let frame = self.frames.get(index)?;
        let transform = frame.transform?;
        self.geometry.shift().compose(&transform)
    }
}

/// How a registration run is configured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegistrationOptions {
    pub starfind: StarfindOptions,
    pub matching: MatchOptions,
    pub ransac: RansacOptions,
    /// Transform class to fit. Siril's default, and ours, is the homography
    /// (ADR 007 D10).
    pub kind: TransformKind,
    pub framing: Framing,
    /// Channel to detect stars in. `None` picks green for colour frames and the
    /// only channel for mono, which is Siril's rule (D10) and the right one:
    /// green carries twice the samples of red or blue on a Bayer sensor.
    pub layer: Option<usize>,
}

impl Default for RegistrationOptions {
    fn default() -> Self {
        Self {
            starfind: StarfindOptions::default(),
            matching: MatchOptions::default(),
            ransac: RansacOptions::default(),
            kind: TransformKind::Homography,
            framing: Framing::default(),
            layer: None,
        }
    }
}

impl RegistrationOptions {
    /// Which channel to register on, for a given frame shape.
    fn layer(&self, dims: &ImageDims) -> usize {
        self.layer
            .unwrap_or(if dims.channels >= 3 { 1 } else { 0 })
            .min(dims.channels - 1)
    }

    /// The star-finder settings for trial `trial`, counting from zero.
    ///
    /// Trial 0 is what the caller asked for. Later trials lower the detection
    /// threshold, then relax the shape checks — in that order, because a
    /// sequence short of stars is far more often under-detected than
    /// mis-shaped, and relaxing the shape checks first would admit cosmic rays
    /// as stars while the real ones were still below threshold.
    fn starfind_for(&self, trial: usize) -> StarfindOptions {
        let mut options = self.starfind;
        match trial {
            0 => {}
            1 => options.sigma *= 0.6,
            2 => options.sigma *= 0.35,
            3 => {
                options.sigma *= 0.35;
                options.relax_checks = true;
            }
            _ => {
                options.sigma *= 0.2;
                options.relax_checks = true;
                options.roundness = 0.1;
            }
        }
        options.normalized()
    }
}

/// Detects stars in every frame, picks a reference, and registers the rest
/// to it.
///
/// Runs inside `runtime`'s pool, so the core and memory limits apply to the
/// whole loop.
pub fn register_sequence(
    set: &FrameSet,
    runtime: &Runtime,
    options: &RegistrationOptions,
) -> Result<Registration, ImageProcError> {
    if set.is_empty() {
        return Err(ImageProcError::EmptySequence);
    }
    let layer = options.layer(&set.dims());

    let mut last_attempt: Option<Registration> = None;
    for trial in 0..MAX_TRIALS_2PASS {
        let starfind = options.starfind_for(trial);
        // Detection is the expensive half and does not depend on the reference,
        // so it happens once per trial and every reference candidate reuses it.
        let star_lists = detect_all(set, runtime, &starfind, layer)?;
        let views: Vec<&[PsfStar]> = star_lists
            .iter()
            .map(|s| s.as_deref().unwrap_or(&[]))
            .collect();
        let qualities = frame_qualities(&views, starfind.max_stars);

        let mut rejected_references: Vec<usize> = Vec::new();
        for _ in 0..MAX_REFERENCE_CANDIDATES {
            let candidates: Vec<FrameQuality> = qualities
                .iter()
                .copied()
                .filter(|q| !rejected_references.contains(&q.index))
                .collect();
            let Some(reference) = select_reference(&candidates) else {
                break;
            };

            let attempt = build_registration(
                set,
                runtime,
                options,
                &star_lists,
                reference,
                &starfind,
                trial + 1,
            )?;

            let registered = attempt.included_count() as f64 / set.len() as f64;
            if registered >= MIN_REGISTERED_FRACTION {
                return Ok(attempt);
            }
            // Keep the best attempt so far: a later, more permissive trial can
            // do worse than an earlier one by admitting junk detections.
            let improved = last_attempt
                .as_ref()
                .is_none_or(|best| attempt.included_count() > best.included_count());
            if improved {
                last_attempt = Some(attempt);
            }
            rejected_references.push(reference);
        }
    }

    // Every trial fell short. The best attempt is still returned rather than an
    // error: a sequence where 60% of frames registered is a usable stack of 60%
    // of the frames, and the caller has the per-frame reasons to report.
    last_attempt.ok_or_else(|| {
        ImageProcError::Registration("no frame in the sequence could be registered".into())
    })
}

/// Registers every frame of an already-detected sequence onto `reference`.
fn build_registration(
    set: &FrameSet,
    runtime: &Runtime,
    options: &RegistrationOptions,
    star_lists: &[Option<Vec<PsfStar>>],
    reference: usize,
    starfind: &StarfindOptions,
    trial: usize,
) -> Result<Registration, ImageProcError> {
    let reference_stars = star_lists[reference]
        .as_deref()
        .expect("the reference was chosen from frames that have stars");

    // Matching is independent per frame and is pure CPU over two star lists, so
    // it parallelizes over frames without a memory cost worth budgeting.
    let mut frames: Vec<FrameRegistration> = runtime.install(|| {
        star_lists
            .par_iter()
            .enumerate()
            .map(|(index, stars)| {
                let path = set.frames()[index].path.clone();
                let Some(stars) = stars.as_deref() else {
                    return FrameRegistration {
                        index,
                        path,
                        transform: None,
                        excluded: Some(Excluded::Unreadable),
                        stars: 0,
                        fwhm: 0.0,
                        roundness: 0.0,
                        inliers: 0,
                        rmse: 0.0,
                    };
                };

                let stats = fwhm_stats(stars);
                let (fwhm, roundness) = stats
                    .map(|s| (s.mean, s.mean_roundness))
                    .unwrap_or((0.0, 0.0));

                let mut registration = FrameRegistration {
                    index,
                    path,
                    transform: None,
                    excluded: None,
                    stars: stars.len(),
                    fwhm,
                    roundness,
                    inliers: 0,
                    rmse: 0.0,
                };

                if index == reference {
                    // The reference maps onto itself, and is included without
                    // being matched against anything.
                    registration.transform = Some(Transform::identity());
                    registration.inliers = stars.len();
                    return registration;
                }
                if stars.is_empty() {
                    registration.excluded = Some(Excluded::NoStars);
                    return registration;
                }
                if stars.len() < options.matching.min_pairs.max(3) {
                    registration.excluded = Some(Excluded::TooFewStars);
                    return registration;
                }

                match register_pair(
                    stars,
                    reference_stars,
                    options.kind,
                    &options.matching,
                    &options.ransac,
                ) {
                    Ok(fit) => {
                        registration.transform = Some(fit.transform);
                        registration.inliers = fit.inlier_count();
                        registration.rmse = fit.rmse;
                    }
                    Err(_) => registration.excluded = Some(Excluded::MatchFailed),
                }
                registration
            })
            .collect()
    });
    frames.sort_by_key(|f| f.index);

    let transforms: Vec<Option<Transform>> = frames.iter().map(|f| f.transform).collect();
    let geometry = output_geometry(options.framing, set.dims(), &transforms)?;

    Ok(Registration {
        reference,
        frames,
        geometry,
        trials: trial,
        starfind: *starfind,
    })
}

/// Star lists for every frame, `None` where the frame could not be read.
///
/// Frames are processed in chunks sized by the memory budget: each worker holds
/// one decoded channel, so the chunk count is the number of frames the budget
/// allows in flight rather than the core count.
fn detect_all(
    set: &FrameSet,
    runtime: &Runtime,
    starfind: &StarfindOptions,
    layer: usize,
) -> Result<Vec<Option<Vec<PsfStar>>>, ImageProcError> {
    let dims = set.dims();
    // One channel as f32, plus the two scratch buffers the statistics and the
    // fitter need. Measured rather than guessed: `ChannelStats::compute` holds
    // two buffers the size of the non-null data (see `stats.rs`).
    let channel_bytes = (dims.width * dims.height * 4 * 3) as u64;
    let in_flight = runtime.budget().max_frames_in_flight(channel_bytes).max(1);

    let results: Vec<(usize, Option<Vec<PsfStar>>)> = runtime.install(|| {
        (0..set.len())
            .collect::<Vec<_>>()
            .par_chunks(set.len().div_ceil(in_flight).max(1))
            .flat_map_iter(|chunk| {
                chunk.iter().map(move |&index| {
                    let Ok(frame) = set.load(index) else {
                        return (index, None);
                    };
                    let layer = layer.min(frame.channels() - 1);
                    let (stars, _) = find_stars_reported(
                        frame.channel(layer),
                        frame.width(),
                        frame.height(),
                        starfind,
                        layer,
                    );
                    (index, Some(stars))
                })
            })
            .collect()
    });

    let mut lists: Vec<Option<Vec<PsfStar>>> = vec![None; set.len()];
    for (index, stars) in results {
        lists[index] = stars;
    }
    Ok(lists)
}
