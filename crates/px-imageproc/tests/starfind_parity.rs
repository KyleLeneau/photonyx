//! Star-detection parity against Siril's `findstar`, on the real-frame corpus
//! (ADR 007, P7-T6).
//!
//! Reference star lists come from `cargo xtask siril-reference --stars`, which
//! drives a local Siril install over `data/corpus-mono/light` and keeps its
//! `.lst` output verbatim. Neither the corpus nor the reference is committed,
//! so these tests skip when they are absent.
//!
//! What is being checked is that the two implementations find the *same stars
//! in the same places*, not that they agree on how many stars a frame has.
//! They do not: the thresholds and rejection rules differ, and this crate's
//! pass is deliberately more permissive at the faint end. The parity claim is
//! one-directional — every star Siril found should be a star we found, at the
//! same position.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use px_imageproc::register::starfind::find_stars_reported;
use px_imageproc::{Frame, PsfStar, StarfindOptions};

/// Fraction of Siril's stars that must be matched within [`MATCH_RADIUS`].
///
/// Not 100%: Siril's list includes blends and stars on the frame edge that
/// this crate's rejection rules drop by design, and a handful of faint
/// detections sit right on both implementations' thresholds.
const MIN_MATCHED_FRACTION: f64 = 0.95;

/// Distance, in pixels, within which two detections are the same star.
const MATCH_RADIUS: f64 = 0.2;

/// Bound on the RMS centroid difference over the matched stars.
const MAX_CENTROID_RMS: f64 = 0.05;

/// Bound on the mean relative FWHM difference over the matched stars.
const MAX_FWHM_BIAS: f64 = 0.02;

/// Siril places the origin at the *corner* of the first pixel and measures y
/// upward from the bottom of the frame; this crate puts integer coordinates at
/// pixel centres and measures y downward from the top, which is the order the
/// samples arrive in. The conversion is therefore
///
/// ```text
/// ours.x = siril.x - 0.5
/// ours.y = height - siril.y - 0.5
/// ```
///
/// Verified on the corpus: with the half-pixel shift the matched RMS is
/// 0.01-0.04 px, without it every match sits at ~0.5 px.
const PIXEL_CENTRE_OFFSET: f64 = 0.5;

/// Cell size, in pixels, of the lookup grid used to find a nearest neighbour.
/// Any value comfortably above `MATCH_RADIUS` works; this one keeps the cells
/// few enough to hash cheaply on a 61 MP frame.
const GRID: f64 = 16.0;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// One star as Siril reported it, already converted to this crate's
/// coordinates.
struct SirilStar {
    x: f64,
    y: f64,
    fwhm_x: f64,
}

struct Reference {
    corpus: PathBuf,
    /// Frame path relative to the corpus, and its star list.
    frames: Vec<(String, PathBuf)>,
}

/// Loads the reference manifest, or `None` when it has not been generated.
fn reference() -> Option<Reference> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"));
    let stars = dir.join("stars");
    let manifest = std::fs::read_to_string(stars.join("frames.txt")).ok()?;

    // The corpus the lists were generated from is the one the stats reference
    // records; this test needs the frames themselves, not just the lists.
    let corpus = root.join("data/corpus-mono");
    if !corpus.is_dir() {
        return None;
    }

    let frames: Vec<(String, PathBuf)> = manifest
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(frame, list)| (frame.to_string(), stars.join(list)))
        .collect();
    (!frames.is_empty()).then_some(Reference { corpus, frames })
}

/// Parses a Siril `findstar` list. Columns are tab-separated with the header in
/// `#` comments: `star# layer B A beta X Y FWHMx FWHMy ...`.
fn parse_star_list(path: &Path, height: usize) -> Vec<SirilStar> {
    let text = std::fs::read_to_string(path).expect("read star list");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let value = |i: usize| fields.get(i)?.trim().parse::<f64>().ok();
            Some(SirilStar {
                x: value(5)? - PIXEL_CENTRE_OFFSET,
                y: height as f64 - value(6)? - PIXEL_CENTRE_OFFSET,
                fwhm_x: value(7)?,
            })
        })
        .collect()
}

/// A uniform grid over the fitted stars, so matching a reference list against
/// them is linear rather than quadratic — the lists run to tens of thousands of
/// stars on a rich field.
struct Grid<'a> {
    stars: &'a [PsfStar],
    cells: std::collections::HashMap<(i64, i64), Vec<usize>>,
}

impl<'a> Grid<'a> {
    fn new(stars: &'a [PsfStar]) -> Self {
        let mut cells: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for (i, star) in stars.iter().enumerate() {
            cells
                .entry(Self::cell(star.x0, star.y0))
                .or_default()
                .push(i);
        }
        Self { stars, cells }
    }

    fn cell(x: f64, y: f64) -> (i64, i64) {
        ((x / GRID).floor() as i64, (y / GRID).floor() as i64)
    }

