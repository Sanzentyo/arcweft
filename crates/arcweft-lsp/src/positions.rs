use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{ClientCapabilities, Position, PositionEncodingKind, Range};
use std::sync::Arc;
use thiserror::Error;

/// LSP position encoding selected during initialize.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PositionEncoding {
    /// UTF-16 code units. This is the LSP default.
    #[default]
    Utf16,
    /// UTF-8 byte offsets.
    Utf8,
    /// UTF-32 scalar-value offsets.
    Utf32,
}

/// Source-aware line index for converting Arcweft byte spans to LSP ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    source: Arc<str>,
    starts: Vec<usize>,
    encoding: PositionEncoding,
}

/// Failure to convert an exact LSP position into a UTF-8 byte offset.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CheckedPositionError {
    /// The requested zero-based line does not exist in the snapshot.
    #[error("LSP line {line} is outside the document")]
    LineOutOfBounds { line: u32 },
    /// The requested character exceeds the selected line's authored content.
    #[error("LSP character {character} on line {line} is outside the line")]
    CharacterOutOfBounds { line: u32, character: u32 },
    /// A UTF-8 position points into, rather than between, Unicode scalars.
    #[error("UTF-8 character {character} on line {line} splits a Unicode scalar")]
    SplitUtf8Scalar { line: u32, character: u32 },
    /// A UTF-16 position points between the surrogate units of one scalar.
    #[error("UTF-16 character {character} on line {line} splits a Unicode scalar")]
    SplitUtf16Scalar { line: u32, character: u32 },
    /// Checked offset or code-unit arithmetic overflowed.
    #[error("LSP position arithmetic overflowed")]
    ArithmeticOverflow,
}

impl PositionEncoding {
    /// Selects the strongest encoding supported by both client and server.
    pub fn negotiate(client: &ClientCapabilities) -> Self {
        let encodings = client
            .general
            .as_ref()
            .and_then(|general| general.position_encodings.as_ref())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if encodings
            .iter()
            .any(|encoding| encoding == &PositionEncodingKind::UTF8)
        {
            Self::Utf8
        } else if encodings
            .iter()
            .any(|encoding| encoding == &PositionEncodingKind::UTF32)
        {
            Self::Utf32
        } else {
            Self::Utf16
        }
    }

    /// LSP wire value for the selected encoding.
    pub const fn as_lsp_kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf16 => PositionEncodingKind::UTF16,
            Self::Utf8 => PositionEncodingKind::UTF8,
            Self::Utf32 => PositionEncodingKind::UTF32,
        }
    }
}

impl LineIndex {
    /// Builds a line index for one source snapshot.
    pub fn new(source: impl Into<Arc<str>>, encoding: PositionEncoding) -> Self {
        let source = source.into();
        let starts = std::iter::once(0)
            .chain(
                source
                    .char_indices()
                    .filter_map(|(index, ch)| (ch == '\n').then_some(index + ch.len_utf8())),
            )
            .collect();
        Self {
            source,
            starts,
            encoding,
        }
    }

    /// Original source text for this snapshot.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Position encoding used by this line index.
    pub const fn position_encoding(&self) -> PositionEncoding {
        self.encoding
    }

    /// Converts a byte offset to an LSP position.
    pub fn position_from_byte_offset(&self, offset: usize) -> Position {
        let offset = self.clamp_to_char_boundary(offset.min(self.source.len()));
        let line = self.starts.partition_point(|start| *start <= offset);
        let line = line.saturating_sub(1);
        let line_start = self.starts[line];
        let character = self.character_units(line_start, offset);
        Position::new(saturating_u32(line), character)
    }

    /// Converts an LSP position back to a UTF-8 byte offset.
    pub fn byte_offset_from_position(&self, position: Position) -> usize {
        let line = usize::try_from(position.line).unwrap_or(usize::MAX);
        let Some(&line_start) = self.starts.get(line) else {
            return self.source.len();
        };
        let line_end = self
            .starts
            .get(line.saturating_add(1))
            .copied()
            .unwrap_or(self.source.len());
        let target = usize::try_from(position.character).unwrap_or(usize::MAX);
        self.offset_in_line(line_start, line_end, target)
    }

