//! Pipeline for producing color sample composites from registered per-filter master lights.
//!
//! Supports RGB, SHO, HOO, LRGB, L+SHO, and L+HOO compositions, plus a mono passthrough
//! when only one filter is available. Output goes to `{out_folder}/{mix_name}/sample_result.*`.
//!

use std::path::{Path, PathBuf};

use siril_sys::{
    Builder, FitsExt, RGBImage, Siril, SirilError,
    commands::{
        Autostretch, Load, Pcc, Platesolve, Rgbcomp, Rmgreen, Satu, Save, Savejpg, Savepng,
        Savetif, Unpurple,
    },
    siril_ext::LoadExt,
};

use crate::{PipelineReporter, error::PipelineError};

// Constants used for scaled stretches
const DEFAULT_BOOST_LEVEL: usize = 1;
const SATURATION_LEVELS: [f64; 3] = [-0.4, 0.6, 1.0];
#[allow(unused)]
const USE_HUMAN_WEIGHTING: [bool; 3] = [true, true, false];
#[allow(unused)]
const ASINH_LEVELS: [f32; 3] = [200.0, 120.0, 150.0];
const STRETCH_LEVELS: [f32; 3] = [-10.0, -2.3, -2.0];

/// A single per-filter stack output from a project's framing config.
#[derive(Debug, Clone)]
pub struct FilteredStack {
    pub name: String,
    pub filter: Option<String>,
    /// Full absolute path to the master light FITS file (with extension).
    pub path: PathBuf,
}

/// The kind of color composition a [`ColorSample`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMixType {
    Rgb,
    Sho,
    Hoo,
    Lrgb,
    LSho,
    LHoo,
}

impl ColorMixType {
    pub fn dir_name(&self) -> &'static str {
        match self {
            ColorMixType::Rgb => "RGB",
            ColorMixType::Sho => "SHO",
            ColorMixType::Hoo => "HOO",
            ColorMixType::Lrgb => "LRGB",
            ColorMixType::LSho => "L-SHO",
            ColorMixType::LHoo => "L-HOO",
        }
    }

    fn is_true_color(self) -> bool {
        matches!(self, ColorMixType::Rgb | ColorMixType::Lrgb)
    }

    fn use_purple_removal(self) -> bool {
        matches!(self, ColorMixType::Sho | ColorMixType::LSho)
    }
}

/// A single sample to produce: a multi-filter composite, or a mono passthrough when no
/// composite mix can be formed from the available filters.
#[derive(Debug, Clone)]
pub enum ColorSample {
    Single {
        name: String,
        image: FilteredStack,
    },
    Composite {
        mix_type: ColorMixType,
        luminance: Option<FilteredStack>,
        red: FilteredStack,
        green: FilteredStack,
        blue: FilteredStack,
    },
}

impl ColorSample {
    pub fn dir_name(&self) -> String {
        match self {
            ColorSample::Single { name, .. } => name.clone(),
            ColorSample::Composite { mix_type, .. } => mix_type.dir_name().to_string(),
        }
    }
}

