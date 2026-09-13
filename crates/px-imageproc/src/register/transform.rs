//! Geometric transforms between frames (ADR 007, P8-T1 / P8-T2 / P8-T3).
//!
//! Registration's output is, per frame, a map from that frame's pixel
//! coordinates onto the reference frame's. Everything here works in
//! homogeneous coordinates with one matrix type, rather than one struct per
//! transform class: a shift *is* a homography whose coefficients happen to be
//! mostly zero, and carrying four representations through composition,
//! inversion, and warping would mean four code paths for every operation.
//!
//! What the transform classes do affect is *estimation*: each is a different
//! least-squares problem with a different minimum number of correspondences,
//! and fitting more parameters than the data supports is how a registration
//! goes subtly wrong. So the class is a parameter to the estimator and a
//! property that can be read back off a matrix ([`Transform::kind`]), not a
//! different type.
//!
//! The homography estimator is the normalized DLT of Hartley & Zisserman,
//! *Multiple View Geometry in Computer Vision*, 2nd ed., §4.1 and §4.4.4 — the
//! normalization is not optional, as §4.4.4 shows: pixel coordinates in the
//! thousands make the raw DLT's 2n×9 system badly conditioned, and the
//! resulting homography can be wrong by pixels.

use nalgebra::{DMatrix, Matrix3, Vector3};

use crate::error::ImageProcError;

/// A point in a frame's pixel coordinates, integer values at pixel centres —
/// the convention [`super::psf::PsfStar`] reports in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to `other`.
    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// One star seen in two frames: `from` in the frame being registered, `to` in
/// the reference.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Correspondence {
    pub from: Point,
    pub to: Point,
}

/// How many degrees of freedom a transform class has, and therefore which
/// estimator applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformKind {
    /// Nothing survived: a singular or non-finite matrix. Carried as a class of
    /// its own because a caller must be able to tell "no transform" from
    /// "identity transform" — one is a failure, the other is a frame that
    /// needs no shifting.
    Null,
    Identity,
    /// Translation only, 2 parameters.
    Shift,
    /// Translation, rotation, uniform scale: 4 parameters.
    Similarity,
    /// Any linear map plus translation: 6 parameters.
    Affine,
    /// Full projective map: 8 parameters.
    Homography,
}

impl TransformKind {
    /// Smallest number of correspondences the class can be estimated from.
    pub fn minimum_pairs(&self) -> usize {
        match self {
            TransformKind::Null | TransformKind::Identity => 0,
            TransformKind::Shift => 1,
            TransformKind::Similarity => 2,
            TransformKind::Affine => 3,
            TransformKind::Homography => 4,
        }
    }
}

/// Tolerance, in matrix coefficients, for classifying a transform.
///
/// The comparisons are against coefficients that are pure numbers — a
/// projective row of `[0, 0, 1]`, a rotation's `a == d` — so one tolerance
/// serves, and it is loose enough that a homography estimated from noisy
/// correspondences of a pure shift is still recognized as a shift. Being
/// generous here is safe: the classification is a *description* of a matrix,
/// never a modification of it.
const CLASSIFY_TOLERANCE: f64 = 1e-6;

/// Tolerance for the projective (bottom) row specifically.
///
/// Much tighter than [`CLASSIFY_TOLERANCE`], because the two rows are not in
/// the same units. The linear coefficients are dimensionless, so 1e-6 of error
/// moves a point 4e-3 px on a 4000 px frame. The bottom row is in 1/pixels and
/// enters through a division, so 1e-6 there is a 0.4% coordinate change at
/// x = 4000 — about 16 px. At 1e-9 the worst-case corner displacement on a
/// 61 MP frame is under 0.02 px, which is the scale the rest of registration is
/// measured at.
const PROJECTIVE_TOLERANCE: f64 = 1e-9;

