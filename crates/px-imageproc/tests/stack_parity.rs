//! Stacking parity against Siril, on the real-frame corpus (ADR 007, P3-T8).
//!
//! Reference masters come from `cargo xtask siril-reference --masters`, which
//! issues the same commands `px --engine siril` issues today. Neither the
//! corpus nor the reference is committed, so these tests skip when absent.

use std::path::{Path, PathBuf};
use std::time::Instant;

use px_fits::card::Value;
use px_fits::reader::FitsReader;
use px_imageproc::runtime::{MemoryLimit, Runtime};
use px_imageproc::stack::{Method, StackOptions, stack};
use px_imageproc::stats::{self, ChannelStats};
use px_imageproc::{Frame, FrameSet};

const ADU: f64 = 65535.0;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

fn corpus() -> PathBuf {
    std::env::var("PX_IMAGEPROC_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root().join("data/corpus-mono"))
}

fn reference_master(kind: &str) -> Option<PathBuf> {
    let path = repo_root()
        .join("data/reference/masters")
        .join(format!("master_{kind}.fits"));
    path.exists().then_some(path)
}

fn frames_of(kind: &str) -> Option<Vec<PathBuf>> {
    let dir = corpus().join(kind);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "fits" | "fit" | "fts"))
        })
        .collect();
    if paths.is_empty() {
        return None;
    }
    paths.sort();
    Some(paths)
}

fn load(path: &Path) -> Frame {
    let reader = FitsReader::open(path).expect("open");
    let hdu = reader.primary_image().expect("image");
    Frame::from_hdu(&hdu).expect("decode")
}

/// Per-pixel difference summary between two frames, in ADU.
struct Diff {
    max_abs: f64,
    rms: f64,
    mean_signed: f64,
    differing: usize,
}

fn compare(ours: &[f32], theirs: &[f32]) -> Diff {
    assert_eq!(ours.len(), theirs.len(), "frames differ in size");
    let mut max_abs = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut sum = 0.0f64;
    let mut differing = 0usize;

    for (a, b) in ours.iter().zip(theirs) {
        let d = (*a as f64 - *b as f64) * ADU;
        if d != 0.0 {
            differing += 1;
        }
        max_abs = max_abs.max(d.abs());
        sum_sq += d * d;
        sum += d;
    }

    Diff {
        max_abs,
        rms: (sum_sq / ours.len() as f64).sqrt(),
        mean_signed: sum / ours.len() as f64,
        differing,
    }
}

/// Reads whole 61 MP sequences; ~seconds optimized, minutes in a debug build.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "reads a full 61 MP sequence; run with --release"
)]
fn median_masters_match_siril() {
    let mut report = String::new();
    let mut any = false;

    for kind in ["bias", "dark"] {
        let (Some(paths), Some(reference)) = (frames_of(kind), reference_master(kind)) else {
            eprintln!(
                "skipping {kind}: need {}/{kind} and a reference master. \
                 Run `cargo xtask siril-reference --masters`.",
                corpus().display()
            );
            continue;
        };
        any = true;

        let set = FrameSet::open(&paths).expect("open set");
        let dims = set.dims();

        // 8 GiB is a realistic desktop budget and forces a real multi-block
        // plan for a 61 MP, 40-frame stack.
        let runtime = Runtime::resolve(MemoryLimit::FixedGb(8.0), None).expect("runtime");

        let started = Instant::now();
        let result = stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack");
        let elapsed = started.elapsed();

        let theirs = load(&reference);
        assert_eq!(
            theirs.dims(),
            dims,
            "{kind}: reference master has different dimensions"
        );

        let diff = compare(result.frame.data(), theirs.data());

        // Saved so the two can be opened side by side.
        let out_dir = repo_root().join("data/output/masters");
        std::fs::create_dir_all(&out_dir).expect("create output dir");
        let out = out_dir.join(format!("master_{kind}_native.fits"));
        result
            .frame
            .write(
                &out,
                &[
                    (
                        "STACKCNT",
                        Value::Integer(set.len() as i64),
                        Some("frames stacked"),
                    ),
                    (
                        "HISTORY",
                        Value::Commentary("px native median stack".into()),
                        None,
                    ),
                ],
            )
            .expect("write master");

        let ours_stats = ChannelStats::compute(result.frame.channel(0), dims.width, dims.height);
        let theirs_stats = ChannelStats::compute(theirs.channel(0), dims.width, dims.height);

        use std::fmt::Write as _;
        let mut section = String::new();
        let _ = writeln!(
            section,
            "master {kind}: {} frames, {}x{}, {} blocks, {:.2}s\n  \
             max |delta| {:.4} ADU, rms {:.5} ADU, mean signed {:+.6} ADU, {} of {} pixels differ\n  \
             median  ours {:.4}  siril {:.4}\n  \
             bgnoise ours {:.4}  siril {:.4}\n  \
             written to {}",
            set.len(),
            dims.width,
            dims.height,
            result.block_count,
            elapsed.as_secs_f64(),
            diff.max_abs,
            diff.rms,
            diff.mean_signed,
            diff.differing,
            result.frame.data().len(),
            ours_stats.median as f64 * ADU,
            theirs_stats.median as f64 * ADU,
            ours_stats.background_noise * ADU,
            theirs_stats.background_noise * ADU,
            out.display(),
        );
        print!("{section}");
        report.push_str(&section);

        // A median is an order statistic: picking the same value out of the
        // same 40 numbers is exact, not approximate. The only slack allowed is
        // the f32 representation of a half-way point for even frame counts.
        assert!(
            diff.max_abs < 0.01,
            "{kind}: worst pixel differs by {:.4} ADU — a median stack should be exact",
            diff.max_abs
        );
        assert!(
            (ours_stats.background_noise - theirs_stats.background_noise).abs() * ADU < 0.06,
            "{kind}: background noise differs"
        );
    }

    if !any {
        eprintln!("no corpus or reference available; nothing compared");
        return;
    }

    let path = repo_root().join("data/output/stack-parity.txt");
    let _ = std::fs::write(&path, &report);
    println!("report written to {}", path.display());
}

