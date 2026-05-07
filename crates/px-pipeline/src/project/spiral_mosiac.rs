//! Pipeline for creating a linear stack (master light) from a set of calibrated light frames.
//! Uses a maximum framing and feathered overlap for spiral based mosiacs (SeeStar)
//! Outputs new linear stack to output folder.
//!

use std::path::PathBuf;

use px_fits::{LinearStackMetadata, MasterLight};
use px_fs::Glob;
use siril_sys::{
    BestRejection, Builder, FitsExt, SequenceFraming,
    commands::{Convert, Load, Platesolve, SeqApplyReg, Seqplatesolve, Stack},
    siril_ext::{CdExt, SaveExt},
};

use crate::{PipelineReporter, error::PipelineError, master_light::master_light_path};

#[derive(bon::Builder)]
pub struct SpiralMosiacPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub light_folders: Vec<PathBuf>,
    pub name: String,
    pub filter: Option<String>,
    pub feather_pixels: Option<f32>,
    pub out_folder: PathBuf,
}

impl SpiralMosiacPipeline {
    pub async fn run(&self, reporter: impl PipelineReporter) -> Result<MasterLight, PipelineError> {
        if self.light_folders.is_empty() {
            return Err(PipelineError::FileNotFound(
                "missing light_folders".to_string(),
            ));
        }

        // Setup siril
        let ext = self.siril_builder.ext();
        let mut siril = self.siril_builder.clone().build().await?;

        // manage the sequence
        let mut prefix = String::from("light_");

        // convert each input directory
        let id = reporter.step_started("[1/3] Converting light frames...");
        let mut start_idx = 1;
        for obs in self.light_folders.clone() {
            let count = obs.count_by_ext(ext.to_string())?;
            siril.cd(&obs).await?;
            siril
                .execute(
                    &Convert::builder(&prefix)
                        .output_dir(siril.initial_directory())
                        .start_index(start_idx)
                        .build(),
                )
                .await?;
            start_idx += count as u8;
        }
        reporter.step_ended(id, "[1/3] Converted light frames", true);

        // Return to working directory
        siril.cd(&siril.initial_directory()).await?;

        //
        // Seq Platsolve (disable near search, use as registration in)
        //
        // Sequence platsolve to get registration information
        let id = reporter.step_started("[2/4] Platesolving light frames...");
        siril
            .execute(&Seqplatesolve::builder(&prefix).radius(0.0).build())
            .await
            .inspect(|_| reporter.step_ended(id, "[2/4] Platesolved light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Platesolving failed", false))?;

        // Register all the images, Generate their transformed version
        let id = reporter.step_started("[3/4] Registering light frames...");
        siril
            .execute(
                &SeqApplyReg::builder(&prefix)
                    .framing(SequenceFraming::Max)
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[3/4] Registered maximum light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Registration failed", false))?;

        // new prefix
        prefix = format!("r_{prefix}");

        // Find the best rejection method
        let rejection = BestRejection::find(start_idx as usize);
        // printer.info(format!("Found best stacking rejection: {:?}", rejection))?;

        // Stack the background extracted images
        let id = reporter.step_started("[4/4] Stacking light frames...");
        siril
            .execute(
                &Stack::builder(prefix)
                    .norm(siril_sys::StackNormFlag::AddScale)
                    .filter_included(true)
                    .output_norm(true)
                    .rgb_equalization(true)
                    .maximize(true)
                    .maybe_feather_pixels(self.feather_pixels)
                    .rejection(rejection.method)
                    .lower_rej(rejection.low_threshold)
                    .higher_rej(rejection.high_threshold)
                    .out("result")
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[4/4] Stacked light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;

        // Load and platsolve the the image, flipping if needed
        siril.execute(&Load::builder("result").build()).await?;
        siril
            .execute(&Platesolve::builder().force(true).build())
            .await?;

        // Output file for the linear_stack
        let filter_output_file = master_light_path(&self.out_folder, &self.name);
        siril.save(filter_output_file.clone()).await?;

        // Load new fit file and get metadata
        let path = filter_output_file.with_added_extension(ext.to_string());
        let stack = LinearStackMetadata::from(path.clone())?;

        let master = MasterLight {
            sources: self.light_folders.clone(),
            path,
            exposure: stack.total_exposure.unwrap_or_default(),
            filter: stack.filter.or(self.filter.clone()),
            binning: stack.binning,
            frame_count: stack.frame_count as usize,
            target_name: stack.target_name,
            target_ra: stack.target_ra,
            target_dec: stack.target_dec,
        };

        Ok(master)
    }
}
