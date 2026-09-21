//! Execution in the native engine.

use px_fits::all_fits_files;
use px_imageproc::stack::Method;

use super::CreateMasterDarkPipeline;
use crate::error::PipelineError;
use crate::meta::CalibrationMetadata;
use crate::model::MasterDark;
use crate::native::{MasterStack, NativeRuntime, run_master_stack};
use crate::{PipelineReporter, RunIn};

impl RunIn<NativeRuntime> for CreateMasterDarkPipeline {
    type Output = MasterDark;

    async fn run(
        &self,
        reporter: impl PipelineReporter,
        env: NativeRuntime,
    ) -> Result<MasterDark, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        let engine = env.use_extension(self.ext.clone()).build()?;
        let name = CalibrationMetadata::from(raw_files.first().unwrap())?.master_dark_name();
        let output = engine.with_extension(&self.out_folder.join(name));

        let id = reporter.step_started("[1/1] Stacking dark frames...");
        let job = MasterStack {
            frames: raw_files.clone(),
            // A median needs no rejection. Darks are long exposures and do
            // collect cosmic rays, but a median over enough of them discards
            // those without any threshold to tune.
            method: Method::Median,
            subtract: None,
            output,
            cards: vec![(
                "IMAGETYP".to_string(),
                px_fits::card::Value::String("DARK".to_string()),
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
                "[1/1] Stacked {} dark frames in {} blocks",
                outcome.frame_count, outcome.blocks
            ),
            true,
        );

        let meta = CalibrationMetadata::from(&outcome.path)?;
        Ok(MasterDark {
            source: self.raw_folder.clone(),
            path: outcome.path,
            temperature: meta.temperature.unwrap_or_default(),
            gain: meta.gain.unwrap_or_default(),
            offset: meta.offset.unwrap_or_default(),
            exposure: meta.exposure.unwrap_or_default(),
            binning: meta.binning,
            frame_count: outcome.frame_count,
            capture_date: meta.capture_date().expect("Missing capture date"),
        })
    }
}
