//! Registration parity against Siril's own homographies (ADR 007, P8-T6).
//!
//! Reference data comes from `cargo xtask siril-reference --registration`,
//! which two-pass registers a run of light frames with a local Siril and keeps
//! the sequence file it writes. That file carries one homography per frame, so
//! the comparison is against the transform Siril actually used, not against a
//! re-derivation of it.
//!
//! The measure is corner reprojection error: the largest distance, over the four
//! corners of the frame, between where Siril's transform sends a corner and
//! where ours does. Matrix coefficients are not compared — they trade off
//! against each other, and two matrices differing in every coefficient can agree
//! everywhere on the image.
//!
//! **A flat corner-error bound is not the whole gate, because on many frames a
//! homography cannot describe the mapping to better than a few tenths of a
//! pixel.** Measured on this corpus, our own fit leaves an RMS residual of
//! 0.106 px median and 0.455 px at worst over ~1,700 correspondences, and those
//! residuals are spatially structured — binned into a 3x3 grid their means run to
//! 0.4 px where noise would give 0.02 px — which is unmodelled optical
//! distortion, not measurement error. Two implementations fitting an inadequate
//! model to the same stars land a fraction of that misfit apart, and no
//! implementation could do better; Siril would not agree with itself on a
//! different star subset either.
//!
//! So the gate is stated in three parts, following the precedent of Phase 3's
//! master-flat bound: an absolute corner bound on the frames where the model
//! does fit, a scale-free bound relative to each frame's own misfit, and a
//! direction-of-difference check that our transform is the better least-squares
//! fit to the same correspondences (ADR 007 D2).
//!
//! Neither the corpus nor the reference is committed, so these tests skip when
//! they are absent.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use nalgebra::Matrix3;
use px_imageproc::register::starmatch::{MatchOptions, pair_by_proximity, register_pair};
use px_imageproc::register::transform::{RansacOptions, Transform, TransformKind};
use px_imageproc::{Frame, PsfStar, StarfindOptions, find_stars};

/// Distance within which a star is paired with a mapped position when scoring
/// the two transforms against one correspondence set. The same figure
/// `register_pair` uses internally.
const PAIR_TOLERANCE: f64 = 2.0;

/// Corner reprojection bound, in pixels, for frames where a homography does
/// describe the mapping — taken to be those whose own fit residual is below the
/// same figure. Measured: 42 of 94 frames qualify, with a median corner error of
/// 0.053 px and a worst of 0.091 px.
const CORNER_TOLERANCE: f64 = 0.1;

/// Scale-free bound for every frame: the disagreement between the two
/// transforms, as a multiple of the RMS residual our own fit leaves on the
/// correspondences. Measured over the corpus at 0.49 median, 0.89 at the 90th
/// percentile and 1.46 at worst, so 2.0 is the bound with room either side.
///
/// This is the part of the gate that means something on a frame the model
/// misfits: it says the two implementations never differ by more than twice the
/// scatter they are both fitting through.
const MAX_CORNER_PER_RESIDUAL: f64 = 2.0;

/// How much worse our fit's residual is allowed to be than Siril's transform on
/// the same correspondences.
///
/// Measured: ours is the better fit on 92 of 94 frames, and worse by 0.1% on the
/// other two — which is two least-squares solutions of the same problem
/// differing in their last digits, not a quality difference.
const MAX_RESIDUAL_EXCESS: f64 = 0.01;

/// Frames that must be compared for the gate to be considered measured.
///
/// The reference is registered against, not compared, and Siril excludes from
/// its own sequence any frame it could not register — there is no transform of
/// its to compare against for those. So the bar is stated a little below the
/// hundred frames the run covers.
const MIN_FRAMES: usize = 90;

/// Siril measures y upward from the bottom of the frame, where this crate
/// measures it downward from the top (ADR 007 Phase 7 records the same
/// difference for star positions). A transform expressed in one convention
/// becomes the other by conjugation with the flip `y -> (height - 1) - y`,
/// which is its own inverse.
fn flip(height: usize) -> Matrix3<f64> {
    Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, (height - 1) as f64, 0.0, 0.0, 1.0)
}

/// One frame's registration data as Siril recorded it.
#[derive(Debug, Clone)]
struct SirilRegistration {
    /// 1-based sequence index.
    index: usize,
    fwhm: f64,
    stars: usize,
    matrix: Matrix3<f64>,
    /// Siril's own selection flag, from the sequence's `I` lines.
    selected: bool,
}

struct Reference {
    corpus: PathBuf,
    /// Sequence index to frame path, relative to the corpus.
    frames: HashMap<usize, String>,
    registrations: Vec<SirilRegistration>,
    /// 1-based index of the frame Siril chose as its reference.
    reference_index: usize,
}

