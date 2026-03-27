// @generated start by xtask merge-siril-commands
pub mod autostretch;
pub use autostretch::Autostretch;
pub mod calibrate;
pub use calibrate::Calibrate;
pub mod calibrate_single;
pub use calibrate_single::CalibrateSingle;
pub mod capabilities;
pub use capabilities::Capabilities;
pub mod cd;
pub use cd::Cd;
pub mod close;
pub use close::Close;
pub mod convert;
pub use convert::Convert;
pub mod dumpheader;
pub use dumpheader::Dumpheader;
pub mod exit;
pub use exit::Exit;
pub mod get;
pub use get::Get;
pub mod getref;
pub use getref::Getref;
pub mod load;
pub use load::Load;
pub mod log;
pub use log::Log;
pub mod merge;
pub use merge::Merge;
pub mod mirrorx;
pub use mirrorx::Mirrorx;
pub mod mirrorx_single;
pub use mirrorx_single::MirrorxSingle;
pub mod mirrory;
pub use mirrory::Mirrory;
pub mod platesolve;
pub use platesolve::Platesolve;
pub mod pwd;
pub use pwd::Pwd;
pub mod register;
pub use register::Register;
pub mod requires;
pub use requires::Requires;
pub mod rgbcomp;
pub use rgbcomp::Rgbcomp;
pub mod rmgreen;
pub use rmgreen::Rmgreen;
pub mod satu;
pub use satu::Satu;
pub mod save;
pub use save::Save;
pub mod savebmp;
pub use savebmp::Savebmp;
pub mod savejpg;
pub use savejpg::Savejpg;
pub mod savejxl;
pub use savejxl::Savejxl;
pub mod savepng;
pub use savepng::Savepng;
pub mod savepnm;
pub use savepnm::Savepnm;
pub mod savetif;
pub use savetif::Savetif;
pub mod savetif32;
pub use savetif32::Savetif32;
pub mod savetif8;
pub use savetif8::Savetif8;
pub mod seqapplyreg;
pub use seqapplyreg::SeqApplyReg;
pub mod seqclean;
pub use seqclean::Seqclean;
pub mod seqheader;
pub use seqheader::Seqheader;
pub mod seqplatesolve;
pub use seqplatesolve::Seqplatesolve;
pub mod seqstat;
pub use seqstat::Seqstat;
pub mod seqsubsky;
pub use seqsubsky::SeqSubSky;
pub mod sequpdate_key;
pub use sequpdate_key::SequpdateKey;
pub mod set;
pub use set::Set;
pub mod set16bits;
pub use set16bits::Set16bits;
pub mod set32bits;
pub use set32bits::Set32bits;
pub mod setcompress;
pub use setcompress::Setcompress;
pub mod setcpu;
pub use setcpu::Setcpu;
pub mod setext;
pub use setext::SetExt;
pub mod setmem;
pub use setmem::Setmem;
pub mod setref;
pub use setref::Setref;
pub mod spcc;
pub use spcc::Spcc;
pub mod split;
pub use split::Split;
pub mod split_cfa;
pub use split_cfa::SplitCfa;
pub mod stack;
pub use stack::Stack;
pub mod stat;
pub use stat::Stat;
pub mod subsky;
pub use subsky::Subsky;
pub mod unpurple;
pub use unpurple::Unpurple;
pub mod update_key;
pub use update_key::UpdateKey;
// @generated end by xtask merge-siril-commands

use std::fmt::Display;

pub enum Argument {
    Positional(Option<String>),
    Flag(String, Option<bool>),
    Option(String, Option<String>),
}

impl Argument {
    pub fn positional(value: impl Into<String>) -> Self {
        Argument::Positional(Some(value.into()))
    }

    pub fn positional_option(value: Option<impl Display>) -> Self {
        Argument::Positional(value.map(|v| v.to_string()))
    }

    pub fn flag(key: impl Into<String>) -> Self {
        Argument::Flag(key.into(), Some(true))
    }

    pub fn flag_option(key: impl Into<String>, value: bool) -> Self {
        Argument::Flag(key.into(), Some(value))
    }

