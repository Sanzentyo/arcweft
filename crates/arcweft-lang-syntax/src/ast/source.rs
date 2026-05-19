use super::{EntityRef, Pattern, Stmt, TextRange, Visibility};
use crate::expr::Expr;
use crate::types::TypeRef;

/// Declarative `source` stream declaration.
///
/// Source declarations are syntax-only at this layer. They preserve the source
/// id or function-like name plus parsed policy/event statements so HIR and
/// later semantic passes do not need to reparse the body text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceItem {
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    name: Option<String>,
    signature_tail: String,
    source_ty: Option<TypeRef>,
    headers: Vec<SourceHeader>,
    handlers: Vec<SourceHandler>,
    body: String,
    body_statements: Vec<Stmt>,
    range: TextRange,
}

pub(crate) struct SourceItemParts {
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: Option<String>,
    pub(crate) signature_tail: String,
    pub(crate) source_ty: Option<TypeRef>,
    pub(crate) headers: Vec<SourceHeader>,
    pub(crate) handlers: Vec<SourceHandler>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) range: TextRange,
}

/// Policy/header entry in a declarative `source` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceHeader {
    From(Expr),
    Backpressure(SourceBackpressurePolicy),
    Replay(SourceReplayPolicy),
    Privacy(SourcePrivacyPolicy),
    Raw(String),
}

/// Backpressure policy syntax preserved from a `source` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBackpressurePolicy {
    Latest,
    Bounded {
        capacity: Expr,
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
    Raw(String),
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

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub const fn source_ty(&self) -> Option<&TypeRef> {
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
