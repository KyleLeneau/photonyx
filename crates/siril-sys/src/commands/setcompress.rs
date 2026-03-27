use bon::Builder;
use strum_macros::Display;
use strum_macros::EnumString;

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
pub struct Setcompress {
    #[builder(start_fn)]
    enabled: bool,
    format: Option<CompressionType>,
    quantization: Option<u8>
}

#[derive(Debug, PartialEq, EnumString, Display, Clone)]
#[strum(serialize_all = "lowercase")]
pub enum CompressionType {
    Rice,
    Gzip1,
    Gzip2
}

impl Command for Setcompress {
    fn name() -> &'static str {
        "setcompress"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional((self.enabled as u8).to_string()),
            Argument::option("type", self.format.clone()),
            Argument::positional_option(self.quantization)
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_no_options() {
        let cmd = Setcompress::builder(false).build();
        assert_eq!(cmd.to_args_string(), "setcompress 0");
    }

    #[test]
    fn enabled_no_options() {
        let cmd = Setcompress::builder(true).build();
        assert_eq!(cmd.to_args_string(), "setcompress 1");
    }

    #[test]
    fn enabled_with_rice_and_quantization() {
        let cmd = Setcompress::builder(true)
            .format(CompressionType::Rice)
            .quantization(16)
            .build();
        assert_eq!(cmd.to_args_string(), "setcompress 1 -type=rice 16");
    }

    #[test]
    fn enabled_with_gzip1() {
        let cmd = Setcompress::builder(true)
            .format(CompressionType::Gzip1)
            .build();
        assert_eq!(cmd.to_args_string(), "setcompress 1 -type=gzip1");
    }

    #[test]
    fn enabled_with_gzip2_and_quantization() {
        let cmd = Setcompress::builder(true)
            .format(CompressionType::Gzip2)
            .quantization(32)
            .build();
        assert_eq!(cmd.to_args_string(), "setcompress 1 -type=gzip2 32");
    }
}
