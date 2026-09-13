//! Star candidate detection (ADR 007, P7-T1).
//!
//! Detection is the cheap half of star finding: it produces a list of pixels
//! that *might* be stars, ordered brightest first, which the profile fitter
//! then fits and filters. Keeping the two apart matters for cost — fitting is two
//! orders of magnitude more expensive per candidate than detecting, so the
//! candidate list is where a frame's work is decided.
//!
//! The method is the standard one for an uncrowded stellar field, described in
//! Stetson, *DAOPHOT: A Computer Program for Crowded-Field Stellar
//! Photometry*, PASP 99, 1987, §II: threshold the image at a fixed multiple of
//! the background noise above the background level, then keep the pixels that
//! are the local maximum of a box whose half-width is the expected star
//! radius. The threshold is expressed in background *noise* rather than in the
//! frame's standard deviation because the latter is dominated by the stars
//! themselves on a rich field, which would make the threshold depend on how
//! many stars there are.

use rayon::prelude::*;

use super::psf::{self, Profile, PsfStar, Rejected};
use crate::stats;

/// Multiplier on `sigma · bgnoise` that sets the detection threshold above the
/// background level.
///
/// At 5 sigma a pure-noise frame of 60 megapixels yields a handful of false
/// candidates, all of which the PSF rejection rules discard;
/// at 3 it yields thousands, and every one of them costs a fit.
const NOISE_SIGMAS: f64 = 5.0;

/// Half-width of the default search box.
///
/// Eleven pixels across holds a seeing-limited star and enough surrounding sky
/// to fit a background against, at the sampling typical of an amateur
/// refractor; it is also what Siril defaults to, so a native run and an
/// `--engine siril` run start from the same field.
pub const DEFAULT_RADIUS: usize = 5;

/// Smallest and largest accepted search radius.
///
/// Below 3 px the box is too small to contain a fittable profile; above 50 px
/// the local-maximum test starts merging genuinely separate stars.
pub const MIN_RADIUS: usize = 3;
pub const MAX_RADIUS: usize = 50;

/// Two detections closer than this fraction of the search radius are the same
/// star seen twice.
///
/// The local-maximum scan cannot itself produce two maxima within one radius
/// of each other, so this only bites after fitting, when two neighbouring
/// candidates converge onto one centroid — a bright star with a faint
/// companion inside its halo, or a saturated core with a flat top.
pub const MAX_RADIUS_RATIO_DUP: f64 = 0.5;

/// Star detection and fitting parameters.
///
/// The defaults are Siril's, so a native run and an `--engine siril` run
/// select comparable star sets; they are also sensible on their own terms for
/// typical seeing at typical sampling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarfindOptions {
    /// Half-width of the search box, in pixels.
    pub radius: usize,
    /// Detection threshold in units of `NOISE_SIGMAS · bgnoise`.
    pub sigma: f64,
    /// Minimum accepted roundness (minor/major axis ratio).
    pub roundness: f64,
    /// Maximum accepted roundness. 1.0 admits perfectly circular stars.
    pub max_roundness: f64,
    /// Amplitude bounds on the fitted profile, in unit-range intensity.
    /// `max_amplitude` of 0 means "no upper bound".
    pub min_amplitude: f32,
    pub max_amplitude: f32,
    /// Refitting passes the solver runs per candidate.
    pub convergence: usize,
    /// Accept structures that are not star-shaped: drops the roundness and
    /// RMSE checks. Needed for comets and for badly trailed subs.
    pub relax_checks: bool,
    /// Cap on the number of fitted stars returned.
    pub max_stars: usize,
    /// Which analytic profile the fitter uses.
    pub profile: Profile,
    /// Smallest Moffat exponent a star may be fitted with. Ignored for a
    /// Gaussian fit.
    pub min_beta: f64,
}

impl Default for StarfindOptions {
    fn default() -> Self {
        Self {
            radius: DEFAULT_RADIUS,
            sigma: 1.0,
            roundness: 0.5,
            max_roundness: 1.0,
            min_amplitude: 0.0,
            max_amplitude: 0.0,
            convergence: 1,
            relax_checks: false,
            max_stars: MAX_STARS_FITTED,
            profile: Profile::Gaussian,
            min_beta: MIN_BETA,
        }
    }
}

