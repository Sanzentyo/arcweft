use crate::style::{TextLayoutStyle, TextOrientation, WritingMode};

/// Unicode Vertical_Orientation property values used by the layout pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VerticalOrientation {
    Upright,
    Rotated,
    TransformedUpright,
    TransformedRotated,
}

/// Final orientation after style policy is applied.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedOrientation {
    Upright,
    SidewaysClockwise,
    SidewaysCounterClockwise,
}

/// A tiny hand-written classifier for the design skeleton.
///
/// Production code should generate this table from Unicode `VerticalOrientation.txt`.
pub fn vertical_orientation(ch: char) -> VerticalOrientation {
    match ch {
        '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}' => match ch {
            'ー' | '。' | '、' | '「' | '」' | '『' | '』' | '（' | '）' => {
                VerticalOrientation::TransformedUpright
            }
            _ => VerticalOrientation::Upright,
        },
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}' => {
            VerticalOrientation::Upright
        }
        'Ａ'..='Ｚ' | 'ａ'..='ｚ' | '０'..='９' => VerticalOrientation::Upright,
        '!'..='~' => VerticalOrientation::Rotated,
        _ if ch.is_ascii() => VerticalOrientation::Rotated,
        _ => VerticalOrientation::Upright,
    }
}

pub fn resolve_orientation(
    writing_mode: WritingMode,
    text_orientation: TextOrientation,
    vertical: VerticalOrientation,
) -> ResolvedOrientation {
    if !writing_mode.is_vertical() {
        return ResolvedOrientation::Upright;
    }

    match text_orientation {
        TextOrientation::Upright => ResolvedOrientation::Upright,
        TextOrientation::Sideways => ResolvedOrientation::SidewaysClockwise,
        TextOrientation::Mixed => match vertical {
            VerticalOrientation::Upright | VerticalOrientation::TransformedUpright => {
                ResolvedOrientation::Upright
            }
            VerticalOrientation::Rotated | VerticalOrientation::TransformedRotated => {
                ResolvedOrientation::SidewaysClockwise
            }
        },
    }
}

pub fn resolve_for_char(ch: char, style: TextLayoutStyle) -> ResolvedOrientation {
    resolve_orientation(
        style.writing_mode,
        style.text_orientation,
        vertical_orientation(ch),
    )
}

#[cfg(test)]
mod tests {
    use super::{ResolvedOrientation, VerticalOrientation, resolve_for_char, vertical_orientation};
    use crate::style::{TextLayoutStyle, WritingMode};

    #[test]
    fn ascii_is_sideways_in_vertical_mixed() {
        let style = TextLayoutStyle {
            writing_mode: WritingMode::VerticalRl,
            ..TextLayoutStyle::default()
        };
        assert_eq!(resolve_for_char('A', style), ResolvedOrientation::SidewaysClockwise);
    }

    #[test]
    fn kana_is_upright_and_prolonged_sound_mark_requires_transform() {
        assert_eq!(vertical_orientation('あ'), VerticalOrientation::Upright);
        assert_eq!(vertical_orientation('ー'), VerticalOrientation::TransformedUpright);
    }
}
