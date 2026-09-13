//! Where the time actually goes, for all five native pipelines (ADR 007, P12-T1).
//!
//! Runs the real pipeline structs over the real-frame corpus with the span
//! recorder on, and writes one report per pipeline to
//! `data/output/profile.txt`. This is the measurement P12-T2 acts on, so it is
//! a test rather than a benchmark: it must exercise the same code path a user
//! gets, on data of the size a user has, and criterion's repeat-until-stable
//! model does neither for a pipeline that reads sixty gigabytes.
//!
//! Nothing here asserts a timing. A wall clock is a property of the machine, and
//! a test that fails when someone profiles on a laptop would be deleted within a
//! week. What it does assert is that every pipeline produced a report — an empty
//! table means the instrumentation has drifted away from the code, which is the
//! failure worth catching.
//!
//! ```text
//! cargo test --release -p px-pipeline --test profile -- --nocapture
//! ```

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use px_imageproc::profile;
use px_pipeline::calibration::calibrate_light::CalibrateLightSetPipeline;
use px_pipeline::calibration::master_bias::CreateMasterBiasPipeline;
use px_pipeline::calibration::master_dark::CreateMasterDarkPipeline;
use px_pipeline::calibration::master_flat::CreateMasterFlatPipeline;
use px_pipeline::project::master_light::CreateMasterLightPipeline;
use px_pipeline::{NativeRuntime, PipelineReporter, RunIn};
use siril_sys::FitsExt;

/// Lights to calibrate. The stage split is per frame and identical for each, so
/// a handful measures it as well as a hundred would and leaves the disk alone.
const CALIBRATE_FRAMES: usize = 8;

/// The budget a default desktop run resolves to. Fixed rather than
/// machine-derived so two profiles taken on the same hardware are comparable.
const BUDGET_GB: f64 = 8.0;

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

fn out_dir(name: &str) -> PathBuf {
    let d = repo_root().join("data/output/profile").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create output dir");
    d
}

fn runtime() -> NativeRuntime {
    NativeRuntime::default()
        .use_memory_fixed_gb(BUDGET_GB)
        .use_extension(FitsExt::FITS)
}

struct Quiet;

impl PipelineReporter for Quiet {
    fn step_started(&self, _message: &str) -> usize {
        0
    }
    fn step_ended(&self, _id: usize, _message: &str, _success: bool) {}
}

/// One pipeline's table, with the wall clock it was taken against.
struct Section {
    name: &'static str,
    detail: String,
    wall: Duration,
    table: String,
}

/// Runs `body` with a clean recorder and captures what it recorded.
async fn measure<F, Fut, T>(name: &'static str, body: F) -> (T, Section)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    profile::reset();
    profile::enable();
    let started = Instant::now();
    let value = body().await;
    let wall = started.elapsed();
    let table = profile::render(wall);
    profile::disable();
    (
        value,
        Section {
            name,
            detail: String::new(),
            wall,
            table,
        },
    )
}

