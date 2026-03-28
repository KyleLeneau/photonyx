use bon::Builder;

use crate::commands::{Argument, Command};

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
pub struct UpdateKey {}

impl Command for UpdateKey {
    fn name() -> &'static str {
        "update_key"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
