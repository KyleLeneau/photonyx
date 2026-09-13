//! Point-spread-function fitting (ADR 007, P7-T2 / P7-T3).
//!
//! Detection gives integer pixel positions; registration needs centroids good
//! to a few hundredths of a pixel, which means fitting an analytic profile to
//! the samples around each candidate and taking its centre.
//!
//! Two profiles are available. The elliptical Gaussian with rotation is the
//! default and the classical choice for stellar photometry (Stetson, PASP 99,
//! 1987, §II; Bertin & Arnouts, *SExtractor*, A&AS 117, 1996, §4 for the
//! moment-based starting estimates used here). The Moffat profile,
//!
//! ```text
//! I(r) = B + A · (1 + r²/α²)^(-β)
//! ```
//!
//! from Moffat, *A Theoretical Investigation of Focal Stellar Images in the
//! Photographic Emulsion*, A&A 3, 1969, has heavier wings and fits a
//! seeing-limited star better in the halo — at the price of an eighth free
//! parameter, which is why it is selectable rather than the default.
//!
//! Both are fitted by Levenberg-Marquardt (Levenberg 1944; Marquardt, *An
//! Algorithm for Least-Squares Estimation of Nonlinear Parameters*, SIAM J.
//! Appl. Math. 11(2), 1963) with an analytic Jacobian. The parameters are few
//! and the derivatives are closed-form, so a finite-difference Jacobian would
//! cost eight extra model evaluations per iteration for strictly less accuracy.

use nalgebra::{DMatrix, DVector};

use super::starfind::{Candidate, StarfindOptions};

/// `2·sqrt(2·ln 2)`: the full width at half maximum of a Gaussian, in units of
/// its standard deviation.
pub const FWHM_PER_SIGMA: f64 = 2.354_820_045_030_949;

/// Iteration cap for the rotating fit.
///
/// The seven- and eight-parameter models include a rotation angle, which is
/// the slowest direction to converge: on an almost-circular star the angle is
/// nearly unconstrained and the solver can wander along it for a long time
/// while chi-squared barely moves. Fits that have not settled within this many
/// iterations are not going to, and the star is better rejected than refined.
pub const MAX_ITER_ANGLE: usize = 50;

/// Relative chi-squared improvement below which the fit is converged.
const CONVERGENCE_TOLERANCE: f64 = 1e-9;

/// Narrowest and widest accepted profile scale, in pixels.
///
/// A sigma below the lower bound is narrower than one pixel and describes a
/// hot pixel rather than a star; the upper bound is tied to the fitting box,
/// since a profile wider than its box is not constrained by the data.
const MIN_SCALE: f64 = 0.3;

/// Largest accepted Moffat beta. Above this the profile is indistinguishable
/// from a Gaussian and beta is unconstrained, so the fit wanders.
const MAX_BETA: f64 = 10.0;

/// Which analytic profile to fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Gaussian,
    Moffat,
}

/// One fitted star.
///
/// Positions are in the source channel's pixel coordinates, with integer
/// values at pixel *centres*: a star centred on the sample at column 100 has
/// `x0 == 100.0`. Every consumer in this crate uses that convention; a
/// conversion belongs at the boundary where a foreign one appears.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PsfStar {
    pub x0: f64,
    pub y0: f64,
    /// Profile scale along the major and minor axes: a Gaussian sigma, or a
    /// Moffat alpha.
    pub sx: f64,
    pub sy: f64,
    /// FWHM along each axis, in pixels. `fwhm_x >= fwhm_y` always — the axes
    /// are ordered, and `angle` follows that ordering.
    pub fwhm_x: f64,
    pub fwhm_y: f64,
    /// Major-axis position angle, degrees counter-clockwise from +x, in
    /// [0, 180).
    pub angle: f64,
    /// Fitted peak above the local background, unit-range.
    pub amplitude: f64,
    /// Fitted local background, unit-range.
    pub background: f64,
    /// Moffat exponent, or `None` for a Gaussian fit.
    pub beta: Option<f64>,
    /// `fwhm_y / fwhm_x`: 1.0 is a circular star, 0 a line.
    pub roundness: f64,
    /// Root-mean-square fit residual, unit-range — the measure the rejection
    /// rules use to tell a star from a galaxy or a cosmic ray.
    pub rmse: f64,
    /// The brightest sample in the fitting box, unit-range. Saturation
    /// detection needs the raw peak, which the fitted amplitude hides.
    pub peak: f64,
    /// Which channel this star was found in.
    pub layer: usize,
}

