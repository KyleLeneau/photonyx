use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// merge_cfa file_CFA0 file_CFA1 file_CFA2 file_CFA3 bayerpattern
/// ```
///
/// Builds a Bayer masked color image from 4 separate images containing the data from Bayer subchannels CFA0, CFA1, CFA2 and CFA3. (The corresponding command to split the CFA pattern into subchannels is **split_cfa**.) This function can be used as part of a workflow applying some processing to the individual Bayer subchannels prior to demosaicing. The fifth parameter **bayerpattern** specifies the Bayer matrix pattern to recreate: **bayerpattern** should be one of 'RGGB', 'BGGR', 'GRBG' or 'GBRG'
///
#[derive(Builder)]
pub struct MergeCfa {}

impl Command for MergeCfa {
    fn name() -> &'static str {
        "merge_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
