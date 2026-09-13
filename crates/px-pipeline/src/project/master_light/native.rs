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

/// Two-pass registration over the collected frames.
fn run_registration(
    engine: &NativeEngine,
    frames: &[PathBuf],
) -> Result<Registration, PipelineError> {
    let set = FrameSet::open(frames)?;
    let options = RegistrationOptions {
        // The reference frame's own geometry, which is what Siril's default
        // `seqapplyreg` produces and what the project's downstream steps expect.
        framing: Framing::Current,
        ..Default::default()
    };
    Ok(register_sequence(&set, engine.runtime(), &options)?)
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

    let dims = job.registration.geometry.dims;
    let warped_bytes = dims.pixel_count() as u64 * 4;
    let budget = engine.budget();
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
        let (paths, levels) = warp_to_disk(engine, &job, &included, temp.path())?;
        let warp_seconds = warp_started.elapsed().as_secs_f64();

        let normalization = normalization_from(&levels, reference);
        let set = FrameSet::open(&paths)?;
        let result = stack_normalized(&set, engine.runtime(), options, None, Some(&normalization))?;
        let blocks = result.block_count;
        let rejected = result.rejected.total() as u64;
        (result.frame, blocks, rejected, warp_seconds)
    } else {
        let (warped, levels) = warp_in_memory(engine, &job, &included)?;
        let warp_seconds = warp_started.elapsed().as_secs_f64();

        let normalization = normalization_from(&levels, reference);
        let loaded = LoadedFrames::new(&warped)?;
        let result = stack_normalized(
            &loaded,
            engine.runtime(),
            options,
            None,
            Some(&normalization),
        )?;
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

/// How many frames may be warped at once, from the memory budget.
///
/// Each one in flight holds its source frame and its warped result at the same
/// time. The concurrency is worth having even though `warp` is already parallel
/// inside: a frame's cost is not all warp — it is a 122 MB read, a decode, the
/// warp, and a robust level measurement, and only the middle two keep every core
/// busy. Overlapping several frames fills the gaps, and measured on a hundred
/// 61 MP frames it is the difference between 169 s and 47 s.
fn warp_concurrency(engine: &NativeEngine, dims: px_imageproc::ImageDims) -> usize {
    let per_frame = dims.pixel_count() as u64 * 4 * 2;
    engine
        .budget()
        .max_frames_in_flight(per_frame)
        .clamp(1, engine.budget().cores)
}

/// Warps every included frame, measuring its photometric level while it is still
/// in memory, and hands each result to `sink` in sequence order.
///
/// The two callers differ only in what they do with a warped frame — keep it or
/// write it — so the concurrency, the ordering and the level measurement live
/// here once.
fn warp_all<T: Send>(
    engine: &NativeEngine,
    job: &StackJob,
    included: &[usize],
    sink: impl Fn(usize, Frame) -> Result<T, PipelineError> + Sync,
) -> Result<(Vec<T>, Vec<Vec<ChannelLevel>>), PipelineError> {
    use rayon::prelude::*;

    let in_flight = warp_concurrency(engine, job.registration.geometry.dims);
    let positions: Vec<(usize, usize)> = included.iter().copied().enumerate().collect();

    let mut produced: Vec<(usize, T, Vec<ChannelLevel>)> = engine.runtime().install(|| {
        positions
            .par_chunks(positions.len().div_ceil(in_flight).max(1))
            .flat_map_iter(|chunk| {
                chunk.iter().map(|&(position, index)| {
                    let frame = warp_one(engine, job, index)?;
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
    job: &StackJob,
    included: &[usize],
) -> Result<(Vec<Frame>, Vec<Vec<ChannelLevel>>), PipelineError> {
    warp_all(engine, job, included, |_, frame| Ok(frame))
}

/// Warps the included frames into `dir`, returning their paths in order.
fn warp_to_disk(
    engine: &NativeEngine,
    job: &StackJob,
    included: &[usize],
    dir: &Path,
) -> Result<(Vec<PathBuf>, Vec<Vec<ChannelLevel>>), PipelineError> {
    warp_all(engine, job, included, |position, frame| {
        let path = dir.join(format!("r_{position:05}.fits"));
        frame.write(&path, &[])?;
        Ok(path)
    })
}

/// The robust level and spread of every channel of one warped frame.
fn measure_levels(frame: &Frame) -> Vec<ChannelLevel> {
    (0..frame.channels())
        .map(|channel| {
            ChannelLevel::measure(frame.channel(channel), frame.width(), frame.height(), false)
        })
        .collect()
}

fn warp_one(engine: &NativeEngine, job: &StackJob, index: usize) -> Result<Frame, PipelineError> {
    let source = Frame::open(&job.frames[index])?;
    let transform = job.registration.output_transform(index).ok_or_else(|| {
        PipelineError::FileNotFound(format!("frame {index} has no registration transform"))
    })?;
    Ok(warp(
        &source,
        &transform,
        job.registration.geometry.dims,
        engine.runtime(),
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
