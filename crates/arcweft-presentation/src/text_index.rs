//! Boundary-checked text index snapshots for platform text-input adapters.
//!
//! Native text APIs often report positions in UTF-16 code units. Arcweft text
//! input carries canonical byte ranges, so adapters resolve native offsets
//! through this immutable snapshot before emitting `PlatformTextInputEvent`
//! values. The same snapshot also owns editor movement boundaries so Web,
//! `TSF`, `AppKit`, `Wayland`, `Android`, and `iOS` adapters do not grow
//! independent caret/deletion semantics.

use crate::text_input::{TextByteOffset, TextRange, TextUtf16Offset};
use core::fmt;

/// Immutable text indexing view for one text-input snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextIndexSnapshot {
    text: String,
    boundaries: Vec<TextIndexBoundary>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextIndexBoundary {
    byte: u32,
    utf16: u32,
}

/// Rejection reason for a native offset or range that cannot be represented as
/// a canonical Arcweft byte range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextIndexError {
    TextTooLong {
        byte_len: usize,
        utf16_len: usize,
    },
    Utf16OffsetOutOfBounds {
        offset: TextUtf16Offset,
        len: TextUtf16Offset,
    },
    Utf16OffsetInsideSurrogatePair {
        offset: TextUtf16Offset,
    },
    ByteOffsetOutOfBounds {
        offset: TextByteOffset,
        len: TextByteOffset,
    },
    ByteOffsetInsideCodePoint {
        offset: TextByteOffset,
    },
    InvertedUtf16Range {
        start: TextUtf16Offset,
        end: TextUtf16Offset,
    },
    InvertedByteRange {
        start: TextByteOffset,
        end: TextByteOffset,
    },
}

impl TextIndexSnapshot {
    /// Builds a snapshot and panics if the text cannot fit Arcweft's current
    /// `u32` text-offset model.
    ///
    /// # Panics
    ///
    /// Panics when the input exceeds Arcweft's current `u32` byte or UTF-16
    /// offset range. Use [`Self::try_new`] when indexing externally supplied
    /// or unbounded text.
    pub fn new(text: impl Into<String>) -> Self {
        Self::try_new(text).expect("text-input snapshot exceeds Arcweft u32 offset range")
    }

