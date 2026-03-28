use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savetif filename [-astro] [-deflate]
/// ```
///
/// Saves current image under the form of a uncompressed TIFF file with 16-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression.
///
/// See also SAVETIF32 and SAVETIF8
///
#[derive(Builder)]
pub struct Savetif {
    // #[builder(start_fn, into)]
    // filename: String
}

impl Command for Savetif {
    fn name() -> &'static str {
        "savetif"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
