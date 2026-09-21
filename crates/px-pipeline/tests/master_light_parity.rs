//! End-to-end master-light parity against Siril (ADR 007, P11-T6).
//!
//! Runs the real `CreateMasterLightPipeline` in the native engine over the same
//! hundred corpus lights Siril stacked, and holds the result to the D2
//! master-light bounds: max and RMS difference in units of Siril's background
//! noise, plus the two assertions that actually constrain quality — the native
//! result must be no noisier and its stars no fatter.
//!
//! The reference comes from `cargo xtask siril-reference --master-light`, so
//! this skips when it is absent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use px_imageproc::register::psf::fwhm_stats;
use px_imageproc::register::starfind::find_stars;
use px_imageproc::stats::ChannelStats;
use px_imageproc::{Frame, StarfindOptions};
use px_pipeline::project::master_light::CreateMasterLightPipeline;
use px_pipeline::{NativeRuntime, PipelineReporter, RunIn};
use siril_sys::FitsExt;

const ADU: f64 = 65535.0;

/// Bound on the sky's RMS difference, in units of Siril's background noise,
/// after the global `-output_norm` factor is removed.
///
/// Measured at 0.32 sigma_bg over the lowest nine brightness deciles of the
/// corpus master. The bound is one noise unit, because that is the level at
/// which a difference would begin to show in the background of a stretched
/// image — which is what this number is about.
const SKY_RMS_TOLERANCE: f64 = 1.0;

/// Brightness decile above which a pixel counts as signal rather than sky.
const SKY_DECILES: usize = 9;

/// The native result may be no more than 2% noisier than Siril's.
const MAX_NOISE_RATIO: f64 = 1.02;

/// And its stars no more than 2% fatter.
const MAX_FWHM_RATIO: f64 = 1.02;

/// Prints what the pipeline reports, because the per-phase timings it emits are
/// half of what this test is measuring.
struct Loud;

impl PipelineReporter for Loud {
    fn step_started(&self, _message: &str) -> usize {
        0
    }
    fn step_ended(&self, _id: usize, message: &str, _success: bool) {
        println!("  {message}");
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// Siril's master light, and the frames it was built from.
struct Reference {
    master: PathBuf,
    frames: Vec<PathBuf>,
    elapsed_seconds: Option<f64>,
}

fn reference() -> Option<Reference> {
    let root = repo_root();
    let dir = std::env::var("PX_SIRIL_REF")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("data/reference"))
        .join("master-light");
    let master = dir.join("master_light.fits");
    let manifest = std::fs::read_to_string(dir.join("frames.txt")).ok()?;
    let corpus = root.join("data/corpus-mono");

    let frames: Vec<PathBuf> = manifest
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| corpus.join(l.trim()))
        .collect();
    if !master.exists() || frames.is_empty() || !frames.iter().all(|f| f.exists()) {
        return None;
    }

    let elapsed_seconds = std::fs::read_to_string(dir.join("master_light.log"))
        .ok()
        .and_then(|log| {
            log.lines()
                .find_map(|l| l.strip_prefix("elapsed_seconds: ")?.trim().parse().ok())
        });
    Some(Reference {
        master,
        frames,
        elapsed_seconds,
    })
}

/// Seeing and star count of a master, which is what "no worse" means for a
/// stack.
fn quality(frame: &Frame) -> (usize, f64) {
    let stars = find_stars(
        frame.channel(0),
        frame.width(),
        frame.height(),
        &StarfindOptions::default(),
    );
    let stats = fwhm_stats(&stars);
    (stars.len(), stats.map(|s| s.median).unwrap_or(f64::NAN))
}

