use bon::Builder;

use crate::{
    UpdateKeyMethod,
    commands::{Argument, Command},
};

/// ```text
/// update_key key value [keycomment]
/// update_key -delete key
/// update_key -modify key newkey
/// update_key -comment comment
/// ```
///
/// Updates FITS keyword. Please note that the validity of **value** is not checked. This verification is the responsibility of the user. It is also possible to delete a key with the **-delete** option in front of the name of the key to be deleted, or to modify the key with the **-modify** option. The latter must be followed by the key to be modified and the new key name. Finally, the **-comment** option, followed by text, adds a comment to the FITS header. Please note that any text containing spaces must be enclosed in double quotation marks
///
#[derive(Builder)]
pub struct UpdateKey {
    #[builder(start_fn)]
    method: UpdateKeyMethod,
}

impl Command for UpdateKey {
    fn name() -> &'static str {
        "update_key"
    }

    fn args(&self) -> Vec<Argument> {
        match &self.method {
            UpdateKeyMethod::Set(key, value, comment) => {
                vec![
                    Argument::positional(key),
                    Argument::positional(value),
                    Argument::positional_option(comment.as_deref()),
                ]
            }
            UpdateKeyMethod::Delete(key) => {
                vec![Argument::flag("delete"), Argument::positional(key)]
            }
            UpdateKeyMethod::Rename(key, new_key) => {
                vec![
                    Argument::flag("modify"),
                    Argument::positional(key),
                    Argument::positional(new_key),
                ]
            }
            UpdateKeyMethod::Comment(comment) => {
                vec![Argument::flag("comment"), Argument::positional(comment)]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_key_and_value() {
        let cmd =
            UpdateKey::builder(UpdateKeyMethod::Set("OBJECT".into(), "M42".into(), None)).build();
        assert_eq!(cmd.to_args_string(), "update_key OBJECT M42");
    }

    #[test]
    fn set_key_value_and_comment() {
        let cmd = UpdateKey::builder(UpdateKeyMethod::Set(
            "OBJECT".into(),
            "M42".into(),
            Some("Target object".into()),
        ))
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "update_key OBJECT M42 'Target object'"
        );
    }

    #[test]
    fn delete_key() {
        let cmd = UpdateKey::builder(UpdateKeyMethod::Delete("OBJECT".into())).build();
        assert_eq!(cmd.to_args_string(), "update_key -delete OBJECT");
    }

    #[test]
    fn rename_key() {
        let cmd =
            UpdateKey::builder(UpdateKeyMethod::Rename("OLDKEY".into(), "NEWKEY".into())).build();
        assert_eq!(cmd.to_args_string(), "update_key -modify OLDKEY NEWKEY");
    }

    #[test]
    fn add_comment() {
        let cmd = UpdateKey::builder(UpdateKeyMethod::Comment("my comment".into())).build();
        assert_eq!(cmd.to_args_string(), "update_key -comment 'my comment'");
    }
}