/// A projective transform in homogeneous pixel coordinates.
///
/// `apply` maps a point in the frame this transform belongs to onto the
/// reference frame. Warping needs the opposite direction — for each output
/// pixel, where to sample the input — which is [`Transform::invert`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    matrix: Matrix3<f64>,
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            matrix: Matrix3::identity(),
        }
    }

    /// A pure translation.
    pub fn shift(dx: f64, dy: f64) -> Self {
        let mut matrix = Matrix3::identity();
        matrix[(0, 2)] = dx;
        matrix[(1, 2)] = dy;
        Self { matrix }
    }

    /// Rotation by `angle` radians counter-clockwise about the origin, uniform
    /// `scale`, then translation.
    pub fn similarity(scale: f64, angle: f64, dx: f64, dy: f64) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            matrix: Matrix3::new(
                scale * cos,
                -scale * sin,
                dx,
                scale * sin,
                scale * cos,
                dy,
                0.0,
                0.0,
                1.0,
            ),
        }
    }

    /// Wraps a matrix, normalizing its scale so that two transforms that
    /// differ only by a homogeneous factor compare equal.
    ///
    /// Returns `None` when the matrix is non-finite or has a zero bottom-right
    /// coefficient, which no transform between two finite image planes has:
    /// `h22 == 0` sends the origin to infinity.
    pub fn from_matrix(matrix: Matrix3<f64>) -> Option<Self> {
        if !matrix.iter().all(|v| v.is_finite()) || matrix[(2, 2)].abs() < f64::MIN_POSITIVE {
            return None;
        }
        Some(Self {
            matrix: matrix / matrix[(2, 2)],
        })
    }

    pub fn matrix(&self) -> Matrix3<f64> {
        self.matrix
    }

    /// Maps a point onto the reference frame.
    pub fn apply(&self, p: Point) -> Point {
        let v = self.matrix * Vector3::new(p.x, p.y, 1.0);
        // A projective map can send a point to infinity. Pixel coordinates
        // cannot represent that, so the degenerate case is reported as a
        // non-finite point rather than silently dividing by zero.
        let w = v.z;
        Point::new(v.x / w, v.y / w)
    }

    /// `self` after `first`: the transform that applies `first`, then `self`.
    pub fn compose(&self, first: &Transform) -> Option<Self> {
        Self::from_matrix(self.matrix * first.matrix)
    }

    /// The inverse map, or `None` when the transform is singular.
    pub fn invert(&self) -> Option<Self> {
        self.matrix.try_inverse().and_then(Self::from_matrix)
    }

    /// The transform class this matrix actually represents.
    ///
    /// Several things downstream depend on knowing: a shift can be warped by a
    /// simple resample where a homography cannot, framing geometry for a pure
    /// shift is exact, and a "homography" that turns out to be a shift with
    /// noise is a sign the estimator was given more freedom than the data
    /// supports.
    pub fn kind(&self) -> TransformKind {
        let m = &self.matrix;
        if !m.iter().all(|v| v.is_finite()) || m.determinant().abs() < CLASSIFY_TOLERANCE {
            return TransformKind::Null;
        }

        let close = |a: f64, b: f64| (a - b).abs() <= CLASSIFY_TOLERANCE;
        let projective = m[(2, 0)].abs() > PROJECTIVE_TOLERANCE
            || m[(2, 1)].abs() > PROJECTIVE_TOLERANCE
            || !close(m[(2, 2)], 1.0);
        if projective {
            return TransformKind::Homography;
        }

        let linear_is_identity = close(m[(0, 0)], 1.0)
            && close(m[(0, 1)], 0.0)
            && close(m[(1, 0)], 0.0)
            && close(m[(1, 1)], 1.0);
        if linear_is_identity {
            return if close(m[(0, 2)], 0.0) && close(m[(1, 2)], 0.0) {
                TransformKind::Identity
            } else {
                TransformKind::Shift
            };
        }

        // A similarity's linear part is `scale · R`: equal diagonal, opposite
        // off-diagonal.
        if close(m[(0, 0)], m[(1, 1)]) && close(m[(0, 1)], -m[(1, 0)]) {
            return TransformKind::Similarity;
        }
        TransformKind::Affine
    }

    /// The largest distance, over the four corners of a `width`×`height` frame,
    /// between this transform's image of a corner and `other`'s.
    ///
    /// This is the measure registration parity is stated in. A difference in
    /// any single matrix coefficient means nothing on its own — coefficients
    /// trade off against each other — whereas corner error is in pixels, is
    /// what a stacked frame actually suffers, and is worst where a small
    /// rotation or projective term does the most damage.
    pub fn corner_error(&self, other: &Transform, width: usize, height: usize) -> f64 {
        let (w, h) = ((width - 1) as f64, (height - 1) as f64);
        [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
            .into_iter()
            .map(|(x, y)| {
                let p = Point::new(x, y);
                self.apply(p).distance(&other.apply(p))
            })
            .fold(0.0, f64::max)
    }

    /// Root-mean-square reprojection residual over `pairs`, in pixels.
    pub fn rmse(&self, pairs: &[Correspondence]) -> f64 {
        if pairs.is_empty() {
            return 0.0;
        }
        let sum: f64 = pairs
            .iter()
            .map(|pair| self.apply(pair.from).distance(&pair.to).powi(2))
            .sum();
        (sum / pairs.len() as f64).sqrt()
    }
}

/// Estimates a transform of class `kind` from `pairs`, in the least-squares
/// sense.
///
/// Every class is fitted through the same Hartley normalization, not just the
/// homography: it costs two small matrix products and removes the dependence
/// on where in the frame the stars happened to be.
pub fn estimate(
    kind: TransformKind,
    pairs: &[Correspondence],
) -> Result<Transform, ImageProcError> {
    let needed = kind.minimum_pairs();
    if pairs.len() < needed {
        return Err(ImageProcError::Registration(format!(
            "{kind:?} needs {needed} correspondences, got {}",
            pairs.len()
        )));
    }
    if matches!(kind, TransformKind::Identity | TransformKind::Null) {
        return Ok(Transform::identity());
    }

    let (from_norm, to_norm, normalized) = normalize(pairs);
    let fitted = match kind {
        TransformKind::Shift => fit_shift(&normalized),
        TransformKind::Similarity => fit_linear::<4>(&normalized)?,
        TransformKind::Affine => fit_linear::<6>(&normalized)?,
        TransformKind::Homography => fit_homography(&normalized)?,
        TransformKind::Identity | TransformKind::Null => unreachable!("handled above"),
    };

    // Undo the normalization: the fitted transform maps normalized source
    // coordinates to normalized destination coordinates, so the real one is
    // `to_norm⁻¹ · fitted · from_norm`.
    let denormalized = to_norm
        .try_inverse()
        .ok_or_else(|| ImageProcError::Registration("degenerate normalization".into()))?
        * fitted
        * from_norm;
    Transform::from_matrix(denormalized)
        .ok_or_else(|| ImageProcError::Registration("estimated a non-finite transform".into()))
}

