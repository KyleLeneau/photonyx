use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// spcc_list { oscsensor | monosensor | redfilter | greenfilter | bluefilter | oscfilter | osclpf | whiteref }
/// ```
///
/// Print a list of SPCC names available for use to define sensors, filters or white references using the **spcc** command. This command requires an argument to set which list is printed: the options are **oscsensor**, **monosensor**, **redfilter**, **greenfilter**, **bluefilter**, **oscfilter**, **osclpf** or **whiteref**.
/// Note that sensor, filter and white reference names may contain spaces: in this case when using them as arguments to the **spcc** command, the entire argument must be enclosed in quotation marks, for example "-whiteref=Average Spiral Galaxy"
///
/// Links: :ref:`spcc <spcc>`
///
#[derive(Builder)]
pub struct SpccList {}

impl Command for SpccList {
    fn name() -> &'static str {
        "spcc_list"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