    /// The nearest fitted star to `(x, y)`, and its distance.
    fn nearest(&self, x: f64, y: f64) -> Option<(&PsfStar, f64)> {
        let (cx, cy) = Self::cell(x, y);
        let mut best: Option<(&PsfStar, f64)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &i in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let star = &self.stars[i];
                    let d = ((star.x0 - x).powi(2) + (star.y0 - y).powi(2)).sqrt();
                    if best.is_none_or(|(_, best_d)| d < best_d) {
                        best = Some((star, d));
                    }
                }
            }
        }
        best
    }
}

/// What one frame's comparison produced.
struct FrameResult {
    frame: String,
    siril_stars: usize,
    our_stars: usize,
    matched: usize,
    centroid_rms: f64,
    fwhm_bias: f64,
}

impl FrameResult {
    fn matched_fraction(&self) -> f64 {
        self.matched as f64 / self.siril_stars as f64
    }
}

fn compare(frame_path: &Path, list_path: &Path, frame_name: &str) -> FrameResult {
    let frame = Frame::open(frame_path).expect("open corpus frame");
    let (width, height) = (frame.width(), frame.height());

    // The reference was generated with Siril's defaults for radius, sigma and
    // roundness, which are this crate's defaults too. The star cap is lifted:
    // the reference list is uncapped, and a cap would make the comparison a
    // measurement of which 2000 stars each implementation happened to keep.
    let options = StarfindOptions {
        max_stars: usize::MAX,
        ..Default::default()
    };
    let (ours, report) = find_stars_reported(frame.channel(0), width, height, &options, 0);
    println!("  {frame_name}: {report:?}");

    let siril = parse_star_list(list_path, height);
    let grid = Grid::new(&ours);

    let mut matched = 0usize;
    let mut sum_squared = 0.0;
    let mut fwhm_error = 0.0;
    for star in &siril {
        let Some((fit, distance)) = grid.nearest(star.x, star.y) else {
            continue;
        };
        if distance > MATCH_RADIUS {
            continue;
        }
        matched += 1;
        sum_squared += distance * distance;
        fwhm_error += (fit.fwhm_x - star.fwhm_x) / star.fwhm_x;
    }

    FrameResult {
        frame: frame_name.to_string(),
        siril_stars: siril.len(),
        our_stars: ours.len(),
        matched,
        centroid_rms: (sum_squared / matched.max(1) as f64).sqrt(),
        fwhm_bias: fwhm_error / matched.max(1) as f64,
    }
}

/// Reads and fits several 61 MP frames, which is minutes in a debug build, so
/// this runs under `--release`:
///
/// ```text
/// cargo test --release -p px-imageproc --test starfind_parity -- --nocapture
/// ```
#[test]
#[cfg_attr(debug_assertions, ignore = "fits 61 MP frames; run with --release")]
fn star_detection_matches_siril_on_real_frames() {
    let Some(reference) = reference() else {
        eprintln!(
            "skipping: no Siril star reference. Run \
             `cargo xtask siril-reference --stars` with a corpus under data/corpus-mono."
        );
        return;
    };

    let mut results = Vec::new();
    for (relative, list) in &reference.frames {
        let path = reference.corpus.join(relative);
        if !path.exists() || !list.exists() {
            eprintln!("  {relative}: missing frame or star list, skipping");
            continue;
        }
        results.push(compare(&path, list, relative));
    }

    assert!(
        results.len() >= 3,
        "the gate is stated over at least three real frames; got {}",
        results.len()
    );

    let mut report = String::new();
    let _ = writeln!(
        report,
        "px-imageproc star detection vs siril findstar\ncorpus: {}\n\
         match radius: {MATCH_RADIUS} px\n",
        reference.corpus.display()
    );

    let mut failures = Vec::new();
    for r in &results {
        let line = format!(
            "  {}\n    siril {:>6} stars  ours {:>6}  matched {:>6} ({:>5.1}%)  \
             centroid rms {:.4} px  fwhm bias {:+.2}%",
            r.frame,
            r.siril_stars,
            r.our_stars,
            r.matched,
            100.0 * r.matched_fraction(),
            r.centroid_rms,
            100.0 * r.fwhm_bias,
        );
        println!("{line}");
        let _ = writeln!(report, "{line}");

        if r.matched_fraction() < MIN_MATCHED_FRACTION {
            failures.push(format!(
                "{}: only {:.1}% of siril's stars matched within {MATCH_RADIUS} px",
                r.frame,
                100.0 * r.matched_fraction()
            ));
        }
        if r.centroid_rms > MAX_CENTROID_RMS {
            failures.push(format!(
                "{}: centroid rms {:.4} px exceeds {MAX_CENTROID_RMS}",
                r.frame, r.centroid_rms
            ));
        }
        if r.fwhm_bias.abs() > MAX_FWHM_BIAS {
            failures.push(format!(
                "{}: fwhm bias {:+.2}% exceeds {:.2}%",
                r.frame,
                100.0 * r.fwhm_bias,
                100.0 * MAX_FWHM_BIAS
            ));
        }
    }

    write_report(&report);
    assert!(
        failures.is_empty(),
        "{} parity failure(s):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("starfind-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
