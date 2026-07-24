//! Speculative receiver parsing and exact caller-cursor rollback boundary.

use super::grammar::parse_authored;
use super::scan::{DelimiterDepth, first_top_level};
use super::{
    ParsedGenericCallee, ParsedGenericMember, ParsedTypeReceiver, TypeToken, TypeTokenCursor,
    TypeTokenKind,
};
use crate::types::{TypeParseError, TypeRef, TypeRefLexemeKind, TypeRefNodePath};

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;

impl<'tokens, 'source> TypeTokenCursor<'tokens, 'source> {
    /// Validates a borrowed token view without changing the caller cursor.
    pub(crate) fn try_new(
        tokens: &'tokens [TypeToken<'source>],
        index: usize,
    ) -> Result<Self, TypeParseError> {
        if index > tokens.len() {
            return Err(TypeParseError::new("type token cursor is out of bounds"));
        }
        let mut previous_end = None;
        for token in tokens {
            if token.range.start() >= token.range.end()
                || previous_end.is_some_and(|end| end > token.range.start())
            {
                return Err(TypeParseError::at(
                    "syntax.type.invalid_token_range",
                    "type token ranges must be non-empty and source ordered",
                    token.range,
                ));
            }
            previous_end = Some(token.range.end());
        }
        Ok(Self { tokens, index })
    }

    /// Speculatively parses one type receiver. This method consumes `self`, so
    /// `None` and `Err` leave the caller's Pratt cursor unchanged. A caller
    /// commits only `ParsedTypeReceiver::next_index()` after success.
    pub(crate) fn parse_receiver(self) -> Result<Option<ParsedTypeReceiver>, TypeParseError> {
        let Some(separator) = receiver_separator(self.tokens, self.index) else {
            return Ok(None);
        };
        let explicit_syntax = has_generic_tokens(self.tokens, self.index, separator);
        let parsed = match parse_authored(self.tokens, self.index, separator) {
            Ok(parsed) => parsed,
            Err(error) if explicit_syntax => return Err(error),
            Err(_) => return Ok(None),
        };
        let explicit_generic = matches!(
            parsed.value(),
            TypeRef::Generic { .. } | TypeRef::TraitBound(_)
        );
        if matches!(self.tokens[separator].kind, TypeTokenKind::PathSeparator) && !explicit_generic
        {
            return Ok(None);
        }
        let receiver_end = parsed.root_source().whole().end();
        if receiver_end != self.tokens[separator].range.start() {
            if !explicit_syntax {
                return Ok(None);
            }
            return Err(TypeParseError::at(
                "syntax.type.receiver_boundary",
                "type receiver must end at the terminal member separator",
                self.tokens[separator].range,
            ));
        }
        Ok(Some(ParsedTypeReceiver {
            authored: parsed,
            next_index: separator,
            receiver_end,
            explicit_generic,
        }))
    }

    /// Speculatively parses `path::<Args>` immediately before an ordinary
    /// call-open token. A non-turbofish generic such as `a < b > (c)` is not a
    /// generic callee and leaves the caller cursor unchanged.
    pub(crate) fn parse_generic_callee(
        self,
    ) -> Result<Option<ParsedGenericCallee>, TypeParseError> {
        let Some(call_open) = generic_call_open(self.tokens, self.index) else {
            return Ok(None);
        };
        if first_top_level(self.tokens, self.index, call_open, |kind| {
            matches!(kind, TypeTokenKind::Dot)
        })
        .is_some_and(|dot| {
            self.tokens
                .get(dot + 1)
                .is_some_and(|token| matches!(token.kind, TypeTokenKind::Identifier(_)))
        }) {
            return Ok(None);
        }
        let explicit_syntax = has_generic_tokens(self.tokens, self.index, call_open);
        let authored = match parse_authored(self.tokens, self.index, call_open) {
            Ok(authored) => authored,
            Err(error) if explicit_syntax => return Err(error),
            Err(_) => return Ok(None),
        };
        let root = TypeRefNodePath::root();
        let has_turbofish = authored.source().lexemes().iter().any(|lexeme| {
            lexeme.owner() == &root && lexeme.kind() == &TypeRefLexemeKind::TurbofishSeparator
        });
        if !has_turbofish
            || !matches!(
                authored.value(),
                TypeRef::Generic { .. } | TypeRef::TraitBound(_)
            )
        {
            return Ok(None);
        }
        Ok(Some(ParsedGenericCallee {
            authored,
            next_index: call_open,
        }))
    }

    /// Speculatively parses `member<Args>` immediately after a dot selector
    /// and before an ordinary call-open token. The dot has already resolved
    /// the expression ambiguity, so this direct generic spelling is valid only
    /// at this dedicated member boundary.
    pub(crate) fn parse_generic_member(
        self,
    ) -> Result<Option<ParsedGenericMember>, TypeParseError> {
        let Some(call_open) = generic_call_open(self.tokens, self.index) else {
            return Ok(None);
        };
        if !has_generic_tokens(self.tokens, self.index, call_open) {
            return Ok(None);
        }
        let authored = parse_authored(self.tokens, self.index, call_open)?;
        if !matches!(
            authored.value(),
            TypeRef::Generic { .. } | TypeRef::TraitBound(_)
        ) {
            return Ok(None);
        }
        Ok(Some(ParsedGenericMember {
            authored,
            next_index: call_open,
        }))
    }
}

