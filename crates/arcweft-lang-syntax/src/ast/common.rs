use core::ops::Range;

/// Half-open byte range in the original source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TextRange {
    start: usize,
    end: usize,
}

/// Arcweft visibility qualifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Visibility {
    Public,
    Crate,
    Super,
}

impl TextRange {
    /// Builds a half-open byte range.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Start byte offset.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset.
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Converts to the standard range type.
    pub fn as_range(&self) -> Range<usize> {
        self.start..self.end
    }
}