    /// Builds a snapshot without losing UTF-8/UTF-16 boundary information.
    pub fn try_new(text: impl Into<String>) -> Result<Self, TextIndexError> {
        let text = text.into();
        let utf16_len = text.encode_utf16().count();
        if text.len() > u32::MAX as usize || utf16_len > u32::MAX as usize {
            return Err(TextIndexError::TextTooLong {
                byte_len: text.len(),
                utf16_len,
            });
        }

        let mut boundaries = Vec::with_capacity(text.chars().count().saturating_add(1));
        let mut byte = 0_u32;
        let mut utf16 = 0_u32;
        boundaries.push(TextIndexBoundary { byte, utf16 });
        for ch in text.chars() {
            byte += char_utf8_len_u32(ch);
            utf16 += char_utf16_len_u32(ch);
            boundaries.push(TextIndexBoundary { byte, utf16 });
        }
        Ok(Self { text, boundaries })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn len_bytes(&self) -> TextByteOffset {
        self.boundaries
            .last()
            .map_or(TextByteOffset(0), |boundary| TextByteOffset(boundary.byte))
    }

    pub fn len_utf16(&self) -> TextUtf16Offset {
        self.boundaries
            .last()
            .map_or(TextUtf16Offset(0), |boundary| {
                TextUtf16Offset(boundary.utf16)
            })
    }

    pub fn byte_offset_for_utf16(
        &self,
        offset: TextUtf16Offset,
    ) -> Result<TextByteOffset, TextIndexError> {
        if offset.0 > self.len_utf16().0 {
            return Err(TextIndexError::Utf16OffsetOutOfBounds {
                offset,
                len: self.len_utf16(),
            });
        }
        self.boundaries
            .iter()
            .find(|boundary| boundary.utf16 == offset.0)
            .map(|boundary| TextByteOffset(boundary.byte))
            .ok_or(TextIndexError::Utf16OffsetInsideSurrogatePair { offset })
    }

    pub fn utf16_offset_for_byte(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextUtf16Offset, TextIndexError> {
        if offset.0 > self.len_bytes().0 {
            return Err(TextIndexError::ByteOffsetOutOfBounds {
                offset,
                len: self.len_bytes(),
            });
        }
        self.boundaries
            .iter()
            .find(|boundary| boundary.byte == offset.0)
            .map(|boundary| TextUtf16Offset(boundary.utf16))
            .ok_or(TextIndexError::ByteOffsetInsideCodePoint { offset })
    }

    pub fn byte_range_from_utf16(
        &self,
        range: TextRange<TextUtf16Offset>,
    ) -> Result<TextRange<TextByteOffset>, TextIndexError> {
        if range.start().0 > range.end().0 {
            return Err(TextIndexError::InvertedUtf16Range {
                start: *range.start(),
                end: *range.end(),
            });
        }
        Ok(TextRange::new(
            self.byte_offset_for_utf16(*range.start())?,
            self.byte_offset_for_utf16(*range.end())?,
        ))
    }

    pub fn utf16_range_from_byte(
        &self,
        range: TextRange<TextByteOffset>,
    ) -> Result<TextRange<TextUtf16Offset>, TextIndexError> {
        if range.start().0 > range.end().0 {
            return Err(TextIndexError::InvertedByteRange {
                start: *range.start(),
                end: *range.end(),
            });
        }
        Ok(TextRange::new(
            self.utf16_offset_for_byte(*range.start())?,
            self.utf16_offset_for_byte(*range.end())?,
        ))
    }

    pub fn replace_utf16_range(
        &self,
        range: TextRange<TextUtf16Offset>,
        replacement: &str,
    ) -> Result<Self, TextIndexError> {
        let bytes = self.byte_range_from_utf16(range)?;
        let mut text = String::with_capacity(
            self.text
                .len()
                .saturating_sub((bytes.end().0 - bytes.start().0) as usize)
                .saturating_add(replacement.len()),
        );
        text.push_str(&self.text[..bytes.start().0 as usize]);
        text.push_str(replacement);
        text.push_str(&self.text[bytes.end().0 as usize..]);
        Self::try_new(text)
    }

    pub fn slice_byte_range(
        &self,
        range: TextRange<TextByteOffset>,
    ) -> Result<&str, TextIndexError> {
        let canonical = self.utf16_range_from_byte(range)?;
        let byte_range = self.byte_range_from_utf16(canonical)?;
        Ok(&self.text[byte_range.start().0 as usize..byte_range.end().0 as usize])
    }

    /// Returns byte offsets for every valid UTF-8/UTF-16 boundary, including 0
    /// and the end offset. Consumers that need editing movement should prefer
    /// the grapheme/word methods below instead of interpreting this iterator as
    /// user-visible caret stops.
    pub fn byte_offsets(&self) -> impl Iterator<Item = TextByteOffset> + '_ {
        self.boundaries
            .iter()
            .map(|boundary| TextByteOffset(boundary.byte))
    }

    /// Validates that a byte offset is a canonical character boundary in this
    /// snapshot and returns it unchanged.
    pub fn validate_byte_offset(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.utf16_offset_for_byte(offset).map(|_| offset)
    }

    /// Validates that a byte range is ordered and lies on canonical text
    /// boundaries. Invalid ranges are rejected; they are never silently clamped.
    pub fn validate_byte_range(
        &self,
        range: TextRange<TextByteOffset>,
    ) -> Result<TextRange<TextByteOffset>, TextIndexError> {
        if range.start().0 > range.end().0 {
            return Err(TextIndexError::InvertedByteRange {
                start: *range.start(),
                end: *range.end(),
            });
        }
        self.validate_byte_offset(*range.start())?;
        self.validate_byte_offset(*range.end())?;
        Ok(range)
    }

    /// Returns true when the range is collapsed.
    pub fn range_is_collapsed(&self, range: TextRange<TextByteOffset>) -> bool {
        range.start() == range.end()
    }

    /// Moves to the previous Unicode scalar boundary. This is exposed for native
    /// APIs whose deletion unit is scalar rather than user-visible grapheme.
    pub fn previous_scalar_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        Ok(self
            .byte_offsets()
            .take_while(|candidate| candidate.0 < offset.0)
            .last()
            .unwrap_or(TextByteOffset(0)))
    }