impl PsfStar {
    /// The mean FWHM over both axes — the single number a frame's seeing is
    /// usually quoted as.
    pub fn fwhm(&self) -> f64 {
        0.5 * (self.fwhm_x + self.fwhm_y)
    }
}

/// Fits a profile to the samples around `candidate`.
///
/// Returns `None` when the box runs off the frame or the solver fails to
/// converge. The returned star is *unfiltered*: the rejection rules are a
/// separate step, because two-pass registration wants to see the fits it
/// threw away.
pub fn fit_candidate(
    channel: &[f32],
    width: usize,
    height: usize,
    candidate: &Candidate,
    options: &StarfindOptions,
) -> Option<PsfStar> {
    let radius = options.radius;
    if candidate.x < radius
        || candidate.y < radius
        || candidate.x + radius >= width
        || candidate.y + radius >= height
    {
        return None;
    }

    let box_ = FitBox::extract(channel, width, candidate.x, candidate.y, radius);
    // The solver runs `convergence` times from its own previous answer. A
    // second pass buys a little accuracy on faint stars, where the
    // moment-based starting guess is poor; beyond the third it is wasted work,
    // which is why the option is documented as 1-3.
    let passes = options.convergence.clamp(1, 3);

    let (params, rmse) = match options.profile {
        Profile::Gaussian => {
            let model = GaussianModel;
            let mut params = box_.initial_gaussian();
            let mut rmse = f64::INFINITY;
            for _ in 0..passes {
                let (p, r) = solve(&model, &box_, params)?;
                params = p;
                rmse = r;
            }
            (params, rmse)
        }
        Profile::Moffat => {
            let model = MoffatModel {
                min_beta: options.min_beta.clamp(0.0, MAX_BETA),
            };
            let mut params = box_.initial_moffat();
            let mut rmse = f64::INFINITY;
            for _ in 0..passes {
                let (p, r) = solve(&model, &box_, params)?;
                params = p;
                rmse = r;
            }
            (params, rmse)
        }
    };

    Some(box_.into_star(&params, rmse, options.profile, candidate.peak as f64))
}

/// The samples a fit sees: one square box of a channel, with its own local
/// coordinate origin at the candidate pixel.
struct FitBox {
    /// Samples, row-major over `(2·radius+1)²`.
    values: Vec<f64>,
    radius: usize,
    /// Image coordinates of the box centre.
    centre_x: usize,
    centre_y: usize,
}

impl FitBox {
    fn extract(channel: &[f32], width: usize, x: usize, y: usize, radius: usize) -> Self {
        let side = 2 * radius + 1;
        let mut values = Vec::with_capacity(side * side);
        for yy in y - radius..=y + radius {
            let row = &channel[yy * width + x - radius..yy * width + x + radius + 1];
            values.extend(row.iter().map(|&v| v as f64));
        }
        Self {
            values,
            radius,
            centre_x: x,
            centre_y: y,
        }
    }

    fn side(&self) -> usize {
        2 * self.radius + 1
    }

    /// Local coordinates of sample `i`, with (0, 0) at the box centre.
    fn coords(&self, i: usize) -> (f64, f64) {
        let side = self.side();
        let r = self.radius as f64;
        ((i % side) as f64 - r, (i / side) as f64 - r)
    }

    /// Background estimate: the median of the box's outermost ring.
    ///
    /// The ring is the part of the box least contaminated by the star, and a
    /// median over it is insensitive to a neighbour intruding on one side.
    fn background(&self) -> f64 {
        let side = self.side();
        let mut ring: Vec<f64> = Vec::with_capacity(4 * side);
        for i in 0..self.values.len() {
            let (x, y) = (i % side, i / side);
            if x == 0 || y == 0 || x == side - 1 || y == side - 1 {
                ring.push(self.values[i]);
            }
        }
        ring.sort_by(f64::total_cmp);
        ring[ring.len() / 2]
    }

    /// Intensity-weighted first and second moments of the background-subtracted
    /// box, which is where the starting centroid and widths come from.
    ///
    /// Negative excursions get zero weight: a moment taken with negative
    /// weights is not a centroid, and noise below the background would drag it
    /// arbitrarily.
    fn moments(&self, background: f64) -> (f64, f64, f64, f64, f64) {
        let mut sum = 0.0;
        let (mut mx, mut my) = (0.0, 0.0);
        let (mut mxx, mut myy) = (0.0, 0.0);
        let mut peak = f64::NEG_INFINITY;
        for (i, &v) in self.values.iter().enumerate() {
            let w = (v - background).max(0.0);
            let (x, y) = self.coords(i);
            sum += w;
            mx += w * x;
            my += w * y;
            peak = peak.max(v);
        }
        if sum <= 0.0 {
            return (0.0, 0.0, 1.0, 1.0, peak);
        }
        let (cx, cy) = (mx / sum, my / sum);
        for (i, &v) in self.values.iter().enumerate() {
            let w = (v - background).max(0.0);
            let (x, y) = self.coords(i);
            mxx += w * (x - cx) * (x - cx);
            myy += w * (y - cy) * (y - cy);
        }
        (cx, cy, (mxx / sum).max(0.25), (myy / sum).max(0.25), peak)
    }

