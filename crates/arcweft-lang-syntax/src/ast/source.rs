use super::common::{TextRange, Visibility};
use super::flow::{AuthoredExpr, Stmt};
use super::ids::EntityRef;
use super::items::Attribute;
use super::pattern::Pattern;
use crate::types::AuthoredTypeRef;
use thiserror::Error;

/// Declarative `source` stream declaration.
///
/// Source declarations are syntax-only at this layer. They preserve the source
/// id or function-like name plus parsed policy/event statements so HIR and
/// later semantic passes do not need to reparse the body text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    name: Option<String>,
    signature_tail: String,
    source_ty: Option<AuthoredTypeRef>,
    headers: Vec<SourceHeader>,
    handlers: Vec<SourceHandler>,
    body: String,
    body_statements: Vec<Stmt>,
    range: TextRange,
}

pub(crate) struct SourceItemParts {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature_tail: String,
    pub(crate) source_ty: Option<AuthoredTypeRef>,
    pub(crate) headers: Vec<SourceHeader>,
    pub(crate) handlers: Vec<SourceHandler>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) range: TextRange,
}

/// Policy/header entry in a declarative `source` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceHeader {
    From(AuthoredExpr),
    Backpressure {
        policy: SourceBackpressurePolicy,
        range: TextRange,
    },
    Replay {
        policy: SourceReplayPolicy,
        range: TextRange,
    },
    Privacy {
        policy: SourcePrivacyPolicy,
        range: TextRange,
    },
    Raw(String),
}

/// Singular source-header slots collected without reparsing or repeated scans.
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceHeaderInventory<'a> {
    from: Option<&'a AuthoredExpr>,
    backpressure: Option<(&'a SourceBackpressurePolicy, TextRange)>,
    replay: Option<(&'a SourceReplayPolicy, TextRange)>,
    privacy: Option<(&'a SourcePrivacyPolicy, TextRange)>,
}

/// Stable source-header names used by duplicate diagnostics and lowerers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceHeaderKind {
    From,
    Backpressure,
    Replay,
    Privacy,
}

/// Duplicate singular header found while constructing an inventory.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("source header `{}` may appear only once", .kind.as_str())]
pub struct DuplicateSourceHeader {
    kind: SourceHeaderKind,
    second_range: Option<TextRange>,
}

impl SourceHeaderKind {
    /// Canonical source spelling of this header.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::From => "from",
            Self::Backpressure => "backpressure",
            Self::Replay => "replay",
            Self::Privacy => "privacy",
        }
    }
}

impl DuplicateSourceHeader {
    /// Header slot that was authored more than once.
    pub const fn kind(self) -> SourceHeaderKind {
        self.kind
    }

    /// Exact range of the second header value, when retained by the AST.
    pub const fn second_range(self) -> Option<TextRange> {
        self.second_range
    }
}

impl<'a> TryFrom<&'a [SourceHeader]> for SourceHeaderInventory<'a> {
    type Error = DuplicateSourceHeader;

    fn try_from(headers: &'a [SourceHeader]) -> Result<Self, Self::Error> {
        let mut inventory = Self::default();
        for header in headers {
            let duplicate = match header {
                SourceHeader::From(expr) => {
                    inventory
                        .from
                        .replace(expr)
                        .is_some()
                        .then_some(DuplicateSourceHeader {
                            kind: SourceHeaderKind::From,
                            second_range: expr.range(),
                        })
                }
                SourceHeader::Backpressure { policy, range } => inventory
                    .backpressure
                    .replace((policy, *range))
                    .is_some()
                    .then_some(DuplicateSourceHeader {
                        kind: SourceHeaderKind::Backpressure,
                        second_range: Some(*range),
                    }),
                SourceHeader::Replay { policy, range } => inventory
                    .replay
                    .replace((policy, *range))
                    .is_some()
                    .then_some(DuplicateSourceHeader {
                        kind: SourceHeaderKind::Replay,
                        second_range: Some(*range),
                    }),
                SourceHeader::Privacy { policy, range } => inventory
                    .privacy
                    .replace((policy, *range))
                    .is_some()
                    .then_some(DuplicateSourceHeader {
                        kind: SourceHeaderKind::Privacy,
                        second_range: Some(*range),
                    }),
                SourceHeader::Raw(_) => None,
            };
            if let Some(duplicate) = duplicate {
                return Err(duplicate);
            }
        }
        Ok(inventory)
    }
}

impl<'a> SourceHeaderInventory<'a> {
    /// `from` expression, when authored.
    pub const fn from(self) -> Option<&'a AuthoredExpr> {
        self.from
    }

    /// Backpressure policy and exact value range, when authored.
    pub const fn backpressure(self) -> Option<(&'a SourceBackpressurePolicy, TextRange)> {
        self.backpressure
    }

    /// Replay policy and exact value range, when authored.
    pub const fn replay(self) -> Option<(&'a SourceReplayPolicy, TextRange)> {
        self.replay
    }

    /// Privacy policy and exact value range, when authored.
    pub const fn privacy(self) -> Option<(&'a SourcePrivacyPolicy, TextRange)> {
        self.privacy
    }
}

/// Backpressure policy syntax preserved from a `source` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBackpressurePolicy {
    Latest,
    Bounded {
        capacity: Option<Box<AuthoredExpr>>,
        overflow: SourceOverflowPolicy,
    },
    BlockingNotAllowed,
    Raw(String),
}

/// Queue overflow policy syntax preserved from a `source` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceOverflowPolicy {
    DropOldest,
    DropNewest,
    Error,
    Coalesce,
    /// The required `overflow` option was omitted during parser recovery.
    Missing,
    /// An authored overflow spelling that is not part of the language.
    Raw {
        value: String,
        range: Option<TextRange>,
    },
}

/// Replay policy syntax preserved from a `source` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceReplayPolicy {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
    Raw(String),
}

/// Privacy policy syntax preserved from a `source` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourcePrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
    Raw(String),
}

/// Structured source event handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceHandler {
    event: SourceEventPattern,
    body: Vec<Stmt>,
}

/// Source event pattern used by a `source` handler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEventPattern {
    Item(Pattern),
    Error(Pattern),
    Progress(Pattern),
    Disconnected,
    PermissionRevoked,
    End,
    Raw(String),
}

impl SourceItem {
    pub(crate) fn from_parts(parts: SourceItemParts) -> Self {
        Self {
            attrs: parts.attrs,
            visibility: parts.visibility,
            id: parts.id,
            name: parts.name,
            signature_tail: parts.signature_tail,
            source_ty: parts.source_ty,
            headers: parts.headers,
            handlers: parts.handlers,
            body: parts.body,
            body_statements: parts.body_statements,
            range: parts.range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub const fn source_ty(&self) -> Option<&AuthoredTypeRef> {
        self.source_ty.as_ref()
    }

    pub fn headers(&self) -> &[SourceHeader] {
        &self.headers
    }

    pub fn handlers(&self) -> &[SourceHandler] {
        &self.handlers
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl SourceHandler {
    pub(crate) const fn new(event: SourceEventPattern, body: Vec<Stmt>) -> Self {
        Self { event, body }
    }

    pub const fn event(&self) -> &SourceEventPattern {
        &self.event
    }

    pub fn body(&self) -> &[Stmt] {
        &self.body
    }
}
