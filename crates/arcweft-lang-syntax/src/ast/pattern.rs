use crate::expr::Expr;
use crate::types::TypeRef;

use super::EntityRef;

/// Pattern syntax used by `let` and line-plan return destructuring.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Ident(String),
    MutIdent(String),
    Literal(Expr),
    Entity(EntityRef),
    Variant {
        path: Option<String>,
        name: String,
        payload: Option<VariantPatternPayload>,
    },
    Discard,
    Tuple(Vec<Pattern>),
    Record {
        path: Option<String>,
        fields: Vec<RecordPatternField>,
        rest: bool,
    },
    BracketSeq {
        items: Vec<Pattern>,
        rest: Option<String>,
    },
    Whole {
        name: String,
        pattern: Box<Pattern>,
    },
    Typed {
        name: String,
        ty: TypeRef,
    },
    Raw(String),
}

/// One field inside a record/struct pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordPatternField {
    name: String,
    pattern: Pattern,
}

/// Payload attached to an enum variant pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantPatternPayload {
    Tuple(Vec<Pattern>),
    Record {
        fields: Vec<RecordPatternField>,
        rest: bool,
    },
}

impl RecordPatternField {
    pub(crate) fn new(name: impl Into<String>, pattern: Pattern) -> Self {
        Self {
            name: name.into(),
            pattern,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }
}
