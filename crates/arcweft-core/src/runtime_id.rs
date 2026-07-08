//! Structured runtime-ID values.
//!
//! Runtime lookup IDs are resolved symbol paths. Source IDs and public/debug
//! labels cross this boundary through explicit constructors. Runtime ID wrappers
//! in `plan` never own source strings such as `flow.main` or public/debug labels
//! such as `flow.chapter.one.main`.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Namespace family attached to source declarations or public/debug labels.
///
/// Canonical runtime lookup IDs do not store this family string. The owning Rust
/// ID type owns the family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdFamily {
    Flow,
    Fragment,
    Entry,
    Line,
    Stream,
    View,
    Asset,
    Pure,
}

/// One validated runtime ID path segment.
///
/// This is intentionally an owned value, not an atom-table index. Runtime plans
/// are small enough that a separate interning table would make the boundary
/// harder to use without buying enough yet.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIdSegment(String);

/// Canonical runtime lookup path.
///
/// This value stores only canonical path segments. It never includes source
/// families such as `flow`/`say` as selector prefixes.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIdPath {
    segments: Vec<RuntimeIdSegment>,
}

/// Anchor for a source-side runtime ID reference before resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdReferenceAnchor {
    /// A fully-qualified project/module root reference.
    Root,
    /// A reference relative to the current declaration scope.
    Current,
    /// A reference relative to an ancestor declaration scope.
    Parent(u16),
}

/// Source-side absolute/relative runtime ID reference.
///
/// Runtime lookup IDs should be resolved before execution. This type is the
/// explicit place for relative selectors to live while parser/HIR/lowering still
/// needs them.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeIdReference {
    anchor: RuntimeIdReferenceAnchor,
    path: RuntimeIdPath,
}

/// Public/debug label deliberately emitted for maps, logs, diagnostics, and
/// AWBC public strings.
///
/// Dots inside a label are label text. Runtime lookup code must not recover a
/// runtime ID by splitting this value.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimePublicLabel(String);

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeIdError {
    #[error("canonical {family} runtime ID is empty")]
    Empty { family: RuntimeIdFamily },
    #[error("canonical {family} runtime ID segment is empty")]
    EmptySegment { family: RuntimeIdFamily },
    #[error("canonical {family} runtime ID contains reserved source-family segment `{segment}`")]
    ReservedFamilySegment {
        family: RuntimeIdFamily,
        segment: String,
    },
    #[error("source entity `{value}` belongs to `{found}`; expected `{expected}`")]
    WrongSourceFamily {
        expected: RuntimeIdFamily,
        found: String,
        value: String,
    },
    #[error("source entity `{value}` is missing a family prefix for {expected}")]
    MissingSourceFamily {
        expected: RuntimeIdFamily,
        value: String,
    },
}

impl RuntimeIdFamily {
    #[must_use]
    pub const fn namespace(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Fragment => "fragment",
            Self::Entry => "entry",
            Self::Line => "say",
            Self::Stream => "stream",
            Self::View => "view",
            Self::Asset => "asset",
            Self::Pure => "pure",
        }
    }

    #[must_use]
    pub fn source_families(self) -> &'static [&'static str] {
        match self {
            Self::Flow => &["flow"],
            Self::Fragment => &["fragment", "frag"],
            Self::Entry => &["entry"],
            Self::Line => &["say", "line"],
            Self::Stream => &["stream"],
            Self::View => &["view"],
            Self::Asset => &["asset"],
            Self::Pure => &["pure"],
        }
    }

    #[must_use]
    pub fn flow_source_families() -> &'static [&'static str] {
        &["flow", "fragment", "frag"]
    }
}

impl fmt::Display for RuntimeIdFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.namespace())
    }
}

impl RuntimeIdSegment {
    pub fn new(family: RuntimeIdFamily, value: &str) -> Result<Self, RuntimeIdError> {
        validate_segment(family, value)?;
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl RuntimeIdPath {
    pub fn from_canonical_str(
        family: RuntimeIdFamily,
        value: &str,
    ) -> Result<Self, RuntimeIdError> {
        if value.is_empty() {
            return Err(RuntimeIdError::Empty { family });
        }
        value
            .split('.')
            .map(|segment| RuntimeIdSegment::new(family, segment))
            .collect::<Result<Vec<_>, _>>()
            .map(|segments| Self { segments })
    }

    pub fn from_source_entity_body(
        expected: RuntimeIdFamily,
        value: &str,
        accepted_families: &[&str],
    ) -> Result<Self, RuntimeIdError> {
        let Some((found, suffix)) = value.split_once('.') else {
            return Err(RuntimeIdError::MissingSourceFamily {
                expected,
                value: value.to_owned(),
            });
        };
        if !accepted_families.contains(&found) {
            return Err(RuntimeIdError::WrongSourceFamily {
                expected,
                found: found.to_owned(),
                value: value.to_owned(),
            });
        }
        Self::from_canonical_str(expected, suffix)
    }

    pub fn from_segments(
        family: RuntimeIdFamily,
        segments: impl IntoIterator<Item = RuntimeIdSegment>,
    ) -> Result<Self, RuntimeIdError> {
        let segments = segments.into_iter().collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(RuntimeIdError::Empty { family });
        }
        Ok(Self { segments })
    }

    #[must_use]
    pub fn segments(&self) -> &[RuntimeIdSegment] {
        &self.segments
    }

    #[must_use]
    pub fn label(&self) -> String {
        self.segments
            .iter()
            .map(RuntimeIdSegment::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl fmt::Display for RuntimeIdPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut segments = self.segments.iter();
        if let Some(first) = segments.next() {
            f.write_str(first.as_str())?;
        }
        for segment in segments {
            f.write_str(".")?;
            f.write_str(segment.as_str())?;
        }
        Ok(())
    }
}

impl RuntimeIdReference {
    #[must_use]
    pub const fn new(anchor: RuntimeIdReferenceAnchor, path: RuntimeIdPath) -> Self {
        Self { anchor, path }
    }

    #[must_use]
    pub const fn anchor(&self) -> RuntimeIdReferenceAnchor {
        self.anchor
    }

    #[must_use]
    pub const fn path(&self) -> &RuntimeIdPath {
        &self.path
    }
}

impl RuntimePublicLabel {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn for_family(family: RuntimeIdFamily, path: &RuntimeIdPath) -> Self {
        Self(format!("{}.{}", family.namespace(), path.label()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RuntimePublicLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_segment(family: RuntimeIdFamily, value: &str) -> Result<(), RuntimeIdError> {
    if value.is_empty() {
        return Err(RuntimeIdError::EmptySegment { family });
    }
    if reserved_family_segment(value) {
        return Err(RuntimeIdError::ReservedFamilySegment {
            family,
            segment: value.to_owned(),
        });
    }
    Ok(())
}

fn reserved_family_segment(value: &str) -> bool {
    matches!(
        value,
        "flow"
            | "fragment"
            | "frag"
            | "entry"
            | "say"
            | "line"
            | "stream"
            | "view"
            | "asset"
            | "pure"
    )
}
