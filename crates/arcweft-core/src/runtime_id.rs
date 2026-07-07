//! Typed runtime-ID boundary helpers.
//!
//! Runtime plan IDs are canonical lookup keys. Source entity references such as
//! `@flow.main` and public/debug labels such as `flow.main` cross into runtime
//! through explicit constructors instead of stringly reuse.

use crate::plan::{EntryRuntimeId, FlowRuntimeId, RuntimeLineId};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Namespace family attached to syntax declarations or public/debug labels.
///
/// Canonical runtime IDs do not carry this family in their stored string; the
/// Rust boundary type or declaration table owns the family instead.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIdFamily {
    Flow,
    Fragment,
    Entry,
    Line,
    View,
    Asset,
    Pure,
}

impl RuntimeIdFamily {
    #[must_use]
    pub const fn namespace(self) -> &'static str {
        match self {
            Self::Flow => "flow",
            Self::Fragment => "fragment",
            Self::Entry => "entry",
            Self::Line => "line",
            Self::View => "view",
            Self::Asset => "asset",
            Self::Pure => "pure",
        }
    }

    #[must_use]
    pub fn source_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Flow => &["flow."],
            Self::Fragment => &["fragment.", "frag."],
            Self::Entry => &["entry."],
            Self::Line => &["say.", "line."],
            Self::View => &["view."],
            Self::Asset => &["asset."],
            Self::Pure => &["pure."],
        }
    }

    #[must_use]
    pub fn flow_source_prefixes() -> &'static [&'static str] {
        &["flow.", "fragment.", "frag."]
    }
}

impl fmt::Display for RuntimeIdFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.namespace())
    }
}

/// Public/debug label deliberately emitted for maps, logs, diagnostics, and
/// AWBC public strings.
///
/// Dots inside the value are ordinary label text. Only the first component added
/// by [`for_family`](Self::for_family) is a family label, and no lookup code may
/// split a `RuntimePublicLabel` to recover a runtime ID.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimePublicLabel(String);

impl RuntimePublicLabel {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn for_family(family: RuntimeIdFamily, canonical_id: &str) -> Self {
        Self(format!("{}.{canonical_id}", family.namespace()))
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

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeIdError {
    #[error("canonical {family} runtime ID is empty")]
    Empty { family: RuntimeIdFamily },
    #[error("canonical {family} runtime ID `{value}` still contains source family prefix `{prefix}`")]
    CanonicalContainsFamilyPrefix {
        family: RuntimeIdFamily,
        value: String,
        prefix: &'static str,
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

impl FlowRuntimeId {
    /// Builds a canonical flow runtime lookup key.
    ///
    /// This accepts the final runtime key only. Use
    /// [`from_source_entity_body`](Self::from_source_entity_body) at source/HIR
    /// boundaries; do not pass strings like `flow.main` here.
    pub fn canonical(value: impl Into<String>) -> Result<Self, RuntimeIdError> {
        canonical_id(RuntimeIdFamily::Flow, value.into(), RuntimeIdFamily::flow_source_prefixes())
            .map(Self)
    }

    /// Converts a source entity body such as `flow.main`, `fragment.intro`, or
    /// `frag.intro` to one canonical flow runtime lookup key.
    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        source_entity_suffix(RuntimeIdFamily::Flow, value, RuntimeIdFamily::flow_source_prefixes())
            .and_then(|suffix| Self::canonical(suffix.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Flow, &self.0)
    }
}

impl EntryRuntimeId {
    pub fn canonical(value: impl Into<String>) -> Result<Self, RuntimeIdError> {
        canonical_id(
            RuntimeIdFamily::Entry,
            value.into(),
            RuntimeIdFamily::Entry.source_prefixes(),
        )
        .map(Self)
    }

    pub fn from_source_entity_body(value: &str) -> Result<Self, RuntimeIdError> {
        source_entity_suffix(
            RuntimeIdFamily::Entry,
            value,
            RuntimeIdFamily::Entry.source_prefixes(),
        )
        .and_then(|suffix| Self::canonical(suffix.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::for_family(RuntimeIdFamily::Entry, &self.0)
    }
}

impl RuntimeLineId {
    /// Line IDs already use content/source families (`say.*`, `line.*`) as
    /// public content IDs, so this method exposes the stored lookup key without
    /// interpreting dots.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn public_label(&self) -> RuntimePublicLabel {
        RuntimePublicLabel::new(self.0.clone())
    }
}

fn canonical_id(
    family: RuntimeIdFamily,
    value: String,
    forbidden_prefixes: &[&'static str],
) -> Result<String, RuntimeIdError> {
    if value.is_empty() {
        return Err(RuntimeIdError::Empty { family });
    }
    if let Some(prefix) = forbidden_prefixes
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
    {
        return Err(RuntimeIdError::CanonicalContainsFamilyPrefix {
            family,
            value,
            prefix,
        });
    }
    Ok(value)
}

fn source_entity_suffix<'a>(
    expected: RuntimeIdFamily,
    value: &'a str,
    accepted_prefixes: &[&'static str],
) -> Result<&'a str, RuntimeIdError> {
    if let Some(prefix) = accepted_prefixes
        .iter()
        .copied()
        .find(|prefix| value.starts_with(prefix))
    {
        return Ok(&value[prefix.len()..]);
    }
    let Some((found, _)) = value.split_once('.') else {
        return Err(RuntimeIdError::MissingSourceFamily {
            expected,
            value: value.to_owned(),
        });
    };
    Err(RuntimeIdError::WrongSourceFamily {
        expected,
        found: found.to_owned(),
        value: value.to_owned(),
    })
}