/// Detects every possible color composition that can be formed from `stacks`.
///
/// Filter name matching is case-insensitive and covers common broadband and narrowband
/// abbreviations. Mixes whose [`ColorMixType::dir_name`] matches an entry in `exclude`
/// (case-insensitive) are skipped. Falls back to one [`ColorSample::Mono`] per stack when
/// no composite mix can be formed.
pub fn detect_color_mixes(stacks: &[FilteredStack], exclude: &[String]) -> Vec<ColorSample> {
    let excluded = |name: &str| exclude.iter().any(|e| e.eq_ignore_ascii_case(name));
    let find = |names: &[&str]| -> Option<FilteredStack> {
        stacks.iter().find(|s| matches_filter(s, names)).cloned()
    };

    let luminance = find(&["L", "Luminance"]);
    let red = find(&["R", "Red"]);
    let green = find(&["G", "Green"]);
    let blue = find(&["B", "Blue"]);
    let sii = find(&["S", "S2", "S-2", "SII", "S-II", "S-Two"]);
    let ha = find(&["H", "Ha", "H-Alpha", "HAlpha"]);
    let oiii = find(&["O", "O3", "O-3", "OIII", "O-III", "O-Three"]);

    let mut samples: Vec<ColorSample> = Vec::new();

    // Add LRGB
    if let (Some(lum), Some(red), Some(green), Some(blue)) = (&luminance, &red, &green, &blue)
        && !excluded(ColorMixType::Lrgb.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Lrgb,
            luminance: Some(lum.clone()),
            red: red.clone(),
            green: green.clone(),
            blue: blue.clone(),
        });
    }

    // Add RGB
    if let (Some(red), Some(green), Some(blue)) = (&red, &green, &blue)
        && !excluded(ColorMixType::Rgb.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Rgb,
            luminance: None,
            red: red.clone(),
            green: green.clone(),
            blue: blue.clone(),
        });
    }

    // Add L-SHO
    if let (Some(lum), Some(sii), Some(ha), Some(oiii)) = (&luminance, &sii, &ha, &oiii)
        && !excluded(ColorMixType::LSho.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::LSho,
            luminance: Some(lum.clone()),
            red: sii.clone(),
            green: ha.clone(),
            blue: oiii.clone(),
        });
    }

    // Add SHO
    if let (Some(sii), Some(ha), Some(oiii)) = (&sii, &ha, &oiii)
        && !excluded(ColorMixType::Sho.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Sho,
            luminance: None,
            red: sii.clone(),
            green: ha.clone(),
            blue: oiii.clone(),
        });
    }

    // Add L-HOO
    if let (Some(lum), Some(ha), Some(oiii)) = (&luminance, &ha, &oiii)
        && !excluded(ColorMixType::LHoo.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::LHoo,
            luminance: Some(lum.clone()),
            red: ha.clone(),
            green: oiii.clone(),
            blue: oiii.clone(),
        });
    }

    // Add HOO
    if let (Some(ha), Some(oiii)) = (&ha, &oiii)
        && !excluded(ColorMixType::Hoo.dir_name())
    {
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Hoo,
            luminance: None,
            red: ha.clone(),
            green: oiii.clone(),
            blue: oiii.clone(),
        });
    }

    if samples.is_empty() {
        for stack in stacks {
            samples.push(ColorSample::Single {
                name: stack.name.clone(),
                image: stack.clone(),
            });
        }
    }

    samples
}

fn matches_filter(stack: &FilteredStack, names: &[&str]) -> bool {
    stack
        .filter
        .as_deref()
        .map(|f| names.iter().any(|n| f.eq_ignore_ascii_case(n)))
        .unwrap_or(false)
}

/// Which file formats [`CreateColorSamplePipeline`] should write for each sample.
#[derive(Debug, Clone, Copy, Default)]
pub struct SampleOutputFormats {
    pub fit: bool,
    pub tiff: bool,
    pub png: bool,
    pub jpg: bool,
}

/// Composes, stretches, and saves a single [`ColorSample`].
///
/// Siril is launched in a temporary scratch directory; input stacks are referenced by
/// absolute path so no files are copied. Outputs are written to `{out_folder}/{mix_name}/`.
#[derive(bon::Builder)]
pub struct CreateColorSamplePipeline {
    pub siril_builder: Builder,
    pub ext: FitsExt,
    pub sample: ColorSample,
    #[builder(default = false)]
    pub enable_pcc: bool,
    pub output_formats: SampleOutputFormats,
    /// Parent directory for sample outputs. Typically `{project_root}/samples`.
    pub out_folder: PathBuf,
}

