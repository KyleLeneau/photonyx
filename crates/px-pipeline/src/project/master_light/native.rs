//! Execution in the native engine (ADR 007, P11-T1 / P11-T3 / P11-T4).
//!
//! The whole master-light path in one place: collect the calibrated lights,
//! two-pass register them, warp each one onto the reference's grid, and combine
//! them with per-frame photometric normalization and the rejection the frame
//! count calls for.
//!
//! Where the warped frames live is decided by the budget, not by a flag. If they
//! all fit, they stay in memory and the stacker reads them from there; if they do
//! not, they spill to a temporary sequence and the stacker streams them back a
//! band at a time. A hundred 61 MP frames is 24 GB of warped `f32`, so the spill
//! path is the normal one on a real session — and it is what keeps a 500-frame
//! run inside 8 GB.

use std::path::{Path, PathBuf};

use px_fits::all_fits_files;
use px_fits::card::Value;
use px_imageproc::register::framing::Framing;
use px_imageproc::register::twopass::{Registration, RegistrationOptions, register_sequence};
use px_imageproc::register::warp::{WarpOptions, warp};
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::stack::normalize::{ChannelLevel, NormMethod, Normalization};
use px_imageproc::stack::{
    Bounds, LoadedFrames, Method, Rejection, StackOptions, stack_normalized,
};
use px_imageproc::{Frame, FrameSet};
use siril_sys::{BestRejection, StackRejection};

use super::{CreateMasterLightPipeline, master_light_path};
use crate::error::PipelineError;
use crate::meta::LinearStackMetadata;
use crate::model::MasterLight;
use crate::native::{NativeEngine, NativeRuntime};
use crate::{PipelineReporter, RunIn};

/// Shares of the configured budget the three phases may each hold.
///
/// The budget is a promise about the *process*, and a phase-by-phase promise is
/// not the same thing: memory is not handed back to the operating system when it
/// is dropped, so a later phase allocates on top of what an earlier one is still
/// holding. Measured on a hundred 61 MP frames at `--max-memory 8G`, with each
/// phase sizing itself to the full 8 GB — every phase individually inside its
/// budget, the run peaking at **19.9 GB**.
///
/// So the budget is partitioned rather than lent out three times. The shares are
/// not equal because the phases are not equally elastic: stacking's band plan is
/// what most wants room, registration is the phase whose freed memory is least
/// likely to come back, and warping needs only enough for a few frames in
/// flight.
const REGISTRATION_BUDGET_SHARE: f64 = 0.3;
const STACK_BUDGET_SHARE: f64 = 0.55;

/// Fraction of the budget the warped frames may occupy before the run spills
/// them to disk.
///
/// The stacker needs the rest: the result, the per-worker bands, and the
/// rejection scratch. Half is conservative and deliberately so — the penalty for
/// guessing wrong is an overrun of a budget the user set, while the penalty for
/// spilling unnecessarily is some I/O.
const MEMORY_PATH_BUDGET_SHARE: f64 = 0.5;

impl RunIn<NativeRuntime> for CreateMasterLightPipeline {
    type Output = MasterLight;

