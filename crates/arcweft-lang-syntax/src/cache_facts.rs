use crate::{ast::flow::AwaitBranchKind, cst::SyntaxKind};

impl SyntaxKind {
    /// Stable compiler-cache spelling used by parse-fact evidence.
    pub const fn cache_fact_tag(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Line => "line",
            Self::Error => "error",
            Self::Whitespace => "whitespace",
            Self::Newline => "newline",
            Self::Comment => "comment",
            Self::DocComment => "doc_comment",
            Self::Ident => "ident",
            Self::Number => "number",
            Self::String => "string",
            Self::EntityRef => "entity_ref",
            Self::Punctuation => "punctuation",
            Self::Text => "text",
        }
    }
}

impl AwaitBranchKind {
    /// Stable compiler-cache spelling used by HIR body fact evidence.
    pub const fn cache_fact_tag(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Error => "error",
            Self::Denied => "denied",
        }
    }
}