/// Builds a master light natively from the same frames Siril used, then
/// compares. Minutes of work on 61 MP frames, so `--release`:
///
/// ```text
/// cargo test --release -p px-pipeline --test master_light_parity -- --nocapture
/// ```
#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "stacks 100 61 MP frames; run with --release"
)]
async fn native_master_light_matches_siril() {
    let Some(reference) = reference() else {
        eprintln!(
            "skipping: no Siril master light. Run \
             `cargo xtask siril-reference --master-light`."
        );
        return;
    };

    // The pipeline collects whole folders, so the frames Siril used are linked
    // into a directory of their own — the corpus holds more lights than the
    // reference run covered.
    let staging = tempfile::tempdir().expect("tempdir");
    let lights = staging.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    for frame in &reference.frames {
        std::os::unix::fs::symlink(frame, lights.join(frame.file_name().expect("name")))
            .expect("link frame");
    }

    let out = repo_root().join("data/output/pipelines/master-light");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("output dir");

    let started = std::time::Instant::now();
    let master = CreateMasterLightPipeline::builder()
        .ext(FitsExt::FITS)
        .light_folders(vec![lights.clone()])
        .name("parity".to_string())
        .out_folder(out.clone())
        .build()
        .run(
            Loud,
            // The budget a default `px` run resolves, so this measures the path
            // users actually take.
            NativeRuntime::default().use_extension(FitsExt::FITS),
        )
        .await
        .expect("native master light");
    let elapsed = started.elapsed().as_secs_f64();

    let ours = Frame::open(&master.path).expect("open native master");
    let theirs = Frame::open(&reference.master).expect("open siril master");
    assert_eq!(
        ours.dims(),
        theirs.dims(),
        "the two masters must have the same shape"
    );

    // Pixels either master left unexposed are skipped: the two need not agree to
    // the pixel on where the stacked footprint ends.
    let mut pairs: Vec<(f64, f64)> = ours
        .data()
        .iter()
        .zip(theirs.data())
        .filter(|(a, b)| **a != 0.0 && **b != 0.0)
        .map(|(a, b)| (*b as f64, *a as f64))
        .collect();
    let compared = pairs.len();
    assert!(compared > 0, "the two masters share no exposed pixels");

    // The global `-output_norm` relation between the two, by least squares over
    // every exposed pixel.
    //
    // Least squares rather than two percentiles: 99% of a master's pixels are
    // sky, so a fit anchored at the 99th percentile is anchored twice in the
    // same place and its slope is whatever the noise there says. The bright
    // pixels are few but they are the only ones with any leverage on the slope,
    // and least squares weights them by how far they are from the mean.
    let n = compared as f64;
    let mean_theirs = pairs.iter().map(|(t, _)| t).sum::<f64>() / n;
    let mean_ours = pairs.iter().map(|(_, o)| o).sum::<f64>() / n;
    let (covariance, variance) = pairs.iter().fold((0.0, 0.0), |(cov, var), (t, o)| {
        let dt = t - mean_theirs;
        (cov + dt * (o - mean_ours), var + dt * dt)
    });
    let scale = if variance > 0.0 {
        covariance / variance
    } else {
        1.0
    };
    let offset = mean_ours - scale * mean_theirs;

    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));

    let ours_stats = ChannelStats::compute(ours.channel(0), ours.width(), ours.height());
    let theirs_stats = ChannelStats::compute(theirs.channel(0), theirs.width(), theirs.height());
    let sigma_bg = theirs_stats.background_noise * ADU;

    // `pairs` is sorted by Siril's brightness, so the deciles are slices of it.
    let decile = compared / 10;
    let rms_over = |slice: &[(f64, f64)]| -> f64 {
        let sum: f64 = slice
            .iter()
            .map(|(theirs, ours)| {
                let d = (ours - (scale * theirs + offset)) * ADU;
                d * d
            })
            .sum();
        (sum / slice.len().max(1) as f64).sqrt()
    };
    let sky_rms = rms_over(&pairs[..SKY_DECILES * decile]);
    let signal_rms = rms_over(&pairs[SKY_DECILES * decile..]);
    let total_rms = rms_over(&pairs);

    // The raw figures too, because they are what a naive diff reports and the
    // report should not hide them.
    let mut raw_max = 0.0f64;
    let mut raw_sum_squared = 0.0f64;
    for (theirs, ours) in &pairs {
        let d = (ours - theirs).abs() * ADU;
        raw_max = raw_max.max(d);
        raw_sum_squared += d * d;
    }
    let raw_rms = (raw_sum_squared / compared as f64).sqrt();

    let noise_ratio = ours_stats.background_noise / theirs_stats.background_noise;
    let (our_stars, our_fwhm) = quality(&ours);
    let (their_stars, their_fwhm) = quality(&theirs);
    let fwhm_ratio = our_fwhm / their_fwhm;

    let frame_count = reference.frames.len();
    let stacked = master.frame_count;
    let mut report = String::new();
    let _ = writeln!(
        report,
        "px-pipeline native master light vs siril\n\
         frames: {frame_count} in, {stacked} stacked by both\n\n\
         output_norm relation: ours = {scale:.5} x siril {offset:+.6}\n\
         (both finish with a min-max rescale keyed to two extreme pixels, so this\n\
         factor is expected and carries no photometric information)\n\n\
         after removing it:\n\
         sky (deciles 1-{SKY_DECILES}): rms {sky_rms:.3} ADU = {:.3} sigma_bg\n\
         signal (decile 10):   rms {signal_rms:.3} ADU = {:.3} sigma_bg\n\
         whole frame:          rms {total_rms:.3} ADU = {:.3} sigma_bg\n\n\
         raw: rms {raw_rms:.3} ADU = {:.3} sigma_bg, max {raw_max:.1} ADU = {:.0} sigma_bg\n\n\
         quality:\n\
         background noise: ours {:.4} ADU, siril {:.4} ADU (ratio {noise_ratio:.4})\n\
         median FWHM:      ours {our_fwhm:.3} px, siril {their_fwhm:.3} px (ratio {fwhm_ratio:.4})\n\
         stars detected:   ours {our_stars}, siril {their_stars}\n\n\
         wall clock: ours {elapsed:.1} s, siril {}\n\
         compared {compared} of {} pixels\n",
        sky_rms / sigma_bg,
        signal_rms / sigma_bg,
        total_rms / sigma_bg,
        raw_rms / sigma_bg,
        raw_max / sigma_bg,
        ours_stats.background_noise * ADU,
        theirs_stats.background_noise * ADU,
        reference
            .elapsed_seconds
            .map(|s| format!("{s:.1} s"))
            .unwrap_or_else(|| "unknown".to_string()),
        ours.data().len(),
    );
    println!("{report}");
    write_report(&report);

    let mut failures = Vec::new();
    if sky_rms / sigma_bg > SKY_RMS_TOLERANCE {
        failures.push(format!(
            "sky rms {:.3} sigma_bg exceeds {SKY_RMS_TOLERANCE}",
            sky_rms / sigma_bg
        ));
    }
    if noise_ratio > MAX_NOISE_RATIO {
        failures.push(format!(
            "native master is {:.1}% noisier than Siril's",
            100.0 * (noise_ratio - 1.0)
        ));
    }
    if fwhm_ratio > MAX_FWHM_RATIO {
        failures.push(format!(
            "native stars are {:.1}% fatter than Siril's",
            100.0 * (fwhm_ratio - 1.0)
        ));
    }
    if our_stars < their_stars {
        failures.push(format!(
            "native master yields {our_stars} stars against Siril's {their_stars}"
        ));
    }
    assert!(
        failures.is_empty(),
        "{} master-light parity failure(s):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

fn write_report(body: &str) {
    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("master-light-parity.txt");
    if std::fs::write(&path, body).is_ok() {
        println!("report written to {}", path.display());
    }
}