/// Default floor on the Moffat exponent.
///
/// Below roughly 1.5 the Moffat profile's wings carry so much of its flux that
/// its integral is only weakly constrained by a finite box, and the fit trades
/// amplitude against beta without improving the residual. Gaussian fits ignore
/// it.
pub const MIN_BETA: f64 = 1.5;

/// Bounds on the candidate batch the fitter works through at a time.
///
/// Small batches would leave the thread pool idle between them; enormous ones
/// would defeat the point of batching, which is to stop early once the star cap
/// is met.
const MIN_FIT_CHUNK: usize = 1024;
const MAX_FIT_CHUNK: usize = 65_536;

/// Ceiling on fitted stars per frame.
///
/// Registration converges on far fewer than this, and the triangle matching of
/// Phase 8 is cubic in the star count, so an unbounded list on a rich field
/// would dominate the whole pipeline's runtime for no gain in accuracy.
pub const MAX_STARS_FITTED: usize = 2000;

impl StarfindOptions {
    /// Clamps the radius into the supported range and rejects a
    /// non-positive sigma.
    pub fn normalized(mut self) -> Self {
        self.radius = self.radius.clamp(MIN_RADIUS, MAX_RADIUS);
        if !self.sigma.is_finite() || self.sigma <= 0.0 {
            self.sigma = 1.0;
        }
        self
    }

    /// Selects the Moffat profile, which is off by default.
    pub fn with_moffat(mut self, min_beta: f64) -> Self {
        self.profile = Profile::Moffat;
        self.min_beta = min_beta;
        self
    }
}

/// A pixel that might be a star: the local maximum of its box, above
/// threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub x: usize,
    pub y: usize,
    /// The peak sample value, unit-range.
    pub peak: f32,
}

/// The background level and noise a detection threshold is built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Background {
    /// Background level — the frame median.
    pub level: f32,
    /// Background noise, from first-order row differences.
    pub noise: f64,
}

impl Background {
    /// Measures the background of one channel, row-major.
    pub fn measure(channel: &[f32], width: usize, height: usize) -> Self {
        Self {
            level: stats::median(channel),
            noise: stats::background_noise(channel, width, height),
        }
    }

    /// The intensity a pixel must exceed to become a candidate.
    pub fn threshold(&self, sigma: f64) -> f32 {
        self.level + (NOISE_SIGMAS * sigma * self.noise) as f32
    }
}

/// Finds star candidates in one channel, row-major, brightest first.
///
/// Candidates come no closer than `radius` to the frame edge: a star whose box
/// runs off the frame has no background annulus to fit against, and its
/// centroid would be biased inward by the truncation.
pub fn detect_candidates(
    channel: &[f32],
    width: usize,
    height: usize,
    options: &StarfindOptions,
) -> Vec<Candidate> {
    let background = Background::measure(channel, width, height);
    detect_candidates_with(channel, width, height, options, &background)
}

/// [`detect_candidates`] against an already-measured background.
///
/// Two-pass registration measures a frame's background once and uses it for
/// both detection and reference scoring; re-deriving it here would double the
/// cost of the cheap half of star finding.
pub fn detect_candidates_with(
    channel: &[f32],
    width: usize,
    height: usize,
    options: &StarfindOptions,
    background: &Background,
) -> Vec<Candidate> {
    let options = options.normalized();
    let radius = options.radius;
    if width <= 2 * radius || height <= 2 * radius {
        return Vec::new();
    }
    // A frame with no measurable noise — a synthetic constant, a fully
    // saturated sub — has no scale to threshold against, and every pixel
    // above the median would be a candidate.
    if background.noise <= 0.0 {
        return Vec::new();
    }
    let threshold = background.threshold(options.sigma);

    let mut candidates: Vec<Candidate> = (radius..height - radius)
        .into_par_iter()
        .flat_map_iter(|y| {
            let row = &channel[y * width..(y + 1) * width];
            (radius..width - radius).filter_map(move |x| {
                let peak = row[x];
                // The threshold test comes first and rejects all but a small
                // fraction of a percent of pixels, so the box scan below —
                // which is the expensive part — runs only on those few.
                if peak <= threshold {
                    return None;
                }
                is_local_max(channel, width, x, y, radius).then_some(Candidate { x, y, peak })
            })
        })
        .collect();

    // Brightest first, which is "magnitude-ordered": magnitude runs backwards
    // to flux. Ties broken by position so the list is reproducible regardless
    // of how the parallel scan interleaved.
    candidates.sort_by(|a, b| {
        b.peak
            .total_cmp(&a.peak)
            .then_with(|| (a.y, a.x).cmp(&(b.y, b.x)))
    });
    candidates
}