impl CreateColorSamplePipeline {
    pub async fn run(&self, reporter: impl PipelineReporter) -> Result<PathBuf, PipelineError> {
        let ext = self.siril_builder.ext();
        let mut siril = self.siril_builder.clone().build().await?;

        let label = self.sample.dir_name();
        let id = reporter.step_started(&format!("[{label}] Composing sample..."));

        let work_sample = copy_to_workdir(&self.sample, &siril.initial_directory())?;
        let compose_result = match &work_sample {
            ColorSample::Single { image, .. } => {
                siril.load_path(image.path.clone()).await.map(|_| true)
            }
            ColorSample::Composite {
                mix_type,
                luminance,
                red,
                green,
                blue,
            } => {
                compose_rgb(
                    &mut siril,
                    *mix_type,
                    self.enable_pcc,
                    luminance.as_ref(),
                    red,
                    green,
                    blue,
                )
                .await
            }
        };

        // Need to track if we pre-stretch along the way
        let pre_stretched: bool;
        if let Err(err) = compose_result {
            reporter.step_ended(id, &format!("✗ [{label}] Composition failed"), false);
            return Err(PipelineError::SirilError(err));
        } else {
            pre_stretched = compose_result.unwrap();
        }

        // # Photometric color calibration and plate solving, will work with no parameter only if the image header
        // # contains correct target coordinates, pixel size and focal length, or if it has already been plate solved
        let mut used_pcc: bool = false;
        if let ColorSample::Composite { mix_type, .. } = &work_sample
            && self.enable_pcc
            && mix_type.is_true_color()
            && !pre_stretched
        {
            let pcc_result = try_pcc(&mut siril).await;
            used_pcc = pcc_result.is_ok();
        }

        // # pre-stretch non-lrgb
        // # if not pre_stretched:
        // #   await siril.command(asinh(asinh_levels[boost_force], human_weighting=use_human_weighting[boost_force]))

        // Auto stretch with boost levels the final images
        siril
            .execute(
                &Autostretch::builder()
                    .linked(used_pcc)
                    .shadows_clipping(STRETCH_LEVELS[DEFAULT_BOOST_LEVEL])
                    .target_background(0.15_f32)
                    .build(),
            )
            .await?;

        let _ = siril.execute(&Rmgreen::builder().build()).await;
        if let ColorSample::Composite { mix_type, .. } = &work_sample
            && mix_type.use_purple_removal()
        {
            siril.execute(&Unpurple::builder().build()).await?;
        }

        let _ = siril
            .execute(
                &Satu::builder(SATURATION_LEVELS[DEFAULT_BOOST_LEVEL])
                    .background_factor(0.15)
                    .build(),
            )
            .await;

        let out_dir = self.out_folder.join(label.clone());
        std::fs::create_dir_all(&out_dir)?;
        let result_base = out_dir.join("sample_result").display().to_string();

        if self.output_formats.fit {
            siril
                .execute(&Save::builder(result_base.clone()).build())
                .await?;
        }
        if self.output_formats.tiff {
            siril
                .execute(&Savetif::builder(result_base.clone()).build())
                .await?;
        }
        if self.output_formats.png {
            siril
                .execute(&Savepng::builder(result_base.clone()).build())
                .await?;
        }
        if self.output_formats.jpg {
            siril
                .execute(&Savejpg::builder(result_base.clone()).build())
                .await?;
        }

        reporter.step_ended(id, &format!("[{label}] Sample completed"), true);

        let _ = ext; // present for future use (e.g. returning the .fit path)
        Ok(out_dir)
    }
}

/// Copies every input FITS file referenced by `sample` into `work_dir` and returns a new
/// `ColorSample` whose paths point to those copies. This ensures Siril operates on copies
/// rather than the originals, preventing mutation of source master lights.
fn copy_to_workdir(sample: &ColorSample, work_dir: &Path) -> Result<ColorSample, std::io::Error> {
    let copy_stack = |stack: &FilteredStack| -> Result<FilteredStack, std::io::Error> {
        let dest = work_dir.join(stack.path.file_name().unwrap());
        std::fs::copy(&stack.path, &dest)?;
        Ok(FilteredStack {
            path: dest,
            ..stack.clone()
        })
    };

    Ok(match sample {
        ColorSample::Single { name, image } => ColorSample::Single {
            name: name.clone(),
            image: copy_stack(image)?,
        },
        ColorSample::Composite {
            mix_type,
            luminance,
            red,
            green,
            blue,
        } => ColorSample::Composite {
            mix_type: *mix_type,
            luminance: luminance.as_ref().map(copy_stack).transpose()?,
            red: copy_stack(red)?,
            green: copy_stack(green)?,
            blue: copy_stack(blue)?,
        },
    })
}

