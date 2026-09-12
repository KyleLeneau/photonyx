//! Statistics parity against Siril, on the real-frame corpus (ADR 007, P2-T4).
//!
//! Reference values come from `cargo xtask siril-reference`, which drives a
//! local Siril install over `data/corpus-mono` and records its `stat main`
//! output. Neither the corpus nor the reference file is committed, so these
//! tests skip when they are absent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use px_fits::reader::FitsReader;
use px_imageproc::stats::{self, ChannelStats};

/// Siril prints `stat` at one decimal place in 16-bit ADU, so two
/// implementations that agree perfectly can still differ by half a unit in
/// the last printed digit. This is the floor on what any comparison against
/// `stat` output can resolve.
const PRINT_PRECISION: f64 = 0.05;

/// Tolerance for quantities that are exact order statistics — a median, a
/// minimum, a maximum, a MAD. These are the same number in both
/// implementations; the only slack is Siril's printed rounding and the f32
/// round-trip through the unit range. Measured worst case over the corpus is
/// 0.002 ADU.
const EXACT_TOLERANCE: f64 = 0.01;

/// Tolerance for accumulated quantities — mean, sigma, sqrt(BWMV), bgnoise.
/// Siril accumulates over raw 16-bit integers where we accumulate over
/// normalized floats, so these carry the representation difference on top of
/// the printed rounding. Measured worst case over the corpus is 0.049 ADU,
/// essentially all of it Siril's rounding.
const ACCUMULATED_TOLERANCE: f64 = PRINT_PRECISION + 0.01;

const ADU: f64 = 65535.0;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/px-imageproc.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

struct Reference {
    corpus: PathBuf,
    frames: Vec<serde_json::Value>,
    siril_version: String,
}

/// Loads the reference file, or `None` when it has not been generated.
fn reference() -> Option<Reference> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"));
    let path = dir.join("stats/stats.json");

    let text = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let corpus = PathBuf::from(json.get("corpus")?.as_str()?);
    if !corpus.is_dir() {
        return None;
    }
    Some(Reference {
        corpus,
        frames: json.get("frames")?.as_array()?.clone(),
        siril_version: json
            .get("siril_version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
    })
}

macro_rules! skip_without_reference {
    () => {
        match reference() {
            Some(r) => r,
            None => {
                eprintln!(
                    "skipping: no Siril reference. Run `cargo xtask siril-reference` \
                     with a corpus under data/corpus-mono."
                );
                return;
            }
        }
    };
}

/// Our estimators for one frame, converted to the ADU units Siril reports in.
struct Measured {
    mean: f64,
    median: f64,
    sigma: f64,
    min: f64,
    max: f64,
    bgnoise: f64,
    mad: f64,
    sqrt_bwmv: f64,
}

fn measure(path: &Path) -> Measured {
    let reader = FitsReader::open(path).expect("open frame");
    let hdu = reader.primary_image().expect("primary image");
    let frame = px_imageproc::Frame::from_hdu(&hdu).expect("decode frame");

    let data = frame.channel(0);
    let s = ChannelStats::compute(data, frame.width(), frame.height());

    // Siril's sqrt(BWMV) is the biweight over *all* non-null samples about the
    // median, not the clipped estimate our normalization uses.
    let kept = stats::non_null(data);
    let sqrt_bwmv = stats::biweight_midvariance(&kept, s.median, s.mad).sqrt();

    Measured {
        mean: s.mean * ADU,
        median: s.median as f64 * ADU,
        sigma: s.sigma * ADU,
        min: s.min as f64 * ADU,
        max: s.max as f64 * ADU,
        bgnoise: s.background_noise * ADU,
        mad: s.mad as f64 * ADU,
        sqrt_bwmv: sqrt_bwmv * ADU,
    }
}

fn field(frame: &serde_json::Value, name: &str) -> f64 {
    frame
        .get(name)
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| panic!("reference record missing {name}"))
}

/// Reads 12 frames of 61 MP each. That is ~14 s optimized and ~4 min in a
/// debug build, so it is marked ignored for debug and runs under `--release`:
///
/// ```text
/// cargo test --release -p px-imageproc --test stats_parity -- --nocapture
/// ```
#[test]
#[cfg_attr(debug_assertions, ignore = "reads 61 MP frames; run with --release")]
fn statistics_match_siril_on_real_frames() {
    let reference = skip_without_reference!();
    println!(
        "comparing against siril {} over {}",
        reference.siril_version,
        reference.corpus.display()
    );

    let mut failures = Vec::new();
    let mut report = String::new();
    let mut worst = 0.0f64;
    let _ = writeln!(
        report,
        "px-imageproc statistics vs siril {} stat main\ncorpus: {}\nunits: 16-bit ADU (siril prints one decimal)\n",
        reference.siril_version,
        reference.corpus.display()
    );

    for record in &reference.frames {
        let relative = record.get("path").and_then(|v| v.as_str()).expect("path");
        let path = reference.corpus.join(relative);
        if !path.exists() {
            eprintln!("  {relative}: missing from corpus, skipping");
            continue;
        }

        let got = measure(&path);
        println!("  {relative}");
        let _ = writeln!(report, "  {relative}");

        let checks: [(&str, f64, f64, f64); 8] = [
            (
                "median",
                got.median,
                field(record, "median"),
                EXACT_TOLERANCE,
            ),
            ("min", got.min, field(record, "min"), EXACT_TOLERANCE),
            ("max", got.max, field(record, "max"), EXACT_TOLERANCE),
            ("mad", got.mad, field(record, "mad"), EXACT_TOLERANCE),
            (
                "mean",
                got.mean,
                field(record, "mean"),
                ACCUMULATED_TOLERANCE,
            ),
            (
                "sigma",
                got.sigma,
                field(record, "sigma"),
                ACCUMULATED_TOLERANCE,
            ),
            (
                "sqrt_bwmv",
                got.sqrt_bwmv,
                field(record, "sqrt_bwmv"),
                ACCUMULATED_TOLERANCE,
            ),
            (
                "bgnoise",
                got.bgnoise,
                field(record, "bgnoise"),
                ACCUMULATED_TOLERANCE,
            ),
        ];

        for (name, ours, siril, tolerance) in checks {
            let delta = ours - siril;
            let line = format!(
                "    {name:<10} ours {ours:>12.3}  siril {siril:>12.1}  delta {delta:>9.3}"
            );
            println!("{line}");
            let _ = writeln!(report, "{line}");
            worst = worst.max(delta.abs());
            if delta.abs() > tolerance {
                failures.push(format!(
                    "{relative} {name}: ours {ours:.3} vs siril {siril:.1} (delta {delta:.3}, tolerance {tolerance})"
                ));
            }
        }
    }

    let _ = writeln!(
        report,
        "\nworst absolute delta across all frames and statistics: {worst:.4} ADU"
    );
    write_report(&report);

    assert!(
        failures.is_empty(),
        "{} statistic(s) outside tolerance:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Saves the comparison next to the corpus so it can be read without
/// re-running the test.
fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("stats-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
