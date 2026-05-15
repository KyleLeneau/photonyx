//! Pipeline for creating a linear stack (master light) from a set of calibrated light frames.
//! Outputs new linear stack to output folder.
//!

use std::path::{Path, PathBuf};

use px_fits::{LinearStackMetadata, MasterLight};
use px_fs::Glob;
use siril_sys::{
    BestRejection, Builder, FitsExt,
    commands::{Convert, Load, Register, SeqApplyReg, SeqSubSky, Stack},
    siril_ext::{CdExt, MirrorxExt, SaveExt},
};

use crate::{PipelineReporter, error::PipelineError};

#[derive(bon::Builder)]
pub struct CreateMasterLightPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub light_folders: Vec<PathBuf>,
    pub name: String,
    pub filter: Option<String>,
    #[builder(default = false)]
    pub background_extract: bool,
    pub out_folder: PathBuf,
}

impl CreateMasterLightPipeline {
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

        // Optional: run bg extraction on every frame before stacking
        if self.background_extract {
            siril.execute(&SeqSubSky::builder(&prefix).build()).await?;
            prefix = format!("bkg_{prefix}");
        }

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

        // Output file for the master_light
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

pub fn master_light_path(folder: &Path, name: &String) -> PathBuf {
    folder.join(format!("{}_LIGHT_master", name))
}

pub fn registered_master_light_path(folder: &Path, name: &String) -> PathBuf {
    folder.join(format!("r_{}_LIGHT_master", name))
}