/// Whether `(x, y)` is the maximum of the `(2·radius+1)²` box centred on it.
///
/// A plateau — a saturated core, or two equal samples — would otherwise
/// produce one candidate per pixel of it, so ties are resolved in favour of
/// the first pixel in scan order: a sample equal to the centre disqualifies it
/// only if it comes earlier.
fn is_local_max(channel: &[f32], width: usize, x: usize, y: usize, radius: usize) -> bool {
    let peak = channel[y * width + x];
    let here = y * width + x;
    for yy in y - radius..=y + radius {
        let row = &channel[yy * width..yy * width + x + radius + 1];
        for (xx, &v) in row.iter().enumerate().skip(x - radius) {
            if v > peak {
                return false;
            }
            if v == peak && yy * width + xx < here {
                return false;
            }
        }
    }
    true
}

/// Drops every detection that lies within `MAX_RADIUS_RATIO_DUP · radius` of
/// an earlier one, keeping the earlier — which, on a brightest-first list, is
/// the brighter.
///
/// `position` reads a centre out of whatever the caller's element type is, so
/// this serves both the pixel-grid candidates and the sub-pixel fitted stars.
pub fn suppress_duplicates<T>(
    items: Vec<T>,
    radius: usize,
    position: impl Fn(&T) -> (f64, f64),
) -> Vec<T> {
    let min_distance = MAX_RADIUS_RATIO_DUP * radius as f64;
    let min_sq = min_distance * min_distance;

    let mut kept: Vec<T> = Vec::with_capacity(items.len());
    let mut centres: Vec<(f64, f64)> = Vec::with_capacity(items.len());
    for item in items {
        let (x, y) = position(&item);
        let duplicate = centres.iter().any(|&(kx, ky)| {
            let (dx, dy) = (x - kx, y - ky);
            dx * dx + dy * dy < min_sq
        });
        if !duplicate {
            centres.push((x, y));
            kept.push(item);
        }
    }
    kept
}

/// What a star-finding pass did, beyond the stars it kept.
///
/// The rejection counts are the diagnostic a thin star list needs: they say
/// whether the threshold was too high (few candidates), the seeing too poor
/// (roundness), or the frame not star-shaped at all (residual).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StarfindReport {
    pub candidates: usize,
    /// Candidates actually fitted. Lower than `candidates` when the star cap
    /// was reached before the list was exhausted; the rejection counts below
    /// cover these only.
    pub considered: usize,
    pub fitted: usize,
    pub kept: usize,
    pub duplicates: usize,
    pub truncated: usize,
    pub rejected_amplitude: usize,
    pub rejected_off_centre: usize,
    pub rejected_width: usize,
    pub rejected_roundness: usize,
    pub rejected_residual: usize,
    pub rejected_beta: usize,
    pub failed_to_converge: usize,
}

impl StarfindReport {
    fn count(&mut self, reason: Rejected) {
        let slot = match reason {
            Rejected::Amplitude => &mut self.rejected_amplitude,
            Rejected::OffCentre => &mut self.rejected_off_centre,
            Rejected::Width => &mut self.rejected_width,
            Rejected::Roundness => &mut self.rejected_roundness,
            Rejected::Residual => &mut self.rejected_residual,
            Rejected::Beta => &mut self.rejected_beta,
        };
        *slot += 1;
    }
}

