use bon::Builder;

use crate::{
    PixelInterpolation, RegistrationTransformation,
    commands::{Argument, Command},
};

/// ```text
/// register sequencename [-2pass] [-selected] [-prefix=] [-scale=]
/// register sequencename ... [-layer=] [-transf=] [-minpairs=] [-maxstars=] [-nostarlist] [-disto=]
/// register sequencename ... [-interp=] [-noclamp]
/// register sequencename ... [-drizzle [-pixfrac=] [-kernel=] [-flat=]]
/// ```
///
/// Finds and optionally performs geometric transforms on images of the sequence given in argument so that they may be superimposed on the reference image. Using stars for registration, this algorithm only works with deep sky images. Star detection options can be changed using **SETFINDSTAR** or the *Dynamic PSF* dialog.
///
/// All images of the sequence will be registered unless the option **-selected** is passed, in that case the excluded images will not be processed.
/// The **-2pass** option will only compute the transforms but not generate the transformed images, **-2pass** adds a preliminary pass to the algorithm to find a good reference image before computing the transforms, based on image quality and framing. To generate transformed images after this pass, use SEQAPPLYREG.
/// If created, the output sequence name will start with the prefix "r\_" unless otherwise specified with **-prefix=** option. The output images can be rescaled by passing a **-scale=** argument with a float value between 0.1 and 3.
///
/// **Image transformation options:**
///
/// The detection is done on the green layer for colour images, unless specified by the **-layer=** option with an argument ranging from 0 to 2 for red to blue.
/// **-transf=** specifies the use of either **shift**, **similarity**, **affine** or **homography** (default) transformations respectively.
/// **-minpairs=** will specify the minimum number of star pairs a frame must have with the reference frame, otherwise the frame will be dropped and excluded from the sequence.
/// **-maxstars=** will specify the maximum number of stars to find within each frame (must be between 100 and 2000). With more stars, a more accurate registration can be computed, but will take more time to run.
/// **-nostarlist** disables saving the star lists to disk.
/// **-disto=** uses distortion terms from a previous platesolve solution (with a SIP order > 1). It takes as parameter either **image** to use the solution contained in the currently loaded image, **file** followed by the path to the image containing the solution or **master** to load automatically the matching distortion master corresponding to each image. When using this option, the polynomials are used both to correct star positions before computing the transformation and to undistort the images when output images are exported.
///
/// **Image interpolation options:**
///
/// By default, transformations are applied to register the images by using interpolation.
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument.
///
/// **Image drizzle options:**
///
/// Otherwise, the images can be exported using HST drizzle algorithm by passing the argument **-drizzle** which can take the additional options:
/// **-pixfrac=** sets the pixel fraction (default = 1.0).
/// The **-kernel=** argument sets the drizzle kernel and must be followed by one of **point**, **turbo**, **square**, **gaussian**, **lanczos2** or **lanczos3**. The default is **square**.
/// The **-flat=** argument specifies a master flat to weight the drizzled input pixels (default is no flat).
///
/// Note: when using **-drizzle** on images taken with a color camera, the input images must not be debayered. In that case, star detection will always occur on the green pixels
///
/// Links: :ref:`setfindstar <setfindstar>`, :ref:`psf <psf>`, :ref:`seqapplyreg <seqapplyreg>`
///
#[derive(Builder)]
pub struct Register {
    #[builder(start_fn, into)]
    base_name: String,
    #[builder(default = false)]
    two_pass: bool,
    #[builder(default = false)]
    selected: bool,
    #[builder(into)]
    prefix: Option<String>,
    scale: Option<f32>,
    layer: Option<u8>,
    trans_func: Option<RegistrationTransformation>,
    min_pairs: Option<u32>,
    max_stars: Option<u32>,
    #[builder(default = false)]
    no_starlist: bool,
    #[builder(into)]
    disto: Option<String>,
    interp: Option<PixelInterpolation>,
    #[builder(default = false)]
    noclamp: bool,
    #[builder(default = false)]
    drizzle: bool,
    pixfrac: Option<f32>,
    #[builder(into)]
    kernel: Option<String>,
    #[builder(into)]
    flat: Option<String>,
}

impl Command for Register {
    fn name() -> &'static str {
        "register"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.base_name),
            Argument::flag_option("2pass", self.two_pass),
            Argument::flag_option("selected", self.selected),
            Argument::option("prefix", self.prefix.as_deref()),
            Argument::option("scale", self.scale),
            Argument::option("layer", self.layer),
            Argument::option("transf", self.trans_func.as_ref()),
            Argument::option("minpairs", self.min_pairs),
            Argument::option("maxstars", self.max_stars),
            Argument::flag_option("nostarlist", self.no_starlist),
            Argument::option("disto", self.disto.as_deref()),
        ];

        if !self.two_pass {
            args.push(Argument::option("interp", self.interp.as_ref()));
        }

        args.push(Argument::flag_option("noclamp", self.noclamp));

        if self.drizzle {
            args.extend([
                Argument::flag_option("drizzle", self.drizzle),
                Argument::option("pixfrac", self.pixfrac),
                Argument::option("kernel", self.kernel.as_deref()),
                Argument::option("flat", self.flat.as_deref()),
            ]);
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use crate::{PixelInterpolation, RegistrationTransformation};

    use super::*;

    #[test]
    fn minimal_sequence_only() {
        let cmd = Register::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "register lights");
    }

    #[test]
    fn two_pass_flag() {
        let cmd = Register::builder("lights").two_pass(true).build();
        assert!(cmd.to_args_string().contains("-2pass"));
    }

    #[test]
    fn selected_flag() {
        let cmd = Register::builder("lights").selected(true).build();
        assert!(cmd.to_args_string().contains("-selected"));
    }

    #[test]
    fn prefix_and_scale() {
        let cmd = Register::builder("lights")
            .prefix("r_")
            .scale(2.0_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-prefix=r_"));
        assert!(s.contains("-scale=2"));
    }

    #[test]
    fn transformation_type() {
        let cmd = Register::builder("lights")
            .trans_func(RegistrationTransformation::Affine)
            .build();
        assert!(cmd.to_args_string().contains("-transf=affine"));
    }

    #[test]
    fn interpolation_included_when_not_two_pass() {
        let cmd = Register::builder("lights")
            .interp(PixelInterpolation::Cubic)
            .build();
        assert!(cmd.to_args_string().contains("-interp=cubic"));
    }

    #[test]
    fn interpolation_excluded_when_two_pass() {
        let cmd = Register::builder("lights")
            .two_pass(true)
            .interp(PixelInterpolation::Cubic)
            .build();
        assert!(!cmd.to_args_string().contains("-interp="));
    }

    #[test]
    fn drizzle_with_options() {
        let cmd = Register::builder("lights")
            .drizzle(true)
            .pixfrac(0.8_f32)
            .kernel("square".to_string())
            .flat("master_flat.fit".to_string())
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-drizzle"));
        assert!(s.contains("-pixfrac=0.8"));
        assert!(s.contains("-kernel=square"));
        assert!(s.contains("-flat=master_flat.fit"));
    }

    #[test]
    fn drizzle_args_excluded_when_drizzle_false() {
        let cmd = Register::builder("lights")
            .pixfrac(0.8_f32)
            .kernel("square".to_string())
            .build();
        let s = cmd.to_args_string();
        assert!(!s.contains("-drizzle"));
        assert!(!s.contains("-pixfrac="));
    }
}
