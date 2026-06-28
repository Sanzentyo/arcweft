use crate::model::{HirFlowItem, HirTopLevelDecl};

impl HirTopLevelDecl {
    /// Stable compiler-cache spelling used by HIR body fact evidence.
    pub const fn cache_fact_tag(&self) -> &'static str {
        match self {
            Self::Callable(_) => "callable",
            Self::State(_) => "state",
            Self::Trait(_) => "trait",
            Self::Impl(_) => "impl",
            Self::Enum(_) => "enum",
            Self::EntityDecl(_) => "entity_decl",
            Self::Entry(_) => "entry",
            Self::ExternCapability(_) => "extern_capability",
            Self::ExternMod(_) => "extern_mod",
            Self::DialogueDefaults(_) => "dialogue_defaults",
            Self::Struct(_) => "struct",
            Self::TypeAlias(_) => "type_alias",
            Self::Hook(_) => "hook",
            Self::MemoFn(_) => "memo_fn",
            Self::Proof(_) => "proof",
            Self::TrustedAxiom(_) => "trusted_axiom",
            Self::Test(_) => "test",
            Self::Bench(_) => "bench",
            Self::Parser(_) => "parser",
            Self::Source(_) => "source",
        }
    }
}

impl HirFlowItem {
    /// Stable compiler-cache spelling used by HIR body fact evidence.
    pub const fn cache_fact_tag(&self) -> &'static str {
        match self {
            Self::Stmt(_) => "stmt",
            Self::Dialogue(_) => "dialogue",
            Self::Choice(_) => "choice",
            Self::LetChoice { .. } => "let_choice",
            Self::LetScope { .. } => "let_scope",
            Self::LetLoop { .. } => "let_loop",
            Self::LetAwait { .. } => "let_await",
            Self::Thread(_) => "thread",
            Self::If(_) => "if",
            Self::IfLet(_) => "if_let",
            Self::Match(_) => "match",
            Self::Loop(_) => "loop",
            Self::While(_) => "while",
            Self::WhileLet(_) => "while_let",
            Self::For(_) => "for",
            Self::Select(_) => "select",
            Self::Borrow(_) => "borrow",
            Self::SourceLocale(_) => "source_locale",
            Self::Scope(_) => "scope",
            Self::Include(_) => "include",
            Self::Await(_) => "await",
        }
    }
}
