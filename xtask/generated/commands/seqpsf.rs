use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqpsf sequencename [channel] [{ -at=x,y | -wcs=ra,dec }] [-followstar]
/// ```
///
/// Same command as PSF but runs on sequences. This is similar to the one-star registration, except results can be used for photometry analysis rather than aligning images and the coordinates of the star can be provided by options.
/// This command is what is called internally by the menu that appears on right click in the image, with the PSF for the sequence entry. If registration data already exist for the sequence, they will can be used to shift the search window in each image. If there is no registration data and if there is significant shift between images in the sequence, the default settings will fail to find stars in the initial position of the search area.
/// The follow star option can then be activated with the argument **-followstar**.
///
/// Results will be displayed in the Plot tab, from which they can also be exported to a comma-separated values (CSV) file for external analysis.
///
/// When creating a light curve, the first star for which seqpsf has been run, marked 'V' in the display, will be considered as the variable star. All others are averaged to create a reference light curve subtracted to the light curve of the variable star.
///
/// Currently, in headless operation, the command prints some analysed data in the console, another command allows several stars to be analysed and plotted as a light curve: LIGHT_CURVE. Arguments are mandatory: the sequence name must be provided ("." may be used to indicate the currently loaded sequence) and when headless it is mandatory to provide the coordinates of the star, with -at= allowing coordinates in pixels to be provided for the target star of -wcs= allowing J2000 equatorial coordinates to be provided
///
/// Links: :ref:`psf <psf>`, :ref:`light_curve <light_curve>`
///
#[derive(Builder)]
pub struct Seqpsf {}

impl Command for Seqpsf {
    fn name() -> &'static str {
        "seqpsf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
