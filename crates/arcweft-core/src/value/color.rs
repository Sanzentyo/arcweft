use serde::{Deserialize, Serialize};

/// One sRGB color stored as four 8-bit channels in red, green, blue, alpha order.
///
/// RGB channels contain sRGB-encoded values. Alpha is linear coverage. This is
/// the runtime representation shared by native execution and AWBC.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeColor {
    rgba: [u8; 4],
}

impl RuntimeColor {
    /// Constructs a color from its sRGB RGBA8 channels.
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            rgba: [red, green, blue, alpha],
        }
    }

    /// Constructs a color from its sRGB RGBA8 channel array.
    #[must_use]
    pub const fn from_rgba8(rgba: [u8; 4]) -> Self {
        Self { rgba }
    }

    /// Returns the sRGB RGBA8 channels in red, green, blue, alpha order.
    #[must_use]
    pub const fn rgba8(self) -> [u8; 4] {
        self.rgba
    }
}
