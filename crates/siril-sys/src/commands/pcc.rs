use bon::Builder;

use crate::{
    IntoArgument, LimitMag, StarCatalog,
    commands::{Argument, Command},
};

/// ```text
/// pcc [-limitmag=[+-]] [-catalog=] [-bgtol=lower,upper]
/// ```
///
/// Run the Photometric Color Correction on the loaded plate-solved image.
///
/// The limit magnitude of stars is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The star catalog used is NOMAD by default, it can be changed by providing **-catalog=apass**, **-catalog=localgaia** or **-catalog=gaia**. If installed locally, the remote NOMAD (the complete version) can be forced by providing **-catalog=nomad**
/// Background reference outlier tolerance can be specified in sigma units using **-bgtol=lower,upper**: these default to -2.8 and +2.0
///
#[derive(Builder)]
pub struct Pcc {
    #[builder(default)]
    limit_mag: LimitMag,
    catalog: Option<StarCatalog>,
    background_tolerance: Option<(f32, f32)>,
}

impl Command for Pcc {
    fn name() -> &'static str {
        "pcc"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            self.limit_mag.to_argument(),
            Argument::option("catalog", self.catalog.as_ref().map(|c| c.to_string())),
            Argument::option(
                "bgtol",
                self.background_tolerance
                    .as_ref()
                    .map(|v| format!("{},{}", v.0, v.1)),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args() {
        let cmd = Pcc::builder().build();
        assert_eq!(cmd.to_args_string(), "pcc");
    }

    #[test]
    fn limitmag_offset_positive() {
        let cmd = Pcc::builder().limit_mag(LimitMag::Offset(1.5)).build();
        assert_eq!(cmd.to_args_string(), "pcc -limitmag=+1.5");
    }

    #[test]
    fn limitmag_offset_negative() {
        let cmd = Pcc::builder().limit_mag(LimitMag::Offset(-1.0)).build();
        assert_eq!(cmd.to_args_string(), "pcc -limitmag=-1");
    }

    #[test]
    fn limitmag_absolute() {
        let cmd = Pcc::builder().limit_mag(LimitMag::Absolute(12.5)).build();
        assert_eq!(cmd.to_args_string(), "pcc -limitmag=12.5");
    }

    #[test]
    fn with_catalog() {
        let cmd = Pcc::builder().catalog(StarCatalog::Apass).build();
        assert_eq!(cmd.to_args_string(), "pcc -catalog=apass");
    }

    #[test]
    fn with_background_tolerance() {
        let cmd = Pcc::builder()
            .background_tolerance((-2.8f32, 2.0f32))
            .build();
        assert_eq!(cmd.to_args_string(), "pcc -bgtol=-2.8,2");
    }

    #[test]
    fn all_options() {
        let cmd = Pcc::builder()
            .limit_mag(LimitMag::Absolute(13.0))
            .catalog(StarCatalog::Gaia)
            .background_tolerance((-3.0f32, 2.5f32))
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "pcc -limitmag=13 -catalog=gaia -bgtol=-3,2.5"
        );
    }
}
