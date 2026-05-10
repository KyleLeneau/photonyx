#![allow(clippy::doc_lazy_continuation)]

use bon::Builder;

use crate::{
    SequenceFilter, StackNormFlag, StackRejection, StackRejectionMapFlag, StackType,
    StackWeightingFlag,
    commands::{Argument, Command},
};

/// ```text
/// stack seqfilename
/// stack seqfilename { sum | min | max } [-output_norm] [-out=filename] [-maximize] [-upscale] [-32b]
/// stack seqfilename { med | median } [-nonorm, -norm=] [-fastnorm] [-rgb_equal] [-output_norm] [-out=filename] [-32b]
/// stack seqfilename { rej | mean } [rejection type] [sigma_low sigma_high]  [-rejmap[s]] [-nonorm, -norm=] [-fastnorm] [-overlap_norm] [-weight={noise|wfwhm|nbstars|nbstack}] [-feather=] [-rgb_equal] [-output_norm] [-out=filename] [-maximize] [-upscale] [-32b]
/// ```
///
/// Stacks the **sequencename** sequence, using options.
///
/// Rejection type:
/// The allowed types are: **sum**, **max**, **min**, **med** (or **median**) and **rej** (or **mean**). If no argument other than the sequence name is provided, sum stacking is assumed.
///
/// Stacking with rejection:
/// Types **rej** or **mean** require the use of additional arguments for rejection type and values. The rejection type is one of **n[one], p[ercentile], s[igma], m[edian], w[insorized], l[inear], g[eneralized], [m]a[d]** for Percentile, Sigma, Median, Winsorized, Linear-Fit, Generalized Extreme Studentized Deviate Test or k-MAD clipping. If omitted, the default Winsorized is used.
/// The **sigma low** and **sigma high** parameters of rejection are mandatory unless **none** is selected.
/// Optionally, rejection maps can be created, showing where pixels were rejected in one (**-rejmap**) or two (**-rejmaps**, for low and high rejections) newly created images.
///
/// Normalization of input images:
/// For **med** (or **median**) and **rej** (or **mean**) stacking types, different types of normalization are allowed: **-norm=add** for additive, **-norm=mul** for multiplicative. Options **-norm=addscale** and **-norm=mulscale** apply same normalization but with scale operations. **-nonorm** is the option to disable normalization. Otherwise addtive with scale method is applied by default.
/// **-fastnorm** option specifies to use faster estimators for location and scale than the default IKSS.
/// **-overlap_norm**, if passed, will compute normalization coeffcients on images overlaps instead of whole images (allowed only if **-maximize** is passed).
///
/// Other options for rejection stacking:
/// Weighting can be applied to the images of the sequences using the option **-weight=** followed by:
/// **noise** to add larger weights to frames with lower background noise.
/// **nbstack** to weight input images based on how many images were used to create them, useful for live stacking.
/// **nbstars** or **wfwhm** to weight input images based on number of stars or wFWHM computed during registration step.
/// **-feather=** option will apply a feathering mask on each image borders over the distance (in pixels) given in argument.
///
/// Outputs:
/// Result image name can be set with the **-out=** option. Otherwise, it will be named as **sequencename**\ \_stacked.fit.
/// **-output_norm** applies a normalization to rescale result in the [0, 1] range (median and mean stacking only).
/// **-maximize** option will use registration data from the sequence to create a stacked image that encompasses all the images of the sequence (applicable to all methods except median stacking).
/// **-upscale** option will upscale the sequence by a factor 2 prior to stacking using the registration data (applicable to all methods except median stacking).
/// **-rgb_equal** will use normalization to equalize color image backgrounds, useful if PCC/SPCC or unlinked AUTOSTRETCH will not be used.
/// **-32b** will override the bitdepth set in Preferences and save the stacked image in 32b.
///
///
/// Filtering out images:
/// Images to be stacked can be selected based on some filters, like manual selection or best FWHM, with some of the **-filter-\*** options.
///
///
/// Links: :ref:`pcc <pcc>`, :ref:`spcc <spcc>`, :ref:`autostretch <autostretch>`
///
/// ```text
/// [-filter-fwhm=value[%|k]] [-filter-wfwhm=value[%|k]] [-filter-round=value[%|k]] [-filter-bkg=value[%|k]]
/// [-filter-nbstars=value[%|k]] [-filter-quality=value[%|k]] [-filter-incl[uded]]
/// ```
/// Best images from the sequence can be stacked by using the filtering arguments. Each of these arguments can remove bad images based on a property their name contains, taken from the registration data, with either of the three types of argument values:
/// - a numeric value for the worse image to keep depending on the type of data used (between 0 and 1 for roundness and quality, absolute values otherwise),
/// - a percentage of best images to keep if the number is followed by a % sign,
/// - or a k value for the k.sigma of the worse image to keep if the number is followed by a k sign.
/// It is also possible to use manually selected images, either previously from the GUI or with the select or unselect commands, using the **-filter-included** argument.
///
#[derive(Builder)]
pub struct Stack {
    #[builder(start_fn, into)]
    base_name: String,

