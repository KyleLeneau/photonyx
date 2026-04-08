use std::ops::Deref;

use bon::Builder;

use crate::{
    UpdateKeyMethod,
    commands::{Argument, Command},
};

/// ```text
/// sequpdate_key sequencename key value [keycomment]
/// sequpdate_key sequencename -delete key
/// sequpdate_key sequencename -modify key newkey
/// sequpdate_key sequencename -comment comment
/// ```
///
/// Same command as UPDATE_KEY but for the sequence **sequencename**. However, this command won't work on SER sequence
///
/// Links: :ref:`update_key <update_key>`
///
#[derive(Builder)]
pub struct SequpdateKey {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(start_fn)]
    method: UpdateKeyMethod,
}

impl Command for SequpdateKey {
    fn name() -> &'static str {
        "sequpdate_key"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::positional(self.sequence.clone())];

        match &self.method {
            UpdateKeyMethod::Set(key, value, comment) => {
                args.push(Argument::positional(key));
                args.push(Argument::positional(value));
                args.push(Argument::positional_option(comment.as_deref()));
            }
            UpdateKeyMethod::Delete(key) => {
                args.push(Argument::flag("delete"));
                args.push(Argument::positional(key));
            }
            UpdateKeyMethod::Rename(key, new_key) => {
                args.push(Argument::flag("modify"));
                args.push(Argument::positional(key));
                args.push(Argument::positional(new_key));
            }
            UpdateKeyMethod::Comment(comment) => {
                args.push(Argument::flag("comment"));
                args.push(Argument::positional(comment.deref()));
            }
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_key_and_value() {
        let cmd = SequpdateKey::builder(
            "lights",
            UpdateKeyMethod::Set("OBJECT".into(), "M42".into(), None),
        )
        .build();
        assert_eq!(cmd.to_args_string(), "sequpdate_key lights OBJECT M42");
    }

    #[test]
    fn set_key_value_and_comment() {
        let cmd = SequpdateKey::builder(
            "lights",
            UpdateKeyMethod::Set("OBJECT".into(), "M42".into(), Some("Target object".into())),
        )
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "sequpdate_key lights OBJECT M42 'Target object'"
        );
    }

    #[test]
    fn delete_key() {
        let cmd = SequpdateKey::builder("lights", UpdateKeyMethod::Delete("OBJECT".into())).build();
        assert_eq!(cmd.to_args_string(), "sequpdate_key lights -delete OBJECT");
    }

    #[test]
    fn rename_key() {
        let cmd = SequpdateKey::builder(
            "lights",
            UpdateKeyMethod::Rename("OLDKEY".into(), "NEWKEY".into()),
        )
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "sequpdate_key lights -modify OLDKEY NEWKEY"
        );
    }

    #[test]
    fn add_comment() {
        let cmd =
            SequpdateKey::builder("lights", UpdateKeyMethod::Comment("my comment".into())).build();
        assert_eq!(
            cmd.to_args_string(),
            "sequpdate_key lights -comment 'my comment'"
        );
    }

    #[test]
    fn sequence_with_spaces_is_quoted() {
        let cmd =
            SequpdateKey::builder("my lights", UpdateKeyMethod::Delete("OBJECT".into())).build();
        assert_eq!(
            cmd.to_args_string(),
            "sequpdate_key 'my lights' -delete OBJECT"
        );
    }
}