    async fn run(
        &self,
        reporter: impl PipelineReporter,
        env: NativeRuntime,
    ) -> Result<MasterLight, PipelineError> {
        if self.light_folders.is_empty() {
            return Err(PipelineError::FileNotFound(
                "missing light_folders".to_string(),
            ));
        }
        // Background extraction is a per-frame model fit the native engine does
        // not implement. Reported rather than skipped: a user who asked for it
        // and silently did not get it would have a different image and no way to
        // know why (ADR 007 D5).
        if self.background_extract {
            return Err(PipelineError::UnsupportedByEngine {
                pipeline: "CreateMasterLight with background extraction".to_string(),
                engine: "native".to_string(),
                detail: ": run it with --engine siril, or turn extract_background off".to_string(),
            });
        }

        // Frame order is the Siril implementation's: each light folder in the
        // order configured, and within a folder whatever `all_fits_files`
        // returns. Registration indices and the sidecar refer to this order.
        let mut frames: Vec<PathBuf> = Vec::new();
        for folder in &self.light_folders {
            frames.extend(all_fits_files(folder)?);
        }
        if frames.len() < 2 {
            return Err(PipelineError::FileNotFound(format!(
                "need at least two light frames to register and stack, found {}",
                frames.len()
            )));
        }

        let engine = env.use_extension(self.ext.clone()).build()?;
        let output = engine.with_extension(&master_light_path(&self.out_folder, &self.name));

        let id = reporter.step_started(&format!(
            "[1/3] Registering {} light frames...",
            frames.len()
        ));
        let job_frames = frames.clone();
        let registration_engine = engine.clone();
        let registration = tokio::task::spawn_blocking(move || {
            run_registration(&registration_engine, &job_frames)
        })
        .await
        .map_err(|e| PipelineError::FileNotFound(format!("registration task failed: {e}")))?
        .inspect_err(|_| reporter.step_ended(id, "✗ Registration failed", false))?;
        reporter.step_ended(
            id,
            &format!(
                "[1/3] Registered {} of {} frames onto frame {} (FWHM {:.2} px){}",
                registration.included_count(),
                frames.len(),
                registration.reference + 1,
                registration.mean_fwhm(),
                if registration.trials > 1 {
                    format!(", {} detection trials", registration.trials)
                } else {
                    String::new()
                }
            ),
            true,
        );

        // Every dropped frame is reported with its reason: a run that quietly
        // stacked 60 of 100 frames is the thing a user most needs told.
        for frame in registration.frames.iter().filter(|f| !f.included()) {
            let reason = frame
                .excluded
                .map(|e| e.to_string())
                .unwrap_or_else(|| "not registered".to_string());
            let name = frame
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            reporter.step_ended(
                reporter.step_started("⚠ Frame dropped"),
                &format!("{name}: {reason}"),
                false,
            );
        }

        let id = reporter.step_started(&format!(
            "[2/3] Warping and stacking {} frames...",
            registration.included_count()
        ));
        let stack_engine = engine.clone();
        let job = StackJob {
            frames,
            registration,
            output: output.clone(),
            extension: engine.ext().to_string(),
        };
        let outcome = tokio::task::spawn_blocking(move || run_warp_and_stack(&stack_engine, job))
            .await
            .map_err(|e| PipelineError::FileNotFound(format!("stacking task failed: {e}")))?
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;
        reporter.step_ended(
            id,
            &format!(
                "[2/3] Stacked {} frames, {} method, {} in {} blocks ({} values rejected) \
                 — warp {:.1}s, combine {:.1}s",
                outcome.frame_count,
                outcome.method,
                if outcome.spilled {
                    "warped frames spilled to disk"
                } else {
                    "warped frames held in memory"
                },
                outcome.blocks,
                outcome.rejected,
                outcome.warp_seconds,
                outcome.stack_seconds
            ),
            true,
        );

        let id = reporter.step_started("[3/3] Reading master light metadata...");
        let stack = LinearStackMetadata::from(output.clone())?;
        reporter.step_ended(id, "[3/3] Master light written", true);

        Ok(MasterLight {
            sources: self.light_folders.clone(),
            path: output,
            exposure: stack.total_exposure.unwrap_or_default(),
            filter: stack.filter.or(self.filter.clone()),
            binning: stack.binning,
            frame_count: stack.frame_count as usize,
            target_name: stack.target_name,
            target_ra: stack.target_ra,
            target_dec: stack.target_dec,
        })
    }
}

/// A runtime holding one phase's share of the configured budget.
fn phase_runtime(engine: &NativeEngine, share: f64) -> Result<Runtime, PipelineError> {
    let budget = engine.budget();
    let bytes = (budget.max_bytes as f64 * share) as u64;
    Ok(Runtime::new(ResourceBudget::new(
        bytes.max(1),
        budget.cores,
    ))?)
}

/// Two-pass registration over the collected frames.
fn run_registration(
    engine: &NativeEngine,
    frames: &[PathBuf],
) -> Result<Registration, PipelineError> {
    let set = FrameSet::open(frames)?;
    let runtime = phase_runtime(engine, REGISTRATION_BUDGET_SHARE)?;
    let options = RegistrationOptions {
        // The reference frame's own geometry, which is what Siril's default
        // `seqapplyreg` produces and what the project's downstream steps expect.
        framing: Framing::Current,
        ..Default::default()
    };
    Ok(register_sequence(&set, &runtime, &options)?)
}

struct StackJob {
    frames: Vec<PathBuf>,
    registration: Registration,
    output: PathBuf,
    extension: String,
}