    /// `[background, amplitude, x0, y0, sx, sy, theta]`.
    fn initial_gaussian(&self) -> Vec<f64> {
        let background = self.background();
        let (cx, cy, vxx, vyy, peak) = self.moments(background);
        vec![
            background,
            (peak - background).max(1e-6),
            cx,
            cy,
            vxx.sqrt().max(MIN_SCALE),
            vyy.sqrt().max(MIN_SCALE),
            0.0,
        ]
    }

    /// `[background, amplitude, x0, y0, ax, ay, theta, beta]`.
    ///
    /// Beta starts at 2.5, which is the middle of the range real seeing
    /// produces; alpha is derived from the moment width so that the starting
    /// Moffat has the same FWHM as the starting Gaussian would.
    fn initial_moffat(&self) -> Vec<f64> {
        let mut params = self.initial_gaussian();
        const BETA0: f64 = 2.5;
        let scale = FWHM_PER_SIGMA / (2.0 * (2f64.powf(1.0 / BETA0) - 1.0).sqrt());
        params[4] = (params[4] * scale).max(MIN_SCALE);
        params[5] = (params[5] * scale).max(MIN_SCALE);
        params.push(BETA0);
        params
    }

    /// Converts solver parameters into a star in image coordinates, ordering
    /// the axes so the major one is `x`.
    fn into_star(self, params: &[f64], rmse: f64, profile: Profile, peak: f64) -> PsfStar {
        let beta = (profile == Profile::Moffat).then(|| params[7]);
        let fwhm_scale = match beta {
            None => FWHM_PER_SIGMA,
            Some(b) => 2.0 * (2f64.powf(1.0 / b) - 1.0).sqrt(),
        };

        let (mut sx, mut sy) = (params[4].abs(), params[5].abs());
        // The model is symmetric under swapping the axes and rotating by 90
        // degrees, so the solver may land on either. Fixing major-axis-first
        // makes roundness and angle single-valued.
        let mut angle = params[6].to_degrees();
        if sy > sx {
            std::mem::swap(&mut sx, &mut sy);
            angle += 90.0;
        }
        // Axes are unsigned, so the angle is modulo 180 degrees.
        angle = angle.rem_euclid(180.0);

        let (fwhm_x, fwhm_y) = (fwhm_scale * sx, fwhm_scale * sy);
        PsfStar {
            x0: self.centre_x as f64 + params[2],
            y0: self.centre_y as f64 + params[3],
            sx,
            sy,
            fwhm_x,
            fwhm_y,
            angle,
            amplitude: params[1],
            background: params[0],
            beta,
            roundness: if fwhm_x > 0.0 { fwhm_y / fwhm_x } else { 0.0 },
            rmse,
            peak,
            layer: 0,
        }
    }
}

/// A parametric surface with closed-form partial derivatives.
trait Model {
    /// Number of free parameters.
    fn params(&self) -> usize;

    /// Evaluates the model at `(x, y)` and writes `∂z/∂pᵢ` into `grad`.
    fn value_and_gradient(&self, p: &[f64], x: f64, y: f64, grad: &mut [f64]) -> f64;

    /// Projects a parameter vector back into the region where the model means
    /// something — positive widths, a bounded exponent.
    fn clamp(&self, p: &mut [f64], radius: f64);
}

struct GaussianModel;

impl Model for GaussianModel {
    fn params(&self) -> usize {
        7
    }

