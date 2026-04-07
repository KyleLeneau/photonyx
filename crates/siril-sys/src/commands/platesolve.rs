use bon::Builder;

use crate::{
    LimitMag, StarCatalog,
    commands::{Argument, Command},
};

/// ```text
/// platesolve [-force] [image_center_coords] [-focal=] [-pixelsize=]
/// platesolve ... [-noflip] [-downscale] [-order=] [-radius=] [-disto=]
/// platesolve ... [-limitmag=[+-]] [-catalog=] [-nocrop]
/// platesolve ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve the loaded image.
/// If the image has already been plate solved nothing will be done, unless the **-force** argument is passed to force a new solve. If WCS or other image metadata is erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from image and settings. See also options to solve blindly with local Astrometry.net
///
/// Unless **-noflip** is specified, if the image is detected as being upside-down, it will be flipped.
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalog and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the catalog given in argument, with possible values: tycho2, nomad, localgaia, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Platesolve {
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

impl Command for Platesolve {
    fn name() -> &'static str {
        "platesolve"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
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
    fn minimal_no_args() {
        let cmd = Platesolve::builder().build();
        assert_eq!(cmd.to_args_string(), "platesolve");
    }

    #[test]
    fn force_flag() {
        let cmd = Platesolve::builder().force(true).build();
        assert_eq!(cmd.to_args_string(), "platesolve -force");
    }

    #[test]
    fn image_center_decimal() {
        let cmd = Platesolve::builder()
            .image_center("83.822,22.0145")
            .build();
        assert_eq!(cmd.to_args_string(), "platesolve 83.822,22.0145");
    }

    #[test]
    fn focal_and_pixelsize() {
        let cmd = Platesolve::builder().focal(500.0).pixelsize(3.8).build();
        assert_eq!(cmd.to_args_string(), "platesolve -focal=500 -pixelsize=3.8");
    }

    #[test]
    fn downscale_and_noflip() {
        let cmd = Platesolve::builder().downscale(true).noflip(true).build();
        let s = cmd.to_args_string();
        assert!(s.contains("-downscale"));
        assert!(s.contains("-noflip"));
    }

    #[test]
    fn order_and_radius() {
        let cmd = Platesolve::builder().order(3).radius(10.0).build();
        assert_eq!(cmd.to_args_string(), "platesolve -order=3 -radius=10");
    }

    #[test]
    fn disto_option() {
        let cmd = Platesolve::builder().disto("my_disto.dat").build();
        assert_eq!(cmd.to_args_string(), "platesolve -disto=my_disto.dat");
    }

    #[test]
    fn limitmag_default_emits_nothing() {
        let cmd = Platesolve::builder().limit_mag(LimitMag::Default).build();
        assert!(!cmd.to_args_string().contains("limitmag"));
    }

    #[test]
    fn limitmag_positive_offset() {
        let cmd = Platesolve::builder()
            .limit_mag(LimitMag::Offset(1.5))
            .build();
        assert_eq!(cmd.to_args_string(), "platesolve -limitmag=+1.5");
    }

    #[test]
    fn limitmag_negative_offset() {
        let cmd = Platesolve::builder()
            .limit_mag(LimitMag::Offset(-1.5))
            .build();
        assert_eq!(cmd.to_args_string(), "platesolve -limitmag=-1.5");
    }

    #[test]
    fn limitmag_zero_offset_emits_nothing() {
        let cmd = Platesolve::builder()
            .limit_mag(LimitMag::Offset(0.0))
            .build();
        assert!(!cmd.to_args_string().contains("limitmag"));
    }

    #[test]
    fn limitmag_absolute() {
        let cmd = Platesolve::builder()
            .limit_mag(LimitMag::Absolute(12.5))
            .build();
        assert_eq!(cmd.to_args_string(), "platesolve -limitmag=12.5");
    }

    #[test]
    fn catalog_gaia() {
        let cmd = Platesolve::builder().catalog(StarCatalog::Gaia).build();
        assert_eq!(cmd.to_args_string(), "platesolve -catalog=gaia");
    }

    #[test]
    fn catalog_ignored_when_local_asnet() {
        let cmd = Platesolve::builder()
            .catalog(StarCatalog::Gaia)
            .local_asnet(true)
            .build();
        let s = cmd.to_args_string();
        assert!(!s.contains("catalog"));
        assert!(s.contains("-localasnet"));
    }

    #[test]
    fn nocrop_flag() {
        let cmd = Platesolve::builder().nocrop(true).build();
        assert_eq!(cmd.to_args_string(), "platesolve -nocrop");
    }

    #[test]
    fn local_asnet_with_blind_options() {
        let cmd = Platesolve::builder()
            .local_asnet(true)
            .blindpos(true)
            .blindres(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "platesolve -localasnet -blindpos -blindres"
        );
    }

    #[test]
    fn blindpos_blindres_ignored_without_local_asnet() {
        let cmd = Platesolve::builder()
            .blindpos(true)
            .blindres(true)
            .build();
        let s = cmd.to_args_string();
        assert!(!s.contains("blindpos"));
        assert!(!s.contains("blindres"));
    }

    #[test]
    fn full_siril_solver_invocation() {
        let cmd = Platesolve::builder()
            .force(true)
            .image_center("83.822,22.0145")
            .focal(500.0)
            .pixelsize(3.8)
            .downscale(true)
            .order(3)
            .limit_mag(LimitMag::Offset(1.0))
            .catalog(StarCatalog::Gaia)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "platesolve -force 83.822,22.0145 -focal=500 -pixelsize=3.8 -downscale -order=3 -limitmag=+1 -catalog=gaia"
        );
    }
}