/// Detects, fits, and filters the stars of one channel, row-major.
///
/// The result is brightest-first and no longer than `options.max_stars`.
pub fn find_stars(
    channel: &[f32],
    width: usize,
    height: usize,
    options: &StarfindOptions,
) -> Vec<PsfStar> {
    find_stars_reported(channel, width, height, options, 0).0
}

/// [`find_stars`] with the pass's bookkeeping, and the channel index to stamp
/// on each star.
pub fn find_stars_reported(
    channel: &[f32],
    width: usize,
    height: usize,
    options: &StarfindOptions,
    layer: usize,
) -> (Vec<PsfStar>, StarfindReport) {
    let options = options.normalized();
    let candidates = {
        let _span = crate::profile::scope("starfind.detect");
        detect_candidates(channel, width, height, &options)
    };

    let mut report = StarfindReport {
        candidates: candidates.len(),
        ..Default::default()
    };

    // Candidates are fitted in brightest-first chunks and the pass stops once
    // the cap is satisfied. On a rich 61 MP frame detection yields well over a
    // hundred thousand candidates against a default cap of two thousand stars,
    // and fitting is two orders of magnitude dearer per candidate than
    // detecting — so fitting all of them would spend most of a frame's budget
    // producing stars that are then discarded. The order is the deterministic
    // brightest-first order, so which stars survive does not depend on where
    // the chunk boundaries fell.
    let chunk = options
        .max_stars
        .saturating_mul(2)
        .clamp(MIN_FIT_CHUNK, MAX_FIT_CHUNK);

    let mut stars: Vec<PsfStar> = Vec::new();
    {
        let _span = crate::profile::scope("starfind.fit");
        for batch in candidates.chunks(chunk) {
            // Every candidate in a batch is independent, which is where the
            // parallelism belongs. Outcomes are collected rather than filtered in
            // place so the rejection counts survive and the order stays the one
            // detection produced — that is what makes the pass reproducible.
            let outcomes: Vec<Result<PsfStar, Option<Rejected>>> = batch
                .par_iter()
                .map(|candidate| {
                    let Some(mut star) =
                        psf::fit_candidate(channel, width, height, candidate, &options)
                    else {
                        return Err(None);
                    };
                    star.layer = layer;
                    match psf::accept(&star, candidate, &options) {
                        Ok(()) => Ok(star),
                        Err(reason) => Err(Some(reason)),
                    }
                })
                .collect();

            report.considered += batch.len();
            for outcome in outcomes {
                match outcome {
                    Ok(star) => stars.push(star),
                    Err(None) => report.failed_to_converge += 1,
                    Err(Some(reason)) => report.count(reason),
                }
            }
            if stars.len() >= options.max_stars {
                break;
            }
        }
    }
    report.fitted = stars.len();

    let before = stars.len();
    let mut stars = suppress_duplicates(stars, options.radius, |s| (s.x0, s.y0));
    report.duplicates = before - stars.len();

    if stars.len() > options.max_stars {
        report.truncated = stars.len() - options.max_stars;
        stars.truncate(options.max_stars);
    }
    report.kept = stars.len();
    (stars, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of faint noise with Gaussian blobs stamped on it, which is the
    /// minimum a detection test needs: a measurable background noise floor to
    /// set a threshold from, and peaks well above it.
    fn frame_with_blobs(width: usize, height: usize, blobs: &[(f64, f64, f32)]) -> Vec<f32> {
        let mut data = vec![0.0f32; width * height];
        for (i, v) in data.iter_mut().enumerate() {
            // A deterministic, spatially incoherent ripple standing in for
            // read noise: pixel-to-pixel variation is what `background_noise`
            // measures.
            *v = 0.01 + 0.0005 * ((i as f32 * 12.9898).sin());
        }
        for &(cx, cy, amplitude) in blobs {
            let sigma = 1.5f64;
            for y in 0..height {
                for x in 0..width {
                    let (dx, dy) = (x as f64 - cx, y as f64 - cy);
                    let r2 = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                    if r2 < 25.0 {
                        data[y * width + x] += amplitude * (-r2).exp() as f32;
                    }
                }
            }
        }
        data
    }

    /// The same renderer the PSF tests use, kept separate because these tests
    /// need many stars rather than one precise one.
    fn frame_with_gaussians(
        width: usize,
        height: usize,
        stars: &[(f64, f64, f64, f64, f64)],
    ) -> Vec<f32> {
        let mut data: Vec<f32> = (0..width * height)
            .map(|i| 0.01 + 0.0005 * ((i as f32 * 12.9898).sin()))
            .collect();
        for &(x0, y0, amplitude, sx, sy) in stars {
            for y in 0..height {
                for x in 0..width {
                    let (dx, dy) = (x as f64 - x0, y as f64 - y0);
                    let q = 0.5 * (dx * dx / (sx * sx) + dy * dy / (sy * sy));
                    if q < 30.0 {
                        data[y * width + x] += (amplitude * (-q).exp()) as f32;
                    }
                }
            }
        }
        data
    }

    #[test]
    fn keeps_round_stars_and_reports_the_pass() {
        let truth = [
            (30.3, 40.6, 0.30, 2.0, 2.0),
            (70.1, 20.9, 0.60, 2.0, 2.0),
            (50.5, 75.2, 0.45, 2.0, 2.0),
        ];
        let data = frame_with_gaussians(100, 100, &truth);

        let (stars, report) = find_stars_reported(&data, 100, 100, &StarfindOptions::default(), 0);

        assert_eq!(report.candidates, 3, "{report:?}");
        assert_eq!(stars.len(), 3, "{report:?}");
        assert_eq!(report.kept, 3);
        // Brightest first survives the fit and filter.
        assert!((stars[0].x0 - 70.1).abs() < 0.02, "{:?}", stars[0]);
        for star in &stars {
            assert!(star.roundness > 0.95, "{star:?}");
        }
    }

    #[test]
    fn a_trailed_star_is_rejected_unless_checks_are_relaxed() {
        // 4:1 elongation, well below the 0.5 roundness floor.
        let data = frame_with_gaussians(100, 100, &[(50.0, 50.0, 0.5, 4.0, 1.0)]);

        let strict = StarfindOptions::default();
        let (stars, report) = find_stars_reported(&data, 100, 100, &strict, 0);
        assert!(stars.is_empty(), "{stars:?}");
        assert_eq!(report.rejected_roundness, 1, "{report:?}");

        let relaxed = StarfindOptions {
            relax_checks: true,
            ..strict
        };
        let (stars, _) = find_stars_reported(&data, 100, 100, &relaxed, 0);
        assert_eq!(stars.len(), 1);
        assert!((stars[0].x0 - 50.0).abs() < 0.05, "{:?}", stars[0]);
    }

    #[test]
    fn a_hot_pixel_is_rejected_as_too_narrow() {
        let mut data = frame_with_gaussians(100, 100, &[]);
        data[50 * 100 + 50] = 0.9;

        let (stars, report) = find_stars_reported(&data, 100, 100, &StarfindOptions::default(), 0);
        assert!(stars.is_empty(), "{stars:?}");
        assert_eq!(report.candidates, 1);
        assert_eq!(
            report.rejected_width + report.rejected_residual,
            1,
            "{report:?}"
        );
    }

    #[test]
    fn amplitude_bounds_filter_the_star_list() {
        let data = frame_with_gaussians(
            100,
            100,
            &[(30.0, 30.0, 0.1, 2.0, 2.0), (70.0, 70.0, 0.8, 2.0, 2.0)],
        );

        let options = StarfindOptions {
            min_amplitude: 0.3,
            ..Default::default()
        };
        let (stars, report) = find_stars_reported(&data, 100, 100, &options, 0);
        assert_eq!(stars.len(), 1, "{report:?}");
        assert!(stars[0].amplitude > 0.3);

        let options = StarfindOptions {
            max_amplitude: 0.3,
            ..Default::default()
        };
        let (stars, _) = find_stars_reported(&data, 100, 100, &options, 0);
        assert_eq!(stars.len(), 1);
        assert!(stars[0].amplitude < 0.3);
    }

    #[test]
    fn the_star_list_is_capped_at_max_stars() {
        let truth: Vec<(f64, f64, f64, f64, f64)> = (0..25)
            .map(|i| {
                let (gx, gy) = (i % 5, i / 5);
                (
                    20.0 + 15.0 * gx as f64,
                    20.0 + 15.0 * gy as f64,
                    0.2 + 0.02 * i as f64,
                    2.0,
                    2.0,
                )
            })
            .collect();
        let data = frame_with_gaussians(120, 120, &truth);

        let options = StarfindOptions {
            max_stars: 10,
            ..Default::default()
        };
        let (stars, report) = find_stars_reported(&data, 120, 120, &options, 0);
        assert_eq!(stars.len(), 10, "{report:?}");
        assert!(report.truncated > 0, "{report:?}");
        // The cap keeps the brightest ten, whose true amplitudes run from
        // 0.5 up; the eleventh would be 0.48.
        assert!(stars.iter().all(|s| s.amplitude > 0.49), "{stars:?}");
    }

    #[test]
    fn the_layer_index_is_stamped_on_each_star() {
        let data = frame_with_gaussians(100, 100, &[(50.0, 50.0, 0.5, 2.0, 2.0)]);
        let (stars, _) = find_stars_reported(&data, 100, 100, &StarfindOptions::default(), 2);
        assert_eq!(stars.len(), 1);
        assert_eq!(stars[0].layer, 2);
    }

    #[test]
    fn finds_every_blob_once_brightest_first() {
        let blobs = [(30.0, 40.0, 0.2f32), (70.0, 20.0, 0.6), (50.0, 75.0, 0.4)];
        let data = frame_with_blobs(100, 100, &blobs);

        let found = detect_candidates(&data, 100, 100, &StarfindOptions::default());

        assert_eq!(found.len(), 3, "one candidate per blob: {found:?}");
        let peaks: Vec<f32> = found.iter().map(|c| c.peak).collect();
        assert!(peaks.windows(2).all(|w| w[0] >= w[1]), "{peaks:?}");
        // Brightest first, so the 0.6 blob leads.
        assert_eq!((found[0].x, found[0].y), (70, 20));
    }

    #[test]
    fn rejects_candidates_inside_the_border_margin() {
        // Centred 4 px in, inside the default 10 px margin.
        let data = frame_with_blobs(100, 100, &[(4.0, 50.0, 0.6)]);
        let found = detect_candidates(&data, 100, 100, &StarfindOptions::default());
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_saturated_plateau_yields_one_candidate() {
        let mut data = frame_with_blobs(100, 100, &[]);
        for y in 49..=51 {
            for x in 49..=51 {
                data[y * 100 + x] = 1.0;
            }
        }
        let found = detect_candidates(&data, 100, 100, &StarfindOptions::default());
        assert_eq!(found.len(), 1, "{found:?}");
        // Ties resolve to the first pixel in scan order.
        assert_eq!((found[0].x, found[0].y), (49, 49));
    }

    #[test]
    fn a_noiseless_frame_yields_nothing() {
        let data = vec![0.5f32; 100 * 100];
        let found = detect_candidates(&data, 100, 100, &StarfindOptions::default());
        assert!(found.is_empty());
    }

    #[test]
    fn duplicate_suppression_keeps_the_first_of_a_close_pair() {
        let items = vec![(10.0, 10.0), (12.0, 10.0), (30.0, 10.0)];
        let kept = suppress_duplicates(items, 10, |&(x, y)| (x, y));
        assert_eq!(kept, vec![(10.0, 10.0), (30.0, 10.0)]);
    }

    #[test]
    fn options_normalize_out_of_range_values() {
        let options = StarfindOptions {
            radius: 1,
            sigma: 0.0,
            ..Default::default()
        }
        .normalized();
        assert_eq!(options.radius, MIN_RADIUS);
        assert_eq!(options.sigma, 1.0);
    }
}
