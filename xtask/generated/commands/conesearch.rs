use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// conesearch [limit_magnitude] [-cat=] [-phot] [-obscode=] [-tag={on|off}] [-log={on|off}] [-trix=] [-out=]
/// ```
///
/// Displays stars from the local catalog by default for the loaded plate solved image, down to the provided **limit_magnitude** (13 by default for most catalogues, except 14.5 for aavso_chart, 20 for solsys, and omitted for pgc).
/// An alternate online catalog can be specified with **-cat=**, taking values
/// - for stars: tycho2, nomad, gaia, localgaia, ppmxl, bsc, apass, gcvs, vsx, simbad, aavso_chart
/// - for exoplanets: exo
/// - for deep-sky: pgc
/// - for solar system objects: solsys (closest `IAU observatory code <https://vo.imcce.fr/webservices/data/displayIAUObsCodes.php>`__ can be passed with the argument **-obscode=** for better position accuracy)
///
/// For stars catalogues containing photometric data, stars with no B-V information will be kept; they can be excluded by passing **-phot**
/// The argument **-trix=** can be passed instead of a catalogue followed by a number between 0 and 511 to plot stars contained in local catalogues trixel of level 3 (for dev usage mainly)
///
/// Some catalogs (bsc, gcvs, pgc, exo, aavso_chart, varisum and solsys) will also display, by default, names alongside markers in the display (GUI only) and list them in the log. For others with larger number of objects, namely vsx and simbad, the information can also be shown but, as it may clutter the display, it is not activated by default. This behavior can be toggled on/off with the options **-tag=on|off** to display names alongside markers and **-log=on|off** to list the objects in the console log
///
/// The list of items that are present in the image can optionally saved to a csv file by passing the argument **-out=**
///
#[derive(Builder)]
pub struct Conesearch {}

impl Command for Conesearch {
    fn name() -> &'static str {
        "conesearch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