    pub fn option(key: impl Into<String>, value: Option<impl Display>) -> Self {
        Argument::Option(key.into(), value.map(|v| v.to_string()))
    }

    fn is_valid(&self) -> bool {
        match self {
            Argument::Positional(value) => value.is_some(),
            Argument::Flag(_, value) => value.is_some_and(|x| x),
            Argument::Option(_, value) => value.is_some(),
        }
    }
}

impl Display for Argument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.is_valid() {
            // Skip
            return Ok(());
        }

        match self {
            Argument::Positional(Some(value)) => {
                if value.contains(" ") {
                    write!(f, "'{}'", value)?;
                } else {
                    write!(f, "{}", value)?;
                }
            }
            Argument::Positional(None) => {}
            Argument::Flag(key, _) => {
                write!(f, "-{}", key)?;
            }
            Argument::Option(key, value) => {
                if let Some(value) = value {
                    if value.contains(" ") {
                        write!(f, "'-{}={}'", key, value)?;
                    } else {
                        write!(f, "-{}={}", key, value)?;
                    }
                }
            }
        }

        Ok(())
    }
}

pub trait Command {
    fn name() -> &'static str;

    fn to_args_string(&self) -> String {
        let mut args = vec![Argument::Positional(Some(Self::name().to_string()))];
        args.extend(self.args());
        let command = args
            .iter()
            .map(|a| a.to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        tracing::info!("command arg string: '{}'", command);
        command
    }

    fn args(&self) -> Vec<Argument>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Argument::Positional ---

    #[test]
    fn positional_renders_plain() {
        assert_eq!(Argument::positional("foo").to_string(), "foo");
    }

    #[test]
    fn positional_with_spaces_is_quoted() {
        assert_eq!(Argument::positional("my file").to_string(), "'my file'");
    }

    #[test]
    fn positional_option_some_renders_value() {
        assert_eq!(Argument::positional_option(Some(1.5_f32)).to_string(), "1.5");
    }

    #[test]
    fn positional_option_none_renders_empty() {
        assert_eq!(Argument::positional_option(Option::<f32>::None).to_string(), "");
    }

    // --- Argument::Flag ---

    #[test]
    fn flag_renders_with_dash() {
        assert_eq!(Argument::flag("v").to_string(), "-v");
    }

    #[test]
    fn flag_option_true_renders_with_dash() {
        assert_eq!(Argument::flag_option("v", true).to_string(), "-v");
    }

    #[test]
    fn flag_option_false_renders_empty() {
        assert_eq!(Argument::flag_option("v", false).to_string(), "");
    }

    // --- Argument::Option ---

    #[test]
    fn option_with_value_renders_as_key_value() {
        assert_eq!(Argument::option("out", Some("result")).to_string(), "-out=result");
    }

    #[test]
    fn option_with_spaced_value_is_quoted() {
        assert_eq!(
            Argument::option("out", Some("my result")).to_string(),
            "'-out=my result'"
        );
    }

    #[test]
    fn option_with_none_renders_empty() {
        assert_eq!(Argument::option("out", Option::<String>::None).to_string(), "");
    }

    // --- Command::to_args_string ---

    struct TestCmd;
    impl Command for TestCmd {
        fn name() -> &'static str {
            "test"
        }
        fn args(&self) -> Vec<Argument> {
            vec![
                Argument::flag_option("v", true),
                Argument::flag_option("q", false),
                Argument::positional("file.fit"),
            ]
        }
    }

    #[test]
    fn to_args_string_prepends_name() {
        assert!(TestCmd.to_args_string().starts_with("test "));
    }

    #[test]
    fn to_args_string_filters_invalid_args() {
        // -q is false so should be excluded
        assert_eq!(TestCmd.to_args_string(), "test -v file.fit");
    }

    struct NoArgsCmd;
    impl Command for NoArgsCmd {
        fn name() -> &'static str {
            "bare"
        }
        fn args(&self) -> Vec<Argument> {
            vec![]
        }
    }

    #[test]
    fn to_args_string_no_args_is_just_name() {
        assert_eq!(NoArgsCmd.to_args_string(), "bare");
    }
}
