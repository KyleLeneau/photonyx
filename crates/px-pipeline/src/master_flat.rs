//! Pipeline for creating a master bias from an array of raw bias frames
//!

use std::path::PathBuf;

use px_fits::{CalibrationMetadata, MasterFlat, all_fits_files};
use siril_sys::{
    Builder, FitsExt,
    commands::{Calibrate, Convert, Stack},
    siril_ext::CdExt,
};

use crate::{PipelineReporter, error::PipelineError};

#[derive(bon::Builder)]
pub struct CreateMasterFlatPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub bias: PathBuf,
    pub filter: String,
    pub out_folder: PathBuf,
}

impl CreateMasterFlatPipeline {
    pub async fn execute(
        &self,
        reporter: impl PipelineReporter,
    ) -> Result<MasterFlat, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        // Setup the output file
        let name = CalibrationMetadata::from(raw_files.first().unwrap())?
            .master_flat_name(self.filter.clone());
        let output_file = self.out_folder.join(name).display().to_string();

        let mut siril = self
            .siril_builder
            .clone()
            .use_extension(self.ext.clone())
            .build()
            .await?;

        // Move to the raw folder to convert into a sequence
        let id = reporter.step_started("[1/3] Converting flat frames...");
        siril.cd(&self.raw_folder).await?;
        siril
            .execute(
                &Convert::builder("flat_")
                    .output_dir(siril.initial_directory())
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[1/3] Converted flat frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Convert failed", false))?;

        // Return to working directory
        siril.cd(&siril.initial_directory()).await?;

        // Calibrate the flat frames using the master bias
        let id = reporter.step_started("[2/3] Calibrating flat frames...");
        siril
            .execute(
                &Calibrate::builder("flat_")
                    .bias(self.bias.display().to_string())
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[2/3] Calibrated flat frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Calibration failed", false))?;

        // Stack with defaults
        let id = reporter.step_started("[3/3] Stacking flat frames...");
        siril
            .execute(
                &Stack::builder("pp_flat_")
                    .stack_type(siril_sys::StackType::Rej)
                    .norm(siril_sys::StackNormFlag::NoNorm)
                    .out(&output_file)
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[3/3] Stacked flat frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;

        // Confirm the output file exists now
        let result = PathBuf::from(output_file).with_added_extension(self.ext.to_string());
        if !result.exists() {
            return Err(PipelineError::OutputFileNotFound(format!(
                "Output file is missing: {:?}",
                result
            )));
        }

        let meta = CalibrationMetadata::from(&result)?;
        let bias = MasterFlat {
            path: result,
            temperature: meta.temperature.unwrap_or_default(),
            gain: meta.gain.unwrap_or_default(),
            offset: meta.offset.unwrap_or_default(),
            filter: meta.filter.unwrap_or(self.filter.clone()),
            binning: meta.binning,
        };
        Ok(bias)
    }
}
