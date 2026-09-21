//! Execution in the native engine.

use px_fits::all_fits_files;
use px_imageproc::stack::Method;

use super::CreateMasterBiasPipeline;
use crate::error::PipelineError;
use crate::meta::CalibrationMetadata;
use crate::model::MasterBias;
use crate::native::{MasterStack, NativeRuntime, run_master_stack};
use crate::{PipelineReporter, RunIn};

impl RunIn<NativeRuntime> for CreateMasterBiasPipeline {
    type Output = MasterBias;

    async fn run(
        &self,
        reporter: impl PipelineReporter,
        env: NativeRuntime,
    ) -> Result<MasterBias, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        let engine = env.use_extension(self.ext.clone()).build()?;
        let name = CalibrationMetadata::from(raw_files.first().unwrap())?.master_bias_name();
        let output = engine.with_extension(&self.out_folder.join(name));

        let id = reporter.step_started("[1/1] Stacking bias frames...");
        let job = MasterStack {
            frames: raw_files.clone(),
            // A median needs no rejection: it is already immune to the
            // occasional cosmic ray, which is what makes it right for a
            // master built from short, featureless exposures.
            method: Method::Median,
            subtract: None,
            output,
            cards: vec![(
                "IMAGETYP".to_string(),
                px_fits::card::Value::String("BIAS".to_string()),
                None,
            )],
        };

        // Minutes of CPU-bound work on a real sequence; it must not run on an
        // async worker thread.
        let outcome = tokio::task::spawn_blocking(move || run_master_stack(&engine, job))
            .await
            .map_err(|e| PipelineError::FileNotFound(format!("stacking task failed: {e}")))?
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;
        reporter.step_ended(
            id,
            &format!(
                "[1/1] Stacked {} bias frames in {} blocks",
                outcome.frame_count, outcome.blocks
            ),
            true,
        );

        let meta = CalibrationMetadata::from(&outcome.path)?;
        Ok(MasterBias {
            source: self.raw_folder.clone(),
            path: outcome.path,
            temperature: meta.temperature.unwrap_or_default(),
            gain: meta.gain.unwrap_or_default(),
            offset: meta.offset.unwrap_or_default(),
            binning: meta.binning,
            frame_count: outcome.frame_count,
            capture_date: meta.capture_date().expect("Missing capture date"),
        })
    }
}
