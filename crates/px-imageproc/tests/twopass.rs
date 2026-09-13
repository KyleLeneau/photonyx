//! The two-pass registration loop (ADR 007, P9-T2 / P9-T4).
//!
//! Works on real files, because that is what the loop does: it opens frames one
//! budget-bounded chunk at a time, and the thing most likely to break is the
//! bookkeeping that ties a star list back to the frame it came from.

use std::path::PathBuf;

use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::register::framing::Framing;
use px_imageproc::register::transform::{Point, Transform};
use px_imageproc::register::twopass::{
    Excluded, MAX_TRIALS_2PASS, RegistrationOptions, register_sequence,
};
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::{FrameSet, ImageDims};

const WIDTH: usize = 600;
const HEIGHT: usize = 500;
const GB: u64 = 1 << 30;

struct SplitMix64(u64);

impl SplitMix64 {
    /// Uniform in [0, 1).
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A star field in sky coordinates: position and brightness.
fn sky(count: usize, seed: u64) -> Vec<(f64, f64, f64)> {
    let mut rng = SplitMix64(seed);
    (0..count)
        .map(|_| {
            (
                0.1 * WIDTH as f64 + 0.8 * WIDTH as f64 * rng.next_unit(),
                0.1 * HEIGHT as f64 + 0.8 * HEIGHT as f64 * rng.next_unit(),
                0.25 + 0.5 * rng.next_unit(),
            )
        })
        .collect()
}

/// Renders one frame of `sky` as seen through `transform` — which maps this
/// frame's pixels onto the sky's reference grid, so rendering maps the other way.
///
/// `scale` multiplies every star's amplitude, which is how a frame is made to
/// look fainter (fewer detectable stars) without moving anything.
fn write_frame(
    dir: &std::path::Path,
    name: &str,
    stars: &[(f64, f64, f64)],
    transform: &Transform,
    fwhm: f64,
    scale: f64,
) -> PathBuf {
    let path = dir.join(format!("{name}.fits"));
    let inverse = transform.invert().expect("invertible");
    let sigma = fwhm / 2.3548;

    let mut rng = SplitMix64(0xFEED_0001);
    let mut data: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|_| (0.02 + 0.0006 * (rng.next_unit() - 0.5)) as f32)
        .collect();

    for &(x, y, amplitude) in stars {
        let p = inverse.apply(Point::new(x, y));
        let reach = (5.0 * sigma).ceil() as isize;
        let (cx, cy) = (p.x.round() as isize, p.y.round() as isize);
        for row in (cy - reach).max(0)..(cy + reach + 1).min(HEIGHT as isize) {
            for column in (cx - reach).max(0)..(cx + reach + 1).min(WIDTH as isize) {
                let (dx, dy) = (column as f64 - p.x, row as f64 - p.y);
                let q = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                if q < 25.0 {
                    data[row as usize * WIDTH + column as usize] +=
                        (scale * amplitude * (-q).exp()) as f32;
                }
            }
        }
    }

    let header =
        HeaderBuilder::primary_image(BitPix::F32, &[WIDTH as u64, HEIGHT as u64]).expect("header");
    let mut writer = FitsWriter::create(&path).expect("create");
    writer.write_image(&header, &data).expect("write");
    writer.finish().expect("finish");
    path
}

fn runtime() -> Runtime {
    Runtime::new(ResourceBudget::new(GB, 4)).expect("runtime")
}

/// A sequence where frame 2 is the sharpest and fullest, so it should be chosen
/// as the reference; every frame is dithered by a few pixels.
fn dithered_sequence(dir: &std::path::Path) -> Vec<PathBuf> {
    let stars = sky(120, 0xA11CE);
    let shifts = [
        (0.0, 0.0),
        (7.0, -4.0),
        (-3.0, 6.0),
        (11.0, 9.0),
        (-8.0, -2.0),
    ];
    let fwhms = [3.4, 3.2, 2.6, 3.6, 3.3];

    shifts
        .iter()
        .zip(fwhms)
        .enumerate()
        .map(|(i, (&(dx, dy), fwhm))| {
            write_frame(
                dir,
                &format!("f{i:02}"),
                &stars,
                &Transform::shift(dx, dy),
                fwhm,
                1.0,
            )
        })
        .collect()
}

