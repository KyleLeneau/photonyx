#![allow(clippy::doc_lazy_continuation)]

use bon::Builder;

use crate::{
    DrizzleKernel, PixelInterpolation, SequenceFraming,
    commands::{Argument, Command},
};

/// ```text
/// seqapplyreg sequencename [-prefix=] [-scale=] [-layer=] [-framing=]
/// seqapplyreg sequencename ... [-interp=] [-noclamp]
/// seqapplyreg sequencename ... [-drizzle [-pixfrac=] [-kernel=] [-flat=]]
/// seqapplyreg sequencename ... [-filter-fwhm=value[%|k]] [-filter-wfwhm=value[%|k]] [-filter-round=value[%|k]] [-filter-bkg=value[%|k]] [-filter-nbstars=value[%|k]] [-filter-quality=value[%|k]] [-filter-incl[uded]]
/// ```
///
/// Applies geometric transforms on images of the sequence given in argument so that they may be superimposed on the reference image, using registration data previously computed (see REGISTER).
/// The output sequence name starts with the prefix **"r\_"** unless otherwise specified with **-prefix=** option.
/// The registration is done on the first layer for which data exists for RGB images unless specified by **-layer=** option (0, 1 or 2 for R, G and B respectively).
/// The output images can be rescaled by passing a **-scale=** argument with a float value between 0.1 and 3.
///
/// Automatic framing of the output sequence can be specified using **-framing=** keyword followed by one of the methods in the list { current \| min \| max \| cog } :
/// **-framing=max** (bounding box) will project each image and compute its shift wrt. reference image. The resulting sequence can then be stacked using option **-maximize** of STACK command which will create the full image encompassing all images of the sequence.
/// **-framing=min** (common area) crops each image to the area it has in common with all images of the sequence.
/// **-framing=cog** determines the best framing position as the center of gravity (cog) of all the images.
///
/// **Image interpolation options:**
/// By default, transformations are applied to register the images by using interpolation.
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument.
///
/// **Image drizzle options:**
/// Otherwise, the images can be exported using HST drizzle algorithm by passing the argument **-drizzle** which can take the additional options:
/// **-pixfrac=** sets the pixel fraction (default = 1.0).
/// The **-kernel=** argument sets the drizzle kernel and must be followed by one of **point**, **turbo**, **square**, **gaussian**, **lanczos2** or **lanczos3**. The default is **square**.
/// The **-flat=** argument specifies a master flat to weight the drizzled input pixels (default is no flat).
///
/// **Filtering out images:**
/// Images to be registered can be selected based on some filters, like those selected or with best FWHM, with some of the **-filter-\*** options.
///
///
/// Links: :ref:`register <register>`, :ref:`stack <stack>`
///
/// With filtering being some of these in no particular order or number:
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
pub struct SeqApplyReg {
    #[builder(start_fn)]
    base_name: String,
    prefix: Option<String>,
    scale: Option<f32>,
    layer: Option<u8>,
    framing: Option<SequenceFraming>,
    interp: Option<PixelInterpolation>,
    #[builder(default = false)]
    noclamp: bool,
    #[builder(default = false)]
    drizzle: bool,
    pixfrac: Option<f32>,
    kernel: Option<DrizzleKernel>,
    flat: Option<String>,
}

impl Command for SeqApplyReg {
    fn name() -> &'static str {
        "seqapplyreg"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.base_name),
            Argument::option("prefix", self.prefix.as_deref()),
            Argument::option("scale", self.scale),
            Argument::option("layer", self.layer),
            Argument::option("framing", self.framing.as_ref()),
            Argument::option("interp", self.interp.as_ref()),
            Argument::flag_option("noclamp", self.noclamp),
        ];

        if self.drizzle {
            args.extend([
                Argument::flag_option("drizzle", self.drizzle),
                Argument::option("pixfrac", self.pixfrac),
                Argument::option("kernel", self.kernel.as_ref()),
                Argument::option("flat", self.flat.as_deref()),
            ]);
        }

        args
    }
}