struct StackOutcome {
    frame_count: usize,
    blocks: usize,
    rejected: u64,
    spilled: bool,
    method: String,
    /// Seconds spent warping and seconds spent combining, reported separately:
    /// they scale differently — warping with the frame count and the kernel,
    /// combining with the frame count and the rejection method — and a run that
    /// is slower than expected is usually slower in exactly one of them.
    warp_seconds: f64,
    stack_seconds: f64,
}

/// Warps every included frame and combines them.
fn run_warp_and_stack(engine: &NativeEngine, job: StackJob) -> Result<StackOutcome, PipelineError> {
    let included: Vec<usize> = job
        .registration
        .included()
        .map(|frame| frame.index)
        .collect();
    if included.len() < 2 {
        return Err(PipelineError::FileNotFound(format!(
            "only {} frame(s) registered; nothing to stack",
            included.len()
        )));
    }

    let runtime = phase_runtime(engine, STACK_BUDGET_SHARE)?;
    let dims = job.registration.geometry.dims;
    let warped_bytes = dims.pixel_count() as u64 * 4;
    let budget = runtime.budget();
    let allowance = (budget.max_bytes as f64 * MEMORY_PATH_BUDGET_SHARE) as u64;
    let spill = warped_bytes.saturating_mul(included.len() as u64) > allowance;

    // Siril's own choice for the frame count, so both engines reject the same
    // way for the same input (ADR 007 D9).
    let best = BestRejection::find(included.len());
    let method = Method::Mean(rejection_from(&best));

    // The registration reference, as an index into the *included* frames: that
    // is the frame everything is normalized onto.
    let reference = included
        .iter()
        .position(|&i| i == job.registration.reference)
        .unwrap_or(0);

    let options = StackOptions::new(method)
        // Siril's `-output_norm`, which rescales the result into the unit range.
        .output_norm(true);

    let warp_started = std::time::Instant::now();
    let (result, blocks, rejected, warp_seconds) = if spill {
        // Honours the configured working directory when there is one: the spill
        // is tens of gigabytes for a real session, and which volume that lands
        // on is a decision a user may well have made deliberately.
        let temp = match engine.work_dir() {
            Some(dir) => {
                std::fs::create_dir_all(dir)?;
                tempfile::tempdir_in(dir)?
            }
            None => tempfile::tempdir()?,
        };
        // Levels are measured as each frame is warped, while it is still in
        // memory. Measuring them afterwards, from the files, would mean a second
        // full read of every frame — 23 GB on a hundred 61 MP frames, for
        // numbers that were free a moment earlier.
        let (paths, levels) = warp_to_disk(engine, &runtime, &job, &included, temp.path())?;
        let warp_seconds = warp_started.elapsed().as_secs_f64();

        let normalization = normalization_from(&levels, reference);
        let set = FrameSet::open(&paths)?;
        let result = stack_normalized(&set, &runtime, options, None, Some(&normalization))?;
        let blocks = result.block_count;
        let rejected = result.rejected.total() as u64;
        (result.frame, blocks, rejected, warp_seconds)
    } else {
        let (warped, levels) = warp_in_memory(engine, &runtime, &job, &included)?;
        let warp_seconds = warp_started.elapsed().as_secs_f64();

        let normalization = normalization_from(&levels, reference);
        let loaded = LoadedFrames::new(&warped)?;
        let result = stack_normalized(&loaded, &runtime, options, None, Some(&normalization))?;
        let blocks = result.block_count;
        let rejected = result.rejected.total() as u64;
        (result.frame, blocks, rejected, warp_seconds)
    };
    let stack_seconds = warp_started.elapsed().as_secs_f64() - warp_seconds;

    let mut master = result;
    // The orientation step the Siril pipeline performs after stacking (P11-T4);
    // without it the two engines' masters differ by a flip.
    master.mirror_x();

    let exposure = total_exposure(&job.frames, &included);
    let cards: Vec<(&str, Value, Option<&str>)> = vec![
        (
            "STACKCNT",
            Value::Integer(included.len() as i64),
            Some("frames stacked"),
        ),
        (
            "LIVETIME",
            Value::Float(exposure),
            Some("total exposure, seconds"),
        ),
        (
            "HISTORY",
            Value::Commentary(format!(
                "px native registration + {} stack of {} frames",
                describe(&best),
                included.len()
            )),
            None,
        ),
    ];
    if let Some(parent) = job.output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    master.write(&job.output, &cards)?;
    let _ = &job.extension;

    Ok(StackOutcome {
        frame_count: included.len(),
        blocks,
        rejected,
        spilled: spill,
        method: describe(&best),
        warp_seconds,
        stack_seconds,
    })
}

