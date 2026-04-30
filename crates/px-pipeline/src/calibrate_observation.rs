//! Pipeline for calibrating an observation set using master frames.
//! Outputs all calibrated frames to output folder.
//!

use std::path::PathBuf;

use px_fits::{CalibratedLight, CalibrationMetadata, all_color_raw_frames, all_fits_files};
use px_fs::OptionPath;
use siril_sys::{
    Builder, ConversionFile, FitsExt,
    commands::{Calibrate, Convert},
    siril_ext::CdExt,
};

use crate::{PipelineReporter, error::PipelineError};

#[derive(bon::Builder)]
pub struct CalibrateObservationSetPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub raw_folder: PathBuf,
    pub bias: Option<PathBuf>,
    pub dark: Option<PathBuf>,
    pub flat: Option<PathBuf>,
    pub out_folder: PathBuf,
}

impl CalibrateObservationSetPipeline {
    pub async fn run(
        &self,
        reporter: impl PipelineReporter,
    ) -> Result<CalibratedLight, PipelineError> {
        let raw_files = all_fits_files(&self.raw_folder)?;
        if raw_files.is_empty() {
            return Err(PipelineError::FileNotFound(
                "raw_folder contains no files".to_string(),
            ));
        }

        let all_color = all_color_raw_frames(&raw_files)?;
        let mut siril = self
            .siril_builder
            .clone()
            .use_extension(self.ext.clone())
            .build()
            .await?;

        // Move to the raw folder to convert into a sequence
        let id = reporter.step_started("[1/3] Converting light frames...");
        siril.cd(&self.raw_folder).await?;
        siril
            .execute(
                &Convert::builder("light_")
                    .output_dir(siril.initial_directory())
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[1/3] Converted light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Convert failed", false))?;

        // Return to working directory
        siril.cd(&siril.initial_directory()).await?;

        // Calibrate the light frames using the master bias
        let id = reporter.step_started("[2/3] Calibrating light frames...");
        siril
            .execute(
                &Calibrate::builder("light_")
                    .maybe_bias(self.bias.some_string())
                    .maybe_dark(self.dark.some_string())
                    .maybe_flat(self.flat.some_string())
                    .cfa(all_color)
                    .debayer(all_color)
                    .equalize_cfa(all_color)
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[2/3] Calibrated light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Calibration failed", false))?;

        // Load the conversion file and move final files to output
        let id = reporter.step_started("[3/3] Moving calibrated light frames...");
        let conversion_file = siril.initial_directory().join("light_conversion.txt");
        let conversion = ConversionFile::new(conversion_file)?;
        match conversion.move_converted_files(&self.out_folder, "pp_") {
            Ok(_) => reporter.step_ended(id, "[3/3] Moved calibrated light frames", true),
            Err(_) => reporter.step_ended(id, "✗ Move failed", false),
        }

        // Try to get the filter from calibrated file OR flat
        let pp_files = all_fits_files(&self.out_folder)?;
        let first = pp_files.first().expect("missing first pp_ file");
        let pp_meta = CalibrationMetadata::from(first)?;

        let flat_filter = self
            .flat
            .as_deref()
            .and_then(|f| CalibrationMetadata::from(f).ok())
            .and_then(|m| m.filter);

        // Get one of the converted files to load metadata
        let light = CalibratedLight {
            source: self.raw_folder.clone(),
            path: self.out_folder.clone(),
            temperature: pp_meta.temperature.unwrap_or_default(),
            gain: pp_meta.gain.unwrap_or_default(),
            offset: pp_meta.offset.unwrap_or_default(),
            exposure: pp_meta.exposure.unwrap_or_default(),
            filter: pp_meta.filter.or(flat_filter).unwrap_or_default(),
            binning: pp_meta.binning,
        };

        Ok(light)
    }
}
