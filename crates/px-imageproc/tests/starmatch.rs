//! Star matching and transform estimation against a known transform
//! (ADR 007, P8-T6, synthetic half).
//!
//! The accuracy bar is stated in corner reprojection error, in pixels, over a
//! 61 MP frame: ≤ 0.01 px for a list recovered under realistic contamination.
//! The real-frame half of P8-T6 lives in `starmatch_parity.rs`, which compares
//! against Siril's own homographies.

use nalgebra::Matrix3;
use px_imageproc::PsfStar;
use px_imageproc::register::starmatch::{MatchOptions, match_stars, register_pair};
use px_imageproc::register::transform::{Point, RansacOptions, Transform, TransformKind};

const WIDTH: usize = 9576;
const HEIGHT: usize = 6388;

/// Corner-error bound for a recovered transform, in pixels.
const CORNER_TOLERANCE: f64 = 0.01;

struct SplitMix64(u64);

impl SplitMix64 {
    /// Uniform in [-1, 1).
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// A star at a position, with an amplitude that orders the list.
fn star(x: f64, y: f64, amplitude: f64) -> PsfStar {
    PsfStar {
        x0: x,
        y0: y,
        sx: 1.8,
        sy: 1.8,
        fwhm_x: 4.2,
        fwhm_y: 4.2,
        angle: 0.0,
        amplitude,
        background: 0.01,
        beta: None,
        roundness: 1.0,
        rmse: 1e-4,
        peak: amplitude,
        layer: 0,
    }
}

/// A star field scattered over the frame, brightest first.
///
/// Scattered rather than gridded: a regular grid produces many congruent
/// triangles, which is the one input on which triangle voting is ambiguous for
/// reasons that have nothing to do with a real sky.
fn reference_field(count: usize, seed: u64) -> Vec<PsfStar> {
    let mut rng = SplitMix64(seed);
    let mut stars: Vec<PsfStar> = (0..count)
        .map(|i| {
            let x = (0.5 + 0.48 * rng.next_unit()) * WIDTH as f64;
            let y = (0.5 + 0.48 * rng.next_unit()) * HEIGHT as f64;
            star(x, y, 1.0 - i as f64 / count as f64)
        })
        .collect();
    stars.sort_by(|a, b| b.amplitude.total_cmp(&a.amplitude));
    stars
}

/// Maps a field through `truth`, optionally perturbing it the way a real frame
/// would: centroid noise, stars that fell off the detection threshold, and
/// spurious detections that are not stars at all.
fn observed(
    reference: &[PsfStar],
    truth: &Transform,
    centroid_noise: f64,
    drop_every: usize,
    spurious: usize,
    seed: u64,
) -> Vec<PsfStar> {
    let mut rng = SplitMix64(seed);
    let inverse = truth.invert().expect("invertible truth");

    let mut stars: Vec<PsfStar> = reference
        .iter()
        .enumerate()
        .filter(|(i, _)| drop_every == 0 || !i.is_multiple_of(drop_every))
        .map(|(_, s)| {
            let mapped = inverse.apply(Point::new(s.x0, s.y0));
            star(
                mapped.x + centroid_noise * rng.next_unit(),
                mapped.y + centroid_noise * rng.next_unit(),
                s.amplitude,
            )
        })
        .collect();

    for i in 0..spurious {
        let x = (0.5 + 0.48 * rng.next_unit()) * WIDTH as f64;
        let y = (0.5 + 0.48 * rng.next_unit()) * HEIGHT as f64;
        // Bright enough to land inside the matcher's brightest-N window, which
        // is where a spurious detection does damage.
        stars.push(star(x, y, 0.9 - 0.01 * i as f64));
    }
    stars.sort_by(|a, b| b.amplitude.total_cmp(&a.amplitude));
    stars
}

fn dither_and_rotate() -> Transform {
    // A realistic sub-to-sub difference: a few tens of pixels of dither, a
    // fraction of a degree of field rotation, and a small projective term.
    Transform::from_matrix(Matrix3::new(
        1.000_12, 0.003_1, -41.7, -0.003_0, 0.999_88, 88.3, 2.1e-8, -1.4e-8, 1.0,
    ))
    .expect("finite")
}

fn assert_recovers(truth: &Transform, target: &[PsfStar], reference: &[PsfStar]) {
    let fit = register_pair(
        target,
        reference,
        TransformKind::Homography,
        &MatchOptions::default(),
        &RansacOptions::default(),
    )
    .expect("registration succeeds");

    let error = fit.transform.corner_error(truth, WIDTH, HEIGHT);
    println!(
        "inliers {}, rmse {:.4} px, corner error {:.5} px",
        fit.inlier_count(),
        fit.rmse,
        error
    );
    assert!(
        error <= CORNER_TOLERANCE,
        "corner error {error:.5} px exceeds {CORNER_TOLERANCE}"
    );
}

#[test]
fn recovers_a_known_homography_from_clean_lists() {
    let truth = dither_and_rotate();
    let reference = reference_field(600, 0xA11CE);
    let target = observed(&reference, &truth, 0.0, 0, 0, 0xB0B);
    assert_recovers(&truth, &target, &reference);
}

#[test]
fn recovers_a_known_homography_under_realistic_contamination() {
    // Centroid noise at twice the measured single-star accuracy, one star in
    // six missing, and ten spurious detections bright enough to matter.
    let truth = dither_and_rotate();
    let reference = reference_field(800, 0xC0FFEE);
    let target = observed(&reference, &truth, 0.02, 6, 10, 0xFEED);
    assert_recovers(&truth, &target, &reference);
}

#[test]
fn recovers_a_pure_dither_shift() {
    // The common case: no rotation at all, just guiding dither.
    let truth = Transform::shift(-37.0, 61.0);
    let reference = reference_field(500, 0x5EED);
    let target = observed(&reference, &truth, 0.03, 0, 0, 0x1234);

    let fit = register_pair(
        &target,
        &reference,
        TransformKind::Homography,
        &MatchOptions::default(),
        &RansacOptions::default(),
    )
    .expect("registration succeeds");
    assert!(fit.transform.corner_error(&truth, WIDTH, HEIGHT) <= CORNER_TOLERANCE);
}

#[test]
fn matching_recovers_the_right_pairs_not_merely_enough_pairs() {
    let truth = dither_and_rotate();
    let reference = reference_field(200, 0x7A57E);
    let target = observed(&reference, &truth, 0.02, 0, 0, 0x9999);

    let pairs = match_stars(&target, &reference, &MatchOptions::default()).expect("match");
    assert!(pairs.len() >= 30, "matched only {}", pairs.len());

    // Every returned correspondence must be the *correct* one: the target star
    // mapped through the truth must land on the reference star it was paired
    // with. A matcher that returns plausible-looking wrong pairs still fits a
    // transform, just the wrong one.
    for pair in &pairs {
        let mapped = truth.apply(pair.from);
        assert!(
            mapped.distance(&pair.to) < 0.1,
            "{:?} maps to {mapped:?}, paired with {:?}",
            pair.from,
            pair.to
        );
    }
}

#[test]
fn a_rotated_field_matches_but_a_reflected_one_does_not() {
    // Handedness is part of the triangle's identity. A reflected field has the
    // same side ratios everywhere, so a matcher that ignored handedness would
    // happily pair stars across a mirror — which no pair of frames from one
    // camera ever needs.
    let reference = reference_field(400, 0xBEEF);
    let rotated = Transform::similarity(1.0, 0.35, 120.0, -80.0);
    let target = observed(&reference, &rotated, 0.02, 0, 0, 0x4321);
    assert_recovers(&rotated, &target, &reference);

    let mirrored: Vec<PsfStar> = target
        .iter()
        .map(|s| star(WIDTH as f64 - s.x0, s.y0, s.amplitude))
        .collect();
    let result = match_stars(&mirrored, &reference, &MatchOptions::default());
    assert!(result.is_err(), "{:?}", result.map(|p| p.len()));
}

#[test]
fn unrelated_fields_do_not_match() {
    // Two different patches of sky. The matcher must report that it found
    // nothing rather than return the best-voted noise: a wrong transform
    // silently ruins a stack, where a failure drops one frame.
    let reference = reference_field(300, 0x1111);
    let other = reference_field(300, 0x2222);
    let result = match_stars(&other, &reference, &MatchOptions::default());
    assert!(result.is_err(), "{:?}", result.map(|p| p.len()));
}

#[test]
fn too_few_stars_to_form_a_triangle_is_an_error() {
    let reference = reference_field(2, 0x3333);
    let result = match_stars(&reference, &reference, &MatchOptions::default());
    assert!(result.is_err());
}

#[test]
fn a_simpler_transform_class_still_recovers_a_shift() {
    // Fitting fewer parameters than the data supports is the safe direction,
    // and it is what Phase 9 falls back to when a frame has few stars.
    let truth = Transform::shift(24.0, -13.0);
    let reference = reference_field(300, 0x4444);
    let target = observed(&reference, &truth, 0.02, 0, 0, 0x5555);

    for kind in [
        TransformKind::Shift,
        TransformKind::Similarity,
        TransformKind::Affine,
        TransformKind::Homography,
    ] {
        let fit = register_pair(
            &target,
            &reference,
            kind,
            &MatchOptions::default(),
            &RansacOptions::default(),
        )
        .expect("fit");
        // No assertion on `kind()` here: it reports what the matrix *is*, and a
        // similarity fitted to a pure shift correctly comes back as a shift —
        // its rotation and scale terms are zero to within the classification
        // tolerance.
        let error = fit.transform.corner_error(&truth, WIDTH, HEIGHT);
        assert!(error <= CORNER_TOLERANCE, "{kind:?}: {error:.5} px");
    }
}
