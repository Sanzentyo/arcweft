/// Layout writing mode. This mirrors the intent already present in Arcweft rich text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
}

impl WritingMode {
    pub const fn is_vertical(self) -> bool {
        matches!(self, Self::VerticalRl | Self::VerticalLr)
    }
}

/// Orientation policy for text inside a vertical writing mode.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TextOrientation {
    #[default]
    Mixed,
    Upright,
    Sideways,
}

/// Inline direction hint before bidi resolution.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum InlineDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

/// Ruby placement hint.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RubyPosition {
    #[default]
    Auto,
    Over,
    Under,
    InterCharacter,
}

/// Text-combine-upright policy.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum TextCombinePolicy {
    None,
    #[default]
    DigitsAuto,
    Digits { max_digits: u8 },
    All,
}

/// Stable, renderer-independent text layout style.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayoutStyle {
    pub writing_mode: WritingMode,
    pub text_orientation: TextOrientation,
    pub direction: InlineDirection,
    pub ruby_position: RubyPosition,
    pub text_combine: TextCombinePolicy,
    pub font_size: f32,
    pub max_inline: f32,
    pub column_gap: f32,
}

impl Default for TextLayoutStyle {
    fn default() -> Self {
        Self {
            writing_mode: WritingMode::HorizontalTb,
            text_orientation: TextOrientation::Mixed,
            direction: InlineDirection::Auto,
            ruby_position: RubyPosition::Auto,
            text_combine: TextCombinePolicy::DigitsAuto,
            font_size: 16.0,
            max_inline: 320.0,
            column_gap: 8.0,
        }
    }
}