    fn value_and_gradient(&self, p: &[f64], x: f64, y: f64, grad: &mut [f64]) -> f64 {
        let (b, a, x0, y0, sx, sy, theta) = (p[0], p[1], p[2], p[3], p[4], p[5], p[6]);
        let (cos, sin) = (theta.cos(), theta.sin());
        let (dx, dy) = (x - x0, y - y0);
        // Rotate into the profile's own axes.
        let u = dx * cos + dy * sin;
        let v = -dx * sin + dy * cos;
        let (inv_sx2, inv_sy2) = (1.0 / (sx * sx), 1.0 / (sy * sy));
        let q = 0.5 * (u * u * inv_sx2 + v * v * inv_sy2);
        let e = (-q).exp();
        let ae = a * e;

        grad[0] = 1.0;
        grad[1] = e;
        grad[2] = ae * (u * cos * inv_sx2 - v * sin * inv_sy2);
        grad[3] = ae * (u * sin * inv_sx2 + v * cos * inv_sy2);
        grad[4] = ae * u * u * inv_sx2 / sx;
        grad[5] = ae * v * v * inv_sy2 / sy;
        grad[6] = -ae * u * v * (inv_sx2 - inv_sy2);
        b + ae
    }

    fn clamp(&self, p: &mut [f64], radius: f64) {
        clamp_common(p, radius);
    }
}

struct MoffatModel {
    min_beta: f64,
}

impl Model for MoffatModel {
    fn params(&self) -> usize {
        8
    }

    fn value_and_gradient(&self, p: &[f64], x: f64, y: f64, grad: &mut [f64]) -> f64 {
        let (b, a, x0, y0, ax, ay, theta, beta) = (p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
        let (cos, sin) = (theta.cos(), theta.sin());
        let (dx, dy) = (x - x0, y - y0);
        let u = dx * cos + dy * sin;
        let v = -dx * sin + dy * cos;
        let (inv_ax2, inv_ay2) = (1.0 / (ax * ax), 1.0 / (ay * ay));
        let s = 1.0 + u * u * inv_ax2 + v * v * inv_ay2;
        let pow = s.powf(-beta);
        // d/dS of A·S^(-beta), the chain-rule factor every geometric
        // derivative below shares.
        let chain = -a * beta * s.powf(-beta - 1.0);

        grad[0] = 1.0;
        grad[1] = pow;
        grad[2] = chain * 2.0 * (-u * cos * inv_ax2 + v * sin * inv_ay2);
        grad[3] = chain * 2.0 * (-u * sin * inv_ax2 - v * cos * inv_ay2);
        grad[4] = chain * -2.0 * u * u * inv_ax2 / ax;
        grad[5] = chain * -2.0 * v * v * inv_ay2 / ay;
        grad[6] = chain * 2.0 * u * v * (inv_ax2 - inv_ay2);
        grad[7] = -a * pow * s.ln();
        b + a * pow
    }

    fn clamp(&self, p: &mut [f64], radius: f64) {
        clamp_common(p, radius);
        p[7] = p[7].clamp(self.min_beta, MAX_BETA);
    }
}

/// The bounds the two models share: widths positive and no wider than the box,
/// and a centre that stays inside it.
///
/// A fit whose centre has left the box is no longer fitting the candidate that
/// produced it, and letting it run would let two neighbouring candidates
/// converge on the same bright star.
fn clamp_common(p: &mut [f64], radius: f64) {
    p[2] = p[2].clamp(-radius, radius);
    p[3] = p[3].clamp(-radius, radius);
    p[4] = p[4].abs().clamp(MIN_SCALE, radius);
    p[5] = p[5].abs().clamp(MIN_SCALE, radius);
}

/// Levenberg-Marquardt least squares, returning the fitted parameters and the
/// RMS residual.
///
/// The normal equations are accumulated directly rather than by forming the
/// full Jacobian: the box is a few hundred samples against eight parameters,
/// so `JᵗJ` is tiny and materializing `J` would be the dominant allocation.
fn solve(model: &impl Model, box_: &FitBox, mut params: Vec<f64>) -> Option<(Vec<f64>, f64)> {
    let n = model.params();
    let radius = box_.radius as f64;
    model.clamp(&mut params, radius);

    let mut lambda = 1e-3;
    let mut chi2 = chi_squared(model, box_, &params)?;
    let mut grad = vec![0.0; n];

    for _ in 0..MAX_ITER_ANGLE {
        // H = JᵗJ, g = Jᵗr, both over the whole box.
        let mut h = DMatrix::<f64>::zeros(n, n);
        let mut g = DVector::<f64>::zeros(n);
        for (i, &z) in box_.values.iter().enumerate() {
            let (x, y) = box_.coords(i);
            let residual = z - model.value_and_gradient(&params, x, y, &mut grad);
            for r in 0..n {
                g[r] += residual * grad[r];
                for c in r..n {
                    h[(r, c)] += grad[r] * grad[c];
                }
            }
        }
        for r in 0..n {
            for c in 0..r {
                h[(r, c)] = h[(c, r)];
            }
        }

        // Marquardt's scaling: damp each parameter in proportion to its own
        // curvature, so the step is invariant to the units the parameters
        // happen to be in (pixels, intensities, radians).
        let diag: Vec<f64> = (0..n).map(|i| h[(i, i)].max(1e-12)).collect();

        let mut improved = false;
        for _ in 0..MAX_ITER_ANGLE {
            let mut damped = h.clone();
            for i in 0..n {
                damped[(i, i)] += lambda * diag[i];
            }
            let Some(delta) = damped.cholesky().map(|c| c.solve(&g)) else {
                lambda *= 10.0;
                continue;
            };

            let mut trial: Vec<f64> = (0..n).map(|i| params[i] + delta[i]).collect();
            model.clamp(&mut trial, radius);
            let Some(trial_chi2) = chi_squared(model, box_, &trial) else {
                lambda *= 10.0;
                continue;
            };

            if trial_chi2 < chi2 {
                let relative = (chi2 - trial_chi2) / chi2.max(f64::MIN_POSITIVE);
                params = trial;
                chi2 = trial_chi2;
                lambda = (lambda * 0.1).max(1e-12);
                improved = true;
                if relative < CONVERGENCE_TOLERANCE {
                    return Some((params, rmse(chi2, box_.values.len())));
                }
                break;
            }
            lambda *= 10.0;
            // Beyond this the damped system is effectively `delta = 0` and no
            // further increase can help.
            if lambda > 1e12 {
                return Some((params, rmse(chi2, box_.values.len())));
            }
        }
        if !improved {
            break;
        }
    }

    Some((params, rmse(chi2, box_.values.len())))
}

/// Sum of squared residuals, or `None` if the model evaluated to a non-finite
/// value — which means the parameters have left the region where it is
/// defined and the step must be rejected rather than accepted as an
/// improvement.
fn chi_squared(model: &impl Model, box_: &FitBox, params: &[f64]) -> Option<f64> {
    let mut grad = vec![0.0; model.params()];
    let mut sum = 0.0;
    for (i, &z) in box_.values.iter().enumerate() {
        let (x, y) = box_.coords(i);
        let value = model.value_and_gradient(params, x, y, &mut grad);
        if !value.is_finite() {
            return None;
        }
        let r = z - value;
        sum += r * r;
    }
    sum.is_finite().then_some(sum)
}

fn rmse(chi2: f64, samples: usize) -> f64 {
    (chi2 / samples as f64).sqrt()
}

/// Why a fitted candidate was not accepted as a star.
///
/// Carried rather than collapsed into a boolean because the counts are how a
/// user diagnoses a frame that registered on twelve stars: a run dominated by
/// `Roundness` means trailing, one dominated by `Residual` means the detection
/// threshold is down in the noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// Amplitude outside the configured bounds, or not positive.
    Amplitude,
    /// The fitted centre left the box, so the fit is no longer describing the
    /// candidate that produced it.
    OffCentre,
    /// A profile scale pinned against its bound: narrower than a pixel, or
    /// wider than the box that constrains it.
    Width,
    /// Too elongated, or — with an upper bound configured — too round.
    Roundness,
    /// The residual is too large a fraction of the amplitude for the structure
    /// to be a star.
    Residual,
    /// A Moffat exponent at its floor, where amplitude and beta trade off
    /// freely and neither is measured.
    Beta,
}