impl SirilRegistration {
    /// Whether Siril kept this frame. A frame it failed to register stays in the
    /// sequence file with its selection flag cleared, and whatever matrix is on
    /// its line is not a transform Siril ever used.
    fn usable(&self) -> bool {
        self.selected
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// Parses Siril's sequence file.
///
/// Three line kinds matter. `S 'name' start nb_images nb_selected fixed_len
/// reference_image ...` names the reference frame, 0-based. `I <index>
/// <selected>` carries Siril's per-frame selection flag. `R<layer> fwhm
/// weighted_fwhm roundness background quality nb_stars H h00 … h22` carries one
/// frame's registration, in sequence order.
fn parse_sequence(text: &str) -> Option<(usize, Vec<SirilRegistration>)> {
    let mut reference_index = None;
    let mut registrations = Vec::new();
    let mut selected: HashMap<usize, bool> = HashMap::new();

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("S ") {
            // The sequence name is quoted and may contain spaces, so fields are
            // counted from after the closing quote.
            let after_name = rest.rsplit_once('\'').map(|(_, tail)| tail)?;
            let fields: Vec<&str> = after_name.split_whitespace().collect();
            // start_index, nb_images, nb_selected, fixed_len, reference_image
            reference_index = fields.get(4)?.parse::<usize>().ok().map(|i| i + 1);
        } else if let Some(rest) = line.strip_prefix("I ") {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            let index = fields.first()?.parse::<usize>().ok()?;
            selected.insert(index, fields.get(1).is_some_and(|f| *f != "0"));
        } else if line.starts_with('R') {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let h = fields.iter().position(|f| *f == "H")?;
            let value = |i: usize| fields.get(i)?.parse::<f64>().ok();
            let coefficients: Vec<f64> = (h + 1..h + 10).filter_map(value).collect();
            if coefficients.len() != 9 {
                continue;
            }
            let index = registrations.len() + 1;
            registrations.push(SirilRegistration {
                index,
                fwhm: value(1)?,
                stars: value(6)? as usize,
                matrix: Matrix3::from_row_slice(&coefficients),
                selected: selected.get(&index).copied().unwrap_or(true),
            });
        }
    }
    Some((reference_index?, registrations))
}

fn reference() -> Option<Reference> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"))
        .join("registration");

    let text = std::fs::read_to_string(dir.join("work/reg_.seq")).ok()?;
    let (reference_index, registrations) = parse_sequence(&text)?;

    let manifest = std::fs::read_to_string(dir.join("frames.txt")).ok()?;
    let frames: HashMap<usize, String> = manifest
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(index, path)| Some((index.trim().parse().ok()?, path.trim().to_string())))
        .collect();

    let corpus = root.join("data/corpus-mono");
    if !corpus.is_dir() || frames.is_empty() {
        return None;
    }
    Some(Reference {
        corpus,
        frames,
        registrations,
        reference_index,
    })
}

/// Our star list for one frame, with Siril's registration defaults (ADR 007
/// D10): radius 5, sigma 1.0, roundness 0.5, 2000 stars.
fn stars_of(path: &Path) -> (Vec<PsfStar>, usize, usize) {
    let frame = Frame::open(path).expect("open corpus frame");
    let stars = find_stars(
        frame.channel(0),
        frame.width(),
        frame.height(),
        &StarfindOptions::default(),
    );
    (stars, frame.width(), frame.height())
}

/// One frame's comparison.
struct FrameResult {
    frame: String,
    siril_stars: usize,
    our_stars: usize,
    inliers: usize,
    /// RMS residual our transform leaves on the correspondences — the frame's
    /// own misfit, and the scale everything else here is measured against.
    our_residual: f64,
    /// RMS residual Siril's transform leaves on the *same* correspondences.
    siril_residual: f64,
    corner_error: f64,
}

impl FrameResult {
    /// Corner disagreement as a multiple of the frame's own misfit.
    fn corner_per_residual(&self) -> f64 {
        if self.our_residual > 0.0 {
            self.corner_error / self.our_residual
        } else {
            0.0
        }
    }

    /// Whether a homography describes this frame's mapping well enough for the
    /// absolute corner bound to be a meaningful test.
    fn model_fits(&self) -> bool {
        self.our_residual < CORNER_TOLERANCE
    }
}

