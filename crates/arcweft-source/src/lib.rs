use core::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAnchor {
    source: SourceName,
    byte_range: Range<usize>,
    start: Option<SourcePosition>,
    end: Option<SourcePosition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceName {
    Path(String),
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
}

impl SourceAnchor {
    pub fn new(source: SourceName, byte_range: Range<usize>) -> Self {
        Self {
            source,
            byte_range,
            start: None,
            end: None,
        }
    }

    #[must_use]
    pub fn with_positions(mut self, start: SourcePosition, end: SourcePosition) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    pub fn generated() -> Self {
        Self::new(SourceName::Generated, 0..0)
    }

    pub fn source(&self) -> &SourceName {
        &self.source
    }

    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }

    pub fn start(&self) -> Option<SourcePosition> {
        self.start
    }

    pub fn end(&self) -> Option<SourcePosition> {
        self.end
    }
}

impl SourceName {
    pub fn path(value: impl Into<String>) -> Self {
        Self::Path(value.into())
    }
}

impl SourcePosition {
    pub const fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}
