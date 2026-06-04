use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{ClientCapabilities, Position, PositionEncodingKind, Range};
use std::sync::Arc;

/// LSP position encoding selected during initialize.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PositionEncoding {
    /// UTF-16 code units. This is the LSP default.
    #[default]
    Utf16,
    /// UTF-8 byte offsets.
    Utf8,
}

/// Source-aware line index for converting Arcweft byte spans to LSP ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    source: Arc<str>,
    starts: Vec<usize>,
    encoding: PositionEncoding,
}

impl PositionEncoding {
    /// Selects the strongest encoding supported by both client and server.
    pub fn negotiate(client: &ClientCapabilities) -> Self {
        client
            .general
            .as_ref()
            .and_then(|general| general.position_encodings.as_ref())
            .and_then(|encodings| {
                encodings
                    .iter()
                    .any(|encoding| encoding == &PositionEncodingKind::UTF8)
                    .then_some(Self::Utf8)
            })
            .unwrap_or(Self::Utf16)
    }

    /// LSP wire value for the selected encoding.
    pub const fn as_lsp_kind(self) -> PositionEncodingKind {
        match self {
            Self::Utf16 => PositionEncodingKind::UTF16,
            Self::Utf8 => PositionEncodingKind::UTF8,
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

    fn offset_in_line(&self, line_start: usize, line_end: usize, target: usize) -> usize {
        let mut units = 0usize;
        for (offset, ch) in self.source[line_start..line_end].char_indices() {
            if units >= target {
                return line_start + offset;
            }
            units = units.saturating_add(match self.encoding {
                PositionEncoding::Utf8 => ch.len_utf8(),
                PositionEncoding::Utf16 => ch.len_utf16(),
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
    fn maps_utf8_positions_when_negotiated() {
        let index = LineIndex::new("a\n猫b\n", PositionEncoding::Utf8);

        assert_eq!(index.position_from_byte_offset(5), Position::new(1, 3));
        assert_eq!(index.byte_offset_from_position(Position::new(1, 3)), 5);
    }
}