    #[builder(default = StackType::Rej)]
    stack_type: StackType,

    #[builder(default = StackNormFlag::NoNorm)]
    norm: StackNormFlag,

    #[builder(default = StackRejection::Winsorized)]
    rejection: StackRejection,

    #[builder(default = 3.0)]
    lower_rej: f32,

    #[builder(default = 3.0)]
    higher_rej: f32,

    create_rejection_maps: Option<StackRejectionMapFlag>,

    filters: Option<Vec<SequenceFilter>>,

    #[builder(default = false)]
    filter_included: bool,

    #[builder(default = false)]
    fast_norm: bool,

    #[builder(default = false)]
    output_norm: bool,

    weighting: Option<StackWeightingFlag>,

    #[builder(default = false)]
    rgb_equalization: bool,

    #[builder(default = false)]
    maximize: bool,

    feather_pixels: Option<f32>,

    #[builder(default = false)]
    normalize_overlap: bool,

    #[builder(into)]
    out: Option<String>,
}

impl Command for Stack {
    fn name() -> &'static str {
        "stack"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.base_name),
            Argument::positional(self.stack_type.to_string()),
        ];

        if matches!(self.stack_type, StackType::Rej) {
            args.push(Argument::positional(self.rejection.to_string()));
            if !matches!(self.rejection, StackRejection::None) {
                args.push(Argument::positional(self.lower_rej.to_string()));
                args.push(Argument::positional(self.higher_rej.to_string()));
            }

            if let Some(map) = &self.create_rejection_maps {
                args.push(Argument::positional(map.to_string()));
            }
        }

        args.push(Argument::positional(self.norm.to_string()));

        if let Some(filters) = &self.filters {
            args.extend(filters.iter().map(SequenceFilter::to_argument));
        }

        args.extend([
            Argument::flag_option("filter-incl", self.filter_included),
            Argument::flag_option("fastnorm", self.fast_norm),
            Argument::flag_option("output_norm", self.output_norm),
        ]);

        if let Some(weighting) = &self.weighting {
            args.push(Argument::positional(weighting.to_string()));
        }

        args.extend([
            Argument::flag_option("rgb_equal", self.rgb_equalization),
            Argument::flag_option("maximize", self.maximize),
            Argument::option("feather", self.feather_pixels),
            Argument::flag_option("overlap_norm", self.normalize_overlap),
            Argument::option("out", self.out.as_deref()),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SequenceFilterType;

    // --- Stack type variants ---

    #[test]
    fn default_is_rej_winsorized() {
        let cmd = Stack::builder("lights").build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("stack lights rej w "));
        assert!(s.contains("-nonorm"));
    }

    #[test]
    fn sum_stacking() {
        let cmd = Stack::builder("lights").stack_type(StackType::Sum).build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("stack lights sum"));
        assert!(!s.contains("rej"));
    }

    #[test]
    fn min_stacking() {
        let cmd = Stack::builder("lights").stack_type(StackType::Min).build();
        assert!(cmd.to_args_string().starts_with("stack lights min"));
    }

    #[test]
    fn max_stacking() {
        let cmd = Stack::builder("lights").stack_type(StackType::Max).build();
        assert!(cmd.to_args_string().starts_with("stack lights max"));
    }

    #[test]
    fn med_stacking() {
        let cmd = Stack::builder("lights").stack_type(StackType::Med).build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("stack lights med"));
        assert!(!s.contains("rej"));
    }

    // --- Rejection ---

    #[test]
    fn rejection_none_omits_sigma_values() {
        let cmd = Stack::builder("lights")
            .rejection(StackRejection::None)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains(" n "), "expected rejection type 'n' in: {s}");
        // sigma values should not follow
        assert!(!s.contains(" n 3"));
    }

    #[test]
    fn rejection_sigma_with_custom_values() {
        let cmd = Stack::builder("lights")
            .rejection(StackRejection::Sigma)
            .lower_rej(2.5)
            .higher_rej(3.5)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("rej s 2.5 3.5"), "got: {s}");
    }

    #[test]
    fn rejection_linear() {
        let cmd = Stack::builder("lights")
            .rejection(StackRejection::Linear)
            .build();
        assert!(cmd.to_args_string().contains("rej l "));
    }

    #[test]
    fn rejection_generalized() {
        let cmd = Stack::builder("lights")
            .rejection(StackRejection::Generalized)
            .build();
        assert!(cmd.to_args_string().contains("rej g "));
    }

    #[test]
    fn rejection_mad() {
        let cmd = Stack::builder("lights")
            .rejection(StackRejection::Mad)
            .build();
        assert!(cmd.to_args_string().contains("rej a "));
    }

    // --- Rejection maps ---

    #[test]
    fn rejection_map_merged() {
        let cmd = Stack::builder("lights")
            .create_rejection_maps(StackRejectionMapFlag::Merged)
            .build();
        assert!(cmd.to_args_string().contains("-rejmap"));
    }

    #[test]
    fn rejection_map_two() {
        let cmd = Stack::builder("lights")
            .create_rejection_maps(StackRejectionMapFlag::Two)
            .build();
        assert!(cmd.to_args_string().contains("-rejmaps"));
    }

    // --- Normalization ---

    #[test]
    fn norm_add() {
        let cmd = Stack::builder("lights").norm(StackNormFlag::Add).build();
        assert!(cmd.to_args_string().contains("-norm=add"));
    }

    #[test]
    fn norm_mul() {
        let cmd = Stack::builder("lights").norm(StackNormFlag::Mul).build();
        assert!(cmd.to_args_string().contains("-norm=mul"));
    }

    #[test]
    fn norm_addscale() {
        let cmd = Stack::builder("lights")
            .norm(StackNormFlag::AddScale)
            .build();
        assert!(cmd.to_args_string().contains("-norm=addscale"));
    }

    #[test]
    fn norm_mulscale() {
        let cmd = Stack::builder("lights")
            .norm(StackNormFlag::MulScale)
            .build();
        assert!(cmd.to_args_string().contains("-norm=mulscale"));
    }

    #[test]
    fn nonorm_default() {
        let cmd = Stack::builder("lights").build();
        assert!(cmd.to_args_string().contains("-nonorm"));
    }

    // --- Flags ---

    #[test]
    fn fast_norm_flag() {
        let cmd = Stack::builder("lights").fast_norm(true).build();
        assert!(cmd.to_args_string().contains("-fastnorm"));
    }

    #[test]
    fn output_norm_flag() {
        let cmd = Stack::builder("lights").output_norm(true).build();
        assert!(cmd.to_args_string().contains("-output_norm"));
    }

    #[test]
    fn rgb_equalization_flag() {
        let cmd = Stack::builder("lights").rgb_equalization(true).build();
        assert!(cmd.to_args_string().contains("-rgb_equal"));
    }

    #[test]
    fn filter_included_flag() {
        let cmd = Stack::builder("lights").filter_included(true).build();
        assert!(cmd.to_args_string().contains("-filter-incl"));
    }

    // --- Weighting ---

    #[test]
    fn weighting_noise() {
        let cmd = Stack::builder("lights")
            .weighting(StackWeightingFlag::Noise)
            .build();
        assert!(cmd.to_args_string().contains("-weight_from_noise"));
    }

    #[test]
    fn weighting_wfwhm() {
        let cmd = Stack::builder("lights")
            .weighting(StackWeightingFlag::WFwhm)
            .build();
        assert!(cmd.to_args_string().contains("-weight_from_wfwhm"));
    }

    #[test]
    fn weighting_nbstars() {
        let cmd = Stack::builder("lights")
            .weighting(StackWeightingFlag::NbStars)
            .build();
        assert!(cmd.to_args_string().contains("-weight_from_nbstars"));
    }

    #[test]
    fn weighting_nbstack() {
        let cmd = Stack::builder("lights")
            .weighting(StackWeightingFlag::NbStack)
            .build();
        assert!(cmd.to_args_string().contains("-weight_from_nbstack"));
    }

    // --- Out option ---

    #[test]
    fn out_option() {
        let cmd = Stack::builder("lights").out("result").build();
        assert!(cmd.to_args_string().contains("-out=result"));
    }

    // --- Filters ---

    #[test]
    fn filter_by_percent() {
        let cmd = Stack::builder("lights")
            .filters(vec![SequenceFilter::ByPercent {
                filter_type: SequenceFilterType::Fwhm,
                percent: 80.0,
            }])
            .build();
        assert!(cmd.to_args_string().contains("-filter-fwhm=80%"));
    }

    #[test]
    fn filter_by_value() {
        let cmd = Stack::builder("lights")
            .filters(vec![SequenceFilter::ByValue {
                filter_type: SequenceFilterType::Quality,
                value: 0.5,
            }])
            .build();
        assert!(cmd.to_args_string().contains("-filter-quality=0.5"));
    }

    #[test]
    fn multiple_filters() {
        let cmd = Stack::builder("lights")
            .filters(vec![
                SequenceFilter::ByPercent {
                    filter_type: SequenceFilterType::Fwhm,
                    percent: 80.0,
                },
                SequenceFilter::ByPercent {
                    filter_type: SequenceFilterType::Roundness,
                    percent: 90.0,
                },
            ])
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-filter-fwhm=80%"));
        assert!(s.contains("-filter-roundness=90%"));
    }

    // --- Base name ---

    #[test]
    fn base_name_with_spaces_is_quoted() {
        let cmd = Stack::builder("my lights").build();
        assert!(cmd.to_args_string().starts_with("stack 'my lights'"));
    }
}