/// Hartley normalization (Hartley & Zisserman §4.4.4): translate each point set
/// to zero mean and scale it so the mean distance from the origin is `sqrt(2)`.
///
/// Returns the two normalizing matrices and the normalized correspondences.
fn normalize(pairs: &[Correspondence]) -> (Matrix3<f64>, Matrix3<f64>, Vec<Correspondence>) {
    let from = normalizer(pairs.iter().map(|p| p.from));
    let to = normalizer(pairs.iter().map(|p| p.to));

    let normalized = pairs
        .iter()
        .map(|pair| Correspondence {
            from: apply_matrix(&from, pair.from),
            to: apply_matrix(&to, pair.to),
        })
        .collect();
    (from, to, normalized)
}

/// The similarity matrix that centres a point set and scales its mean radius to
/// `sqrt(2)`.
fn normalizer(points: impl Iterator<Item = Point> + Clone) -> Matrix3<f64> {
    let count = points.clone().count() as f64;
    let (sum_x, sum_y) = points
        .clone()
        .fold((0.0, 0.0), |(sx, sy), p| (sx + p.x, sy + p.y));
    let (cx, cy) = (sum_x / count, sum_y / count);

    let mean_radius = points
        .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
        .sum::<f64>()
        / count;
    // A point set with no spread — every correspondence at one position —
    // cannot be scaled, and is left at unit scale for the estimator to fail on
    // honestly.
    let scale = if mean_radius > f64::MIN_POSITIVE {
        std::f64::consts::SQRT_2 / mean_radius
    } else {
        1.0
    };

    Matrix3::new(
        scale,
        0.0,
        -scale * cx,
        0.0,
        scale,
        -scale * cy,
        0.0,
        0.0,
        1.0,
    )
}

fn apply_matrix(m: &Matrix3<f64>, p: Point) -> Point {
    let v = m * Vector3::new(p.x, p.y, 1.0);
    Point::new(v.x / v.z, v.y / v.z)
}

/// Mean translation, which is the least-squares shift.
fn fit_shift(pairs: &[Correspondence]) -> Matrix3<f64> {
    let n = pairs.len() as f64;
    let (dx, dy) = pairs.iter().fold((0.0, 0.0), |(dx, dy), pair| {
        (dx + pair.to.x - pair.from.x, dy + pair.to.y - pair.from.y)
    });
    Transform::shift(dx / n, dy / n).matrix
}

/// Least squares for the classes whose model is linear in its parameters:
/// similarity (4) and affine (6).
///
/// Both are one normal-equation solve of a `2n × P` system. They differ only in
/// which row pattern they contribute, which is what `P` selects.
fn fit_linear<const P: usize>(pairs: &[Correspondence]) -> Result<Matrix3<f64>, ImageProcError> {
    let rows = 2 * pairs.len();
    let mut a = DMatrix::<f64>::zeros(rows, P);
    let mut b = DMatrix::<f64>::zeros(rows, 1);

    for (i, pair) in pairs.iter().enumerate() {
        let (x, y) = (pair.from.x, pair.from.y);
        let (r0, r1) = (2 * i, 2 * i + 1);
        match P {
            // Similarity: [x', y'] = [a·x − b·y + tx, b·x + a·y + ty].
            4 => {
                a[(r0, 0)] = x;
                a[(r0, 1)] = -y;
                a[(r0, 2)] = 1.0;
                a[(r1, 0)] = y;
                a[(r1, 1)] = x;
                a[(r1, 3)] = 1.0;
            }
            // Affine: the two rows are independent, three parameters each.
            6 => {
                a[(r0, 0)] = x;
                a[(r0, 1)] = y;
                a[(r0, 2)] = 1.0;
                a[(r1, 3)] = x;
                a[(r1, 4)] = y;
                a[(r1, 5)] = 1.0;
            }
            _ => unreachable!("only the 4- and 6-parameter models are linear in this form"),
        }
        b[(r0, 0)] = pair.to.x;
        b[(r1, 0)] = pair.to.y;
    }

    let solution = a
        .clone()
        .svd(true, true)
        .solve(&b, f64::EPSILON)
        .map_err(|e| ImageProcError::Registration(format!("least squares failed: {e}")))?;

    let p = |i: usize| solution[(i, 0)];
    Ok(match P {
        4 => Matrix3::new(p(0), -p(1), p(2), p(1), p(0), p(3), 0.0, 0.0, 1.0),
        6 => Matrix3::new(p(0), p(1), p(2), p(3), p(4), p(5), 0.0, 0.0, 1.0),
        _ => unreachable!(),
    })
}

