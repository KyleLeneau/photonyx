use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// spcc [-limitmag=[+-]] [ { -monosensor= [ -rfilter= ] [-gfilter=] [-bfilter=] | -oscsensor= [-oscfilter=] [-osclpf=] } ] [-whiteref=] [ -narrowband [-rwl=] [-gwl=] [-bwl=] [-rbw=] [-gbw=] [-bbw=] ] [-bgtol=lower,upper] [ -atmos [-obsheight=] { [-pressure=] | [-slp=] } ]
/// ```
///
/// Run the Spectrophotometric Color Correction on the loaded platesolved image.
///
/// The limit magnitude of stars is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The star catalog used for SPCC is always Gaia DR3: by default the local Gaia DR3 xp_sampled catalog will be used if available but this can be overridden with **-catalog={gaia \| localgaia}**.
///
/// The names of sensors and filters can be specified using the following options: **-monosensor=**, **-rfilter=**, **-gfilter=**, **-bfilter=** or **-oscsensor=**, **-oscfilter=**, **-osclpf=**; the name of the white reference can be specified using the **-whiteref=** option. In all cases the name must be provided exactly as it is in the combo boxes in the SPCC tool. Note that sensor, filter and white reference names may contain spaces: in this case when using them as arguments to the **spcc** command, the entire argument must be enclosed in quotation marks, for example "-whiteref=Average Spiral Galaxy".
///
/// Narrowband mode can be selected using the argument **-narrowband**, in which case the previous filter arguments are ignored and NB filter wavelengths and bandwidths can be provided using **-rwl=**, **-rbw=**, **-gwl=**, **-gbw=**, **-bwl=** and **-bbw=**.
///
/// If one of the spectral data argument is omitted, the previously used value will be used.
///
/// Background reference outlier tolerance can be specified in sigma units using **-bgtol=lower,upper**: these default to -2.8 and +2.0.
///
/// Atmospheric correction can be applied by passing **-atmos**. In this case the following optional arguments apply: **-obsheight=** specifies the observer's height above sea level in metres (default 10), **-pressure=** specifies local atmospheric pressure at the observing site in hPa, or **-slp=** specifies sea-level atmospheric pressure in hPa (default pressure is 1013.25 hPa at sea level)
///
#[derive(Builder)]
pub struct Spcc {}

impl Command for Spcc {
    fn name() -> &'static str {
        "spcc"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
