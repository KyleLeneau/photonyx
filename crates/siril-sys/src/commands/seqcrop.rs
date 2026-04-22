use bon::Builder;

use crate::{Rect, commands::{Argument, Command}};

/// ```text
/// seqcrop sequencename x y width height [-prefix=]
/// ```
///
/// Crops the sequence given in argument **sequencename**. Only selected images in the sequence are processed.
///
/// The crop selection is specified by the upper left corner position **x** and **y** and the selection **width** and **height**, like for CROP.
/// The output sequence name starts with the prefix "cropped\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`crop <crop>`
///
#[derive(Builder)]
pub struct Seqcrop {
    #[builder(start_fn, into)]
    sequencename: String,
    #[builder(start_fn)]
    rect: Rect,
    prefix: Option<String>
}

impl Command for Seqcrop {
    fn name() -> &'static str {
        "seqcrop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequencename),
            Argument::positional(self.rect.x.to_string()),
            Argument::positional(self.rect.y.to_string()),
            Argument::positional(self.rect.width.to_string()),
            Argument::positional(self.rect.height.to_string()),
            Argument::option("prefix", self.prefix.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn minimal() {
        let cmd = Seqcrop::builder("seq", Rect { x: 10, y: 20, width: 100, height: 200 }).build();
        assert_eq!(cmd.to_args_string(), "seqcrop seq 10 20 100 200");
    }

    #[test]
    fn with_prefix() {
        let cmd = Seqcrop::builder("seq", Rect { x: 0, y: 0, width: 50, height: 50 })
            .prefix("out_".to_string())
            .build();
        assert_eq!(cmd.to_args_string(), "seqcrop seq 0 0 50 50 -prefix=out_");
    }
}
