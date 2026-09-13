//! Warp parity against Siril's `seqapplyreg` (ADR 007, P10-T4 / Phase 10 gate).
//!
//! The comparison uses *Siril's own* homography rather than ours, which is the
//! point: with the transform held fixed, what is left is the interpolation
//! kernel and the clamping pass. Registration accuracy is the Phase 8 gate and
//! is measured separately — folding the two together would leave a difference
//! here unattributable.
//!
//! Reference data comes from `cargo xtask siril-reference --warp`, so these
//! tests skip when it is absent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use nalgebra::Matrix3;
use px_imageproc::register::transform::Transform;
use px_imageproc::register::warp::{Interpolation, WarpOptions, warp};
use px_imageproc::runtime::{MemoryLimit, Runtime};
use px_imageproc::{ChannelStats, Frame};

/// The gate: RMS difference from Siril's warped frame, in units of the frame's
/// own background noise.
const RMS_TOLERANCE: f64 = 0.5;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

struct Reference {
    /// Sequence index, the original frame, and Siril's warped output.
    frames: Vec<(usize, PathBuf, PathBuf)>,
    /// One homography per sequence index, already in this crate's coordinates.
    transforms: Vec<Option<Transform>>,
    reference_index: usize,
}

/// Siril measures y upward from the bottom; this crate measures it downward from
/// the top. A transform converts by conjugation with the flip, which is its own
/// inverse.
fn flip(height: usize) -> Matrix3<f64> {
    Matrix3::new(1.0, 0.0, 0.0, 0.0, -1.0, (height - 1) as f64, 0.0, 0.0, 1.0)
}

fn reference(height: usize) -> Option<Reference> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"))
        .join("registration");
    let work = dir.join("work");
    let sequence = std::fs::read_to_string(work.join("reg_.seq")).ok()?;
    let manifest = std::fs::read_to_string(dir.join("frames.txt")).ok()?;
    let corpus = root.join("data/corpus-mono");

    let f = flip(height);
    let mut transforms = vec![None];
    let mut reference_index = 0;
    for line in sequence.lines() {
        if let Some(rest) = line.strip_prefix("S ") {
            let tail = rest.rsplit_once('\'')?.1;
            let fields: Vec<&str> = tail.split_whitespace().collect();
            reference_index = fields.get(4)?.parse::<usize>().ok()? + 1;
        } else if line.starts_with('R') {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let at = fields.iter().position(|x| *x == "H")?;
            let coefficients: Vec<f64> = (at + 1..at + 10)
                .filter_map(|i| fields.get(i)?.trim().parse::<f64>().ok())
                .collect();
            transforms.push(if coefficients.len() == 9 {
                Transform::from_matrix(f * Matrix3::from_row_slice(&coefficients) * f)
            } else {
                None
            });
        }
    }

    // Only the frames Siril actually warped, which is a handful.
    let frames: Vec<(usize, PathBuf, PathBuf)> = manifest
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(index, relative)| {
            let index: usize = index.trim().parse().ok()?;
            let warped = work.join(format!("r_reg_{index:05}.fits"));
            warped
                .exists()
                .then(|| (index, corpus.join(relative.trim()), warped))
        })
        .collect();
    (!frames.is_empty()).then_some(Reference {
        frames,
        transforms,
        reference_index,
    })
}

/// Differences between two frames, over the pixels both of them exposed.
struct Comparison {
    compared: usize,
    skipped: usize,
    max_delta: f64,
    rms_delta: f64,
    background_noise: f64,
}

impl Comparison {
    fn rms_in_sigma(&self) -> f64 {
        self.rms_delta / self.background_noise
    }

    fn max_in_sigma(&self) -> f64 {
        self.max_delta / self.background_noise
    }
}

/// Compares two warped frames in ADU, over the pixels where both have data.
///
/// A pixel either of them left unexposed is skipped rather than counted as a
/// large difference: the two implementations need not agree to the pixel on
/// where the footprint ends, and that edge is not what this measures.
fn compare(ours: &Frame, theirs: &Frame) -> Comparison {
    const ADU: f64 = 65535.0;
    let stats = ChannelStats::compute(theirs.channel(0), theirs.width(), theirs.height());

    let mut compared = 0usize;
    let mut skipped = 0usize;
    let mut max_delta = 0.0f64;
    let mut sum_squared = 0.0f64;
    for (a, b) in ours.data().iter().zip(theirs.data()) {
        if *a == 0.0 || *b == 0.0 {
            skipped += 1;
            continue;
        }
        let delta = (*a as f64 - *b as f64).abs() * ADU;
        compared += 1;
        max_delta = max_delta.max(delta);
        sum_squared += delta * delta;
    }

    Comparison {
        compared,
        skipped,
        max_delta,
        rms_delta: (sum_squared / compared.max(1) as f64).sqrt(),
        background_noise: stats.background_noise * ADU,
    }
}

