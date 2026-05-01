//! Pipeline for creating a linear stack (master light) from a set of calibrated light frames.
//! Outputs new linear stack to output folder.
//!

use std::path::PathBuf;

use px_fits::{Binning, MasterLight};
use px_fs::Glob;
use siril_sys::{
    BestRejection, Builder, FitsExt,
    commands::{Convert, Load, Register, SeqApplyReg, Stack},
    siril_ext::{CdExt, MirrorxExt, SaveExt},
};

use crate::{PipelineReporter, error::PipelineError};

#[derive(bon::Builder)]
pub struct CreateMasterLightPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub light_folders: Vec<PathBuf>,
    pub name: String,
    pub out_folder: PathBuf,
    #[builder(default = false)]
    pub extract_background: bool,
}

impl CreateMasterLightPipeline {
    pub async fn run(&self, reporter: impl PipelineReporter) -> Result<MasterLight, PipelineError> {
        // TODO: validate light_folder.len() > 0

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

        // TODO: Optional: run bg extraction on every frame before stacking
        // if extract_background:
        //     await siril.command(seqsubsky(prefix))
        //     prefix = f"bkg_{prefix}"

        // Register all the images
        let id = reporter.step_started("[2/3] Registering light frames...");
        siril
            .execute(&Register::builder(&prefix).two_pass(true).build())
            .await?;

        // Generate their transformed version
        siril
            .execute(&SeqApplyReg::builder(&prefix).build())
            .await?;

        reporter.step_ended(id, "[2/3] Registered light frames", true);
        prefix = format!("r_{prefix}");

        // Find the best rejection method
        let rejection = BestRejection::find(start_idx as usize);
        // printer.info(format!("Found best stacking rejection: {:?}", rejection))?;

        // Stack the background extracted images
        let id = reporter.step_started("[3/3] Stacking light frames...");
        siril
            .execute(
                &Stack::builder(prefix)
                    .norm(siril_sys::StackNormFlag::AddScale)
                    .filter_included(true)
                    .output_norm(true)
                    .rgb_equalization(true)
                    .rejection(rejection.method)
                    .lower_rej(rejection.low_threshold)
                    .higher_rej(rejection.high_threshold)
                    .out("result")
                    .build(),
            )
            .await
            .inspect(|_| reporter.step_ended(id, "[3/3] Stacked light frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Stacking failed", false))?;

        // Load and flip the image if needed
        siril.execute(&Load::builder("result").build()).await?;
        siril.mirrorx(true).await?;
        siril.execute(&Load::builder("result").build()).await?;

        // TODO: output file naming and config for HDR
        // TODO: include date?
        // TODO: Save this output file name to the project config?

        // Output file for the linear_stack
        let filter_output_file = self.out_folder.join(format!("{}_linear_stack", self.name));
        siril.save(filter_output_file.clone()).await?;

        // TODO: Split and save RGB from OSC image

        // TODO: Load new fit file and get metadata

        let master = MasterLight {
            sources: self.light_folders.clone(),
            path: filter_output_file.with_added_extension(ext.to_string()),
            exposure: 0.0,
            filter: "".to_string(),
            binning: Binning::default(),
            frame_count: 0,
            target_name: "".to_string(),
            target_ra: None,
            target_dec: None,
        };

        Ok(master)
    }
}