/// Reads and registers a hundred 61 MP frames, which is minutes even optimized,
/// so this runs under `--release`:
///
/// ```text
/// cargo test --release -p px-imageproc --test starmatch_parity -- --nocapture
/// ```
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "registers 61 MP frames; run with --release"
)]
fn transforms_match_sirils_on_real_frames() {
    let Some(reference) = reference() else {
        eprintln!(
            "skipping: no Siril registration reference. Run \
             `cargo xtask siril-reference --registration` with a corpus under data/corpus-mono."
        );
        return;
    };

    let reference_path = reference.corpus.join(
        reference
            .frames
            .get(&reference.reference_index)
            .expect("reference frame in manifest"),
    );
    let (reference_stars, width, height) = stars_of(&reference_path);
    println!(
        "siril's reference frame is #{} with {} stars of ours",
        reference.reference_index,
        reference_stars.len()
    );

    let to_our_convention = {
        let f = flip(height);
        move |m: Matrix3<f64>| f * m * f
    };

    let mut results = Vec::new();
    let mut siril_skipped = Vec::new();
    for registration in &reference.registrations {
        if registration.index == reference.reference_index {
            continue;
        }
        if !registration.usable() {
            // Siril could not register this frame, so there is nothing of its to
            // compare against. Whether *we* can is recorded but not gated: it is
            // upside either way.
            if let Some(relative) = reference.frames.get(&registration.index) {
                let path = reference.corpus.join(relative);
                let ours = path.exists().then(|| {
                    let (stars, _, _) = stars_of(&path);
                    register_pair(
                        &stars,
                        &reference_stars,
                        TransformKind::Homography,
                        &MatchOptions::default(),
                        &RansacOptions::default(),
                    )
                    .ok()
                });
                let outcome = match ours.flatten() {
                    Some(fit) => format!("we registered it on {} stars", fit.inlier_count()),
                    None => "we could not register it either".to_string(),
                };
                println!("  {relative}: siril skipped this frame; {outcome}");
                siril_skipped.push(format!("{relative}: {outcome}"));
            }
            continue;
        }
        let Some(relative) = reference.frames.get(&registration.index) else {
            continue;
        };
        let path = reference.corpus.join(relative);
        if !path.exists() {
            eprintln!("  {relative}: missing from corpus, skipping");
            continue;
        }

        let siril = Transform::from_matrix(to_our_convention(registration.matrix))
            .expect("siril's matrix is finite");

        let (stars, _, _) = stars_of(&path);
        let fit = register_pair(
            &stars,
            &reference_stars,
            TransformKind::Homography,
            &MatchOptions::default(),
            &RansacOptions::default(),
        );
        let fit = match fit {
            Ok(fit) => fit,
            Err(e) => {
                // A frame Siril registered and we could not is a parity
                // failure, not something to skip over.
                results.push(FrameResult {
                    frame: relative.clone(),
                    siril_stars: registration.stars,
                    our_stars: stars.len(),
                    inliers: 0,
                    our_residual: f64::NAN,
                    siril_residual: f64::NAN,
                    corner_error: f64::INFINITY,
                });
                eprintln!("  {relative}: registration failed: {e}");
                continue;
            }
        };

        // Both transforms are scored on one correspondence set, so the
        // comparison is of transforms and not of which stars each paired up.
        let pairs = pair_by_proximity(&stars, &reference_stars, &fit.transform, PAIR_TOLERANCE);
        results.push(FrameResult {
            frame: relative.clone(),
            siril_stars: registration.stars,
            our_stars: stars.len(),
            inliers: fit.inlier_count(),
            our_residual: fit.transform.rmse(&pairs),
            siril_residual: siril.rmse(&pairs),
            corner_error: fit.transform.corner_error(&siril, width, height),
        });
        let last = results.last().expect("just pushed");
        println!(
            "  {} (siril fwhm {:.2}): corner {:.4} px = {:.2}x residual; \
             residual ours {:.4} siril {:.4}; {} inliers",
            last.frame,
            registration.fwhm,
            last.corner_error,
            last.corner_per_residual(),
            last.our_residual,
            last.siril_residual,
            last.inliers,
        );
    }

    assert!(
        results.len() >= MIN_FRAMES - 1,
        "the gate is stated over {MIN_FRAMES} real frames; compared {} (plus the reference)",
        results.len()
    );

    let finite: Vec<&FrameResult> = results
        .iter()
        .filter(|r| r.corner_error.is_finite())
        .collect();
    let worst = finite
        .iter()
        .max_by(|a, b| a.corner_error.total_cmp(&b.corner_error))
        .expect("at least one frame registered");
    let worst_ratio = finite
        .iter()
        .max_by(|a, b| a.corner_per_residual().total_cmp(&b.corner_per_residual()))
        .expect("at least one frame registered");
    let well_fitted: Vec<&&FrameResult> = finite.iter().filter(|r| r.model_fits()).collect();

    let mut corners: Vec<f64> = finite.iter().map(|r| r.corner_error).collect();
    corners.sort_by(f64::total_cmp);
    let median_corner = corners[corners.len() / 2];
    let mean_corner = corners.iter().sum::<f64>() / corners.len() as f64;
    let ours_better = finite
        .iter()
        .filter(|r| r.our_residual <= r.siril_residual)
        .count();

    let mut report = String::new();
    let _ = writeln!(
        report,
        "px-imageproc registration vs siril two-pass homographies\n\
         corpus: {}\nframes compared: {}\nreference frame: #{}\n\
         corner error: median {:.4} px, mean {:.4} px, worst {:.4} px ({})\n\
         worst corner error as a multiple of the frame's own misfit: {:.2}x ({})\n\
         frames a homography fits (residual < {CORNER_TOLERANCE} px): {} of {}\n\
         frames where our fit beats siril's on the same pairs: {} of {}\n",
        reference.corpus.display(),
        results.len(),
        reference.reference_index,
        median_corner,
        mean_corner,
        worst.corner_error,
        worst.frame,
        worst_ratio.corner_per_residual(),
        worst_ratio.frame,
        well_fitted.len(),
        finite.len(),
        ours_better,
        finite.len(),
    );
    for r in &results {
        let _ = writeln!(
            report,
            "  {:<70} corner {:>8.4} px ({:>5.2}x)  residual ours {:>7.4} siril {:>7.4}  \
             inliers {:>5}  stars ours {:>5} siril {:>5}",
            r.frame,
            r.corner_error,
            r.corner_per_residual(),
            r.our_residual,
            r.siril_residual,
            r.inliers,
            r.our_stars,
            r.siril_stars
        );
    }
    if !siril_skipped.is_empty() {
        let _ = writeln!(
            report,
            "\nframes siril itself could not register ({}):",
            siril_skipped.len()
        );
        for line in &siril_skipped {
            let _ = writeln!(report, "  {line}");
        }
    }
    write_report(&report);

    println!(
        "corner error: median {median_corner:.4} px, worst {:.4} px; worst ratio {:.2}x; \
         our fit better on {ours_better} of {}",
        worst.corner_error,
        worst_ratio.corner_per_residual(),
        finite.len(),
    );

    let mut failures = Vec::new();
    for r in &results {
        if !r.corner_error.is_finite() {
            failures.push(format!(
                "{}: we failed to register a frame siril did",
                r.frame
            ));
            continue;
        }
        if r.model_fits() && r.corner_error > CORNER_TOLERANCE {
            failures.push(format!(
                "{}: corner error {:.4} px exceeds {CORNER_TOLERANCE} px on a frame whose own \
                 residual is only {:.4} px",
                r.frame, r.corner_error, r.our_residual
            ));
        }
        if r.corner_per_residual() > MAX_CORNER_PER_RESIDUAL {
            failures.push(format!(
                "{}: corner error {:.4} px is {:.2}x its own residual of {:.4} px",
                r.frame,
                r.corner_error,
                r.corner_per_residual(),
                r.our_residual
            ));
        }
        if r.our_residual > r.siril_residual * (1.0 + MAX_RESIDUAL_EXCESS) {
            failures.push(format!(
                "{}: our residual {:.4} px is worse than siril's {:.4} px by more than {:.0}%",
                r.frame,
                r.our_residual,
                r.siril_residual,
                100.0 * MAX_RESIDUAL_EXCESS
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} parity failure(s) over {} frames:\n  {}",
        failures.len(),
        results.len(),
        failures.join("\n  ")
    );
}

/// The sequence parser is pure text handling, so it is worth a test that does
/// not need Siril, a corpus, or half a minute.
#[test]
fn parses_a_sequence_file() {
    let text = "\
#Siril sequence file. Contains list of images, selection, registration data and statistics
S 'lt_' 1 5 5 5 3 6 0 0 0
L 1
I 1 1
R0 3.81518 8.58617 0.775037 0 390.997 712 H 0.999374 -0.000439936 -8.5163 0.00034417 0.999349 7.26516 -1.33637e-08 -3.13874e-08 1
I 2 0
R0 2.18529 2.18529 0.775616 0 391.471 1900 H 1 0 0 0 1 0 0 0 1
M0-0 61171488 61171488 365.153 361 179.24
";
    let (reference_index, registrations) = parse_sequence(text).expect("parses");
    assert_eq!(reference_index, 4, "stored 0-based, reported 1-based");
    assert_eq!(registrations.len(), 2);
    assert!(registrations[0].usable(), "selection flag from the I line");
    assert_eq!(registrations[0].stars, 712);
    assert!((registrations[0].fwhm - 3.81518).abs() < 1e-9);
    assert!((registrations[0].matrix[(0, 2)] + 8.5163).abs() < 1e-9);
    assert_eq!(registrations[1].matrix, Matrix3::identity());
    assert!(
        !registrations[1].usable(),
        "a cleared selection flag means siril did not register the frame"
    );
}

/// The flip is a conjugation, so applying it twice is the identity — the
/// property that makes one function serve both directions.
#[test]
fn the_coordinate_flip_is_its_own_inverse() {
    let f = flip(6388);
    assert!((f * f - Matrix3::identity()).norm() < 1e-12);
}

fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("registration-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