    /// Converts an LSP position to an exact UTF-8 byte offset without clamping.
    ///
    /// Positions use the encoding negotiated for this index. A position in the
    /// middle of a UTF-8 scalar or UTF-16 surrogate pair is rejected rather
    /// than moved to a nearby boundary.
    pub fn try_byte_offset_from_position(
        &self,
        position: Position,
    ) -> Result<usize, CheckedPositionError> {
        let line =
            usize::try_from(position.line).map_err(|_| CheckedPositionError::ArithmeticOverflow)?;
        let Some(&line_start) = self.starts.get(line) else {
            return Err(CheckedPositionError::LineOutOfBounds {
                line: position.line,
            });
        };
        let line_end = self.authored_line_end(line, line_start);
        let target = usize::try_from(position.character)
            .map_err(|_| CheckedPositionError::ArithmeticOverflow)?;

        match self.encoding {
            PositionEncoding::Utf8 => {
                let line_len = line_end
                    .checked_sub(line_start)
                    .ok_or(CheckedPositionError::ArithmeticOverflow)?;
                if target > line_len {
                    return Err(CheckedPositionError::CharacterOutOfBounds {
                        line: position.line,
                        character: position.character,
                    });
                }
                let offset = line_start
                    .checked_add(target)
                    .ok_or(CheckedPositionError::ArithmeticOverflow)?;
                if !self.source.is_char_boundary(offset) {
                    return Err(CheckedPositionError::SplitUtf8Scalar {
                        line: position.line,
                        character: position.character,
                    });
                }
                Ok(offset)
            }
            PositionEncoding::Utf16 => {
                let mut units = 0usize;
                for (relative_offset, scalar) in self.source[line_start..line_end].char_indices() {
                    if units == target {
                        return line_start
                            .checked_add(relative_offset)
                            .ok_or(CheckedPositionError::ArithmeticOverflow);
                    }
                    let next_units = units
                        .checked_add(scalar.len_utf16())
                        .ok_or(CheckedPositionError::ArithmeticOverflow)?;
                    if target < next_units {
                        return Err(CheckedPositionError::SplitUtf16Scalar {
                            line: position.line,
                            character: position.character,
                        });
                    }
                    units = next_units;
                }
                if units == target {
                    Ok(line_end)
                } else {
                    Err(CheckedPositionError::CharacterOutOfBounds {
                        line: position.line,
                        character: position.character,
                    })
                }
            }
            PositionEncoding::Utf32 => {
                let mut units = 0usize;
                for (relative_offset, _) in self.source[line_start..line_end].char_indices() {
                    if units == target {
                        return line_start
                            .checked_add(relative_offset)
                            .ok_or(CheckedPositionError::ArithmeticOverflow);
                    }
                    units = units
                        .checked_add(1)
                        .ok_or(CheckedPositionError::ArithmeticOverflow)?;
                }
                if units == target {
                    Ok(line_end)
                } else {
                    Err(CheckedPositionError::CharacterOutOfBounds {
                        line: position.line,
                        character: position.character,
                    })
                }
            }
        }
    }

    fn authored_line_end(&self, line: usize, line_start: usize) -> usize {
        let mut end = self
            .starts
            .get(line.saturating_add(1))
            .copied()
            .unwrap_or(self.source.len());
        if end > line_start && self.source.as_bytes().get(end - 1) == Some(&b'\n') {
            end -= 1;
        }
        if end > line_start && self.source.as_bytes().get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        end
    }

    fn offset_in_line(&self, line_start: usize, line_end: usize, target: usize) -> usize {
        let mut units = 0usize;
        for (offset, ch) in self.source[line_start..line_end].char_indices() {
            if units >= target {
                return line_start + offset;
            }
            units = units.saturating_add(match self.encoding {
                PositionEncoding::Utf8 => ch.len_utf8(),
                PositionEncoding::Utf16 => ch.len_utf16(),
                PositionEncoding::Utf32 => 1,
            });
        }
        line_end
    }

