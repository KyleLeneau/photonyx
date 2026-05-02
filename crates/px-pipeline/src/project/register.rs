//! Pipeline for registering (align) a set linear stacks together.
//! Outputs new registered linear stacks to an output folder.
//!

use std::path::PathBuf;

use siril_sys::{
    Builder, ConversionFile, FitsExt, SequenceFraming,
    commands::{Convert, Register, SeqApplyReg},
};

use crate::{PipelineReporter, all_paths_exist, error::PipelineError};

#[derive(bon::Builder)]
pub struct RegisterMasterLightPipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub master_lights: Vec<PathBuf>,
    pub out_folder: PathBuf,
}

impl RegisterMasterLightPipeline {
    /// Registers linear stacks to their minimum frame overlap.
    ///
    pub async fn run_min_frame(
        &self,
        reporter: impl PipelineReporter,
    ) -> Result<(), PipelineError> {
        // Validate all the input folders exist
        all_paths_exist(self.master_lights.clone())?;

        // Setup siril
        let mut siril = self
            .siril_builder
            .clone()
            .use_extension(self.ext.clone())
            .build()
            .await?;

        for input in &self.master_lights {
            std::fs::copy(
                input,
                siril.initial_directory().join(input.file_name().unwrap()),
            )?;
        }

        // Manage the next prefix
        let prefix = "stack_";

        // convert what's in temp directory
        siril.execute(&Convert::builder(prefix).build()).await?;

        // Register all the images
        let id = reporter.step_started("[1/2] Registering stack frames...");
        siril
            .execute(&Register::builder(prefix).build())
            .await
            .inspect(|_| reporter.step_ended(id, "[1/2] Registered stack frames", true))
            .inspect_err(|_| reporter.step_ended(id, "✗ Registration failed", false))?;

        // Generate their transformed version
        siril
            .execute(
                &SeqApplyReg::builder(prefix)
                    .framing(SequenceFraming::Min)
                    .build(),
            )
            .await?;
        // let prefix = format!("r_{prefix}");

        // Move converted files
        let id = reporter.step_started("[2/2] Moving registered stack frames...");
        let conversion_file = siril.initial_directory().join("stack_conversion.txt");
        let conversion = ConversionFile::new(conversion_file)?;
        match conversion.move_converted_files(&self.out_folder, "r_") {
            Ok(_) => reporter.step_ended(id, "[2/2] Moved registered stack frames", true),
            Err(_) => reporter.step_ended(id, "✗ Move failed", false),
        }

        Ok(())
    }
}
