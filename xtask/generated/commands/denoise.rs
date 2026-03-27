use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// denoise [-nocosmetic] [-mod=m] [ -vst | -da3d | -sos=n [-rho=r] ] [-indep]
/// ```
///
/// Denoises the image using the non-local Bayesian algorithm described by `Lebrun, Buades and Morel <https://www.ipol.im/pub/art/2013/16>`__.
///
/// It is strongly recommended to apply cosmetic correction to remove salt and pepper noise before running denoise, and by default this command will apply cosmetic correction automatically. However, if this has already been carried out earlier in the workflow it may be disabled here using the optional command **-nocosmetic**.
///
/// An optional argument **-mod=m** may be given, where 0 <= m <= 1. The output pixel is computed as : *out=m x d + (1 − m) x in*, where *d* is the denoised pixel value. A modulation value of 1 will apply no modulation. If the parameter is omitted, it defaults to 1.
///
/// The optional argument **-vst** can be used to apply the generalised Anscombe variance stabilising transform prior to NL-Bayes. This is useful with photon-starved images such as single subs, where the noise follows a Poisson or Poisson-Gaussian distribution rather than being primarily Gaussian. It cannot be used in conjunction with DA3D or SOS, and for denoising stacked images it is usually not beneficial.
///
/// The optional argument **-da3d** can be used to enable Data-Adaptive Dual Domain Denoising (DA3D) as a final stage denoising algorithm. This uses the output of BM3D as a guide image to refine the denoising. It improves detail and reduces staircasing artefacts.
///
/// The optional argument **-sos=\ n** can be used to enable Strengthen-Operate-Subtract (SOS) iterative denoise boosting, with the number of iterations specified by n. In particular, this booster may produce better results if the un-boosted NL-Bayes algorithm produces artefacts in background areas. If both -da3d and -sos=n are specified, the last to be specified will apply.
///
/// The optional argument **-rho=r** may be specified, where 0 < r < 1. This is used by the SOS booster to determine the amount of noisy image added in to the intermediate result between each iteration. If -sos=n is not specified then the parameter is ignored.
///
/// The default is not to apply DA3D or SOS, as the improvement in denoising is usually relatively small and these techniques requires additional processing time.
///
/// In very rare cases, blocky coloured artefacts may be found in the output when denoising colour images. The optional argument **-indep** can be used to prevent this by denoising each channel separately. This is slower but will eliminate artefacts
///
#[derive(Builder)]
pub struct Denoise {}

impl Command for Denoise {
    fn name() -> &'static str {
        "denoise"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
