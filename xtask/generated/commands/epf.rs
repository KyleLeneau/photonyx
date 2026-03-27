use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// epf [-guided] [-d=] [-si=] [-ss=] [-mod=] [-guideimage=]
/// ```
///
/// Applies an edge preserving filter. By default a bilateral filter is applied; a guided filter can be specified using the argument **-guided**. The filter diameter defaults to 3 and can be set using **-d=**. Be careful with values of d greater than 20 as the algorithm can be computationally expensive.
///
/// The intensity filtering sigma value can be set using **-si=** and the spatial sigma value can be set using **-ss=**. Sigma values represent the difference in pixel values over which the filter acts strongly: for 32-bit images the value should be between 0 and 1.0, whereas for 16-bit images it should be between 0 and 65535. The defaults if not specified are for both to be set to 11. If **-d=0** is set then the filter diameter will be set automatically based on the value of **-ss**. *Note that when applying a guided filter, only* **-sc** *applies.*
///
/// When specifying a guided filter, a guide image may be set using **-guideimage=**. The default if no guide image is specified is to perform a self-guided filter. *Note: the guide image must have the same dimensions as the image to be filtered!*
///
/// The strength of the filter can be modulated using the **-mod=** argument. If mod = 1.0 the full effect of the filter will be applied; for mod less than 1.0 a proportion of the original image will be mixed with the result, and for mod = 0.0 no filtering will be applied
///
#[derive(Builder)]
pub struct Epf {}

impl Command for Epf {
    fn name() -> &'static str {
        "epf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
