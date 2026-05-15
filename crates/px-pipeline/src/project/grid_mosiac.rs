//! Pipeline for creating a linear stack (master light) from a set of calibrated light frames.
//! Uses a maximum framing and feathered overlap for spiral based mosiacs (SeeStar)
//! Outputs new linear stack to output folder.
//!

use std::path::PathBuf;

use px_fits::{LinearStackMetadata, MasterLight};
use siril_sys::{
    Builder, FitsExt, SequenceFraming,
    commands::{Convert, Load, Platesolve, SeqApplyReg, SeqSubSky, Seqplatesolve, Stack},
    siril_ext::SaveExt,
};

use crate::{PipelineReporter, error::PipelineError, master_light::master_light_path};

#[derive(bon::Builder)]
pub struct GridMosiacPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub tile_master_lights: Vec<PathBuf>,
    pub name: String,
    pub filter: Option<String>,
    pub feather_pixels: Option<f32>,
    #[builder(default = false)]
    pub background_extract: bool,
    pub out_folder: PathBuf,
}

impl GridMosiacPipeline {
    pub async fn run(&self, reporter: impl PipelineReporter) -> Result<MasterLight, PipelineError> {
        if self.tile_master_lights.is_empty() {
            return Err(PipelineError::FileNotFound(
                "missing tile_master_lights".to_string(),
            ));
        }

        // Setup siril
        let ext = self.siril_builder.ext();
        let mut siril = self.siril_builder.clone().build().await?;

        for input in &self.tile_master_lights {
            std::fs::copy(
                input,
                siril.initial_directory().join(input.file_name().unwrap()),
            )?;
        }

        // manage the sequence
        let mut prefix = String::from("mosiac_");

        // convert what's in temp directory
        siril.execute(&Convert::builder(&prefix).build()).await?;

        // Optional: run bg extraction on every frame before stacking
        if self.background_extract {
            siril.execute(&SeqSubSky::builder(&prefix).build()).await?;
            prefix = format!("bkg_{prefix}");
        }

        //
        // Seq Platsolve (use as registration in)
        //
        // Sequence platsolve to get registration information
        let id = reporter.step_started("[1/3] Platesolving tile light frames...");
        siril
            .execute(&Seqplatesolve::builder(&prefix).build())
            .await
            .inspect(|_| reporter.step_ended(id, "[1/3] Platesolved tile light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Platesolving failed", false))?;

        // Register all the images, Generate their transformed version
        let id = reporter.step_started("[2/3] Registering tile light frames...");
        siril
            .execute(
                &SeqApplyReg::builder(&prefix)
                    .framing(SequenceFraming::Max)
                    .build(),
            )
            .await
            .inspect(|_| {
                reporter.step_ended(id, "[2/3] Registered maximum tile light frames", true)
            })
            .inspect_err(|_| reporter.step_ended(id, "✗ Registration failed", false))?;

        // new prefix
        prefix = format!("r_{prefix}");

        // Stack the background extracted images
        let id = reporter.step_started("[3/3] Stacking tile light frames...");
        siril
            .execute(
                &Stack::builder(prefix)
                    .norm(siril_sys::StackNormFlag::AddScale)
                    .filter_included(true)
                    .output_norm(true)
                    .rgb_equalization(true)
                    .maximize(true)
                    // .maybe_feather_pixels(self.feather_pixels)
                    .rejection(siril_sys::StackRejection::None)
                    .normalize_overlap(true)
                    .out("result")
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[3/3] Stacked tile light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;

        // Load and platsolve the the image, flipping if needed
        siril.execute(&Load::builder("result").build()).await?;
        siril
            .execute(&Platesolve::builder().force(true).build())
            .await?;

        // Output file for the master_light
        let filter_output_file = master_light_path(&self.out_folder, &self.name);
        siril.save(filter_output_file.clone()).await?;

        // Load new fit file and get metadata
        let path = filter_output_file.with_added_extension(ext.to_string());
        let stack = LinearStackMetadata::from(path.clone())?;

        let master = MasterLight {
            sources: self.tile_master_lights.clone(),
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
