pub mod convert;
pub use convert::Convert;

pub mod setext;
pub use setext::SetExt;

pub mod cd;
pub use cd::Cd;

pub mod load;
pub use load::Load;

pub mod calibrate;
pub use calibrate::Calibrate;

pub mod mirrorx;
pub use mirrorx::Mirrorx;

pub mod register;
pub use register::Register;

pub mod save;
pub use save::Save;

pub mod seqapplyreg;
pub use seqapplyreg::SeqApplyReg;

pub mod seqsubsky;
pub use seqsubsky::SeqSubSky;

pub mod split;
pub use split::Split;

pub mod stack;
pub use stack::Stack;

use std::fmt::Display;

pub enum Argument {
    Positional(String),
    Flag(String, Option<bool>),
    Option(String, Option<String>),
}

impl Argument {
    pub fn positional(value: impl Into<String>) -> Self {
        Argument::Positional(value.into())
    }

    pub fn flag(key: impl Into<String>, value: bool) -> Self {
        Argument::Flag(key.into(), Some(value))
    }

    pub fn option(key: impl Into<String>, value: Option<impl Display>) -> Self {
        Argument::Option(key.into(), value.map(|v| v.to_string()))
    }

    fn is_valid(&self) -> bool {
        match self {
            Argument::Positional(_) => true,
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
            Argument::Positional(value) => {
                if value.contains(" ") {
                    write!(f, "'{}'", value)?;
                } else {
                    write!(f, "{}", value)?;
                }
            }
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
        let mut args = vec![Argument::Positional(Self::name().to_string())];
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

// [x] setext - builder
// [x] setext - command
// [x] set32bits - builder
// [ ] set32bits - command

// [x] convert
// [x] cd
// [x] register
// [x] load
// [x] mirrorx
// [x] calibrate
// [x] save
// [x] seqapplyreg
// [x] stack
// [x] seqsubsky
// [x] split