#[tokio::test]
#[cfg_attr(
    debug_assertions,
    ignore = "runs all five pipelines over a 61 MP corpus; run with --release"
)]
async fn profile_all_five_pipelines() {
    let (Some(bias_raw), Some(dark_raw), Some(flat_raw), Some(lights)) = (
        corpus("bias"),
        corpus("dark"),
        corpus("flat"),
        corpus("light"),
    ) else {
        eprintln!("skipping: needs data/corpus-mono/{{bias,dark,flat,light}}");
        return;
    };

    let mut sections = Vec::new();

    // --- 1. master bias -----------------------------------------------------
    let out = out_dir("bias");
    let (bias, mut section) = measure("master bias", || async {
        CreateMasterBiasPipeline::builder()
            .ext(FitsExt::FITS)
            .raw_folder(bias_raw)
            .out_folder(out)
            .build()
            .run(Quiet, runtime())
            .await
            .expect("native master bias")
    })
    .await;
    section.detail = format!("{} frames, median", bias.frame_count);
    sections.push(section);

    // --- 2. master dark -----------------------------------------------------
    let out = out_dir("dark");
    let (dark, mut section) = measure("master dark", || async {
        CreateMasterDarkPipeline::builder()
            .ext(FitsExt::FITS)
            .raw_folder(dark_raw)
            .out_folder(out)
            .build()
            .run(Quiet, runtime())
            .await
            .expect("native master dark")
    })
    .await;
    section.detail = format!("{} frames, median", dark.frame_count);
    sections.push(section);

    // --- 3. master flat -----------------------------------------------------
    let out = out_dir("flat");
    let bias_path = bias.path.clone();
    let (flat, mut section) = measure("master flat", || async {
        CreateMasterFlatPipeline::builder()
            .ext(FitsExt::FITS)
            .raw_folder(flat_raw)
            .bias(bias_path)
            // The corpus is a single-filter set; the name only reaches the
            // master's metadata.
            .filter("H".to_string())
            .out_folder(out)
            .build()
            .run(Quiet, runtime())
            .await
            .expect("native master flat")
    })
    .await;
    section.detail = format!("{} frames, bias subtracted in flight", flat.frame_count);
    sections.push(section);

    // --- 4. calibrate lights ------------------------------------------------
    // A folder of its own, because the pipeline calibrates everything it finds
    // and the corpus holds more lights than this stage needs.
    let staging = tempfile::tempdir().expect("tempdir");
    let raw = staging.path().join("raw");
    std::fs::create_dir_all(&raw).expect("raw dir");
    let mut light_files: Vec<PathBuf> = std::fs::read_dir(&lights)
        .expect("read lights")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "fits" || e == "fit"))
        .collect();
    light_files.sort();
    for frame in light_files.iter().take(CALIBRATE_FRAMES) {
        std::os::unix::fs::symlink(frame, raw.join(frame.file_name().expect("name")))
            .expect("link light");
    }

    let out = out_dir("calibrated");
    let (_, mut section) = measure("calibrate lights", || async {
        CalibrateLightSetPipeline::builder()
            .ext(FitsExt::FITS)
            .raw_folder(raw)
            .out_folder(out)
            .bias(bias.path.clone())
            .dark(dark.path.clone())
            .flat(flat.path.clone())
            .build()
            .run(Quiet, runtime())
            .await
            .expect("native calibrate lights")
    })
    .await;
    section.detail = format!("{CALIBRATE_FRAMES} frames, mono (no debayer)");
    sections.push(section);

    // --- 5. master light ----------------------------------------------------
    let out = out_dir("master-light");
    let (master, mut section) = measure("master light", || async {
        CreateMasterLightPipeline::builder()
            .ext(FitsExt::FITS)
            .light_folders(vec![lights])
            .name("profile".to_string())
            .out_folder(out)
            .build()
            .run(Quiet, runtime())
            .await
            .expect("native master light")
    })
    .await;
    section.detail = format!(
        "{} frames registered, warped and stacked",
        master.frame_count
    );
    sections.push(section);

    let mut report = String::new();
    let _ = writeln!(
        report,
        "px native pipeline profile\n\
         budget {BUDGET_GB} GB, {} cores\n",
        std::thread::available_parallelism().map_or(0, |n| n.get())
    );
    for section in &sections {
        let _ = writeln!(
            report,
            "== {} ({}) ==\n{}",
            section.name, section.detail, section.table
        );
    }
    println!("{report}");

    let dir = repo_root().join("data/output");
    if std::fs::create_dir_all(&dir).is_ok() {
        let path = dir.join("profile.txt");
        if std::fs::write(&path, &report).is_ok() {
            println!("report written to {}", path.display());
        }
    }

    for section in &sections {
        assert!(
            !section.table.is_empty(),
            "{} recorded no spans in {:.1} s — the instrumentation has drifted \
             away from the code it is supposed to be measuring",
            section.name,
            section.wall.as_secs_f64()
        );
    }
}
