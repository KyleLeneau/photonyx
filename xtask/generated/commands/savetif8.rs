use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savetif8 filename [-astro] [-deflate]
/// ```
///
/// Same command as SAVETIF but the output file is saved in 8-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression
///
/// Links: :ref:`savetif <savetif>`
///
#[derive(Builder)]
pub struct Savetif8 {}

impl Command for Savetif8 {
    fn name() -> &'static str {
        "savetif8"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