/// The master a median stack produces must be quieter than any single frame
/// that went into it — by roughly sqrt(n) for independent noise. This holds
/// without any reference implementation to compare against.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "reads a full 61 MP sequence; run with --release"
)]
fn a_median_master_is_quieter_than_its_inputs() {
    let Some(paths) = frames_of("bias") else {
        eprintln!("skipping: no bias frames in {}", corpus().display());
        return;
    };

    let set = FrameSet::open(&paths).expect("open set");
    let runtime = Runtime::resolve(MemoryLimit::FixedGb(8.0), None).expect("runtime");
    let result = stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack");

    let single = set.load(0).expect("load one frame");
    let single_noise = stats::background_noise(single.channel(0), single.width(), single.height());
    let master_noise = stats::background_noise(
        result.frame.channel(0),
        result.frame.width(),
        result.frame.height(),
    );

    let n = set.len() as f64;
    let improvement = single_noise / master_noise;

    // Bounds, and why they are where they are.
    //
    // Ceiling: averaging n independent samples cannot beat sqrt(n), and the
    // mean is the best estimator there is — so a median that appeared to beat
    // it would mean the output is not what it claims to be (smoothed,
    // degenerate, or reduced over the wrong axis).
    //
    // Floor: the ideal for a median is sqrt(n) divided by the median's
    // efficiency penalty of sqrt(pi/2) ~ 1.253, giving ~5.0x at 40 frames.
    // Real bias frames fall short of that because a chunk of their noise is
    // fixed pattern — read-noise structure that is identical in every frame
    // and therefore survives any amount of averaging. This corpus measures
    // 4.14x against that 5.0x ideal, implying ~82% of single-frame noise is
    // the random part, which is ordinary for a CMOS sensor. The floor is set
    // below that with margin, but high enough to catch a stack that quietly
    // combined only half the frames (which would land near 2.9x).
    let ceiling = n.sqrt();
    let ideal = n.sqrt() / (std::f64::consts::FRAC_PI_2).sqrt();
    let floor = 3.5;

    println!(
        "single-frame noise {:.4} ADU, master {:.4} ADU\n\
         improvement {improvement:.2}x over {} frames (floor {floor:.2}, median ideal {ideal:.2}, mean ceiling {ceiling:.2})",
        single_noise * ADU,
        master_noise * ADU,
        set.len(),
    );

    assert!(
        improvement > floor,
        "median of {} frames only improved noise by {improvement:.2}x (floor {floor:.2})",
        set.len()
    );
    assert!(
        improvement < ceiling,
        "improvement of {improvement:.2}x beats the {ceiling:.2}x limit for averaging {} \
         independent samples — the output is not a straight median of the inputs",
        set.len()
    );
}