/// Largest residual a star may leave, as a fraction of its fitted amplitude.
///
/// A real star's residual is the noise, which on a detectable star is a few
/// percent of its peak at worst. A galaxy, a cosmic ray track, or two blended
/// stars leave a residual that is structured rather than random and a much
/// larger fraction of the amplitude. The bound is deliberately loose: rejecting
/// a marginal star costs one correspondence, while admitting a non-star biases
/// a transform.
const MAX_RELATIVE_RMSE: f64 = 0.25;

/// Margin, in pixels, by which a fitted centre must stay inside its box.
///
/// The solver clamps the centre at the box edge, so a fit that wanted to go
/// further sits exactly on the bound. Testing against a slightly smaller box
/// catches that without depending on the clamp's exact behaviour.
const CENTRE_MARGIN: f64 = 0.5;

/// Whether a fitted star passes the detection criteria.
///
/// `relax_checks` drops the two shape tests — roundness and residual — and
/// keeps the ones that distinguish a measurement from a non-measurement. That
/// is the point of the relaxed mode: a comet or a badly trailed sub is not
/// star-shaped, but its centroid is still a usable fiducial.
pub fn accept(
    star: &PsfStar,
    candidate: &Candidate,
    options: &StarfindOptions,
) -> Result<(), Rejected> {
    let radius = options.radius as f64;

    if !star.amplitude.is_finite() || star.amplitude <= f64::from(options.min_amplitude) {
        return Err(Rejected::Amplitude);
    }
    if options.max_amplitude > 0.0 && star.amplitude >= f64::from(options.max_amplitude) {
        return Err(Rejected::Amplitude);
    }

    let bound = radius - CENTRE_MARGIN;
    if (star.x0 - candidate.x as f64).abs() > bound || (star.y0 - candidate.y as f64).abs() > bound
    {
        return Err(Rejected::OffCentre);
    }

    if !(star.sx.is_finite() && star.sy.is_finite()) || star.sy <= MIN_SCALE || star.sx >= radius {
        return Err(Rejected::Width);
    }

    if let Some(beta) = star.beta
        && beta <= options.min_beta
    {
        return Err(Rejected::Beta);
    }

    if options.relax_checks {
        return Ok(());
    }

    if star.roundness < options.roundness || star.roundness > options.max_roundness {
        return Err(Rejected::Roundness);
    }
    if !star.rmse.is_finite() || star.rmse > MAX_RELATIVE_RMSE * star.amplitude {
        return Err(Rejected::Residual);
    }
    Ok(())
}

