//! Execution in the native engine.

use std::path::{Path, PathBuf};

use px_fits::card::Value;
use px_fits::{all_color_raw_frames, all_fits_files};
use px_imageproc::bayer::BayerPattern;
use px_imageproc::calibrate::{self, CalibrationOptions, CalibrationReport, Masters};
use px_imageproc::debayer::debayer_frame_with;
use px_imageproc::profile;
use px_imageproc::{Frame, bayer};

use super::CalibrateLightSetPipeline;
use crate::error::PipelineError;
use crate::meta::{CalibrationMetadata, ObservationMetadata};
use crate::model::CalibratedLight;
use crate::native::{NativeEngine, NativeRuntime};
use crate::{PipelineReporter, RunIn};

impl RunIn<NativeRuntime> for CalibrateLightSetPipeline {
    type Output = CalibratedLight;

    async fn run(
        &self,
        reporter: impl PipelineReporter,
        env: NativeRuntime,
    ) -> Result<CalibratedLight, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        let engine = env.use_extension(self.ext.clone()).build()?;
        let is_cfa = all_color_raw_frames(&raw_files)?;

        let id = reporter.step_started(&format!(
            "[1/1] Calibrating {} light frames...",
            raw_files.len()
        ));

        let job = Job {
            frames: raw_files.clone(),
            bias: self.bias.clone(),
            dark: self.dark.clone(),
            flat: self.flat.clone(),
            out_folder: self.out_folder.clone(),
            extension: self.ext.to_string(),
            is_cfa,
        };

        let outcome = tokio::task::spawn_blocking(move || run_calibration(&engine, job))
            .await
            .map_err(|e| PipelineError::FileNotFound(format!("calibration task failed: {e}")))?
            .inspect_err(|_| reporter.step_ended(id, "✗ Calibration failed", false))?;

        reporter.step_ended(
            id,
            &format!(
                "[1/1] Calibrated {} light frames ({} at a time){}",
                outcome.count,
                outcome.in_flight,
                if is_cfa { ", debayered" } else { "" }
            ),
            true,
        );

        // A frame whose sky came out below zero means the masters do not match
        // the lights — most often a master bias subtracted on top of a dark
        // that already contains it. Worth saying out loud: the frames are
        // still written, but they are not usable.
        if outcome.worst_negative_ratio > 0.5 {
            reporter.step_ended(
                reporter.step_started("⚠ Calibration warning"),
                &format!(
                    "{:.0}% of pixels came out negative — check that the masters match \
                     (a raw-stacked dark already contains the bias)",
                    outcome.worst_negative_ratio * 100.0
                ),
                false,
            );
        }

        let flat_filter = self
            .flat
            .as_deref()
            .and_then(|f| CalibrationMetadata::from(f).ok())
            .and_then(|m| m.filter);
        let pp_files = all_fits_files(&self.out_folder)?;
        let obs_meta = ObservationMetadata::from(pp_files)?;

        Ok(CalibratedLight {
            source: self.raw_folder.clone(),
            path: self.out_folder.clone(),
            temperature: obs_meta.temperature.unwrap_or_default(),
            gain: obs_meta.gain.unwrap_or_default(),
            offset: obs_meta.offset.unwrap_or_default(),
            exposure: obs_meta.exposure.unwrap_or_default(),
            filter: obs_meta.filter.clone().or(flat_filter).unwrap_or_default(),
            binning: obs_meta.binning,
            frame_count: obs_meta.frame_count,
            target_name: obs_meta.target_name.clone(),
            target_ra: obs_meta.target_ra,
            target_dec: obs_meta.target_dec,
            capture_date: obs_meta.capture_date().expect("Missing capture date"),
            site_lat: obs_meta.site_lat,
            site_long: obs_meta.site_long,
        })
    }
}

struct Job {
    frames: Vec<PathBuf>,
    bias: Option<PathBuf>,
    dark: Option<PathBuf>,
    flat: Option<PathBuf>,
    out_folder: PathBuf,
    extension: String,
    is_cfa: bool,
}

struct Outcome {
    count: usize,
    in_flight: usize,
    worst_negative_ratio: f32,
}

