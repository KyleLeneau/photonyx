use bon::Builder;

use crate::{
    LimitMag, StarCatalog,
    commands::{Argument, Command},
};

/// ```text
/// seqplatesolve sequencename [image_center_coords] [-focal=] [-pixelsize=]
/// seqplatesolve sequencename ... [-downscale] [-order=] [-radius=] [-force] [-noreg] [-disto=]
/// seqplatesolve sequencename ... [-limitmag=[+-]] [-catalog=] [-nocrop] [-nocache]
/// seqplatesolve sequencename ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve a sequence. A new sequence will be created with the prefix "ps\_" if the input sequence is SER, otherwise, the images headers will be updated. In case of SER, providing the metadata is mandatory and the output sequence will be in the FITS cube format, as SER cannot store WCS data.
/// If WCS or other image metadata are erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from images and settings. See also options to solve blindly with local Astrometry.net
///
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// Images already solved will be skipped by default. This can be disabled by passing the option **-force**.
/// Using this command will update registration data unless the option **-noreg** is passed.
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalogue and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the remote catalog given in argument, with possible values: tycho2, nomad, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
/// When using online catalogues, a single catalogue extraction will be done for the entire sequence. If there is a lot of drift or different sampling, that may not succeed for all images. This can be disabled by passing the argument **-nocache**, in which case metadata from each image will be used (except for the forced values like center coordinates, pixel size and/or focal length).
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Seqplatesolve {
    #[builder(start_fn)]
    sequence: String,
    /// Force a new plate solve even if the image is already solved.
    #[builder(default = false)]
    force: bool,
    /// Approximate image center coordinates (decimal degrees or HMS/DMS with colon separators).
    #[builder(into)]
    image_center: Option<String>,
    /// Focal length in mm.
    focal: Option<f64>,
    /// Pixel size in microns.
    pixelsize: Option<f64>,
    /// Downscale the image before star detection for faster solving on large images.
    #[builder(default = false)]
    downscale: bool,
    /// Disable automatic image flip when the image is detected as upside-down.
    #[builder(default = false)]
    noflip: bool,
    /// SIP polynomial order for distortion (1–5).
    order: Option<u8>,
    /// Search cone radius for near-field retry.
    radius: Option<f64>,
    /// Path to save or load the distortion file.
    #[builder(into)]
    disto: Option<String>,
    /// Limit magnitude mode. Defaults to automatic computation from field of view.
    #[builder(default)]
    limit_mag: LimitMag,
    /// Star catalog to use. Ignored when `local_asnet` is true.
    catalog: Option<StarCatalog>,
    /// Disable cropping for large fields of view (>5 degrees).
    #[builder(default = false)]
    nocrop: bool,
    /// Use local Astrometry.net solver instead of the built-in Siril solver.
    #[builder(default = false)]
    local_asnet: bool,
    /// Solve blindly for position (only valid with `local_asnet`).
    #[builder(default = false)]
    blindpos: bool,
    /// Solve blindly for resolution (only valid with `local_asnet`).
    #[builder(default = false)]
    blindres: bool,
}

impl Command for Seqplatesolve {
    fn name() -> &'static str {
        "seqplatesolve"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(self.sequence.clone()),
            Argument::flag_option("force", self.force),
            Argument::positional_option(self.image_center.as_deref()),
            Argument::option("focal", self.focal),
            Argument::option("pixelsize", self.pixelsize),
            Argument::flag_option("downscale", self.downscale),
            Argument::flag_option("noflip", self.noflip),
            Argument::option("order", self.order),
            Argument::option("radius", self.radius),
            Argument::option("disto", self.disto.as_deref()),
        ];

        match &self.limit_mag {
            LimitMag::Default => {}
            LimitMag::Offset(v) if *v != 0.0 => {
                let s = if *v > 0.0 {
                    format!("+{}", v)
                } else {
                    v.to_string()
                };
                args.push(Argument::option("limitmag", Some(s)));
            }
            LimitMag::Offset(_) => {}
            LimitMag::Absolute(v) => {
                args.push(Argument::option("limitmag", Some(v.to_string())));
            }
        }

        if !self.local_asnet {
            args.push(Argument::option(
                "catalog",
                self.catalog.as_ref().map(|c| c.to_string()),
            ));
        }

        args.push(Argument::flag_option("nocrop", self.nocrop));

        if self.local_asnet {
            args.push(Argument::flag("localasnet"));
            args.push(Argument::flag_option("blindpos", self.blindpos));
            args.push(Argument::flag_option("blindres", self.blindres));
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_only() {
        let cmd = Seqplatesolve::builder("lights".to_string()).build();
        assert_eq!(cmd.to_args_string(), "seqplatesolve lights");
    }

    #[test]
    fn force_flag() {
        let cmd = Seqplatesolve::builder("lights".to_string()).force(true).build();
        assert_eq!(cmd.to_args_string(), "seqplatesolve lights -force");
    }

    #[test]
    fn image_center_coords() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .image_center("83.822,22.0145")
            .build();
        assert_eq!(cmd.to_args_string(), "seqplatesolve lights 83.822,22.0145");
    }

    #[test]
    fn focal_and_pixelsize() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .focal(500.0)
            .pixelsize(3.8)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqplatesolve lights -focal=500 -pixelsize=3.8"
        );
    }

    #[test]
    fn downscale_and_noflip() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .downscale(true)
            .noflip(true)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-downscale"));
        assert!(s.contains("-noflip"));
    }

    #[test]
    fn order_and_radius() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .order(3)
            .radius(10.0)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqplatesolve lights -order=3 -radius=10"
        );
    }

    #[test]
    fn disto_option() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .disto("my_disto.dat")
            .build();
        assert_eq!(cmd.to_args_string(), "seqplatesolve lights -disto=my_disto.dat");
    }

    #[test]
    fn limitmag_default_emits_nothing() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .limit_mag(LimitMag::Default)
            .build();
        assert!(!cmd.to_args_string().contains("limitmag"));
    }

    #[test]
    fn limitmag_positive_offset() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .limit_mag(LimitMag::Offset(1.5))
            .build();
        assert!(cmd.to_args_string().contains("-limitmag=+1.5"));
    }

    #[test]
    fn limitmag_negative_offset() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .limit_mag(LimitMag::Offset(-1.0))
            .build();
        assert!(cmd.to_args_string().contains("-limitmag=-1"));
    }

    #[test]
    fn limitmag_zero_offset_emits_nothing() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .limit_mag(LimitMag::Offset(0.0))
            .build();
        assert!(!cmd.to_args_string().contains("limitmag"));
    }

    #[test]
    fn limitmag_absolute() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .limit_mag(LimitMag::Absolute(12.5))
            .build();
        assert!(cmd.to_args_string().contains("-limitmag=12.5"));
    }

    #[test]
    fn catalog_option() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .catalog(StarCatalog::Gaia)
            .build();
        assert!(cmd.to_args_string().contains("-catalog=gaia"));
    }

    #[test]
    fn nocrop_flag() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .nocrop(true)
            .build();
        assert!(cmd.to_args_string().contains("-nocrop"));
    }

    #[test]
    fn local_asnet_emits_localasnet_flag() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .local_asnet(true)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-localasnet"));
        assert!(!s.contains("-catalog"));
    }

    #[test]
    fn local_asnet_with_blindpos_and_blindres() {
        let cmd = Seqplatesolve::builder("lights".to_string())
            .local_asnet(true)
            .blindpos(true)
            .blindres(true)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-localasnet"));
        assert!(s.contains("-blindpos"));
        assert!(s.contains("-blindres"));
    }
}