fn warp_with(source: &Frame, transform: &Transform, runtime: &Runtime, clamp: bool) -> Frame {
    warp(
        source,
        transform,
        source.dims(),
        runtime,
        &WarpOptions {
            interpolation: Interpolation::Lanczos4,
            clamp,
        },
    )
    .expect("warp")
}

/// Warps several 61 MP frames twice each, so this runs under `--release`:
///
/// ```text
/// cargo test --release -p px-imageproc --test warp_parity -- --nocapture
/// ```
#[test]
#[cfg_attr(debug_assertions, ignore = "warps 61 MP frames; run with --release")]
fn warped_frames_match_sirils() {
    // The height is needed to read the reference, and comes from the first
    // frame Siril warped.
    let Some(probe) = reference(1).map(|r| r.frames[0].1.clone()) else {
        eprintln!(
            "skipping: no Siril warp reference. Run \
             `cargo xtask siril-reference --warp`."
        );
        return;
    };
    let height = Frame::open(&probe).expect("open frame").height();
    let reference = reference(height).expect("reference");
    let runtime = Runtime::resolve(MemoryLimit::Ratio(0.9), None).expect("runtime");

    let mut report = String::new();
    let _ = writeln!(
        report,
        "px-imageproc warp vs siril seqapplyreg (Lanczos-4, clamping on)\n\
         transform: siril's own homography, so this measures interpolation only\n"
    );

    let mut failures = Vec::new();
    for (index, original, theirs_path) in &reference.frames {
        let source = Frame::open(original).expect("open source");
        let theirs = Frame::open(theirs_path).expect("open siril's warped frame");
        assert_eq!(
            source.dims(),
            theirs.dims(),
            "siril's output should be the same shape (FRAMING_CURRENT)"
        );

        let transform = reference
            .transforms
            .get(*index)
            .copied()
            .flatten()
            .expect("a homography for every warped frame");

        let clamped = warp_with(&source, &transform, &runtime, true);
        let unclamped = warp_with(&source, &transform, &runtime, false);
        let with = compare(&clamped, &theirs);
        let without = compare(&unclamped, &theirs);

        let is_reference = *index == reference.reference_index;
        let line = format!(
            "  #{index}{}: rms {:.4} ADU = {:.3} sigma_bg, max {:.1} ADU = {:.1} sigma_bg, \
             {} compared, {} skipped | clamping off: rms {:.3} sigma_bg",
            if is_reference { " (reference)" } else { "" },
            with.rms_delta,
            with.rms_in_sigma(),
            with.max_delta,
            with.max_in_sigma(),
            with.compared,
            with.skipped,
            without.rms_in_sigma(),
        );
        println!("{line}");
        let _ = writeln!(report, "{line}");

        if with.rms_in_sigma() > RMS_TOLERANCE {
            failures.push(format!(
                "#{index}: rms {:.3} sigma_bg exceeds {RMS_TOLERANCE}",
                with.rms_in_sigma()
            ));
        }
        // The reference frame warps under the identity, so it is the one case
        // where the answer is known exactly: it must come back unchanged.
        if is_reference && with.max_delta > 0.01 {
            failures.push(format!(
                "#{index} is the reference and must be copied exactly; max delta {:.4} ADU",
                with.max_delta
            ));
        }
        // Clamping is supposed to bring us *closer* to Siril, which also runs
        // it. If it does not, one of the two clamps is wrong.
        if !is_reference && with.rms_in_sigma() > without.rms_in_sigma() {
            failures.push(format!(
                "#{index}: clamping made agreement worse ({:.3} vs {:.3} sigma_bg)",
                with.rms_in_sigma(),
                without.rms_in_sigma()
            ));
        }
    }

    write_report(&report);
    assert!(
        failures.is_empty(),
        "{} warp parity failure(s):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("warp-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