    fn character_units(&self, line_start: usize, offset: usize) -> u32 {
        let units = self.source[line_start..offset]
            .chars()
            .map(|ch| match self.encoding {
                PositionEncoding::Utf8 => ch.len_utf8(),
                PositionEncoding::Utf16 => ch.len_utf16(),
                PositionEncoding::Utf32 => 1,
            })
            .sum();
        saturating_u32(units)
    }

    fn clamp_to_char_boundary(&self, mut offset: usize) -> usize {
        while !self.source.is_char_boundary(offset) {
            offset = offset.saturating_sub(1);
        }
        offset
    }
}

impl LspPositionMapper for LineIndex {
    fn range_from_byte_span(&self, start: usize, end: usize) -> Range {
        Range {
            start: self.position_from_byte_offset(start),
            end: self.position_from_byte_offset(end),
        }
    }
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_multiline_utf16_ranges() {
        let index = LineIndex::new("a\n猫b\n", PositionEncoding::Utf16);

        let range = index.range_from_byte_span(2, 6);

        assert_eq!(range.start, Position::new(1, 0));
        assert_eq!(range.end, Position::new(1, 2));
    }

    #[test]
    fn maps_utf16_surrogate_pairs_as_two_code_units() {
        let index = LineIndex::new("a\n😀b\n", PositionEncoding::Utf16);

        assert_eq!(index.position_from_byte_offset(6), Position::new(1, 2));
        assert_eq!(index.byte_offset_from_position(Position::new(1, 2)), 6);
    }

    #[test]
    fn maps_utf8_positions_when_negotiated() {
        let index = LineIndex::new("a\n猫b\n", PositionEncoding::Utf8);

        assert_eq!(index.position_from_byte_offset(5), Position::new(1, 3));
        assert_eq!(index.byte_offset_from_position(Position::new(1, 3)), 5);
    }

    #[test]
    fn maps_utf32_positions_as_unicode_scalars() {
        let index = LineIndex::new("a\n😀猫b\n", PositionEncoding::Utf32);

        assert_eq!(index.position_from_byte_offset(6), Position::new(1, 1));
        assert_eq!(index.position_from_byte_offset(9), Position::new(1, 2));
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 2)),
            Ok(9)
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 4)),
            Err(CheckedPositionError::CharacterOutOfBounds {
                line: 1,
                character: 4,
            })
        );
    }

    #[test]
    fn checked_utf8_positions_preserve_exact_scalar_boundaries() {
        let index = LineIndex::new("a\n猫b\r\n", PositionEncoding::Utf8);

        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 3)),
            Ok(5)
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 1)),
            Err(CheckedPositionError::SplitUtf8Scalar {
                line: 1,
                character: 1,
            })
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 5)),
            Err(CheckedPositionError::CharacterOutOfBounds {
                line: 1,
                character: 5,
            })
        );
    }

    #[test]
    fn checked_utf16_positions_reject_split_surrogate_pairs() {
        let index = LineIndex::new("a\n😀b\n", PositionEncoding::Utf16);

        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 2)),
            Ok(6)
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(1, 1)),
            Err(CheckedPositionError::SplitUtf16Scalar {
                line: 1,
                character: 1,
            })
        );
    }

    #[test]
    fn checked_positions_reject_missing_lines_and_newline_units() {
        let index = LineIndex::new("value\r\n", PositionEncoding::Utf16);

        assert_eq!(
            index.try_byte_offset_from_position(Position::new(0, 5)),
            Ok(5)
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(0, 6)),
            Err(CheckedPositionError::CharacterOutOfBounds {
                line: 0,
                character: 6,
            })
        );
        assert_eq!(
            index.try_byte_offset_from_position(Position::new(2, 0)),
            Err(CheckedPositionError::LineOutOfBounds { line: 2 })
        );
    }
}