/// The direct linear transform for a homography: the null vector of the `2n × 9`
/// constraint matrix, taken as the right singular vector of its smallest
/// singular value.
fn fit_homography(pairs: &[Correspondence]) -> Result<Matrix3<f64>, ImageProcError> {
    // At least 9 rows, so that the SVD's right factor is the full 9x9 and
    // includes the null direction being solved for. `nalgebra` computes a thin
    // SVD: the exactly-four-pair case builds an 8x9 matrix whose right factor
    // is 8x9, and the smallest of those eight singular vectors is *not* the
    // null vector — the homography that comes back satisfies the four pairs it
    // was given and nothing else. The padding rows are zero, which leaves AᵗA
    // and therefore the null space untouched.
    let mut a = DMatrix::<f64>::zeros((2 * pairs.len()).max(9), 9);
    for (i, pair) in pairs.iter().enumerate() {
        let (x, y) = (pair.from.x, pair.from.y);
        let (u, v) = (pair.to.x, pair.to.y);
        let (r0, r1) = (2 * i, 2 * i + 1);
        // The two rows are the cross-product constraint `to × (H · from) = 0`,
        // dropping its third (dependent) row.
        a[(r0, 0)] = -x;
        a[(r0, 1)] = -y;
        a[(r0, 2)] = -1.0;
        a[(r0, 6)] = u * x;
        a[(r0, 7)] = u * y;
        a[(r0, 8)] = u;
        a[(r1, 3)] = -x;
        a[(r1, 4)] = -y;
        a[(r1, 5)] = -1.0;
        a[(r1, 6)] = v * x;
        a[(r1, 7)] = v * y;
        a[(r1, 8)] = v;
    }

    let svd = a.svd(false, true);
    let v_t = svd
        .v_t
        .ok_or_else(|| ImageProcError::Registration("SVD produced no right factors".into()))?;
    // `nalgebra`'s SVD does not promise sorted singular values, so the
    // smallest is found rather than assumed to be last.
    let smallest = svd
        .singular_values
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i)
        .ok_or_else(|| ImageProcError::Registration("empty SVD".into()))?;
    let h = v_t.row(smallest);

    Ok(Matrix3::new(
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8],
    ))
}

/// Reprojection distance, in pixels, within which a correspondence supports a
/// transform.
///
/// 3.0 px is OpenCV's `defaultRANSACReprojThreshold`, which is also what Siril
/// registers with. It is deliberately several times the centroid accuracy of a
/// single star (0.02 px on a good one): the threshold's job is to separate
/// correct correspondences from wrong ones — a wrong pair is typically tens or
/// hundreds of pixels out — not to judge how well a correct one was measured.
/// That second job belongs to the refinement pass, which re-cuts on the
/// residual distribution the data actually has.
pub const DEFAULT_REPROJECTION_THRESHOLD: f64 = 3.0;

/// Probability that the sampling loop has drawn at least one all-inlier
/// minimal set, which is what sets the adaptive iteration count.
const DEFAULT_CONFIDENCE: f64 = 0.995;

/// Hard ceiling on sampling iterations, for the case where the inlier ratio is
/// so low that the adaptive count is astronomical.
const DEFAULT_MAX_ITERATIONS: usize = 2000;

/// Rounds of reject-and-refit the refinement pass performs.
const MAX_REFINE_ROUNDS: usize = 5;

/// Residual, in multiples of the current RMSE, beyond which refinement drops a
/// correspondence.
const REFINE_SIGMAS: f64 = 3.0;

/// How a robust fit is configured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RansacOptions {
    pub threshold: f64,
    pub confidence: f64,
    pub max_iterations: usize,
    /// Seed for the sampling sequence. Fixed rather than drawn from the
    /// environment: a registration that produced a different transform on a
    /// re-run would make every parity test in this crate meaningless, and
    /// "which random minimal sets were tried" is not information a user has any
    /// use for.
    pub seed: u64,
    /// Fewest correspondences a fit is allowed to rest on, over and above the
    /// class minimum. Siril's registration default is 10 (ADR 007 D10).
    pub min_pairs: usize,
}

impl Default for RansacOptions {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_REPROJECTION_THRESHOLD,
            confidence: DEFAULT_CONFIDENCE,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            seed: 0x5741_1F7A_0000_0007,
            min_pairs: 10,
        }
    }
}

/// A fitted transform and the evidence for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub transform: Transform,
    /// Indices into the input correspondences that support the transform.
    pub inliers: Vec<usize>,
    /// RMS reprojection residual over the inliers, in pixels.
    pub rmse: f64,
    /// Sampling iterations actually performed.
    pub iterations: usize,
}

impl Fit {
    pub fn inlier_count(&self) -> usize {
        self.inliers.len()
    }
}

/// Deterministic PRNG for the sampling loop.
///
/// Splitmix64 rather than a dependency: the loop needs uniform indices and
/// reproducibility, and nothing about the statistical quality of a minimal-set
/// sampler justifies pulling `rand` into the algorithm core.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`, via Lemire's multiply-shift. Biased by less than one
    /// part in 2^64 over an index range, which is immaterial here.
    fn below(&mut self, n: usize) -> usize {
        ((self.next_u64() as u128 * n as u128) >> 64) as usize
    }
}