/// Bytes a single in-flight warp holds, per pixel of the output.
///
/// Four buffers and two masks, and counting them honestly matters: an earlier
/// version charged only the source and the result — eight bytes — and a run
/// configured for 8 GB peaked at 19.9 GB of resident memory, because a clamped
/// Lanczos-4 warp also holds the area-interpolated guide and two byte masks, and
/// the level measurement holds two scratch buffers of its own.
///
/// | Buffer | Bytes per pixel |
/// |---|---|
/// | source frame (f32) | 4 |
/// | warped output (f32) | 4 |
/// | clamping guide (f32) | 4 |
/// | undershoot mask + its dilation (bool) | 2 |
/// | robust level scratch, subsampled (f32) | 2 |
const WARP_BYTES_PER_PIXEL: u64 = 16;

/// Sampling stride for the per-frame level measurement.
///
/// A quarter of a 61 MP channel is 15 million samples, which fixes a background
/// level far more precisely than the difference between two frames' skies, and
/// it cuts the estimator's scratch — the buffers that decide how many frames fit
/// in the budget at once — from eight bytes per pixel to two.
const LEVEL_STRIDE: usize = 4;

/// Share of the *whole* budget the concurrent warps may hold.
///
/// Of the whole budget rather than of the phase's share, because the two phases
/// overlap in the only sense that matters: the allocator does not hand the
/// warp's buffers back to the operating system when they are dropped, so they
/// are still resident while the stacker plans its bands. Measured on an 8 GB run
/// — warp phase peak 5.1 GB, stack phase peak 4.9 GB in isolation, and 8.9 GB
/// when the two ran back to back. A quarter for warping plus three fifths for
/// stacking leaves room for that overlap.
const WARP_BUDGET_SHARE: f64 = 0.25;

/// How many frames may be warped at once, from the memory budget.
///
/// The concurrency is worth having even though `warp` is already parallel
/// inside: a frame's cost is not all warp — it is a 122 MB read, a decode, the
/// warp, and a robust level measurement, and only the middle two keep every core
/// busy. Overlapping several frames fills the gaps, and measured on a hundred
/// 61 MP frames it is the difference between 169 s and 38 s.
fn warp_concurrency(engine: &NativeEngine, dims: px_imageproc::ImageDims) -> usize {
    let per_frame = dims.pixel_count() as u64 * WARP_BYTES_PER_PIXEL;
    let allowance = (engine.budget().max_bytes as f64 * WARP_BUDGET_SHARE) as u64;
    ((allowance / per_frame.max(1)) as usize).clamp(1, engine.budget().cores)
}

/// Warps every included frame, measuring its photometric level while it is still
/// in memory, and hands each result to `sink` in sequence order.
///
/// The two callers differ only in what they do with a warped frame — keep it or
/// write it — so the concurrency, the ordering and the level measurement live
/// here once.
fn warp_all<T: Send>(
    engine: &NativeEngine,
    runtime: &Runtime,
    job: &StackJob,
    included: &[usize],
    sink: impl Fn(usize, Frame) -> Result<T, PipelineError> + Sync,
) -> Result<(Vec<T>, Vec<Vec<ChannelLevel>>), PipelineError> {
    use rayon::prelude::*;

    let in_flight = warp_concurrency(engine, job.registration.geometry.dims);
    let positions: Vec<(usize, usize)> = included.iter().copied().enumerate().collect();

    let mut produced: Vec<(usize, T, Vec<ChannelLevel>)> = runtime.install(|| {
        positions
            .par_chunks(positions.len().div_ceil(in_flight).max(1))
            .flat_map_iter(|chunk| {
                chunk.iter().map(|&(position, index)| {
                    let frame = warp_one(runtime, job, index)?;
                    let levels = measure_levels(&frame);
                    Ok((position, sink(position, frame)?, levels))
                })
            })
            .collect::<Result<Vec<_>, PipelineError>>()
    })?;

    produced.sort_by_key(|(position, _, _)| *position);
    let mut outputs = Vec::with_capacity(produced.len());
    let mut levels = Vec::with_capacity(produced.len());
    for (_, output, level) in produced {
        outputs.push(output);
        levels.push(level);
    }
    Ok((outputs, levels))
}