/// Calibrates every frame, synchronously.
///
/// The masters are loaded once and shared; the lights are processed several at
/// a time, with the concurrency taken from the memory budget rather than from
/// the core count. A colour frame needs its CFA buffer *and* three output
/// channels live at once — sixteen bytes per pixel, about 1 GB for a 61 MP
/// sensor — so the core count alone would be a poor guide.
fn run_calibration(engine: &NativeEngine, job: Job) -> Result<Outcome, PipelineError> {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    std::fs::create_dir_all(&job.out_folder)?;

    let bias = job.bias.as_deref().map(Frame::open).transpose()?;
    let dark = job.dark.as_deref().map(Frame::open).transpose()?;
    let mut flat = job.flat.as_deref().map(Frame::open).transpose()?;

    // The sensor's own colour response lives in the flat as well as the
    // optics'. Dividing by it unchanged would neutralize the colour balance of
    // every light, so the flat's channels are levelled first.
    let pattern = if job.is_cfa {
        let first = Frame::open(&job.frames[0])?;
        let pattern = bayer::detect(first.header());
        if let (Some(flat), true) = (flat.as_mut(), pattern != BayerPattern::None) {
            calibrate::equalize_cfa(flat, pattern)?;
        }
        pattern
    } else {
        BayerPattern::None
    };

    let options = CalibrationOptions {
        pattern,
        ..Default::default()
    };

    // Bytes one frame needs in flight: the frame itself, plus three channels
    // of output when it will be demosaiced.
    let sample = Frame::open(&job.frames[0])?;
    let per_pixel = if job.is_cfa { 4 + 12 } else { 4 };
    let frame_bytes = (sample.width() * sample.height() * per_pixel) as u64;
    let masters_bytes = [&bias, &dark, &flat]
        .iter()
        .filter_map(|m| m.as_ref())
        .map(|m| m.data().len() as u64 * 4)
        .sum::<u64>();
    let budget = engine.budget();
    let available = budget.max_bytes.saturating_sub(masters_bytes);
    let in_flight = px_imageproc::runtime::ResourceBudget::new(available, budget.cores)
        .max_frames_in_flight(frame_bytes);
    drop(sample);

    // Negative ratios are reported as parts per million so they can live in an
    // atomic without a lock.
    let worst_negative = AtomicU32::new(0);

    let results: Vec<Result<(), PipelineError>> = engine.runtime().install(|| {
        job.frames
            .par_chunks(job.frames.len().div_ceil(in_flight).max(1))
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .map(|path| {
                        let report = calibrate_one(
                            path,
                            &bias,
                            &dark,
                            &flat,
                            &options,
                            job.is_cfa,
                            engine,
                            &job.out_folder,
                            &job.extension,
                        )?;
                        let ppm = (report.negative_ratio * 1_000_000.0) as u32;
                        worst_negative.fetch_max(ppm, Ordering::Relaxed);
                        Ok(())
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    });

    for result in results {
        result?;
    }

    Ok(Outcome {
        count: job.frames.len(),
        in_flight,
        worst_negative_ratio: worst_negative.load(Ordering::Relaxed) as f32 / 1_000_000.0,
    })
}

#[allow(clippy::too_many_arguments)]
fn calibrate_one(
    path: &Path,
    bias: &Option<Frame>,
    dark: &Option<Frame>,
    flat: &Option<Frame>,
    options: &CalibrationOptions,
    is_cfa: bool,
    engine: &NativeEngine,
    out_folder: &Path,
    extension: &str,
) -> Result<CalibrationReport, PipelineError> {
    let mut frame = {
        let _span = profile::scope("calibrate.read");
        Frame::open(path)?
    };

    let masters = Masters {
        bias: bias.as_ref(),
        dark: dark.as_ref(),
        flat: flat.as_ref(),
        ..Default::default()
    };
    let report = {
        let _span = profile::scope("calibrate.apply");
        calibrate::calibrate(&mut frame, &masters, options)?
    };

    if is_cfa && options.pattern != BayerPattern::None {
        let _span = profile::scope("calibrate.debayer");
        debayer_frame_with(&mut frame, engine.debayer(), options.pattern)?;
    }

    // `pp_` is the prefix the Siril engine writes, and the observation index
    // and every downstream command look for it.
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "frame".to_string());
    let out = out_folder.join(format!("pp_{stem}.{extension}"));

    let _span = profile::scope("calibrate.write");
    frame.write(
        &out,
        &[(
            "HISTORY",
            Value::Commentary("px native calibration".to_string()),
            None,
        )],
    )?;
    Ok(report)
}