    /// Moves to the next Unicode scalar boundary.
    pub fn next_scalar_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        Ok(self
            .byte_offsets()
            .find(|candidate| candidate.0 > offset.0)
            .unwrap_or_else(|| self.len_bytes()))
    }

    /// Moves to the previous shared Arcweft grapheme boundary. The implementation
    /// is intentionally conservative and groups common combining marks,
    /// variation selectors, regional indicators, emoji skin-tone modifiers, and
    /// zero-width-joiner emoji runs so adapter-specific deletion code does not
    /// split the cases covered by Arcweft fixtures.
    pub fn previous_grapheme_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        Ok(self
            .grapheme_boundaries()
            .into_iter()
            .take_while(|candidate| candidate.0 < offset.0)
            .last()
            .unwrap_or(TextByteOffset(0)))
    }

    /// Moves to the next shared Arcweft grapheme boundary.
    pub fn next_grapheme_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        Ok(self
            .grapheme_boundaries()
            .into_iter()
            .find(|candidate| candidate.0 > offset.0)
            .unwrap_or_else(|| self.len_bytes()))
    }

    /// Moves to the previous word boundary. Word movement is defined over the
    /// shared snapshot, not per adapter. It first skips whitespace/punctuation
    /// left of the caret and then lands at the start of the preceding word-like
    /// run.
    pub fn previous_word_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        let mut last_word_start = TextByteOffset(0);
        let mut in_word = false;
        for (byte, ch) in self.text.char_indices() {
            let current = TextByteOffset(u32::try_from(byte).unwrap_or(u32::MAX));
            if current.0 >= offset.0 {
                break;
            }
            let word = is_word_char(ch);
            if word && !in_word {
                last_word_start = current;
            }
            in_word = word;
        }
        Ok(last_word_start)
    }

    /// Moves to the next word boundary. It first skips the current word-like run
    /// and then skips following non-word separators.
    pub fn next_word_boundary(
        &self,
        offset: TextByteOffset,
    ) -> Result<TextByteOffset, TextIndexError> {
        self.validate_byte_offset(offset)?;
        let mut seen_word = false;
        let mut left_word = false;
        for (byte, ch) in self.text.char_indices() {
            let current = TextByteOffset(u32::try_from(byte).unwrap_or(u32::MAX));
            if current.0 < offset.0 {
                continue;
            }
            let word = is_word_char(ch);
            if word {
                seen_word = true;
                if left_word {
                    return Ok(current);
                }
            } else if seen_word {
                left_word = true;
            }
        }
        Ok(self.len_bytes())
    }

    /// Returns the closest valid byte offset for a horizontal hit-test character
    /// slot. This is a presentation/editor rule, not a native range conversion:
    /// invalid native offsets still use the fallible conversion APIs above.
    pub fn byte_offset_for_grapheme_slot(&self, slot: usize) -> TextByteOffset {
        self.grapheme_boundaries()
            .into_iter()
            .nth(slot)
            .unwrap_or_else(|| self.len_bytes())
    }

    /// Shared grapheme boundaries used by movement, deletion, pointer hit-test,
    /// and deterministic fixture geometry.
    pub fn grapheme_boundaries(&self) -> Vec<TextByteOffset> {
        let mut boundaries = vec![TextByteOffset(0)];
        let mut previous_was_joiner = false;
        let mut regional_indicator_run = 0_u8;
        for (byte, ch) in self.text.char_indices() {
            if byte == 0 {
                previous_was_joiner = ch == '\u{200d}';
                regional_indicator_run = u8::from(is_regional_indicator(ch));
                continue;
            }
            let should_join_previous = previous_was_joiner
                || is_grapheme_extend(ch)
                || is_variation_selector(ch)
                || is_emoji_modifier(ch)
                || (is_regional_indicator(ch) && regional_indicator_run % 2 == 1);
            if !should_join_previous {
                boundaries.push(TextByteOffset(u32::try_from(byte).unwrap_or(u32::MAX)));
            }
            previous_was_joiner = ch == '\u{200d}';
            regional_indicator_run = if is_regional_indicator(ch) {
                regional_indicator_run.saturating_add(1)
            } else {
                0
            };
        }
        let end = self.len_bytes();
        if boundaries.last().copied() != Some(end) {
            boundaries.push(end);
        }
        boundaries
    }
}