/// Warps the included frames, keeping them in memory.
fn warp_in_memory(
    engine: &NativeEngine,
    runtime: &Runtime,
    job: &StackJob,
    included: &[usize],
) -> Result<(Vec<Frame>, Vec<Vec<ChannelLevel>>), PipelineError> {
    warp_all(engine, runtime, job, included, |_, frame| Ok(frame))
}

/// Warps the included frames into `dir`, returning their paths in order.
fn warp_to_disk(
    engine: &NativeEngine,
    runtime: &Runtime,
    job: &StackJob,
    included: &[usize],
    dir: &Path,
) -> Result<(Vec<PathBuf>, Vec<Vec<ChannelLevel>>), PipelineError> {
    warp_all(engine, runtime, job, included, |position, frame| {
        let path = dir.join(format!("r_{position:05}.fits"));
        frame.write(&path, &[])?;
        // Flushed as it is written rather than left to the kernel's own
        // schedule. Tens of gigabytes of dirty file pages are charged to this
        // process's resident set until they reach the disk, which shows up as a
        // run overrunning a budget it is in fact inside.
        if let Ok(file) = std::fs::File::open(&path) {
            let _ = file.sync_all();
        }
        Ok(path)
    })
}

/// The robust level and spread of every channel of one warped frame.
fn measure_levels(frame: &Frame) -> Vec<ChannelLevel> {
    (0..frame.channels())
        .map(|channel| ChannelLevel::measure_subsampled(frame.channel(channel), LEVEL_STRIDE))
        .collect()
}

fn warp_one(runtime: &Runtime, job: &StackJob, index: usize) -> Result<Frame, PipelineError> {
    let source = Frame::open(&job.frames[index])?;
    let transform = job.registration.output_transform(index).ok_or_else(|| {
        PipelineError::FileNotFound(format!("frame {index} has no registration transform"))
    })?;
    Ok(warp(
        &source,
        &transform,
        job.registration.geometry.dims,
        runtime,
        &WarpOptions::default(),
    )?)
}

/// Normalization coefficients from already-measured per-frame levels.
fn normalization_from(levels: &[Vec<ChannelLevel>], reference: usize) -> Normalization {
    let reference_levels = &levels[reference.min(levels.len() - 1)];
    let coefficients = levels
        .iter()
        .map(|frame| {
            frame
                .iter()
                .zip(reference_levels)
                .map(|(frame, reference)| {
                    px_imageproc::stack::normalize::coefficients(
                        NormMethod::AdditiveScaling,
                        *reference,
                        *frame,
                    )
                })
                .collect()
        })
        .collect();

    Normalization {
        coefficients,
        method: NormMethod::AdditiveScaling,
        reference,
    }
}

/// Total exposure over the stacked frames, from their `EXPTIME`/`EXPOSURE`
/// cards.
fn total_exposure(frames: &[PathBuf], included: &[usize]) -> f64 {
    included
        .iter()
        .filter_map(|&index| {
            let reader = px_fits::reader::FitsReader::open(&frames[index]).ok()?;
            let header = reader.primary_image().ok()?.header().clone();
            header.get_f64("EXPTIME").or(header.get_f64("EXPOSURE"))
        })
        .sum()
}

/// Siril's rejection choice, as this crate's rejection.
///
/// GESDT has no native implementation (ADR 007 P3-T5) and `BestRejection` never
/// selects it, so it falls back to Winsorized rather than failing a stack that
/// asked for something reasonable.
fn rejection_from(best: &BestRejection) -> Rejection {
    let bounds = Bounds {
        low: best.low_threshold,
        high: best.high_threshold,
    };
    match best.method {
        StackRejection::None => Rejection::None,
        StackRejection::Percentile => Rejection::Percentile(bounds),
        StackRejection::Sigma => Rejection::Sigma(bounds),
        StackRejection::Median => Rejection::SigmaMedian(bounds),
        StackRejection::Mad => Rejection::Mad(bounds),
        StackRejection::Winsorized | StackRejection::Generalized => Rejection::Winsorized(bounds),
        StackRejection::Linear => Rejection::LinearFit(bounds),
    }
}

fn describe(best: &BestRejection) -> String {
    format!(
        "{:?} {:.1}/{:.1}",
        best.method, best.low_threshold, best.high_threshold
    )
}
