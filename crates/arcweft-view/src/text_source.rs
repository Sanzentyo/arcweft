//! Unified View text source table.
//!
//! Public View DSL exposes one `Text` view. The retained renderer-facing
//! source table distinguishes plain strings, localized text keys, rich text
//! documents, and dialogue display-frame projections without exposing separate
//! public `Text` and `RichText` view views.

use crate::{TextSourceId, ViewError};
use arcweft_id::{PublicId, TextKey};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewTextSource {
    Plain(String),
    Localized(TextKey),
    RichTextDocument(ViewRichTextHandle),
    DisplayFrame(ViewRichTextHandle),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewRichTextHandle {
    id: PublicId,
    revision: u64,
    range: Option<ViewTextByteRange>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewTextByteRange {
    start: u32,
    end: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewTextSourceTable {
    sources: BTreeMap<TextSourceId, ViewTextSource>,
    next: u32,
}

impl ViewTextSource {
    pub fn plain(text: impl Into<String>) -> Self {
        Self::Plain(text.into())
    }

    pub const fn localized(key: TextKey) -> Self {
        Self::Localized(key)
    }

    pub const fn rich_text_document(handle: ViewRichTextHandle) -> Self {
        Self::RichTextDocument(handle)
    }

    pub const fn display_frame(handle: ViewRichTextHandle) -> Self {
        Self::DisplayFrame(handle)
    }
}

impl ViewRichTextHandle {
    pub const fn new(id: PublicId, revision: u64) -> Self {
        Self {
            id,
            revision,
            range: None,
        }
    }

    #[must_use]
    pub const fn with_range(mut self, range: ViewTextByteRange) -> Self {
        self.range = Some(range);
        self
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn range(&self) -> Option<ViewTextByteRange> {
        self.range
    }
}

impl ViewTextByteRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> u32 {
        self.start
    }

    pub const fn end(self) -> u32 {
        self.end
    }
}

impl ViewTextSourceTable {
    pub fn insert(&mut self, source: ViewTextSource) -> Result<TextSourceId, ViewError> {
        let id = TextSourceId(self.next);
        self.next = self
            .next
            .checked_add(1)
            .ok_or(ViewError::CapacityExceeded)?;
        self.insert_with_id(id, source)?;
        Ok(id)
    }

    pub fn insert_with_id(
        &mut self,
        id: TextSourceId,
        source: ViewTextSource,
    ) -> Result<(), ViewError> {
        if self.sources.insert(id, source).is_some() {
            return Err(ViewError::CapacityExceeded);
        }
        self.next = self.next.max(id.0.saturating_add(1));
        Ok(())
    }

    pub fn get(&self, id: TextSourceId) -> Option<&ViewTextSource> {
        self.sources.get(&id)
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}
