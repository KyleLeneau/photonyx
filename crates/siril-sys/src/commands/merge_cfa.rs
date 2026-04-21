use bon::Builder;

use crate::{
    BayerPattern,
    commands::{Argument, Command},
};

/// ```text
/// merge_cfa file_CFA0 file_CFA1 file_CFA2 file_CFA3 bayerpattern
/// ```
///
/// Builds a Bayer masked color image from 4 separate images containing the data from Bayer subchannels CFA0, CFA1, CFA2 and CFA3. (The corresponding command to split the CFA pattern into subchannels is **split_cfa**.) This function can be used as part of a workflow applying some processing to the individual Bayer subchannels prior to demosaicing. The fifth parameter **bayerpattern** specifies the Bayer matrix pattern to recreate: **bayerpattern** should be one of 'RGGB', 'BGGR', 'GRBG' or 'GBRG'
///
#[derive(Builder)]
pub struct MergeCfa {
    #[builder(start_fn, into)]
    cfa0: String,
    #[builder(start_fn, into)]
    cfa1: String,
    #[builder(start_fn, into)]
    cfa2: String,
    #[builder(start_fn, into)]
    cfa3: String,
    #[builder(start_fn)]
    pattern: BayerPattern,
}

impl Command for MergeCfa {
    fn name() -> &'static str {
        "merge_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.cfa0),
            Argument::positional(&self.cfa1),
            Argument::positional(&self.cfa2),
            Argument::positional(&self.cfa3),
            Argument::positional(self.pattern.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_args() {
        let cmd = MergeCfa::builder(
            "cfa0.fit",
            "cfa1.fit",
            "cfa2.fit",
            "cfa3.fit",
            BayerPattern::RGGB,
        )
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "merge_cfa cfa0.fit cfa1.fit cfa2.fit cfa3.fit RGGB"
        );
    }

    #[test]
    fn bggr_pattern() {
        let cmd = MergeCfa::builder(
            "cfa0.fit",
            "cfa1.fit",
            "cfa2.fit",
            "cfa3.fit",
            BayerPattern::BGGR,
        )
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "merge_cfa cfa0.fit cfa1.fit cfa2.fit cfa3.fit BGGR"
        );
    }

    #[test]
    fn files_with_spaces_are_quoted() {
        let cmd = MergeCfa::builder(
            "cfa 0.fit",
            "cfa 1.fit",
            "cfa 2.fit",
            "cfa 3.fit",
            BayerPattern::GRBG,
        )
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "merge_cfa 'cfa 0.fit' 'cfa 1.fit' 'cfa 2.fit' 'cfa 3.fit' GRBG"
        );
    }
}
