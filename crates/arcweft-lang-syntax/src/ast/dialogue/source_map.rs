//! Provenance between normalized dialogue content and authored source bytes.

use crate::ast::common::TextRange;

/// Provenance from normalized dialogue-content byte ranges to document bytes.
///
/// Dialogue content removes indentation and normalizes physical line endings to
/// `\n`. Tokens therefore use byte ranges relative to
/// [`DialogueContent::raw`](super::DialogueContent::raw),
/// and this map is the single boundary for projecting those ranges back into
/// the authored `.arcw` document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContentSourceMap {
    segments: Vec<DialogueContentSourceSegment>,
    content_len: usize,
    source_anchor: usize,
}

/// One monotonic piece of dialogue-content source provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueContentSourceSegment {
    content_range: TextRange,
    source_range: TextRange,
    kind: DialogueContentSourceSegmentKind,
}

/// How a dialogue-content provenance segment was produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueContentSourceSegmentKind {
    /// Bytes copied unchanged from the authored document.
    Copied,
    /// One normalized `\n` representing an authored line boundary, including
    /// removed surrounding indentation and an LF or CRLF terminator.
    NormalizedNewline,
}

impl DialogueContentSourceMap {
    pub(crate) fn new(
        segments: Vec<DialogueContentSourceSegment>,
        content_len: usize,
        source_anchor: usize,
    ) -> Self {
        debug_assert!(segments.iter().all(|segment| {
            segment.content_range.start() <= segment.content_range.end()
                && segment.source_range.start() <= segment.source_range.end()
                && match segment.kind {
                    DialogueContentSourceSegmentKind::Copied => {
                        segment.content_range.end() - segment.content_range.start()
                            == segment.source_range.end() - segment.source_range.start()
                    }
                    DialogueContentSourceSegmentKind::NormalizedNewline => {
                        segment.content_range.end() - segment.content_range.start() == 1
                    }
                }
        }));
        debug_assert!(
            segments
                .windows(2)
                .all(|pair| pair[0].content_range.end() == pair[1].content_range.start())
        );
        debug_assert!(
            segments
                .last()
                .is_none_or(|segment| segment.content_range.end() == content_len)
        );
        Self {
            segments,
            content_len,
            source_anchor,
        }
    }

    pub(crate) fn identity(content_len: usize, source_start: usize) -> Self {
        let segments = (content_len > 0)
            .then(|| {
                DialogueContentSourceSegment::copied(
                    TextRange::new(0, content_len),
                    TextRange::new(source_start, source_start + content_len),
                )
            })
            .into_iter()
            .collect();
        Self::new(segments, content_len, source_start)
    }

    /// Ordered provenance segments covering the normalized content.
    pub fn segments(&self) -> &[DialogueContentSourceSegment] {
        &self.segments
    }

    /// Normalized dialogue-content byte length covered by this map.
    pub const fn content_len(&self) -> usize {
        self.content_len
    }

    /// Authored insertion point used when the dialogue content is empty.
    pub const fn source_anchor(&self) -> usize {
        self.source_anchor
    }

    /// Projects one content-relative range into the original document.
    pub fn source_range(&self, relative: TextRange) -> Option<TextRange> {
        if relative.start() > relative.end() || relative.end() > self.content_len {
            return None;
        }
        let start = self.source_offset(relative.start())?;
        let end = self.source_offset(relative.end())?;
        (start <= end).then(|| TextRange::new(start, end))
    }

    /// Maps one authored document offset into normalized content space.
    pub fn content_offset(&self, source_offset: usize) -> Option<usize> {
        if self.segments.is_empty() {
            return (source_offset == self.source_anchor).then_some(0);
        }
        self.segments
            .iter()
            .find_map(|segment| segment.content_offset(source_offset))
    }

    pub(crate) fn slice(&self, relative: TextRange) -> Option<Self> {
        let projected = self.source_range(relative)?;
        let mut segments = Vec::new();
        for segment in &self.segments {
            let start = segment.content_range.start().max(relative.start());
            let end = segment.content_range.end().min(relative.end());
            if start >= end {
                continue;
            }
            let source_start = segment.source_offset(start)?;
            let source_end = segment.source_offset(end)?;
            segments.push(DialogueContentSourceSegment {
                content_range: TextRange::new(start - relative.start(), end - relative.start()),
                source_range: TextRange::new(source_start, source_end),
                kind: segment.kind,
            });
        }
        Some(Self::new(
            segments,
            relative.end() - relative.start(),
            projected.start(),
        ))
    }

    fn source_offset(&self, content_offset: usize) -> Option<usize> {
        if content_offset > self.content_len {
            return None;
        }
        if self.segments.is_empty() {
            return (content_offset == 0).then_some(self.source_anchor);
        }
        self.segments
            .iter()
            .find_map(|segment| segment.source_offset(content_offset))
            .or_else(|| {
                (content_offset == self.content_len)
                    .then(|| {
                        self.segments
                            .last()
                            .map(|segment| segment.source_range.end())
                    })
                    .flatten()
            })
    }
}

impl DialogueContentSourceSegment {
    pub(crate) const fn copied(content_range: TextRange, source_range: TextRange) -> Self {
        Self {
            content_range,
            source_range,
            kind: DialogueContentSourceSegmentKind::Copied,
        }
    }

    pub(crate) const fn normalized_newline(
        content_range: TextRange,
        source_range: TextRange,
    ) -> Self {
        Self {
            content_range,
            source_range,
            kind: DialogueContentSourceSegmentKind::NormalizedNewline,
        }
    }

    /// Covered range in [`DialogueContent::raw`](super::DialogueContent::raw).
    pub const fn content_range(&self) -> TextRange {
        self.content_range
    }

    /// Corresponding authored document range.
    pub const fn source_range(&self) -> TextRange {
        self.source_range
    }

    /// Whether this segment copied bytes or normalized a line boundary.
    pub const fn kind(&self) -> DialogueContentSourceSegmentKind {
        self.kind
    }

    fn source_offset(&self, content_offset: usize) -> Option<usize> {
        if content_offset < self.content_range.start() || content_offset > self.content_range.end()
        {
            return None;
        }
        match self.kind {
            DialogueContentSourceSegmentKind::Copied => {
                Some(self.source_range.start() + content_offset - self.content_range.start())
            }
            DialogueContentSourceSegmentKind::NormalizedNewline => {
                if content_offset == self.content_range.start() {
                    Some(self.source_range.start())
                } else if content_offset == self.content_range.end() {
                    Some(self.source_range.end())
                } else {
                    None
                }
            }
        }
    }

    fn content_offset(&self, source_offset: usize) -> Option<usize> {
        if source_offset < self.source_range.start() || source_offset > self.source_range.end() {
            return None;
        }
        match self.kind {
            DialogueContentSourceSegmentKind::Copied => {
                Some(self.content_range.start() + source_offset - self.source_range.start())
            }
            DialogueContentSourceSegmentKind::NormalizedNewline => {
                if source_offset == self.source_range.start() {
                    Some(self.content_range.start())
                } else if source_offset == self.source_range.end() {
                    Some(self.content_range.end())
                } else {
                    None
                }
            }
        }
    }
}
