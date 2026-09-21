//! Execution in the native engine.

use px_fits::all_fits_files;
use px_fits::card::Value;
use px_imageproc::stack::{Bounds, Method, Rejection};

use super::CreateMasterFlatPipeline;
use crate::error::PipelineError;
use crate::meta::CalibrationMetadata;
use crate::model::MasterFlat;
use crate::native::{MasterStack, NativeRuntime, run_master_stack};
use crate::{PipelineReporter, RunIn};

/// Rejection used for flats, matching what the Siril pipeline asks for.
///
/// Flats are bright and uniform, so the outliers worth removing are dust
/// motes drifting between exposures and the occasional cosmic ray — few, and
/// far from the mean. Winsorized clipping handles that without the
/// sensitivity to a small frame count that plain sigma clipping has.
const FLAT_REJECTION: Rejection = Rejection::Winsorized(Bounds {
    low: 3.0,
    high: 3.0,
});

impl RunIn<NativeRuntime> for CreateMasterFlatPipeline {
    type Output = MasterFlat;

    async fn run(
        &self,
        reporter: impl PipelineReporter,
        env: NativeRuntime,
    ) -> Result<MasterFlat, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }
        if !self.bias.exists() {
            return Err(PipelineError::FileNotFound(format!(
                "master bias not found: {}",
                self.bias.display()
            )));
        }

        let engine = env.use_extension(self.ext.clone()).build()?;
        let name = CalibrationMetadata::from(raw_files.first().unwrap())?
            .master_flat_name(self.filter.clone());
        let output = engine.with_extension(&self.out_folder.join(name));

        let id = reporter.step_started("[1/1] Calibrating and stacking flat frames...");
        let job = MasterStack {
            frames: raw_files.clone(),
            method: Method::Mean(FLAT_REJECTION),
            // The bias comes off every flat as its block is read, so the
            // calibrated flats never reach the disk.
            subtract: Some(self.bias.clone()),
            output,
            cards: vec![
                (
                    "FILTER".to_string(),
                    Value::String(self.filter.clone()),
                    Some("filter this flat corrects".to_string()),
                ),
                (
                    "IMAGETYP".to_string(),
                    Value::String("FLAT".to_string()),
                    None,
                ),
            ],
        };

        let outcome = tokio::task::spawn_blocking(move || run_master_stack(&engine, job))
            .await
            .map_err(|e| PipelineError::FileNotFound(format!("stacking task failed: {e}")))?
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;
        reporter.step_ended(
            id,
            &format!(
                "[1/1] Stacked {} flat frames in {} blocks ({} values rejected)",
                outcome.frame_count, outcome.blocks, outcome.rejected
            ),
            true,
        );

        let meta = CalibrationMetadata::from(&outcome.path)?;
        Ok(MasterFlat {
            source: self.raw_folder.clone(),
            path: outcome.path,
            temperature: meta.temperature.unwrap_or_default(),
            gain: meta.gain.unwrap_or_default(),
            offset: meta.offset.unwrap_or_default(),
            filter: meta.filter.clone().unwrap_or(self.filter.clone()),
            binning: meta.binning,
            frame_count: outcome.frame_count,
            capture_date: meta.capture_date().expect("Missing capture date"),
        })
    }
}
