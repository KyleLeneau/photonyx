use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setcompress 0/1 [-type=] [q]
/// ```
///
/// Defines if images are compressed or not.
///
/// **0** means no compression while **1** enables compression.
/// If compression is enabled, the type must be explicitly written in the option **-type=** ("rice", "gzip1", "gzip2").
/// Associated to the compression, the quantization value must be within [0, 256] range.
///
/// For example, "setcompress 1 -type=rice 16" sets the rice compression with a quantization of 16
///
#[derive(Builder)]
pub struct Setcompress {}

impl Command for Setcompress {
    fn name() -> &'static str {
        "setcompress"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
