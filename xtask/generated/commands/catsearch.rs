use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// catsearch name
/// ```
///
/// Searches an object by **name** and adds it to the user annotation catalog. The object is first searched in the annotation catalogs, if not found a request is made to SIMBAD.
/// The object can be a solar system object, in which case a prefix, 'a:' for asteroid, 'p:' for planet, 'c:' for comet, 'dp:' for dwarf planet or 's:' for natural satellite, is required before the object name. The search is done for the date, time and observing location found in the image header, using the `IMCCE Miriade service <https://ssp.imcce.fr/webservices/miriade/howto/ephemcc/#howto-sso>`__
///
#[derive(Builder)]
pub struct Catsearch {}

impl Command for Catsearch {
    fn name() -> &'static str {
        "catsearch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
