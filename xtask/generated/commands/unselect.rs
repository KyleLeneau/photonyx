use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unselect sequencename from to
/// ```
///
/// Allows easy mass unselection of images in the sequence **sequencename** (from **from** to **to** included). See SELECT
///
/// Links: :ref:`select <select>`
///
#[derive(Builder)]
pub struct Unselect {}

impl Command for Unselect {
    fn name() -> &'static str {
        "unselect"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
