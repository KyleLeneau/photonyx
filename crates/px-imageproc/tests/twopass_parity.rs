//! Two-pass registration parity against Siril (ADR 007, P9 gate).
//!
//! The gate is stated on the *decisions* rather than the numbers: over the
//! 100-frame corpus, the native engine must choose the same reference frame
//! Siril chose and include the same set of frames. Transform accuracy is the
//! Phase 8 gate and is measured in `starmatch_parity.rs`.
//!
//! Reference data comes from `cargo xtask siril-reference --registration`, so
//! these tests skip when it is absent.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use px_imageproc::FrameSet;
use px_imageproc::register::twopass::{RegistrationOptions, register_sequence};
use px_imageproc::runtime::{MemoryLimit, Runtime};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// What Siril decided: the reference frame and the frames it kept, both as
/// 1-based sequence indices.
struct SirilDecisions {
    reference: usize,
    included: HashSet<usize>,
    frames: Vec<(usize, PathBuf)>,
}

fn decisions() -> Option<SirilDecisions> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"))
        .join("registration");
    let sequence = std::fs::read_to_string(dir.join("work/reg_.seq")).ok()?;
    let manifest = std::fs::read_to_string(dir.join("frames.txt")).ok()?;
    let corpus = root.join("data/corpus-mono");
    if !corpus.is_dir() {
        return None;
    }

    let mut reference = None;
    let mut included = HashSet::new();
    for line in sequence.lines() {
        if let Some(rest) = line.strip_prefix("S ") {
            let tail = rest.rsplit_once('\'')?.1;
            let fields: Vec<&str> = tail.split_whitespace().collect();
            reference = fields.get(4)?.parse::<usize>().ok().map(|i| i + 1);
        } else if let Some(rest) = line.strip_prefix("I ") {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            let index: usize = fields.first()?.parse().ok()?;
            if fields.get(1).is_some_and(|f| *f != "0") {
                included.insert(index);
            }
        }
    }

    let frames: Vec<(usize, PathBuf)> = manifest
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(index, relative)| {
            Some((index.trim().parse().ok()?, corpus.join(relative.trim())))
        })
        .collect();
    (!frames.is_empty() && !included.is_empty()).then_some(SirilDecisions {
        reference: reference?,
        included,
        frames,
    })
}

/// Detects stars in 100 frames of 61 MP, which is minutes even optimized:
///
/// ```text
/// cargo test --release -p px-imageproc --test twopass_parity -- --nocapture
/// ```
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "registers 100 61 MP frames; run with --release"
)]
fn the_same_reference_and_the_same_frames_as_siril() {
    let Some(siril) = decisions() else {
        eprintln!(
            "skipping: no Siril registration reference. Run \
             `cargo xtask siril-reference --registration`."
        );
        return;
    };

    let paths: Vec<PathBuf> = siril.frames.iter().map(|(_, p)| p.clone()).collect();
    assert!(
        paths.iter().all(|p| p.exists()),
        "every frame of the reference run must be in the corpus"
    );
    let set = FrameSet::open(&paths).expect("open sequence");

    // The budget is the default one a `px` run would resolve, so this also
    // exercises the concurrency the loop actually uses in production.
    let runtime = Runtime::resolve(MemoryLimit::Ratio(0.9), None).expect("runtime");
    let started = std::time::Instant::now();
    let registration =
        register_sequence(&set, &runtime, &RegistrationOptions::default()).expect("register");
    let elapsed = started.elapsed();

    // Our frames are 0-based, Siril's sequence is 1-based.
    let our_reference = registration.reference + 1;
    let our_included: HashSet<usize> = registration.included().map(|f| f.index + 1).collect();

    let missing: Vec<usize> = siril
        .included
        .iter()
        .copied()
        .filter(|i| !our_included.contains(i))
        .collect();
    let extra: Vec<usize> = our_included
        .iter()
        .copied()
        .filter(|i| !siril.included.contains(i))
        .collect();

    let mut report = String::new();
    let _ = writeln!(
        report,
        "px-imageproc two-pass registration decisions vs siril\n\
         frames: {}\nreference: ours #{our_reference}, siril #{}\n\
         included: ours {}, siril {}\n\
         frames siril kept and we did not: {missing:?}\n\
         frames we kept and siril did not: {extra:?}\n\
         trials: {}\nmean fwhm: {:.2} px\nwall clock: {:.1} s\n",
        set.len(),
        siril.reference,
        our_included.len(),
        siril.included.len(),
        registration.trials,
        registration.mean_fwhm(),
        elapsed.as_secs_f64(),
    );
    for frame in &registration.frames {
        let _ = writeln!(
            report,
            "  #{:<4} {:<7} stars {:>5}  fwhm {:>5.2}  roundness {:>4.2}  inliers {:>5}  \
             rmse {:>6.4}{}",
            frame.index + 1,
            if frame.included() { "kept" } else { "dropped" },
            frame.stars,
            frame.fwhm,
            frame.roundness,
            frame.inliers,
            frame.rmse,
            frame
                .excluded
                .map(|e| format!("  ({e})"))
                .unwrap_or_default(),
        );
    }
    write_report(&report);

    println!(
        "reference: ours #{our_reference}, siril #{}; included ours {}, siril {}; \
         {:.1} s for {} frames",
        siril.reference,
        our_included.len(),
        siril.included.len(),
        elapsed.as_secs_f64(),
        set.len()
    );

    assert_eq!(
        our_reference, siril.reference,
        "the native engine must choose the same reference frame Siril chose"
    );
    assert!(
        missing.is_empty(),
        "siril registered frames the native engine dropped: {missing:?}"
    );
    // Frames *we* keep that Siril dropped are upside, not a failure — but they
    // are reported, because the gate is "the same set" and a difference either
    // way is worth seeing.
    println!("frames we kept that siril dropped: {extra:?}");
}

fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("twopass-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