/// Fits a transform of class `kind` to `pairs`, rejecting outliers by RANSAC
/// (Fischler & Bolles, *Random Sample Consensus*, Comm. ACM 24(6), 1981).
///
/// Minimal sets are drawn at random, scored by how many correspondences they
/// reproject within the threshold, and the best consensus set is refitted in
/// full. The iteration count adapts: every time the best inlier ratio improves,
/// the number of draws needed to reach `confidence` is recomputed, so a clean
/// star list costs a handful of iterations and only a contaminated one pays for
/// thousands.
pub fn ransac(
    kind: TransformKind,
    pairs: &[Correspondence],
    options: &RansacOptions,
) -> Result<Fit, ImageProcError> {
    let minimum = kind.minimum_pairs().max(1);
    let floor = options.min_pairs.max(minimum);
    if pairs.len() < floor {
        return Err(ImageProcError::Registration(format!(
            "{kind:?} needs at least {floor} correspondences, got {}",
            pairs.len()
        )));
    }

    let mut rng = SplitMix64(options.seed);
    let mut best: Option<(Vec<usize>, f64)> = None;
    let mut required = options.max_iterations;
    let mut iterations = 0usize;

    while iterations < required.min(options.max_iterations) {
        iterations += 1;

        let sample = draw_sample(&mut rng, pairs.len(), minimum);
        let subset: Vec<Correspondence> = sample.iter().map(|&i| pairs[i]).collect();
        let Ok(candidate) = estimate(kind, &subset) else {
            continue;
        };

        let (inliers, residual) = consensus(&candidate, pairs, options.threshold);
        let better = match &best {
            None => true,
            // More support wins; equal support is broken on the tighter fit, so
            // the result does not depend on which equally-sized consensus set
            // the sampler happened to find first.
            Some((best_inliers, best_residual)) => {
                inliers.len() > best_inliers.len()
                    || (inliers.len() == best_inliers.len() && residual < *best_residual)
            }
        };
        if better {
            let ratio = inliers.len() as f64 / pairs.len() as f64;
            best = Some((inliers, residual));
            required = iterations_for(ratio, minimum, options.confidence);
        }
    }

    let (inliers, _) = best.ok_or_else(|| {
        ImageProcError::Registration("no minimal set produced a usable transform".into())
    })?;
    if inliers.len() < floor {
        return Err(ImageProcError::Registration(format!(
            "only {} of {} correspondences agree within {} px",
            inliers.len(),
            pairs.len(),
            options.threshold
        )));
    }

    // The reported transform is refitted over the whole consensus set, not the
    // minimal set that found it: the minimal set fixes *which* correspondences
    // are trusted, and all of them together fix the transform.
    let subset: Vec<Correspondence> = inliers.iter().map(|&i| pairs[i]).collect();
    let transform = estimate(kind, &subset)?;
    Ok(Fit {
        rmse: transform.rmse(&subset),
        transform,
        inliers,
        iterations,
    })
}

/// [`ransac`] followed by reject-and-refit rounds (ADR 007 P8-T5).
///
/// RANSAC's threshold is fixed and generous, so its consensus set still holds
/// correspondences that agree to 2 px when the rest agree to 0.05 px. Each
/// round recomputes the residual spread of the surviving pairs and drops those
/// beyond `REFINE_SIGMAS` of it, then refits — the cut tightens with the data
/// instead of being stated in advance. It stops when a round rejects nothing,
/// when the next round would leave too few pairs, or when the RMSE stops
/// improving.
pub fn refine(
    kind: TransformKind,
    pairs: &[Correspondence],
    options: &RansacOptions,
) -> Result<Fit, ImageProcError> {
    let mut fit = ransac(kind, pairs, options)?;
    let floor = options.min_pairs.max(kind.minimum_pairs().max(1));

    for _ in 0..MAX_REFINE_ROUNDS {
        let cut = REFINE_SIGMAS * fit.rmse;
        // A fit with no residual spread at all is already exact; tightening
        // the cut further would reject every pair on floating-point noise.
        if cut <= f64::EPSILON {
            break;
        }

        let kept: Vec<usize> = fit
            .inliers
            .iter()
            .copied()
            .filter(|&i| fit.transform.apply(pairs[i].from).distance(&pairs[i].to) <= cut)
            .collect();
        if kept.len() == fit.inliers.len() || kept.len() < floor {
            break;
        }

        let subset: Vec<Correspondence> = kept.iter().map(|&i| pairs[i]).collect();
        let Ok(transform) = estimate(kind, &subset) else {
            break;
        };
        let rmse = transform.rmse(&subset);
        if rmse >= fit.rmse {
            // Rejecting these pairs did not improve the fit, so they were not
            // outliers and the previous answer stands.
            break;
        }
        fit = Fit {
            transform,
            inliers: kept,
            rmse,
            iterations: fit.iterations,
        };
    }
    Ok(fit)
}