impl ParsedTypeReceiver {
    pub(crate) const fn authored(&self) -> &crate::types::AuthoredTypeRef {
        &self.authored
    }

    pub(crate) fn into_authored(self) -> crate::types::AuthoredTypeRef {
        debug_assert_eq!(
            self.receiver_end(),
            self.authored.root_source().whole().end()
        );
        debug_assert_eq!(
            self.explicit_generic(),
            matches!(
                self.authored.value(),
                TypeRef::Generic { .. } | TypeRef::TraitBound(_)
            )
        );
        self.authored
    }

    pub(crate) const fn next_index(&self) -> usize {
        self.next_index
    }

    pub(crate) const fn receiver_end(&self) -> usize {
        self.receiver_end
    }

    pub(crate) const fn explicit_generic(&self) -> bool {
        self.explicit_generic
    }
}

impl ParsedGenericCallee {
    pub(crate) const fn authored(&self) -> &crate::types::AuthoredTypeRef {
        &self.authored
    }

    pub(crate) const fn next_index(&self) -> usize {
        self.next_index
    }

    pub(crate) fn into_authored(self) -> crate::types::AuthoredTypeRef {
        self.authored
    }
}

impl ParsedGenericMember {
    pub(crate) const fn authored(&self) -> &crate::types::AuthoredTypeRef {
        &self.authored
    }

    pub(crate) const fn next_index(&self) -> usize {
        self.next_index
    }

    pub(crate) fn into_authored(self) -> crate::types::AuthoredTypeRef {
        self.authored
    }
}

fn receiver_separator(tokens: &[TypeToken<'_>], start: usize) -> Option<usize> {
    let mut delimiters = DelimiterDepth::default();
    let mut index = start;
    while index < tokens.len() {
        if delimiters.is_top_level()
            && matches!(
                tokens[index].kind,
                TypeTokenKind::Dot | TypeTokenKind::PathSeparator
            )
            && index + 2 < tokens.len()
            && matches!(tokens[index + 1].kind, TypeTokenKind::Identifier(_))
            && matches!(tokens[index + 2].kind, TypeTokenKind::OpenParen)
        {
            return Some(index);
        }
        if delimiters.is_top_level()
            && matches!(tokens[index].kind, TypeTokenKind::OpenParen)
            && index != start
        {
            return None;
        }
        if !delimiters.advance(tokens[index].kind) {
            return None;
        }
        index += 1;
    }
    None
}

fn generic_call_open(tokens: &[TypeToken<'_>], start: usize) -> Option<usize> {
    let mut delimiters = DelimiterDepth::default();
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if delimiters.is_top_level() && matches!(token.kind, TypeTokenKind::OpenParen) {
            return Some(index);
        }
        if !delimiters.advance(token.kind) {
            return None;
        }
    }
    None
}

fn has_generic_tokens(tokens: &[TypeToken<'_>], start: usize, end: usize) -> bool {
    first_top_level(tokens, start, end, |kind| {
        matches!(kind, TypeTokenKind::OpenAngle)
    })
    .is_some()
}
