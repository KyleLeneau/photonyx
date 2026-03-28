use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savetif32 filename [-astro] [-deflate]
/// ```
///
/// Same command as SAVETIF but the output file is saved in 32-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression
///
/// Links: :ref:`savetif <savetif>`
///
#[derive(Builder)]
pub struct Savetif32 {
    // #[builder(start_fn, into)]
    // filename: String
}

impl Command for Savetif32 {
    fn name() -> &'static str {
        "savetif32"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