/// Draws `count` distinct indices below `n`.
///
/// Rejection sampling against the few already drawn, which is the cheap way
/// round at these sizes: `count` is 1 to 4 and `n` is hundreds, so a collision
/// is rare and a shuffle of the whole index range per iteration would cost more
/// than the fit it feeds.
fn draw_sample(rng: &mut SplitMix64, n: usize, count: usize) -> Vec<usize> {
    let mut sample = Vec::with_capacity(count);
    while sample.len() < count {
        let candidate = rng.below(n);
        if !sample.contains(&candidate) {
            sample.push(candidate);
        }
    }
    sample
}

/// The correspondences `candidate` reprojects within `threshold`, and their RMS
/// residual.
fn consensus(candidate: &Transform, pairs: &[Correspondence], threshold: f64) -> (Vec<usize>, f64) {
    let mut inliers = Vec::new();
    let mut sum_squared = 0.0;
    for (i, pair) in pairs.iter().enumerate() {
        let distance = candidate.apply(pair.from).distance(&pair.to);
        if distance <= threshold {
            inliers.push(i);
            sum_squared += distance * distance;
        }
    }
    let rmse = if inliers.is_empty() {
        f64::INFINITY
    } else {
        (sum_squared / inliers.len() as f64).sqrt()
    };
    (inliers, rmse)
}

/// Iterations needed for `confidence` that some minimal set of `size` was drawn
/// entirely from inliers, given an inlier `ratio`.
///
/// `log(1 − confidence) / log(1 − ratio^size)`, the standard adaptive-stopping
/// rule. Returns 1 for a ratio of 1 (the first draw was already clean) and the
/// caller's ceiling for a ratio of 0.
fn iterations_for(ratio: f64, size: usize, confidence: f64) -> usize {
    if ratio <= 0.0 {
        return usize::MAX;
    }
    let all_inliers = ratio.powi(size as i32);
    if all_inliers >= 1.0 {
        return 1;
    }
    let needed = (1.0 - confidence).ln() / (1.0 - all_inliers).ln();
    if !needed.is_finite() || needed < 1.0 {
        return 1;
    }
    needed.ceil() as usize
}

/// Fixtures shared by this module's two test modules.
#[cfg(test)]
mod tests_support {
    use super::*;

    pub const WIDTH: usize = 4000;
    pub const HEIGHT: usize = 3000;

    /// A spread of points across a frame, which is what the corner-error and
    /// estimation tests need: correspondences clustered in one corner constrain
    /// a projective term barely at all.
    pub fn grid() -> Vec<Point> {
        let mut points = Vec::new();
        for row in 0..5 {
            for column in 0..5 {
                points.push(Point::new(
                    100.0 + 900.0 * column as f64,
                    80.0 + 700.0 * row as f64,
                ));
            }
        }
        points
    }

