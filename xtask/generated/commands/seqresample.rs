use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqresample sequencename { -scale= | -width= | -height= } [-interp=] [-prefix=]
/// ```
///
/// Scales the sequence given in argument **sequencename**. Only selected images in the sequence are processed.
///
/// The scale factor is specified either by the **-scale=** argument or by setting the output width, height or maximum dimension using the **-width=**, **-height=** or **-maxdim=** options.
///
/// An interpolation method may be specified using the **-interp=** argument followed by one of the methods in the list **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}.. Clamping is applied for cubic and lanczos interpolation.
///
/// The output sequence name starts with the prefix "scaled\_" unless otherwise specified with **-prefix=** option
///
#[derive(Builder)]
pub struct Seqresample {}

impl Command for Seqresample {
    fn name() -> &'static str {
        "seqresample"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
