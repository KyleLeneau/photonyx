use std::path::Path;

use anyhow::Result;
use px_cli::StackProjectArgs;
use px_configuration::ProjectLinearStack;
use px_conventions::observation::ObservationPath;
use px_fs::Glob;
use siril_sys::{
    BestRejection, Builder, FitsExt,
    commands::{Convert, Load, Register, SeqApplyReg, Stack},
    siril_ext::{CdExt, MirrorxExt, SaveExt},
};

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn stack_project_observations(
    args: StackProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    // Find the project dir and config to work with
    let (project_dir, config) = match super::find_and_load_project(args.project) {
        Ok(tuple) => tuple,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    printer.info(format!(
        "project_dir: {:?}, config: {:?}",
        project_dir.display(),
        config
    ))?;

    for stack in config.linear_stacks {
        let ext = FitsExt::FIT;
        let builder = Builder::default()
            .output_sink(siril_sys::OutputSink::Inherit)
            .use_extension(ext.clone());

        stack_linear(builder, stack, &project_dir, printer).await?;
        // utils::wait_for_confirm(printer).await;
    }

    Ok(ExitStatus::Success)
}

async fn stack_linear<'a>(
    siril_builder: Builder<'a>,
    stack: ProjectLinearStack,
    project_dir: &Path,
    printer: Printer,
) -> Result<()> {
    // Setup siril
    let ext = siril_builder.ext();
    let mut siril = siril_builder.build().await?;

    // manage the sequence
    let mut prefix = String::from("light_");

    // convert each input directory
    let mut start_idx = 1;
    for obs in stack.observations {
        let observation = ObservationPath::single(&obs.path)?;
        let count = observation.pp_path().count_by_ext(ext.to_string())?;
        siril.cd(observation.pp_path().to_path_buf()).await?;
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

    // Return to working directory
    siril.cd(siril.initial_directory()).await?;

    // TODO: Optional: run bg extraction on every frame before stacking
    // if extract_background:
    //     await siril.command(seqsubsky(prefix))
    //     prefix = f"bkg_{prefix}"

    // Register all the images
    siril
        .execute(&Register::builder(&prefix).two_pass(true).build())
        .await?;

    // Generate their transformed version
    siril
        .execute(&SeqApplyReg::builder(&prefix).build())
        .await?;
    prefix = format!("r_{prefix}");

    // Find the best rejection method
    let rejection = BestRejection::find(start_idx as usize);
    printer.info(format!("Found best stacking rejection: {:?}", rejection))?;

    // Stack the background extracted images
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
        .await?;

    // Load and flip the image if needed
    siril.execute(&Load::builder("result").build()).await?;
    siril.mirrorx(true).await?;
    siril.execute(&Load::builder("result").build()).await?;

    // TODO: output file naming and config for HDR
    // TODO: include date?
    // TODO: Save this output file name to the project config?

    // Output file for the linear_stack
    let filter_output_file = project_dir.join(format!("{}_linear_stack", stack.filter));
    siril.save(filter_output_file).await?;

    // TODO: Split and save RGB from OSC image

    printer.success(format!("{} linear stack complete", stack.filter))?;

    Ok(())
}
