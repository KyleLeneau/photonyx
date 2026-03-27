use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// profile -from=x,y -to=x,y [-tri] [-cfa] [-arcsec] { [-savedat] | [-filename=] } [-layer=] [-width=] [-spacing=] ["-title=My Plot"]
/// ```
///
/// Generates an intensity profile plot between 2 points in the image, also known as a *cut*. The arguments may be provided in any order. The arguments **-to=x,y** and **-from=x,y** are mandatory.
///
/// The argument **-layer=\ {red \| green \| blue \| lum \| col}** specifies which channel (or luminance or colour) to plot if the image is color. It may also be used with the **-tri** option, which generates 3 parallel equispaced profiles each separated by **-spacing=** pixels, but note that for tri profiles the **col** option will be treated the same as **lum**.
///
/// The option **-cfa** selects CFA mode, which generates 4 profiles: 1 for each CFA channel in a Bayer patterned image. This option cannot be used with color images or mono images with no Bayer pattern, and cannot be used at the same time as the **-tri** option.
///
/// The option **-arcsec** causes the x axis to display distance in arcsec, if the necessary metadata is available. If not provided or if metadata is not available, distance will be shown in pixel units.
///
/// The argument **-savedat** will cause the data files to be saved: the filename will be written to the log. Alternatively the argument **-filename=** can be used to specify a filename to write the data file to. (The **-filename=** option implies **-savedat**.)
///
/// The argument **"-title=\ My Title"** sets a custom title "My Title"
///
#[derive(Builder)]
pub struct Profile {}

impl Command for Profile {
    fn name() -> &'static str {
        "profile"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