    pub fn pairs_under(t: &Transform) -> Vec<Correspondence> {
        grid()
            .into_iter()
            .map(|from| Correspondence {
                from,
                to: t.apply(from),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::*;
    use super::*;

    #[test]
    fn apply_and_invert_round_trip() {
        let t = Transform::similarity(1.02, 0.03, 12.5, -7.25);
        let inverse = t.invert().expect("invertible");
        for p in grid() {
            let back = inverse.apply(t.apply(p));
            assert!(back.distance(&p) < 1e-9, "{p:?} -> {back:?}");
        }
    }

    #[test]
    fn composition_is_application_in_order() {
        let first = Transform::shift(10.0, -4.0);
        let second = Transform::similarity(1.0, 0.2, 0.0, 0.0);
        let composed = second.compose(&first).expect("composable");
        for p in grid() {
            let stepwise = second.apply(first.apply(p));
            assert!(composed.apply(p).distance(&stepwise) < 1e-9);
        }
    }

    #[test]
    fn kind_classifies_each_class() {
        assert_eq!(Transform::identity().kind(), TransformKind::Identity);
        assert_eq!(Transform::shift(3.0, -2.0).kind(), TransformKind::Shift);
        assert_eq!(
            Transform::similarity(1.01, 0.02, 3.0, 4.0).kind(),
            TransformKind::Similarity
        );

        let affine =
            Transform::from_matrix(Matrix3::new(1.0, 0.1, 5.0, 0.0, 1.0, -3.0, 0.0, 0.0, 1.0))
                .expect("finite");
        assert_eq!(affine.kind(), TransformKind::Affine);

        let projective =
            Transform::from_matrix(Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1e-5, 2e-6, 1.0))
                .expect("finite");
        assert_eq!(projective.kind(), TransformKind::Homography);

        let singular =
            Transform::from_matrix(Matrix3::new(1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 0.0, 1.0))
                .expect("finite");
        assert_eq!(singular.kind(), TransformKind::Null);
    }

    #[test]
    fn a_zero_bottom_right_coefficient_is_not_a_transform() {
        assert!(
            Transform::from_matrix(Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0))
                .is_none()
        );
        assert!(Transform::from_matrix(Matrix3::repeat(f64::NAN)).is_none());
    }

    #[test]
    fn homogeneous_scale_does_not_change_a_transform() {
        let t = Transform::similarity(1.0, 0.1, 4.0, 5.0);
        let scaled = Transform::from_matrix(t.matrix() * -3.75).expect("finite");
        assert!(scaled.corner_error(&t, WIDTH, HEIGHT) < 1e-9);
    }

    #[test]
    fn corner_error_is_measured_in_pixels() {
        let a = Transform::identity();
        let b = Transform::shift(3.0, 4.0);
        assert!((a.corner_error(&b, WIDTH, HEIGHT) - 5.0).abs() < 1e-12);

        // A rotation of one milliradian about the origin moves the far corner
        // by about its radius times the angle.
        let rotated = Transform::similarity(1.0, 1e-3, 0.0, 0.0);
        let expected = ((WIDTH - 1) as f64).hypot((HEIGHT - 1) as f64) * 1e-3;
        let error = a.corner_error(&rotated, WIDTH, HEIGHT);
        assert!((error - expected).abs() / expected < 1e-3, "{error}");
    }

    #[test]
    fn each_estimator_recovers_its_own_class_exactly() {
        let cases = [
            (TransformKind::Shift, Transform::shift(17.5, -23.25)),
            (
                TransformKind::Similarity,
                Transform::similarity(1.015, 0.021, 17.5, -23.25),
            ),
            (
                TransformKind::Affine,
                Transform::from_matrix(Matrix3::new(
                    1.003, 0.012, 17.5, -0.008, 0.997, -23.25, 0.0, 0.0, 1.0,
                ))
                .expect("finite"),
            ),
            (
                TransformKind::Homography,
                Transform::from_matrix(Matrix3::new(
                    1.003, 0.012, 17.5, -0.008, 0.997, -23.25, 3e-7, -5e-7, 1.0,
                ))
                .expect("finite"),
            ),
        ];

        for (kind, truth) in cases {
            let fitted = estimate(kind, &pairs_under(&truth)).expect("estimate");
            let error = fitted.corner_error(&truth, WIDTH, HEIGHT);
            assert!(error < 1e-6, "{kind:?}: corner error {error:.3e} px");
            assert_eq!(fitted.kind(), kind, "{kind:?} misclassified");
        }
    }

    #[test]
    fn a_simpler_estimator_ignores_the_freedom_it_does_not_have() {
        // Given correspondences from a rotation, the shift estimator must
        // return the best shift rather than failing or producing a rotation.
        let truth = Transform::similarity(1.0, 0.05, 10.0, 10.0);
        let shift = estimate(TransformKind::Shift, &pairs_under(&truth)).expect("estimate");
        assert_eq!(shift.kind(), TransformKind::Shift);
        assert!(shift.rmse(&pairs_under(&truth)) > 1.0);
    }

    #[test]
    fn estimation_needs_the_minimum_number_of_pairs() {
        let truth = Transform::shift(1.0, 1.0);
        let all = pairs_under(&truth);
        for kind in [
            TransformKind::Shift,
            TransformKind::Similarity,
            TransformKind::Affine,
            TransformKind::Homography,
        ] {
            let minimum = kind.minimum_pairs();
            assert!(estimate(kind, &all[..minimum - 1]).is_err(), "{kind:?}");
            assert!(estimate(kind, &all[..minimum]).is_ok(), "{kind:?}");
        }
    }

    #[test]
    fn a_homography_from_exactly_four_pairs_is_exact() {
        // The minimum for the class, and the case where a thin SVD silently
        // returns the wrong singular vector if the constraint matrix is not
        // padded to nine rows.
        let truth = Transform::from_matrix(Matrix3::new(
            1.002, 0.009, 14.0, -0.006, 0.998, -21.0, 2.4e-7, -3.1e-7, 1.0,
        ))
        .expect("finite");
        let corners = [
            Point::new(40.0, 30.0),
            Point::new(3900.0, 60.0),
            Point::new(80.0, 2900.0),
            Point::new(3950.0, 2950.0),
        ];
        let pairs: Vec<Correspondence> = corners
            .into_iter()
            .map(|from| Correspondence {
                from,
                to: truth.apply(from),
            })
            .collect();

        let fitted = estimate(TransformKind::Homography, &pairs).expect("estimate");
        let error = fitted.corner_error(&truth, WIDTH, HEIGHT);
        assert!(error < 1e-6, "corner error {error:.3e} px");
    }

    #[test]
    fn normalization_makes_the_dlt_survive_real_pixel_magnitudes() {
        // Corner-to-corner correspondences on a 9576x6388 frame with a genuine
        // projective term: the unnormalized DLT is measurably wrong on this
        // input, which is why Hartley normalization is not optional.
        let truth = Transform::from_matrix(Matrix3::new(
            1.0002, 0.0031, -41.7, -0.0029, 0.9998, 88.3, 4.1e-7, -2.7e-7, 1.0,
        ))
        .expect("finite");
        let pairs: Vec<Correspondence> = [
            (12.0, 19.0),
            (9400.0, 31.0),
            (60.0, 6300.0),
            (9500.0, 6350.0),
            (4800.0, 3200.0),
        ]
        .into_iter()
        .map(|(x, y)| {
            let from = Point::new(x, y);
            Correspondence {
                from,
                to: truth.apply(from),
            }
        })
        .collect();

        let fitted = estimate(TransformKind::Homography, &pairs).expect("estimate");
        let error = fitted.corner_error(&truth, 9576, 6388);
        assert!(error < 1e-6, "corner error {error:.3e} px");
    }
}

#[cfg(test)]
mod robust_tests {
    use super::tests_support::*;
    use super::*;