#[test]
fn registers_a_dithered_sequence_and_picks_the_sharpest_reference() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");

    let registration =
        register_sequence(&set, &runtime(), &RegistrationOptions::default()).expect("register");

    assert_eq!(
        registration.reference, 2,
        "the sharpest frame is the reference"
    );
    assert_eq!(registration.included_count(), paths.len());
    assert_eq!(registration.trials, 1, "clean data needs one trial");

    // The reference maps onto itself, and every other frame's transform must
    // recover the dither it was rendered with, relative to the reference's.
    let reference_shift = (-3.0, 6.0);
    let expected = [
        (0.0, 0.0),
        (7.0, -4.0),
        (-3.0, 6.0),
        (11.0, 9.0),
        (-8.0, -2.0),
    ];
    for (index, (dx, dy)) in expected.iter().enumerate() {
        let transform = registration.frames[index]
            .transform
            .expect("every frame registered");
        let truth = Transform::shift(dx - reference_shift.0, dy - reference_shift.1);
        let error = transform.corner_error(&truth, WIDTH, HEIGHT);
        assert!(error < 0.05, "frame {index}: corner error {error:.4} px");
    }
}

#[test]
fn the_reference_maps_onto_itself_exactly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");
    let registration =
        register_sequence(&set, &runtime(), &RegistrationOptions::default()).expect("register");

    let reference = &registration.frames[registration.reference];
    assert_eq!(reference.transform, Some(Transform::identity()));
    assert!(reference.included());
}

#[test]
fn a_starless_frame_is_excluded_with_a_reason() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stars = sky(120, 0xB0B);
    let mut paths: Vec<PathBuf> = (0..4)
        .map(|i| {
            write_frame(
                dir.path(),
                &format!("g{i:02}"),
                &stars,
                &Transform::shift(3.0 * i as f64, -2.0 * i as f64),
                3.0,
                1.0,
            )
        })
        .collect();
    // A frame with nothing in it but sky: clouded out.
    paths.push(write_frame(
        dir.path(),
        "clouded",
        &stars,
        &Transform::identity(),
        3.0,
        0.0,
    ));

    let set = FrameSet::open(&paths).expect("open set");
    let registration =
        register_sequence(&set, &runtime(), &RegistrationOptions::default()).expect("register");

    let clouded = registration.frames.last().expect("five frames");
    assert!(!clouded.included());
    assert_eq!(clouded.excluded, Some(Excluded::NoStars));
    assert_eq!(clouded.stars, 0);
    assert_eq!(registration.included_count(), 4);

    // And the included frames are the other four, with their reasons empty.
    for frame in registration.included() {
        assert!(frame.excluded.is_none());
        assert!(frame.stars > 50, "{frame:?}");
    }
}

#[test]
fn an_over_strict_threshold_is_recovered_by_a_later_trial() {
    // The detector is started at a threshold a thousand times too high, which
    // finds nothing; the loop has to open it up. This is the mechanism that
    // rescues a real faint sequence, exercised without having to guess an
    // amplitude that happens to straddle the threshold.
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");

    let mut options = RegistrationOptions::default();
    options.starfind.sigma = 1000.0;

    let registration = register_sequence(&set, &runtime(), &options).expect("register");
    assert!(registration.trials > 1, "should have needed a retry");
    assert!(registration.trials <= MAX_TRIALS_2PASS);
    assert_eq!(registration.included_count(), paths.len());
    assert!(
        registration.starfind.sigma < options.starfind.sigma,
        "the accepted trial used a lower threshold: {}",
        registration.starfind.sigma
    );
}

#[test]
fn a_sharp_odd_frame_out_does_not_become_the_reference() {
    // The frame showing a different patch of sky has the most stars, so quality
    // ranking alone would choose it — and then nothing else would register. The
    // loop has to fall back to the next candidate.
    let dir = tempfile::tempdir().expect("tempdir");
    let stars = sky(120, 0xC0FFEE);
    let elsewhere = sky(140, 0xDEAD);
    let mut paths: Vec<PathBuf> = (0..4)
        .map(|i| {
            write_frame(
                dir.path(),
                &format!("i{i:02}"),
                &stars,
                &Transform::shift(2.0 * i as f64, i as f64),
                3.0,
                1.0,
            )
        })
        .collect();
    paths.push(write_frame(
        dir.path(),
        "elsewhere",
        &elsewhere,
        &Transform::identity(),
        2.4,
        1.0,
    ));

    let set = FrameSet::open(&paths).expect("open set");
    let registration =
        register_sequence(&set, &runtime(), &RegistrationOptions::default()).expect("register");

    assert!(
        registration.reference < 4,
        "reference should be one of the four frames of the same field, got {}",
        registration.reference
    );
    assert_eq!(registration.included_count(), 4);
    let stranger = registration.frames.last().expect("five frames");
    assert_eq!(stranger.excluded, Some(Excluded::MatchFailed));
    assert!(
        stranger.stars > 50,
        "it has stars, they just are not these stars"
    );
}

