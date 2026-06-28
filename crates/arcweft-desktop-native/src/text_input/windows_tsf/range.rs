use arcweft_presentation::text_index::{TextIndexError, TextIndexSnapshot};
use arcweft_presentation::text_input::{
    TextByteOffset, TextInputSecurityPolicy, TextRange, TextRevision, TextUtf16Offset,
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct TsfAcp(pub i32);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TsfAcpRange {
    start: TsfAcp,
    end: TsfAcp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TsfTextSnapshot {
    revision: TextRevision,
    index: TextIndexSnapshot,
    security: TextInputSecurityPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TsfRangeError {
    NegativeAcp {
        value: i32,
    },
    Unordered {
        start: u32,
        end: u32,
    },
    StaleSnapshot {
        expected: TextRevision,
        actual: TextRevision,
    },
    SecureRedacted,
    InvalidTextIndex(TextIndexError),
}

impl TsfAcpRange {
    pub const fn new(start: TsfAcp, end: TsfAcp) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> TsfAcp {
        self.start
    }

    pub const fn end(self) -> TsfAcp {
        self.end
    }

    pub fn to_utf16_range(self) -> Result<TextRange<TextUtf16Offset>, TsfRangeError> {
        let start = self.start.try_to_utf16_offset()?;
        let end = self.end.try_to_utf16_offset()?;
        if start.get() > end.get() {
            return Err(TsfRangeError::Unordered {
                start: start.get(),
                end: end.get(),
            });
        }
        Ok(TextRange::new(start, end))
    }

    pub fn to_canonical_byte_range(
        self,
        snapshot: &TsfTextSnapshot,
        expected_revision: TextRevision,
    ) -> Result<TextRange<TextByteOffset>, TsfRangeError> {
        snapshot.ensure_revision(expected_revision)?;
        if snapshot.security == TextInputSecurityPolicy::SecureRedacted {
            return Err(TsfRangeError::SecureRedacted);
        }
        snapshot
            .index()
            .byte_range_from_utf16(self.to_utf16_range()?)
            .map_err(TsfRangeError::InvalidTextIndex)
    }
}

impl TsfAcp {
    pub fn try_to_utf16_offset(self) -> Result<TextUtf16Offset, TsfRangeError> {
        u32::try_from(self.0)
            .map(TextUtf16Offset)
            .map_err(|_| TsfRangeError::NegativeAcp { value: self.0 })
    }
}

impl TsfTextSnapshot {
    pub fn plain(revision: TextRevision, text: impl Into<String>) -> Self {
        Self {
            revision,
            index: TextIndexSnapshot::new(text),
            security: TextInputSecurityPolicy::Plain,
        }
    }

    pub fn secure_redacted(revision: TextRevision) -> Self {
        Self {
            revision,
            index: TextIndexSnapshot::new(""),
            security: TextInputSecurityPolicy::SecureRedacted,
        }
    }

    pub const fn revision(&self) -> TextRevision {
        self.revision
    }

    pub fn text(&self) -> &str {
        self.index.as_str()
    }

    pub const fn security(&self) -> TextInputSecurityPolicy {
        self.security
    }

    pub const fn index(&self) -> &TextIndexSnapshot {
        &self.index
    }

    fn ensure_revision(&self, expected: TextRevision) -> Result<(), TsfRangeError> {
        if self.revision == expected {
            Ok(())
        } else {
            Err(TsfRangeError::StaleSnapshot {
                expected,
                actual: self.revision,
            })
        }
    }
}