/// returns true if pre-stretched in this function; this means that if PCC is enabled, it will have been called in here
async fn compose_rgb(
    siril: &mut Siril,
    mix_type: ColorMixType,
    enable_pcc: bool,
    luminance: Option<&FilteredStack>,
    red: &FilteredStack,
    green: &FilteredStack,
    blue: &FilteredStack,
) -> Result<bool, SirilError> {
    let mut pre_stretched: bool = false;
    let rgb = RGBImage::RGB(
        loadable(&red.path),
        loadable(&green.path),
        loadable(&blue.path),
    );

    if let Some(lum) = luminance {
        // Case 2: 4 images in input to create a simple LRGB image

        if mix_type.is_true_color() && enable_pcc {
            // we run pcc before stretching: we need to create an RGB image from the 3 colors only to calibrate it,
            // save that as a color image, then do the LRGB composition using the luminance and this color image
            siril
                .execute(
                    &Rgbcomp::builder(rgb)
                        .out("temporary_rgb_composition")
                        .build(),
                )
                .await?;

            siril
                .execute(&Load::builder("temporary_rgb_composition").build())
                .await?;

            let pcc_result = try_pcc(siril).await;
            if pcc_result.is_err() {
                // PCC can fail for many reasons, like not having installed the NOMAD directory locally or being
                // able to access the VizieR Web service, or being able to plate solve the image
                // In that case, we fall back to the non-PCC LRGB composition
                non_pcc_lrgb_compose(siril, lum, red, green, blue).await?;
            } else {
                // pcc success
                siril
                    .execute(
                        &Autostretch::builder()
                            .shadows_clipping(-4.5_f32)
                            .target_background(0.15_f32)
                            .build(),
                    )
                    .await?;
                siril
                    .execute(&Save::builder("temporary_rgb_composition").build())
                    .await?;

                let combined = RGBImage::Single("temporary_rgb_composition".to_string());
                siril
                    .execute(
                        &Rgbcomp::builder(combined)
                            .luminance(loadable(&lum.path))
                            .out("color_sample_composite")
                            .build(),
                    )
                    .await?;
            }

            pre_stretched = true;
        } else {
            non_pcc_lrgb_compose(siril, luminance.unwrap(), red, green, blue).await?;
            pre_stretched = true;
        }
    } else {
        // Case 1: 3 images in input to create a simple RGB image (or any non-mixed channel colors)
        siril
            .execute(&Rgbcomp::builder(rgb).out("color_sample_composite").build())
            .await?;
    }

    // Load a final result before exiting function
    siril
        .execute(&Load::builder("color_sample_composite").build())
        .await?;
    Ok(pre_stretched)
}

async fn non_pcc_lrgb_compose(
    siril: &mut Siril,
    luminance: &FilteredStack,
    red: &FilteredStack,
    green: &FilteredStack,
    blue: &FilteredStack,
) -> Result<(), SirilError> {
    // assembling luminance and other images as RGB, would work for non-PCC LRGB but also for
    // L, S-II, H-Alpha, O-III (hubble palette with luminance)

    // as we are pre-stretching images as required by the luminance-based composition, we must ensure we
    // don't stretch the same image twice, for example in case of L-HOO combination

    // Load, stretch and save each image
    let files: [String; 4] = [
        loadable(&luminance.path),
        loadable(&red.path),
        loadable(&green.path),
        loadable(&blue.path),
    ];

    for image in files {
        siril.execute(&Load::builder(image.clone()).build()).await?;
        siril
            .execute(
                &Autostretch::builder()
                    .shadows_clipping(-4.5_f32)
                    .target_background(0.15_f32)
                    .build(),
            )
            .await?;
        siril.execute(&Save::builder(image).build()).await?;
    }

    let rgb = RGBImage::RGB(
        loadable(&red.path),
        loadable(&green.path),
        loadable(&blue.path),
    );

    siril
        .execute(
            &Rgbcomp::builder(rgb)
                .luminance(loadable(&luminance.path))
                .out("color_sample_composite")
                .build(),
        )
        .await?;
    Ok(())
}

/// Tries to run pcc on loaded image, if it failes then tries to platsolve and try again
async fn try_pcc(siril: &mut Siril) -> Result<(), SirilError> {
    let pcc_result = siril.execute(&Pcc::builder().build()).await;
    if pcc_result.is_err() {
        let solve_result = siril
            .execute(&Platesolve::builder().force(true).build())
            .await;
        return match solve_result {
            Ok(_) => siril.execute(&Pcc::builder().build()).await.map(|_| ()),
            Err(e) => Err(e),
        };
    }
    Ok(())
}

/// Returns the absolute path string with the file extension stripped, which is how Siril
/// expects image filenames when loading or referencing files in commands like `rgbcomp`.
fn loadable(path: &Path) -> String {
    path.with_extension("").display().to_string()
}