/// Summary statistics of a frame's star list.
///
/// Two-pass registration ranks frames on these (ADR 007 Phase 9): the FWHM is
/// the frame's seeing and the star count is how much of the field came
/// through, and the reference is the frame that is best on both at once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FwhmStats {
    pub count: usize,
    /// Mean, median and spread of the major-axis FWHM, in pixels.
    pub mean: f64,
    pub median: f64,
    pub sigma: f64,
    pub min: f64,
    pub max: f64,
    /// Mean roundness, which distinguishes poor seeing from tracking error:
    /// bad seeing inflates the FWHM isotropically, a drive error only along
    /// one axis.
    pub mean_roundness: f64,
}

/// Summarizes a star list, or `None` when it is empty.
///
/// The FWHM quoted is the *major* axis rather than the mean of both, because
/// it is the one that bounds what registration can recover: a trailed frame
/// with a 3 px minor axis and a 9 px major axis is a 9 px frame.
pub fn fwhm_stats(stars: &[PsfStar]) -> Option<FwhmStats> {
    if stars.is_empty() {
        return None;
    }
    let n = stars.len() as f64;
    let mut widths: Vec<f64> = stars.iter().map(|s| s.fwhm_x).collect();
    widths.sort_by(f64::total_cmp);

    let mean = widths.iter().sum::<f64>() / n;
    let variance = widths.iter().map(|w| (w - mean) * (w - mean)).sum::<f64>() / n;
    let median = if widths.len().is_multiple_of(2) {
        0.5 * (widths[widths.len() / 2 - 1] + widths[widths.len() / 2])
    } else {
        widths[widths.len() / 2]
    };

    Some(FwhmStats {
        count: stars.len(),
        mean,
        median,
        sigma: variance.sqrt(),
        min: widths[0],
        max: widths[widths.len() - 1],
        mean_roundness: stars.iter().map(|s| s.roundness).sum::<f64>() / n,
    })
}

/// Keeps the stars whose fitted amplitude is at least `min_amplitude`.
///
/// This is the re-filter a frame needs when its star count hits the cap: the
/// cap truncates an arbitrary field at an arbitrary place, so two frames of the
/// same field can end up with lists that overlap poorly. Cutting both at an
/// *amplitude* instead makes the selection a property of the sky rather than of
/// the detection order.
pub fn filter_stars_by_amplitude(stars: &[PsfStar], min_amplitude: f64) -> Vec<PsfStar> {
    stars
        .iter()
        .copied()
        .filter(|s| s.amplitude >= min_amplitude)
        .collect()
}