#[test]
fn minimum_framing_trims_the_output_to_the_common_area() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");

    let options = RegistrationOptions {
        framing: Framing::Minimum,
        ..Default::default()
    };
    let registration = register_sequence(&set, &runtime(), &options).expect("register");

    let trimmed = registration.geometry.dims;
    assert!(trimmed.width < WIDTH, "{trimmed:?}");
    assert!(trimmed.height < HEIGHT, "{trimmed:?}");

    // The output transform of the reference is the framing shift itself, so the
    // ROI origin lands on the output origin.
    let shifted = registration
        .output_transform(registration.reference)
        .expect("reference transform");
    let origin = shifted.apply(Point::new(registration.geometry.x, registration.geometry.y));
    assert!(origin.x.abs() < 1e-9 && origin.y.abs() < 1e-9, "{origin:?}");
}

#[test]
fn current_framing_keeps_the_reference_geometry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");
    let registration =
        register_sequence(&set, &runtime(), &RegistrationOptions::default()).expect("register");
    assert_eq!(registration.geometry.dims, ImageDims::new(WIDTH, HEIGHT, 1));
}

#[test]
fn a_tight_budget_changes_concurrency_but_not_the_result() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = dithered_sequence(dir.path());
    let set = FrameSet::open(&paths).expect("open set");
    let options = RegistrationOptions::default();

    let generous = register_sequence(&set, &runtime(), &options).expect("register");
    // Enough for one frame at a time and no more.
    let tight = Runtime::new(ResourceBudget::new(8 * 1024 * 1024, 4)).expect("runtime");
    let frugal = register_sequence(&set, &tight, &options).expect("register");

    assert_eq!(generous.reference, frugal.reference);
    assert_eq!(generous.included_count(), frugal.included_count());
    for (a, b) in generous.frames.iter().zip(frugal.frames.iter()) {
        assert_eq!(a.transform, b.transform, "frame {}", a.index);
    }
}

#[test]
fn registration_data_round_trips_through_a_sidecar_table() {
    use px_imageproc::register::persist::{read_registration, write_registration};

    let dir = tempfile::tempdir().expect("tempdir");
    let stars = sky(120, 0x9001);
    let mut paths = dithered_sequence(dir.path());
    // Include an excluded frame, so the reason and the "no transform" state are
    // part of what round-trips.
    paths.push(write_frame(
        dir.path(),
        "clouded",
        &stars,
        &Transform::identity(),
        3.0,
        0.0,
    ));
    let set = FrameSet::open(&paths).expect("open set");

    let options = RegistrationOptions {
        framing: Framing::Minimum,
        ..Default::default()
    };
    let written = register_sequence(&set, &runtime(), &options).expect("register");
    assert!(
        written.frames.iter().any(|f| !f.included()),
        "need an excluded frame"
    );

    let sidecar = dir.path().join("registration.fits");
    write_registration(&sidecar, &written).expect("write sidecar");
    let read = read_registration(&sidecar).expect("read sidecar");

    assert_eq!(read.reference, written.reference);
    assert_eq!(read.trials, written.trials);
    assert_eq!(read.geometry.dims, written.geometry.dims);
    assert_eq!(
        (read.geometry.x, read.geometry.y),
        (written.geometry.x, written.geometry.y)
    );
    assert_eq!(read.starfind, written.starfind);
    assert_eq!(read.frames.len(), written.frames.len());

    for (after, before) in read.frames.iter().zip(written.frames.iter()) {
        assert_eq!(after.index, before.index);
        assert_eq!(after.path, before.path);
        assert_eq!(after.excluded, before.excluded);
        assert_eq!(after.included(), before.included());
        assert_eq!(after.stars, before.stars);
        assert_eq!(after.inliers, before.inliers);
        // FWHM, roundness and RMSE are stored as `E` (f32), so they come back
        // to f32 precision. The transform is stored as `D` and must be exact:
        // a matrix rounded to f32 moves a 9576 px corner by ~0.001 px, which is
        // a tenth of the Phase 8 parity gate.
        assert!((after.fwhm - before.fwhm).abs() < 1e-5, "{after:?}");
        assert!((after.roundness - before.roundness).abs() < 1e-6);
        assert!((after.rmse - before.rmse).abs() < 1e-6);
        assert_eq!(after.transform, before.transform, "frame {}", after.index);
    }
}

#[test]
fn a_sidecar_without_registration_cards_is_a_typed_error() {
    use px_imageproc::register::persist::read_registration;

    let dir = tempfile::tempdir().expect("tempdir");
    let stars = sky(10, 0x1234);
    let frame = write_frame(
        dir.path(),
        "plain",
        &stars,
        &Transform::identity(),
        3.0,
        1.0,
    );
    assert!(read_registration(&frame).is_err());
}