const fn char_utf8_len_u32(ch: char) -> u32 {
    match ch.len_utf8() {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        _ => unreachable!(),
    }
}

const fn char_utf16_len_u32(ch: char) -> u32 {
    match ch.len_utf16() {
        1 => 1,
        2 => 2,
        _ => unreachable!(),
    }
}

fn is_word_char(ch: char) -> bool {
    ch == '_'
        || ch.is_alphanumeric()
        || matches!(ch, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{9fff}')
}

fn is_grapheme_extend(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{0483}'..='\u{0489}'
            | '\u{0591}'..='\u{05bd}'
            | '\u{05bf}'
            | '\u{05c1}'..='\u{05c2}'
            | '\u{05c4}'..='\u{05c5}'
            | '\u{05c7}'
            | '\u{0610}'..='\u{061a}'
            | '\u{064b}'..='\u{065f}'
            | '\u{0670}'
            | '\u{06d6}'..='\u{06dc}'
            | '\u{06df}'..='\u{06e4}'
            | '\u{06e7}'..='\u{06e8}'
            | '\u{06ea}'..='\u{06ed}'
            | '\u{0900}'..='\u{0903}'
            | '\u{093a}'
            | '\u{093c}'
            | '\u{0941}'..='\u{0948}'
            | '\u{094d}'
            | '\u{0951}'..='\u{0957}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

fn is_variation_selector(ch: char) -> bool {
    matches!(ch, '\u{fe00}'..='\u{fe0f}' | '\u{e0100}'..='\u{e01ef}')
}

fn is_emoji_modifier(ch: char) -> bool {
    matches!(ch, '\u{1f3fb}'..='\u{1f3ff}')
}

fn is_regional_indicator(ch: char) -> bool {
    matches!(ch, '\u{1f1e6}'..='\u{1f1ff}')
}

impl fmt::Display for TextIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextTooLong {
                byte_len,
                utf16_len,
            } => write!(
                f,
                "text-input snapshot too long: {byte_len} bytes, {utf16_len} UTF-16 units"
            ),
            Self::Utf16OffsetOutOfBounds { offset, len } => write!(
                f,
                "UTF-16 offset {offset:?} is outside snapshot length {len:?}"
            ),
            Self::Utf16OffsetInsideSurrogatePair { offset } => {
                write!(f, "UTF-16 offset {offset:?} splits a surrogate pair")
            }
            Self::ByteOffsetOutOfBounds { offset, len } => {
                write!(
                    f,
                    "byte offset {offset:?} is outside snapshot length {len:?}"
                )
            }
            Self::ByteOffsetInsideCodePoint { offset } => {
                write!(f, "byte offset {offset:?} splits a UTF-8 code point")
            }
            Self::InvertedUtf16Range { start, end } => {
                write!(f, "UTF-16 range start {start:?} is after end {end:?}")
            }
            Self::InvertedByteRange { start, end } => {
                write!(f, "byte range start {start:?} is after end {end:?}")
            }
        }
    }
}

impl std::error::Error for TextIndexError {}
