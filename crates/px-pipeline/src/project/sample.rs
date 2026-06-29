//! Pipeline for producing color sample composites from registered per-filter master lights.
//!
//! Supports RGB, SHO, HOO, LRGB, L+SHO, and L+HOO compositions, plus a mono passthrough
//! when only one filter is available. Output goes to `{out_folder}/{mix_name}/sample_result.*`.
//!

use std::path::{Path, PathBuf};

use siril_sys::{
    Builder, FitsExt, RGBImage, Siril, SirilError,
    commands::{
        Autostretch, Load, Pcc, Rgbcomp, Rmgreen, Satu, Save, Savejpg, Savepng, Savetif, Unpurple,
    },
    siril_ext::LoadExt,
};

use crate::{PipelineReporter, error::PipelineError};

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
    Mono {
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
            ColorSample::Mono { name, .. } => name.clone(),
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

    if let (Some(red), Some(green), Some(blue)) = (&red, &green, &blue)
        && !excluded(ColorMixType::Rgb.dir_name())
    {
        if let Some(lum) = &luminance
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
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Rgb,
            luminance: None,
            red: red.clone(),
            green: green.clone(),
            blue: blue.clone(),
        });
    }

    if let (Some(sii), Some(ha), Some(oiii)) = (&sii, &ha, &oiii)
        && !excluded(ColorMixType::Sho.dir_name())
    {
        if let Some(lum) = &luminance
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
        samples.push(ColorSample::Composite {
            mix_type: ColorMixType::Sho,
            luminance: None,
            red: sii.clone(),
            green: ha.clone(),
            blue: oiii.clone(),
        });
    } else if let (Some(ha), Some(oiii)) = (&ha, &oiii)
        && !excluded(ColorMixType::Hoo.dir_name())
    {
        if let Some(lum) = &luminance
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
            samples.push(ColorSample::Mono {
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

        let compose_result = match &self.sample {
            ColorSample::Mono { image, .. } => siril.load_path(image.path.clone()).await,
            ColorSample::Composite {
                mix_type,
                luminance,
                red,
                green,
                blue,
            } => {
                compose(
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

        if let Err(err) = compose_result {
            reporter.step_ended(id, &format!("✗ [{label}] Composition failed"), false);
            return Err(PipelineError::SirilError(err));
        }

        siril
            .execute(
                &Autostretch::builder()
                    .shadows_clipping(-2.3_f32)
                    .target_background(0.15_f32)
                    .build(),
            )
            .await?;

        siril.execute(&Rmgreen::builder().build()).await?;

        if let ColorSample::Composite { mix_type, .. } = &self.sample
            && mix_type.use_purple_removal()
        {
            siril.execute(&Unpurple::builder().build()).await?;
        }

        siril
            .execute(&Satu::builder(0.6).background_factor(0.1).build())
            .await?;

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

async fn compose(
    siril: &mut Siril,
    mix_type: ColorMixType,
    enable_pcc: bool,
    luminance: Option<&FilteredStack>,
    red: &FilteredStack,
    green: &FilteredStack,
    blue: &FilteredStack,
) -> Result<(), SirilError> {
    let rgb = RGBImage::RGB(
        loadable(&red.path),
        loadable(&green.path),
        loadable(&blue.path),
    );

    siril
        .execute(
            &Rgbcomp::builder(rgb)
                .maybe_luminance(luminance.map(|l| loadable(&l.path)))
                .out("color_sample_composite")
                .build(),
        )
        .await?;

    siril
        .execute(&Load::builder("color_sample_composite").build())
        .await?;

    if enable_pcc && mix_type.is_true_color() {
        siril.execute(&Pcc::builder().build()).await?;
    }

    Ok(())
}

/// Returns the absolute path string with the file extension stripped, which is how Siril
/// expects image filenames when loading or referencing files in commands like `rgbcomp`.
fn loadable(path: &Path) -> String {
    path.with_extension("").display().to_string()
}
