//! Pipeline for creating a master bias from an array of raw bias frames
//!

use std::path::PathBuf;

use px_fits::{CalibrationMetadata, MasterBias, all_fits_files};
use siril_sys::{
    Builder, FitsExt,
    commands::{Convert, Stack},
    siril_ext::CdExt,
};

use crate::error::PipelineError;

#[derive(bon::Builder)]
pub struct CreateMasterBiasPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub out_folder: PathBuf,
}

impl CreateMasterBiasPipeline {
    pub async fn run(&self) -> Result<MasterBias, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        // Setup the output file
        let name = CalibrationMetadata::from(raw_files.first().unwrap())?.master_bias_name();
        let output_file = self.out_folder.join(name).display().to_string();

        let mut siril = self
            .siril_builder
            .clone()
            .use_extension(self.ext.clone())
            .build()
            .await?;

        // Move to the raw folder to convert into a sequence
        siril.cd(&self.raw_folder).await?;
        siril
            .execute(
                &Convert::builder("bias_")
                    .output_dir(siril.initial_directory())
                    .build(),
            )
            .await?;

        // Return to working directory
        siril.cd(&siril.initial_directory()).await?;

        // Stack with defaults
        siril
            .execute(
                &Stack::builder("bias_")
                    .stack_type(siril_sys::StackType::Med)
                    .out(&output_file)
                    .build(),
            )
            .await?;

        // Confirm the output file exists now
        let result = PathBuf::from(output_file).with_added_extension(self.ext.to_string());
        if !result.exists() {
            return Err(PipelineError::OutputFileNotFound(format!(
                "Output file is missing: {:?}",
                result
            )));
        }

        let meta = CalibrationMetadata::from(&result)?;
        let bias = MasterBias {
            source: self.raw_folder.clone(),
            path: result,
            temperature: meta.temperature.unwrap_or_default(),
            gain: meta.gain.unwrap_or_default(),
            offset: meta.offset.unwrap_or_default(),
            binning: meta.binning,
            frame_count: raw_files.len(),
            capture_date: meta.capture_date().expect("Missing capture date"),
        };
        Ok(bias)
    }
}
