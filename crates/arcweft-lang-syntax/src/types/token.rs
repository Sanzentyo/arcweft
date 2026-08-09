//! Authoritative token transaction for authored type references.

mod grammar;
mod scan;

use crate::ast::common::TextRange;
use crate::types::{AuthoredTypeRef, TypeParseError};

/// One token kind understood by the type grammar.
///
/// The attached parser borrows identifier payloads from its existing token
/// stream and maps punctuation into this closed vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeTokenKind<'source> {
    Identifier(&'source str),
    Lifetime(&'source str),
    Integer(&'source str),
    Bang,
    Ampersand,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    OpenAngle,
    CloseAngle,
    Comma,
    Dot,
    PathSeparator,
    Colon,
    Equals,
    Pipe,
    ThinArrow,
    Other,
}

/// One borrowed type-grammar token and its exact original source range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeToken<'source> {
    kind: TypeTokenKind<'source>,
    range: TextRange,
}

impl<'source> TypeToken<'source> {
    /// Creates a parser-owned view over an existing lexical token.
    pub(crate) const fn from_parser(kind: TypeTokenKind<'source>, range: TextRange) -> Self {
        Self { kind, range }
    }
}

/// Parses tokens borrowed from an already-active parser transaction without
/// lexing or consulting source text again.
pub(crate) fn parse_tokens(tokens: &[TypeToken<'_>]) -> Result<AuthoredTypeRef, TypeParseError> {
    if tokens.is_empty() {
        return Err(TypeParseError::new("expected type"));
    }
    grammar::parse_authored(tokens, 0, tokens.len())
}
