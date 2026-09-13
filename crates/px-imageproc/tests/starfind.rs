//! Star detection and PSF fitting against analytic truth (ADR 007, P7-T6).
//!
//! A synthetic field is the only place the *true* centroid of a star is known,
//! so this is where the accuracy bounds are stated: a recovered centroid within
//! 0.02 px and a recovered FWHM within 1% of the profile that was rendered.
//! The parity test in `starfind_parity.rs` then checks those fits against
//! Siril's on real frames, where neither implementation knows the truth.

use px_imageproc::register::psf::fwhm_stats;
use px_imageproc::register::starfind::find_stars_reported;
use px_imageproc::{Profile, PsfStar, StarfindOptions, find_stars};

/// Centroid bound, in pixels.
const CENTROID_TOLERANCE: f64 = 0.02;

/// FWHM bound, as a fraction of the rendered width.
const FWHM_TOLERANCE: f64 = 0.01;

/// `2·sqrt(2·ln 2)`.
const FWHM_PER_SIGMA: f64 = 2.354_820_045_030_949;

struct SplitMix64(u64);

impl SplitMix64 {
    /// Uniform in [-1, 1).
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// One rendered star: centre, peak above background, and profile widths.
#[derive(Debug, Clone, Copy)]
struct Truth {
    x: f64,
    y: f64,
    amplitude: f64,
    sx: f64,
    sy: f64,
    theta: f64,
}

/// Point-samples elliptical Gaussians onto a frame over a flat background plus
/// uniform noise.
///
/// Point sampling rather than pixel integration, because that is the model
/// being fitted: integrating the profile over each pixel would introduce a
/// width bias of its own and the test would be measuring that instead of the
/// solver.
fn render(width: usize, height: usize, background: f64, noise: f64, stars: &[Truth]) -> Vec<f32> {
    let mut rng = SplitMix64(0x5741_1F7A_0000_0001);
    let mut data: Vec<f32> = (0..width * height)
        .map(|_| (background + noise * rng.next_unit()) as f32)
        .collect();

    for star in stars {
        let (cos, sin) = (star.theta.cos(), star.theta.sin());
        // Only the pixels the profile can reach are touched, so rendering a
        // thousand stars onto a large frame stays linear in the star count.
        let reach = (6.0 * star.sx.max(star.sy)).ceil() as isize;
        let (cx, cy) = (star.x.round() as isize, star.y.round() as isize);
        for y in (cy - reach).max(0)..(cy + reach + 1).min(height as isize) {
            for x in (cx - reach).max(0)..(cx + reach + 1).min(width as isize) {
                let (dx, dy) = (x as f64 - star.x, y as f64 - star.y);
                let u = dx * cos + dy * sin;
                let v = -dx * sin + dy * cos;
                let q = 0.5 * (u * u / (star.sx * star.sx) + v * v / (star.sy * star.sy));
                if q < 30.0 {
                    data[y as usize * width + x as usize] += (star.amplitude * (-q).exp()) as f32;
                }
            }
        }
    }
    data
}

/// A grid of stars with per-star sub-pixel offsets, amplitudes, and widths, so
/// the test is not measuring one lucky phase relative to the pixel grid.
fn synthetic_field(count_per_side: usize, spacing: f64, margin: f64) -> Vec<Truth> {
    let mut rng = SplitMix64(0x0BAD_C0DE_1234_5678);
    let mut stars = Vec::new();
    for row in 0..count_per_side {
        for column in 0..count_per_side {
            let jitter_x = 0.5 * rng.next_unit();
            let jitter_y = 0.5 * rng.next_unit();
            stars.push(Truth {
                x: margin + spacing * column as f64 + jitter_x,
                y: margin + spacing * row as f64 + jitter_y,
                amplitude: 0.15 + 0.05 * (row + column) as f64,
                sx: 1.8,
                sy: 1.8,
                theta: 0.0,
            });
        }
    }
    stars
}

/// The fitted star nearest `truth`, and its distance.
fn nearest(stars: &[PsfStar], truth: &Truth) -> Option<(PsfStar, f64)> {
    stars
        .iter()
        .map(|s| {
            let d = ((s.x0 - truth.x).powi(2) + (s.y0 - truth.y).powi(2)).sqrt();
            (*s, d)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[test]
fn recovers_every_star_of_a_synthetic_field() {
    let stars = synthetic_field(8, 60.0, 40.0);
    let width = 520;
    let height = 520;
    let data = render(width, height, 0.02, 0.0004, &stars);

    let options = StarfindOptions::default();
    let (found, report) = find_stars_reported(&data, width, height, &options, 0);
    assert_eq!(found.len(), stars.len(), "{report:?}");

    let mut worst_centroid = 0.0f64;
    let mut worst_fwhm = 0.0f64;
    for truth in &stars {
        let (fit, distance) = nearest(&found, truth).expect("a fitted star near every truth");
        worst_centroid = worst_centroid.max(distance);

        let truth_fwhm = FWHM_PER_SIGMA * truth.sx;
        let relative = (fit.fwhm() - truth_fwhm).abs() / truth_fwhm;
        worst_fwhm = worst_fwhm.max(relative);
    }

    println!("worst centroid error {worst_centroid:.4} px, worst FWHM error {worst_fwhm:.4}");
    assert!(
        worst_centroid <= CENTROID_TOLERANCE,
        "worst centroid error {worst_centroid:.4} px exceeds {CENTROID_TOLERANCE}"
    );
    assert!(
        worst_fwhm <= FWHM_TOLERANCE,
        "worst FWHM error {:.2}% exceeds {:.2}%",
        100.0 * worst_fwhm,
        100.0 * FWHM_TOLERANCE
    );
}

#[test]
fn recovers_elongated_stars_when_checks_are_relaxed() {
    // A trailed field: every star 2.5:1 and rotated, which the default
    // roundness floor rejects by design.
    let truth: Vec<Truth> = synthetic_field(4, 80.0, 50.0)
        .into_iter()
        .map(|t| Truth {
            sx: 3.0,
            sy: 1.2,
            theta: 0.6,
            ..t
        })
        .collect();
    let data = render(400, 400, 0.02, 0.0004, &truth);

    let strict = find_stars(&data, 400, 400, &StarfindOptions::default());
    assert!(strict.is_empty(), "{strict:?}");

    let options = StarfindOptions {
        relax_checks: true,
        ..Default::default()
    };
    let found = find_stars(&data, 400, 400, &options);
    assert_eq!(found.len(), truth.len());

    for t in &truth {
        let (fit, distance) = nearest(&found, t).expect("a fit near every truth");
        assert!(distance <= CENTROID_TOLERANCE, "{distance:.4} px: {fit:?}");
        let truth_fwhm = FWHM_PER_SIGMA * t.sx;
        assert!(
            (fit.fwhm_x - truth_fwhm).abs() / truth_fwhm <= FWHM_TOLERANCE,
            "fwhm_x {} vs {truth_fwhm}",
            fit.fwhm_x
        );
        assert!(
            (fit.angle - t.theta.to_degrees()).abs() < 1.0,
            "{}",
            fit.angle
        );
    }
}

#[test]
fn the_moffat_profile_recovers_the_same_centroids() {
    let stars = synthetic_field(4, 80.0, 50.0);
    let data = render(400, 400, 0.02, 0.0004, &stars);

    let options = StarfindOptions {
        profile: Profile::Moffat,
        ..Default::default()
    };
    let (found, report) = find_stars_reported(&data, 400, 400, &options, 0);
    assert_eq!(found.len(), stars.len(), "{report:?}");

    for truth in &stars {
        let (fit, distance) = nearest(&found, truth).expect("a fit near every truth");
        // A Moffat fitted to a Gaussian star gets the centre right and the
        // width approximately right, which is the published trade: its extra
        // parameter buys wing accuracy, not centroid accuracy.
        assert!(distance <= 0.05, "{distance:.4} px: {fit:?}");
        assert!(fit.beta.is_some());
    }
}

#[test]
fn pure_noise_yields_no_stars() {
    // The false-positive rate matters as much as the detection rate: a
    // transform fitted to noise peaks is worse than no transform.
    let data = render(512, 512, 0.02, 0.002, &[]);
    let (found, report) = find_stars_reported(&data, 512, 512, &StarfindOptions::default(), 0);
    assert!(found.is_empty(), "{found:?} from {report:?}");
}

#[test]
fn fwhm_stats_describe_the_rendered_field() {
    let stars = synthetic_field(5, 70.0, 50.0);
    let data = render(450, 450, 0.02, 0.0004, &stars);
    let found = find_stars(&data, 450, 450, &StarfindOptions::default());

    let stats = fwhm_stats(&found).expect("stars found");
    let truth_fwhm = FWHM_PER_SIGMA * 1.8;
    assert_eq!(stats.count, stars.len());
    assert!(
        (stats.median - truth_fwhm).abs() / truth_fwhm <= FWHM_TOLERANCE,
        "median fwhm {} vs {truth_fwhm}",
        stats.median
    );
    // Every star was rendered at the same width, so the spread is noise only.
    assert!(stats.sigma < 0.05 * truth_fwhm, "sigma {}", stats.sigma);
    assert!(stats.mean_roundness > 0.95, "{}", stats.mean_roundness);
}
