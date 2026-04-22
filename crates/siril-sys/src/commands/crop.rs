use bon::Builder;

use crate::{Rect, commands::{Argument, Command}};

/// ```text
/// crop [x y width height]
/// ```
///
/// Crops to a selected area of the loaded image.
///
/// If a selection is active, no further arguments are required. Otherwise, or in scripts, arguments have to be given, with **x** and **y** being the coordinates of the top left corner, and **width** and **height** the size of the selection. Alternatively, the selection can be made using the BOXSELECT command
///
/// Links: :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Crop {
    #[builder(start_fn)]
    rect: Rect,
}

impl Command for Crop {
    fn name() -> &'static str {
        "crop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.rect.x.to_string()),
            Argument::positional(self.rect.y.to_string()),
            Argument::positional(self.rect.width.to_string()),
            Argument::positional(self.rect.height.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    #[test]
    fn crop_with_rect() {
        let cmd = Crop::builder(Rect { x: 10, y: 20, width: 100, height: 200 }).build();
        assert_eq!(cmd.to_args_string(), "crop 10 20 100 200");
    }

    #[test]
    fn crop_zero_origin() {
        let cmd = Crop::builder(Rect { x: 0, y: 0, width: 50, height: 50 }).build();
        assert_eq!(cmd.to_args_string(), "crop 0 0 50 50");
    }
}
