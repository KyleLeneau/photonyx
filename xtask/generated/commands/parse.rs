use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// parse str [-r]
/// ```
///
/// Parses the string **str** using the information contained in the header of the image currently loaded. Main purpose of this command is to debug path parsing of header keys which can be used in other commands.
/// Option **-r** specifies the string is to be interpreted in read mode. In read mode, all wildcards defined in string **str** are used to find a file name matching the pattern. Otherwise, default mode is write mode and wildcards, if any, are removed from the string to be parsed.
///
/// If **str** starts with *$def* prefix, it will be recognized as a reserved keyword and looked for in the strings stored in gui_prepro.dark_lib, gui_prepro.flat_lib, gui_prepro.bias_lib or gui_prepro.stack_default for *$defdark*, *$defflat*, *$defbias* or *$defstack* respectively.
/// The keyword *$seqname$* can also be used when a sequence is loaded
///
#[derive(Builder)]
pub struct Parse {}

impl Command for Parse {
    fn name() -> &'static str {
        "parse"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