/// The amplitude that keeps the brightest `count` stars.
///
/// Returns 0 when the list is already no longer than `count`, which keeps
/// everything.
pub fn amplitude_threshold_for(stars: &[PsfStar], count: usize) -> f64 {
    if stars.len() <= count || count == 0 {
        return 0.0;
    }
    let mut amplitudes: Vec<f64> = stars.iter().map(|s| s.amplitude).collect();
    amplitudes.sort_by(|a, b| b.total_cmp(a));
    amplitudes[count - 1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register::starfind::detect_candidates;

    /// Point-samples an elliptical Gaussian onto a frame, over a faint
    /// deterministic noise floor so detection has a scale to threshold
    /// against.
    fn render(width: usize, height: usize, stars: &[(f64, f64, f64, f64, f64, f64)]) -> Vec<f32> {
        let mut data: Vec<f32> = (0..width * height)
            .map(|i| 0.01 + 0.0005 * ((i as f32 * 12.9898).sin()))
            .collect();
        for &(x0, y0, amplitude, sx, sy, theta) in stars {
            for y in 0..height {
                for x in 0..width {
                    let (dx, dy) = (x as f64 - x0, y as f64 - y0);
                    let u = dx * theta.cos() + dy * theta.sin();
                    let v = -dx * theta.sin() + dy * theta.cos();
                    let q = 0.5 * (u * u / (sx * sx) + v * v / (sy * sy));
                    if q < 30.0 {
                        data[y * width + x] += (amplitude * (-q).exp()) as f32;
                    }
                }
            }
        }
        data
    }

    fn fit_one(data: &[f32], width: usize, height: usize, profile: Profile) -> PsfStar {
        let options = StarfindOptions {
            profile,
            ..Default::default()
        };
        let candidates = detect_candidates(data, width, height, &options);
        assert_eq!(candidates.len(), 1, "{candidates:?}");
        fit_candidate(data, width, height, &candidates[0], &options).expect("fit converges")
    }

    fn star_with(fwhm_x: f64, roundness: f64, amplitude: f64) -> PsfStar {
        PsfStar {
            x0: 0.0,
            y0: 0.0,
            sx: fwhm_x / FWHM_PER_SIGMA,
            sy: fwhm_x * roundness / FWHM_PER_SIGMA,
            fwhm_x,
            fwhm_y: fwhm_x * roundness,
            angle: 0.0,
            amplitude,
            background: 0.0,
            beta: None,
            roundness,
            rmse: 0.0,
            peak: amplitude,
            layer: 0,
        }
    }

    #[test]
    fn fwhm_stats_summarize_the_list() {
        let stars = [
            star_with(3.0, 1.0, 0.5),
            star_with(4.0, 0.8, 0.4),
            star_with(5.0, 0.9, 0.3),
            star_with(8.0, 0.9, 0.2),
        ];
        let stats = fwhm_stats(&stars).expect("non-empty");

        assert_eq!(stats.count, 4);
        assert!((stats.mean - 5.0).abs() < 1e-12);
        assert!((stats.median - 4.5).abs() < 1e-12);
        assert!((stats.min - 3.0).abs() < 1e-12);
        assert!((stats.max - 8.0).abs() < 1e-12);
        assert!((stats.sigma - 1.870_828_693_386_971).abs() < 1e-9);
        assert!((stats.mean_roundness - 0.9).abs() < 1e-12);
    }

    #[test]
    fn fwhm_stats_of_an_empty_list_is_none() {
        assert!(fwhm_stats(&[]).is_none());
    }

    #[test]
    fn amplitude_filtering_keeps_the_brightest_n() {
        let stars = [
            star_with(3.0, 1.0, 0.1),
            star_with(3.0, 1.0, 0.9),
            star_with(3.0, 1.0, 0.5),
            star_with(3.0, 1.0, 0.3),
        ];
        let threshold = amplitude_threshold_for(&stars, 2);
        assert!((threshold - 0.5).abs() < 1e-12);

        let kept = filter_stars_by_amplitude(&stars, threshold);
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|s| s.amplitude >= 0.5));

        // A cap no smaller than the list keeps everything.
        assert_eq!(amplitude_threshold_for(&stars, 4), 0.0);
        assert_eq!(filter_stars_by_amplitude(&stars, 0.0).len(), 4);
    }

    #[test]
    fn recovers_a_circular_gaussian() {
        let data = render(64, 64, &[(31.4, 28.7, 0.5, 2.0, 2.0, 0.0)]);
        let star = fit_one(&data, 64, 64, Profile::Gaussian);

        assert!((star.x0 - 31.4).abs() < 0.02, "x0 = {}", star.x0);
        assert!((star.y0 - 28.7).abs() < 0.02, "y0 = {}", star.y0);
        let truth = FWHM_PER_SIGMA * 2.0;
        assert!(
            (star.fwhm() - truth).abs() / truth < 0.01,
            "fwhm = {} vs {truth}",
            star.fwhm()
        );
        assert!(
            (star.amplitude - 0.5).abs() < 0.01,
            "A = {}",
            star.amplitude
        );
        assert!(star.roundness > 0.98, "roundness = {}", star.roundness);
    }

    #[test]
    fn recovers_an_elongated_rotated_gaussian() {
        let theta = 30f64.to_radians();
        let data = render(64, 64, &[(32.0, 32.0, 0.6, 3.0, 1.5, theta)]);
        let star = fit_one(&data, 64, 64, Profile::Gaussian);

        assert!((star.x0 - 32.0).abs() < 0.02, "x0 = {}", star.x0);
        assert!((star.y0 - 32.0).abs() < 0.02, "y0 = {}", star.y0);
        assert!((star.sx - 3.0).abs() / 3.0 < 0.01, "sx = {}", star.sx);
        assert!((star.sy - 1.5).abs() / 1.5 < 0.01, "sy = {}", star.sy);
        assert!((star.angle - 30.0).abs() < 1.0, "angle = {}", star.angle);
        assert!((star.roundness - 0.5).abs() < 0.01, "{}", star.roundness);
    }

    #[test]
    fn the_major_axis_is_always_x() {
        // Elongated along y: the solver may fit it either way round, and the
        // reported axes must come out ordered regardless.
        let data = render(64, 64, &[(32.0, 32.0, 0.6, 1.5, 3.2, 0.0)]);
        let star = fit_one(&data, 64, 64, Profile::Gaussian);
        assert!(star.fwhm_x >= star.fwhm_y);
        assert!((star.sx - 3.2).abs() / 3.2 < 0.01, "sx = {}", star.sx);
        assert!((0.0..180.0).contains(&star.angle), "{}", star.angle);
        // Elongated along y with no rotation is a 90-degree position angle.
        assert!((star.angle - 90.0).abs() < 1.0, "angle = {}", star.angle);
    }

    #[test]
    fn a_gaussian_fit_leaves_a_small_residual() {
        let data = render(64, 64, &[(32.0, 32.0, 0.5, 2.0, 2.0, 0.0)]);
        let star = fit_one(&data, 64, 64, Profile::Gaussian);
        // The only thing the model cannot explain is the noise floor, which is
        // 5e-4 peak to peak.
        assert!(star.rmse < 1e-3, "rmse = {}", star.rmse);
    }

    #[test]
    fn moffat_recovers_the_centre_and_fwhm_of_a_gaussian_star() {
        // A Gaussian is not a Moffat, so the profile parameters differ; the
        // centroid and the half-maximum width are what must survive the
        // substitution, because that is all registration uses.
        let data = render(64, 64, &[(30.25, 33.75, 0.5, 2.0, 2.0, 0.0)]);
        let star = fit_one(&data, 64, 64, Profile::Moffat);

        assert!((star.x0 - 30.25).abs() < 0.05, "x0 = {}", star.x0);
        assert!((star.y0 - 33.75).abs() < 0.05, "y0 = {}", star.y0);
        let truth = FWHM_PER_SIGMA * 2.0;
        assert!(
            (star.fwhm() - truth).abs() / truth < 0.05,
            "fwhm = {} vs {truth}",
            star.fwhm()
        );
        assert!(star.beta.is_some_and(|b| (1.5..=MAX_BETA).contains(&b)));
    }

    #[test]
    fn the_analytic_jacobian_matches_finite_differences() {
        // The Jacobian is the one part of a least-squares fit that can be
        // wrong without the fit obviously failing: a bad derivative just
        // slows convergence. This checks each partial directly.
        let models: [(&dyn Model, Vec<f64>); 2] = [
            (&GaussianModel, vec![0.01, 0.5, 0.3, -0.2, 2.0, 1.4, 0.4]),
            (
                &MoffatModel { min_beta: 1.5 },
                vec![0.01, 0.5, 0.3, -0.2, 2.0, 1.4, 0.4, 2.5],
            ),
        ];
        for (model, params) in models {
            let n = model.params();
            let mut analytic = vec![0.0; n];
            for &(x, y) in &[(1.0, 2.0), (-3.0, 0.5), (0.0, 0.0), (4.5, -4.5)] {
                model.value_and_gradient(&params, x, y, &mut analytic);
                for i in 0..n {
                    let h = 1e-6 * params[i].abs().max(1e-3);
                    let mut scratch = vec![0.0; n];
                    let mut up = params.clone();
                    up[i] += h;
                    let mut down = params.clone();
                    down[i] -= h;
                    let numeric = (model.value_and_gradient(&up, x, y, &mut scratch)
                        - model.value_and_gradient(&down, x, y, &mut scratch))
                        / (2.0 * h);
                    assert!(
                        (analytic[i] - numeric).abs() < 1e-5 * numeric.abs().max(1.0),
                        "param {i} at ({x}, {y}): analytic {} vs numeric {numeric}",
                        analytic[i]
                    );
                }
            }
        }
    }
}
