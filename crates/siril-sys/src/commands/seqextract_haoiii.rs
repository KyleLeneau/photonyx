use bon::Builder;

use crate::{
    ExtractResample,
    commands::{Argument, Command},
};

/// ```text
/// seqextract_HaOIII sequencename [-resample=]
/// ```
///
/// Same command as EXTRACT_HAOIII but for the sequence **sequencename**.
///
/// The output sequences names start with the prefixes "Ha\_" and "OIII\_"
///
#[derive(Builder)]
pub struct SeqextractHaOIII {
    #[builder(start_fn, into)]
    sequence: String,
    resample: Option<ExtractResample>,
}

impl Command for SeqextractHaOIII {
    fn name() -> &'static str {
        "seqextract_HaOIII"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequence),
            Argument::option("resample", self.resample.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_only() {
        let cmd = SeqextractHaOIII::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqextract_HaOIII lights");
    }

    #[test]
    fn resample_ha() {
        let cmd = SeqextractHaOIII::builder("lights")
            .resample(ExtractResample::HA)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqextract_HaOIII lights -resample=ha"
        );
    }

    #[test]
    fn resample_oiii() {
        let cmd = SeqextractHaOIII::builder("lights")
            .resample(ExtractResample::OIII)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqextract_HaOIII lights -resample=oiii"
        );
    }
}