    #[test]
    fn ransac_ignores_gross_outliers() {
        let truth = Transform::similarity(1.004, 0.013, 21.5, -8.75);
        let mut pairs = pairs_under(&truth);
        // A quarter of the correspondences are wrong by hundreds of pixels,
        // which is what a mismatched triangle produces.
        for (i, pair) in pairs.iter_mut().enumerate() {
            if i % 4 == 0 {
                pair.to = Point::new(pair.to.x + 400.0 - 13.0 * i as f64, pair.to.y - 250.0);
            }
        }

        let fit = ransac(TransformKind::Similarity, &pairs, &RansacOptions::default())
            .expect("robust fit");
        assert_eq!(fit.inlier_count(), pairs.len() - pairs.len().div_ceil(4));
        let error = fit.transform.corner_error(&truth, WIDTH, HEIGHT);
        assert!(error < 1e-6, "corner error {error:.3e} px");
    }

    #[test]
    fn ransac_is_reproducible_and_seed_dependent() {
        let truth = Transform::shift(31.0, -17.0);
        let mut pairs = pairs_under(&truth);
        pairs[3].to = Point::new(0.0, 0.0);

        let options = RansacOptions::default();
        let first = ransac(TransformKind::Shift, &pairs, &options).expect("fit");
        let again = ransac(TransformKind::Shift, &pairs, &options).expect("fit");
        assert_eq!(first, again);

        // A different seed may take a different path, but must land on the
        // same transform — the consensus set is a property of the data.
        let other = ransac(
            TransformKind::Shift,
            &pairs,
            &RansacOptions {
                seed: 0xDEAD_BEEF,
                ..options
            },
        )
        .expect("fit");
        assert!(
            other
                .transform
                .corner_error(&first.transform, WIDTH, HEIGHT)
                < 1e-9
        );
    }

    #[test]
    fn the_adaptive_count_stops_early_on_clean_data() {
        let truth = Transform::similarity(1.0, 0.01, 5.0, 5.0);
        let pairs = pairs_under(&truth);
        let fit =
            ransac(TransformKind::Similarity, &pairs, &RansacOptions::default()).expect("fit");
        // Every correspondence is an inlier, so one good draw is enough.
        assert!(fit.iterations <= 3, "{} iterations", fit.iterations);
        assert_eq!(fit.inlier_count(), pairs.len());
    }

    #[test]
    fn too_few_correspondences_is_an_error_not_a_guess() {
        let truth = Transform::shift(1.0, 2.0);
        let pairs = pairs_under(&truth);
        let options = RansacOptions::default();
        assert!(ransac(TransformKind::Homography, &pairs[..9], &options).is_err());

        assert!(ransac(TransformKind::Homography, &pairs[..10], &options).is_ok());
    }

    #[test]
    fn refinement_tightens_a_fit_ransac_left_loose() {
        // One correspondence off by 2 px: inside RANSAC's 3 px threshold, but
        // forty times the spread of the rest.
        let truth = Transform::similarity(1.002, 0.008, 11.0, -5.0);
        let mut pairs = pairs_under(&truth);
        pairs[7].to = Point::new(pairs[7].to.x + 2.0, pairs[7].to.y + 0.5);

        let options = RansacOptions::default();
        let loose = ransac(TransformKind::Similarity, &pairs, &options).expect("fit");
        let tight = refine(TransformKind::Similarity, &pairs, &options).expect("fit");

        assert_eq!(
            loose.inlier_count(),
            pairs.len(),
            "the 2 px pair is inside 3 px"
        );
        assert_eq!(tight.inlier_count(), pairs.len() - 1, "refinement drops it");
        assert!(
            tight.rmse < loose.rmse / 10.0,
            "{} vs {}",
            tight.rmse,
            loose.rmse
        );
        assert!(
            tight.transform.corner_error(&truth, WIDTH, HEIGHT)
                < loose.transform.corner_error(&truth, WIDTH, HEIGHT)
        );
    }

    #[test]
    fn refinement_leaves_a_clean_fit_alone() {
        let truth = Transform::similarity(1.0, 0.02, 9.0, 9.0);
        let pairs = pairs_under(&truth);
        let options = RansacOptions::default();
        let fit = refine(TransformKind::Similarity, &pairs, &options).expect("fit");
        assert_eq!(fit.inlier_count(), pairs.len());
    }

    #[test]
    fn the_adaptive_iteration_formula_matches_its_definition() {
        // Half the pairs are inliers and a shift needs one: 1 - 0.5 per draw,
        // so reaching 99.5% takes ceil(ln 0.005 / ln 0.5) = 8 draws.
        assert_eq!(iterations_for(0.5, 1, 0.995), 8);
        // A homography needs four, so the same ratio costs far more.
        assert_eq!(iterations_for(0.5, 4, 0.995), 83);
        assert_eq!(iterations_for(1.0, 4, 0.995), 1);
        assert_eq!(iterations_for(0.0, 4, 0.995), usize::MAX);
    }
}
