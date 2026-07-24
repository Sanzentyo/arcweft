//! Authoritative token transaction for authored type references.

mod cursor;
mod grammar;
mod lexer;
mod scan;

use crate::ast::common::TextRange;
use crate::types::{AuthoredTypeRef, TypeParseError};

/// One token kind understood by the type grammar.
///
/// Expression parsing borrows identifier payloads from its existing token
/// stream and maps punctuation into this closed vocabulary. Standalone type
/// parsing uses the same vocabulary through the source lexer adapter.
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

/// Immutable cursor over one original parser token transaction.
#[derive(Clone, Copy)]
pub(crate) struct TypeTokenCursor<'tokens, 'source> {
    tokens: &'tokens [TypeToken<'source>],
    index: usize,
}

/// Successful speculative parse of a type-shaped path-member receiver.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedTypeReceiver {
    authored: AuthoredTypeRef,
    next_index: usize,
    receiver_end: usize,
    explicit_generic: bool,
}

/// Successful speculative parse of an ordinary turbofish generic callee.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedGenericCallee {
    authored: AuthoredTypeRef,
    next_index: usize,
}

/// Successful speculative parse of generic arguments attached to a selected
/// value member, such as `value.collect<Vec<T>>()`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedGenericMember {
    authored: AuthoredTypeRef,
    next_index: usize,
}

impl<'source> TypeToken<'source> {
    /// Creates a parser-owned view over an existing lexical token.
    pub(crate) const fn from_parser(kind: TypeTokenKind<'source>, range: TextRange) -> Self {
        Self { kind, range }
    }
}

/// Parses a standalone source fragment through the same token grammar used by
/// expression receiver lookahead.
pub(super) fn parse_source_at(
    source: &str,
    base: usize,
) -> Result<AuthoredTypeRef, TypeParseError> {
    let tokens = lexer::lex_source(source, base)?;
    if tokens.is_empty() {
        return Err(TypeParseError::new("expected type"));
    }
    grammar::parse_authored(&tokens, 0, tokens.len())
}
