//! End-to-end master-frame pipelines in the native engine (ADR 007, P5-T7).
//!
//! Runs the real pipeline structs — the same ones `px` calls — against the
//! real-frame corpus, and compares their output to masters Siril produced from
//! the same frames. Skips when the corpus or the references are absent, so a
//! checkout without them stays green.

use std::path::{Path, PathBuf};

use px_imageproc::Frame;
use px_imageproc::stats::ChannelStats;
use px_pipeline::calibration::master_bias::CreateMasterBiasPipeline;
use px_pipeline::calibration::master_dark::CreateMasterDarkPipeline;
use px_pipeline::calibration::master_flat::CreateMasterFlatPipeline;
use px_pipeline::{NativeRuntime, PipelineReporter, RunIn};
use siril_sys::FitsExt;

const ADU: f64 = 65535.0;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn corpus(kind: &str) -> Option<PathBuf> {
    let p = repo_root().join("data/corpus-mono").join(kind);
    (p.is_dir() && std::fs::read_dir(&p).ok()?.next().is_some()).then_some(p)
}

fn reference(name: &str) -> Option<PathBuf> {
    let p = repo_root()
        .join("data/reference/masters")
        .join(format!("{name}.fits"));
    p.exists().then_some(p)
}

fn out_dir(name: &str) -> PathBuf {
    let d = repo_root().join("data/output/pipelines").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create output dir");
    d
}

/// Silent reporter: these tests care about the frames, not the progress bars.
struct Quiet;

impl PipelineReporter for Quiet {
    fn step_started(&self, _message: &str) -> usize {
        0
    }
    fn step_ended(&self, _id: usize, _message: &str, _success: bool) {}
}

fn runtime() -> NativeRuntime {
    // A realistic desktop budget, and one that forces a genuine multi-block
    // plan for a 61 MP, 40-frame stack.
    NativeRuntime::default()
        .use_memory_fixed_gb(8.0)
        .use_extension(FitsExt::FITS)
}

struct Comparison {
    max_abs: f64,
    rms: f64,
}

/// Pixels differing by more than `threshold` ADU.
fn a_vs_b_count(ours: &Path, theirs: &Path, threshold: f64) -> usize {
    let a = Frame::open(ours).expect("open native master");
    let b = Frame::open(theirs).expect("open siril master");
    a.data()
        .iter()
        .zip(b.data())
        .filter(|(x, y)| (((**x as f64) - (**y as f64)) * ADU).abs() > threshold)
        .count()
}

fn compare(ours: &Path, theirs: &Path) -> (Comparison, ChannelStats, ChannelStats) {
    let a = Frame::open(ours).expect("open native master");
    let b = Frame::open(theirs).expect("open siril master");
    assert_eq!(a.dims(), b.dims(), "masters differ in shape");

    let mut max_abs = 0.0f64;
    let mut sum_sq = 0.0f64;
    for (x, y) in a.data().iter().zip(b.data()) {
        let d = (*x as f64 - *y as f64) * ADU;
        max_abs = max_abs.max(d.abs());
        sum_sq += d * d;
    }

    let stats_a = ChannelStats::compute(a.channel(0), a.width(), a.height());
    let stats_b = ChannelStats::compute(b.channel(0), b.width(), b.height());
    (
        Comparison {
            max_abs,
            rms: (sum_sq / a.data().len() as f64).sqrt(),
        },
        stats_a,
        stats_b,
    )
}

#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "stacks a full 61 MP sequence; run with --release"
)]
async fn native_master_bias_matches_siril() {
    let (Some(raw), Some(expected)) = (corpus("bias"), reference("master_bias")) else {
        eprintln!("skipping: needs data/corpus-mono/bias and a Siril reference master");
        return;
    };
    let out = out_dir("bias");

    let master = CreateMasterBiasPipeline::builder()
        .ext(FitsExt::FITS)
        .raw_folder(raw)
        .out_folder(out)
        .build()
        .run(Quiet, runtime())
        .await
        .expect("native master bias");

    assert_eq!(master.frame_count, 40);
    assert_eq!(master.gain, 100);

    let (diff, ours, theirs) = compare(&master.path, &expected);
    println!(
        "master bias: max |delta| {:.4} ADU, rms {:.5} ADU\n  median ours {:.3} siril {:.3}\n  \
         bgnoise ours {:.4} siril {:.4}\n  {}",
        diff.max_abs,
        diff.rms,
        ours.median as f64 * ADU,
        theirs.median as f64 * ADU,
        ours.background_noise * ADU,
        theirs.background_noise * ADU,
        master.path.display(),
    );

    // A median is an order statistic; the only slack is the f32 rounding of a
    // half-way point for an even frame count.
    assert!(diff.max_abs < 0.01, "max delta {:.4} ADU", diff.max_abs);
}

