use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setmem ratio
/// ```
///
/// Sets a new ratio of used memory on free memory.
///
/// **Ratio** value should be between 0.05 and 2, depending on other activities of the machine. A higher ratio should allow siril to process faster, but setting the ratio of used memory above 1 will require the use of on-disk memory, which is very slow and unrecommended, even sometimes not supported, leading to system crash. A fixed amount of memory can also be set in the generic settings, with SET, instead of a ratio
///
/// Links: :ref:`set <set>`
///
#[derive(Builder)]
pub struct Setmem {
    #[builder(start_fn)]
    ratio: f64,
}

impl Command for Setmem {
    fn name() -> &'static str {
        "setmem"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(format!("{:.2}", self.ratio))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_ratio() {
        let cmd = Setmem::builder(0.05).build();
        assert_eq!(cmd.to_args_string(), "setmem 0.05");
    }

    #[test]
    fn normal_ratio() {
        let cmd = Setmem::builder(0.9).build();
        assert_eq!(cmd.to_args_string(), "setmem 0.90");
    }

    #[test]
    fn high_ratio() {
        let cmd = Setmem::builder(2.0).build();
        assert_eq!(cmd.to_args_string(), "setmem 2.00");
    }
}
