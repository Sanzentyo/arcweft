//! Boundary-checked text index snapshots for platform text-input adapters.
//!
//! Native text APIs often report positions in UTF-16 code units. Arcweft text
//! input carries canonical byte ranges, so adapters resolve native offsets
//! through this immutable snapshot before emitting `PlatformTextInputEvent`
//! values.

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