#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "stacks a full 61 MP sequence; run with --release"
)]
async fn native_master_dark_matches_siril() {
    let (Some(raw), Some(expected)) = (corpus("dark"), reference("master_dark")) else {
        eprintln!("skipping: needs data/corpus-mono/dark and a Siril reference master");
        return;
    };
    let out = out_dir("dark");

    let master = CreateMasterDarkPipeline::builder()
        .ext(FitsExt::FITS)
        .raw_folder(raw)
        .out_folder(out)
        .build()
        .run(Quiet, runtime())
        .await
        .expect("native master dark");

    assert_eq!(master.frame_count, 40);
    assert!(
        (master.exposure - 300.0).abs() < 0.01,
        "exposure {} should come from the frames",
        master.exposure
    );

    let (diff, ours, theirs) = compare(&master.path, &expected);
    println!(
        "master dark: max |delta| {:.4} ADU, rms {:.5} ADU\n  median ours {:.3} siril {:.3}\n  \
         bgnoise ours {:.4} siril {:.4}",
        diff.max_abs,
        diff.rms,
        ours.median as f64 * ADU,
        theirs.median as f64 * ADU,
        ours.background_noise * ADU,
        theirs.background_noise * ADU,
    );
    assert!(diff.max_abs < 0.01, "max delta {:.4} ADU", diff.max_abs);
}

#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "stacks a full 61 MP sequence; run with --release"
)]
async fn native_master_flat_matches_siril() {
    let (Some(raw), Some(bias), Some(expected)) = (
        corpus("flat"),
        reference("master_bias"),
        reference("master_flat_winsorized"),
    ) else {
        eprintln!("skipping: needs data/corpus-mono/flat and Siril reference masters");
        return;
    };
    let out = out_dir("flat");

    // Siril's own master bias, so the comparison isolates the flat pipeline
    // rather than compounding two differences.
    let master = CreateMasterFlatPipeline::builder()
        .ext(FitsExt::FITS)
        .raw_folder(raw)
        .bias(bias)
        .filter("H".to_string())
        .out_folder(out)
        .build()
        .run(Quiet, runtime())
        .await
        .expect("native master flat");

    assert_eq!(
        master.filter, "H",
        "the filter must reach the output header"
    );

    let (diff, ours, theirs) = compare(&master.path, &expected);
    let sigma_bg = theirs.background_noise * ADU;
    println!(
        "master flat: max |delta| {:.4} ADU ({:.4} sigma_bg), rms {:.5} ADU ({:.5} sigma_bg)\n  \
         median ours {:.3} siril {:.3}\n  bgnoise ours {:.4} siril {:.4}\n  {} frames",
        diff.max_abs,
        diff.max_abs / sigma_bg,
        diff.rms,
        diff.rms / sigma_bg,
        ours.median as f64 * ADU,
        theirs.median as f64 * ADU,
        ours.background_noise * ADU,
        theirs.background_noise * ADU,
        master.frame_count,
    );

    // ADR 007's RMS bound for a master flat.
    assert!(
        diff.rms < 0.02 * sigma_bg,
        "rms delta {:.5} ADU = {:.4} sigma_bg (bound 0.02)",
        diff.rms,
        diff.rms / sigma_bg
    );

    // Neither the max delta nor the exact set of differing pixels is gated
    // here, and the ADR records why.
    //
    // Rejection is a discrete per-pixel decision. A value sitting on the
    // clipping threshold goes one way in one implementation and the other way
    // in another, and that single flip moves the pixel's mean by
    // (outlier - mean) / n — around 5-20 ADU for a dust mote over 59 frames.
    // Two implementations written independently cannot agree on every such
    // pixel, and this one is written independently on purpose: it is a
    // clean-room implementation from the published description of Winsorized
    // clipping, not a translation of Siril's.
    //
    // What that leaves is a small population of boundary disagreements, which
    // is measured and reported rather than gated tightly.
    let differing = a_vs_b_count(&master.path, &expected, 0.01);
    let total = 9576 * 6388;
    println!(
        "  {differing} of {total} pixels differ by more than 0.01 ADU ({:.4}%); \
         max delta {:.3} ADU = {:.3} sigma_bg",
        100.0 * differing as f64 / total as f64,
        diff.max_abs,
        diff.max_abs / sigma_bg,
    );
    assert!(
        differing < total / 200,
        "{differing} pixels differ, over 0.5% of the frame — too many to be boundary ties"
    );

    // The worst single pixel must stay below the gap between Siril's own two
    // rejection methods on this same data (165.7 ADU, measured in
    // tests/calibrate_parity.rs). Being closer to Siril's Winsorized master
    // than Siril's own methods are to each other is the meaningful statement.
    assert!(
        diff.max_abs < 165.0,
        "worst pixel differs by {:.2} ADU, more than Siril's own Winsorized-vs-linear gap",
        diff.max_abs
    );

    // Background noise is the quality measure that actually matters for a
    // flat: it says the two masters are equally smooth.
    let noise_shift =
        (ours.background_noise - theirs.background_noise).abs() / theirs.background_noise;
    assert!(
        noise_shift < 0.001,
        "background noise differs by {:.4}% (bound 0.1%)",
        noise_shift * 100.0
    );

    // The mean flat level must be within 0.1% — it is the divisor every light
    // is scaled by, so a systematic shift here shifts every calibrated frame.
    let level_shift = (ours.mean - theirs.mean).abs() / theirs.mean;
    assert!(
        level_shift < 0.001,
        "flat level differs by {:.4}% (bound 0.1%)",
        level_shift * 100.0
    );
}
