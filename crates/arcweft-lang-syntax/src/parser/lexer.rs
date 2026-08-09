//! Exact one-pass tokenization for the private lossless document grammar.

mod id_ref;
mod lifetime;
mod literal;

use arcweft_source::SourceRange;

pub(super) use id_ref::typed_entity_reference;
pub(super) use lifetime::typed_lifetime_registry_path;
pub(super) use literal::typed_literal;

use crate::grammar::kinds::SyntaxKind;
use crate::name::{is_identifier_continue, is_identifier_start};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LexToken {
    pub(super) kind: SyntaxKind,
    pub(super) range: SourceRange,
}

impl LexToken {
    pub(super) const fn kind(self) -> SyntaxKind {
        self.kind
    }

    pub(super) const fn range(self) -> SourceRange {
        self.range
    }
}

/// One source component of the current lexer-owned literal token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LiteralLexemeComponent {
    part: LiteralLexemePart,
    range: SourceRange,
}

impl LiteralLexemeComponent {
    pub(super) const fn part(self) -> LiteralLexemePart {
        self.part
    }

    pub(super) const fn range(self) -> SourceRange {
        self.range
    }
}

/// Closed lexical role inventory shared by literal semantic owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiteralLexemePart {
    Body,
    Prefix,
    Suffix,
    Unit,
}

fn token_local_range(token: LexToken, start: usize, end: usize) -> SourceRange {
    debug_assert!(start <= end);
    debug_assert!(end <= token.range().end().saturating_sub(token.range().start()));
    SourceRange::new(
        token
            .range()
            .start()
            .checked_add(start)
            .expect("token-local range start remains in the source document"),
        token
            .range()
            .start()
            .checked_add(end)
            .expect("token-local range end remains in the source document"),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlockCommentKind {
    Ordinary,
    Documentation,
}

pub(super) struct DocumentLexer<'a> {
    source: &'a str,
    cursor: usize,
    end: usize,
    block_comment: Option<BlockCommentKind>,
}

impl<'a> DocumentLexer<'a> {
    pub(super) const fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            end: source.len(),
            block_comment: None,
        }
    }

    pub(super) fn lex(mut self) -> Box<[LexToken]> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens.into_boxed_slice()
    }

    fn next_token(&mut self) -> Option<LexToken> {
        if self.cursor == self.end {
            return None;
        }
        let start = self.cursor;
        let rest = &self.source[start..self.end];
        let (kind, len) = if let Some(comment) = self.block_comment {
            self.block_comment_token(rest, comment)
        } else {
            self.regular_token(rest)?
        };
        self.cursor += len;
        Some(LexToken {
            kind,
            range: SourceRange::new(start, self.cursor),
        })
    }

    fn regular_token(&mut self, source: &str) -> Option<(SyntaxKind, usize)> {
        if let Some(len) = newline_len(source) {
            return Some((SyntaxKind::NewlineToken, len));
        }
        let first = source.chars().next()?;
        if first.is_whitespace() {
            return Some((
                SyntaxKind::WhitespaceToken,
                take_while(source, |character| {
                    character.is_whitespace() && !matches!(character, '\r' | '\n')
                }),
            ));
        }
        if source.starts_with("///") || source.starts_with("//!") {
            return Some((SyntaxKind::DocCommentToken, take_until_newline(source)));
        }
        if source.starts_with("//") {
            return Some((SyntaxKind::CommentToken, take_until_newline(source)));
        }
        if source.starts_with("/**") || source.starts_with("/*!") {
            self.block_comment = Some(BlockCommentKind::Documentation);
            return Some(self.block_comment_token(source, BlockCommentKind::Documentation));
        }
        if source.starts_with("/*") {
            self.block_comment = Some(BlockCommentKind::Ordinary);
            return Some(self.block_comment_token(source, BlockCommentKind::Ordinary));
        }
        if let Some((len, closed)) = raw_string_len(source) {
            return Some((
                if closed {
                    SyntaxKind::RawStringToken
                } else {
                    SyntaxKind::UnterminatedStringToken
                },
                len,
            ));
        }
        if first == '"' {
            let (len, closed) = quoted_token(source, '"');
            return Some(if closed {
                let suffix_end = len.saturating_add('c'.len_utf8());
                if source.as_bytes().get(len) == Some(&b'c')
                    && source
                        .get(suffix_end..)
                        .and_then(|tail| tail.chars().next())
                        .is_none_or(|character| !is_identifier_continue(character))
                {
                    (SyntaxKind::CharacterToken, suffix_end)
                } else {
                    (SyntaxKind::StringToken, len)
                }
            } else {
                (
                    SyntaxKind::UnterminatedStringToken,
                    unescaped_recovery_square_close(&source[..len]).unwrap_or(len),
                )
            });
        }
        if first == '\'' {
            return Some((SyntaxKind::LifetimeToken, lifetime_token_len(source)));
        }
        if first == '@' {
            return Some((
                SyntaxKind::EntityReferenceToken,
                entity_reference_len(source),
            ));
        }
        if is_identifier_start(first) {
            let len = take_while(source, is_identifier_continue);
            let spelling = &source[..len];
            return Some((
                if is_keyword(spelling) {
                    SyntaxKind::KeywordToken
                } else {
                    SyntaxKind::IdentifierToken
                },
                len,
            ));
        }
        if first.is_ascii_digit() {
            return Some((SyntaxKind::NumberToken, number_len(source)));
        }
        if let Some(len) = punctuation_len(source) {
            return Some((SyntaxKind::PunctuationToken, len));
        }
        Some((SyntaxKind::TextToken, first.len_utf8()))
    }

    fn block_comment_token(
        &mut self,
        source: &str,
        comment: BlockCommentKind,
    ) -> (SyntaxKind, usize) {
        if let Some(len) = newline_len(source) {
            return (SyntaxKind::NewlineToken, len);
        }
        let close = source.find("*/").map(|index| index + 2);
        let newline = source.find(['\r', '\n']);
        let len = match (close, newline) {
            (Some(close), Some(newline)) if close <= newline => {
                self.block_comment = None;
                close
            }
            (Some(close), None) => {
                self.block_comment = None;
                close
            }
            (_, Some(newline)) => newline,
            (None, None) => source.len(),
        };
        (
            match comment {
                BlockCommentKind::Ordinary => SyntaxKind::CommentToken,
                BlockCommentKind::Documentation => SyntaxKind::DocCommentToken,
            },
            len,
        )
    }
}

const fn newline_len(source: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    match bytes {
        [b'\r', b'\n', ..] => Some(2),
        [b'\r' | b'\n', ..] => Some(1),
        _ => None,
    }
}

fn take_until_newline(source: &str) -> usize {
    source.find(['\r', '\n']).unwrap_or(source.len())
}

fn take_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, character)| predicate(*character))
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0)
}

fn raw_string_len(source: &str) -> Option<(usize, bool)> {
    let bytes = source.as_bytes();
    if bytes.first() != Some(&b'r') {
        return None;
    }
    let mut quote = 1;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - 1;
    let body_start = quote + 1;
    let mut search = body_start;
    while let Some(relative) = source[search..].find('"') {
        let close_quote = search + relative;
        let suffix_end = close_quote + 1 + hashes;
        if suffix_end <= bytes.len()
            && bytes[close_quote + 1..suffix_end]
                .iter()
                .all(|byte| *byte == b'#')
        {
            return Some((suffix_end, true));
        }
        search = close_quote + 1;
    }
    Some((source.len(), false))
}

fn quoted_token(source: &str, delimiter: char) -> (usize, bool) {
    let mut escaped = false;
    for (index, character) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == delimiter {
            return (index + character.len_utf8(), true);
        } else if matches!(character, '\r' | '\n') {
            return (index, false);
        }
    }
    (source.len(), false)
}

/// Leaves an unescaped square close visible to the parser when an unterminated
/// string would otherwise swallow the rest of a dialogue tag and its siblings.
/// A normally closed string remains one token, including any `]` in its body.
fn unescaped_recovery_square_close(source: &str) -> Option<usize> {
    let mut escaped = false;
    source.char_indices().find_map(|(index, character)| {
        if escaped {
            escaped = false;
            return None;
        }
        if character == '\\' {
            escaped = true;
            None
        } else {
            (character == ']').then_some(index)
        }
    })
}

fn entity_reference_len(source: &str) -> usize {
    let rest = source
        .strip_prefix('@')
        .expect("entity-reference dispatch retains its leading marker");
    if let Some(delimited) = rest.strip_prefix('<') {
        let recovery = take_while(delimited, |character| !character.is_whitespace());
        return delimited
            .find('>')
            .filter(|close| *close < recovery)
            .map_or(2 + recovery, |close| close + 3);
    }
    let len = take_while(rest, |character| {
        is_identifier_continue(character) || matches!(character, '.' | ':' | '-' | '/')
    });
    let mut token_len = len + 1;
    // A terminal colon introduces an authored suite or declaration type; it
    // is not part of the entity reference. Colons followed by another ID
    // segment (for example `@asset:.room`) remain inside the token.
    while token_len > '@'.len_utf8() && source.as_bytes().get(token_len - 1) == Some(&b':') {
        token_len -= 1;
    }
    let spelling = &source[..token_len];
    if let Some(reference) = spelling.strip_suffix("...")
        && reference.len() > '@'.len_utf8()
        && reference
            .chars()
            .next_back()
            .is_some_and(|character| !matches!(character, '.' | ':'))
    {
        return reference.len();
    }
    token_len
}

fn number_len(source: &str) -> usize {
    let bytes = source.as_bytes();
    let (_, mut cursor) = number_body_bounds(source);
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'%') {
        cursor += 1;
    }
    cursor
}

fn number_body_bounds(source: &str) -> (usize, usize) {
    let bytes = source.as_bytes();
    if bytes.first() == Some(&b'0')
        && let Some(prefix) = bytes.get(1).copied()
        && matches!(prefix, b'x' | b'X' | b'b' | b'B' | b'o' | b'O')
    {
        let radix = match prefix {
            b'x' | b'X' => 16,
            b'b' | b'B' => 2,
            b'o' | b'O' => 8,
            _ => unreachable!("matched the closed radix-prefix inventory"),
        };
        let mut cursor = 2;
        while bytes
            .get(cursor)
            .is_some_and(|byte| *byte == b'_' || char::from(*byte).is_digit(radix))
        {
            cursor += 1;
        }
        return (2, cursor);
    }

    let mut cursor = 1;
    while bytes
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
    {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') && bytes.get(cursor + 1) != Some(&b'.') {
        cursor += 1;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            cursor += 1;
        }
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            cursor += 1;
        }
    }
    (0, cursor)
}

fn punctuation_len(source: &str) -> Option<usize> {
    const MULTI: &[&str] = &[
        "...", "..=", "===", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "..",
        "+=", "-=", "*=", "/=", "%=", "**", "|>", "<-",
    ];
    MULTI
        .iter()
        .find_map(|punctuation| source.starts_with(punctuation).then_some(punctuation.len()))
        .or_else(|| {
            source.chars().next().and_then(|character| {
                "(){}[]<>,.;:+-*/%=!?&|^~#@"
                    .contains(character)
                    .then_some(character.len_utf8())
            })
        })
}

fn is_recovery_lifetime_continue(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

fn lifetime_token_len(source: &str) -> usize {
    let mut cursor = '\''.len_utf8();
    let bytes = source.as_bytes();
    while cursor < source.len() {
        let tail = &source[cursor..];
        let character = tail
            .chars()
            .next()
            .expect("cursor remains on a source character boundary");
        if is_recovery_lifetime_continue(character) || character == '.' {
            cursor += character.len_utf8();
            continue;
        }
        if bytes.get(cursor) == Some(&b'?') {
            cursor += '?'.len_utf8();
        }
        break;
    }
    cursor
}

fn is_keyword(spelling: &str) -> bool {
    matches!(
        spelling,
        "agent"
            | "action"
            | "activity"
            | "as"
            | "assert"
            | "await"
            | "bench"
            | "break"
            | "callable"
            | "capability"
            | "choice"
            | "character"
            | "close"
            | "continue"
            | "crate"
            | "debug"
            | "defer"
            | "defaults"
            | "else"
            | "ensures"
            | "enum"
            | "entry"
            | "extern"
            | "false"
            | "flow"
            | "for"
            | "fn"
            | "goto"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "lifetime"
            | "layer"
            | "loop"
            | "match"
            | "metric"
            | "mod"
            | "move"
            | "mut"
            | "on"
            | "out"
            | "predicate"
            | "proof"
            | "pub"
            | "requires"
            | "res"
            | "return"
            | "scope"
            | "select"
            | "self"
            | "signal"
            | "source"
            | "state"
            | "struct"
            | "style"
            | "super"
            | "test"
            | "thread"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "view"
            | "wait"
            | "where"
            | "while"
            | "yield"
    )
}

#[cfg(test)]
mod tests {
    use super::DocumentLexer;
    use crate::grammar::kinds::SyntaxKind;

    #[test]
    fn closed_string_keeps_square_close_inside_one_token() {
        let source = r#""a]b""#;
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind(), SyntaxKind::StringToken);
        assert_eq!(&source[tokens[0].range().as_range()], source);
    }

    #[test]
    fn unclosed_string_exposes_unescaped_square_close_for_recovery() {
        let source = r#""unfinished]next"#;
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind(), SyntaxKind::UnterminatedStringToken);
        assert_eq!(&source[tokens[0].range().as_range()], r#""unfinished"#);
        assert_eq!(tokens[1].kind(), SyntaxKind::PunctuationToken);
        assert_eq!(&source[tokens[1].range().as_range()], "]");
        assert_eq!(tokens[2].kind(), SyntaxKind::IdentifierToken);
        assert_eq!(&source[tokens[2].range().as_range()], "next");
    }

    #[test]
    fn unclosed_string_keeps_escaped_square_close_inside_token() {
        let source = r#""unfinished\]next"#;
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind(), SyntaxKind::UnterminatedStringToken);
        assert_eq!(&source[tokens[0].range().as_range()], source);
    }

    #[test]
    fn delimited_entity_reference_is_one_exact_token() {
        let source = "@<source.events>:";
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind(), SyntaxKind::EntityReferenceToken);
        assert_eq!(&source[tokens[0].range().as_range()], "@<source.events>");
        assert_eq!(tokens[1].kind(), SyntaxKind::PunctuationToken);
        assert_eq!(&source[tokens[1].range().as_range()], ":");
    }

    #[test]
    fn unclosed_delimited_entity_reference_stops_at_its_first_recovery_boundary() {
        let source = "@<source.events: Source<Event, Error>";
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens[0].kind(), SyntaxKind::EntityReferenceToken);
        assert_eq!(&source[tokens[0].range().as_range()], "@<source.events:");
        assert!(tokens.iter().any(|token| {
            token.kind() == SyntaxKind::PunctuationToken && &source[token.range().as_range()] == ">"
        }));
    }

    #[test]
    fn entity_reference_suffix_stops_before_an_ordinary_spread_operator() {
        for source in ["@flow.main...", "@flow:..opening..."] {
            let tokens = DocumentLexer::new(source).lex();

            assert_eq!(tokens.len(), 2, "{source}");
            assert_eq!(
                tokens[0].kind(),
                SyntaxKind::EntityReferenceToken,
                "{source}"
            );
            assert_eq!(tokens[1].kind(), SyntaxKind::PunctuationToken, "{source}");
            assert_eq!(&source[tokens[1].range().as_range()], "...", "{source}");
        }

        for source in ["@...", "@...outer.leaf", "@flow:...outer.leaf"] {
            let tokens = DocumentLexer::new(source).lex();

            assert_eq!(tokens.len(), 1, "{source}");
            assert_eq!(
                tokens[0].kind(),
                SyntaxKind::EntityReferenceToken,
                "{source}"
            );
            assert_eq!(&source[tokens[0].range().as_range()], source, "{source}");
        }
    }

    #[test]
    fn lifetime_tokens_retain_valid_and_recoverable_names_as_one_token() {
        for source in [
            "'scene",
            "'9",
            "'a١",
            "'line.focus?",
            "'line..focus",
            "'line.",
            "'",
        ] {
            let tokens = DocumentLexer::new(source).lex();

            assert_eq!(tokens.len(), 1, "{source}");
            assert_eq!(tokens[0].kind(), SyntaxKind::LifetimeToken, "{source}");
            assert_eq!(&source[tokens[0].range().as_range()], source);
        }
    }

    #[test]
    fn ordinary_postfix_question_remains_separate_from_a_value_token() {
        let source = "value?";
        let tokens = DocumentLexer::new(source).lex();

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].kind(), SyntaxKind::IdentifierToken);
        assert_eq!(tokens[1].kind(), SyntaxKind::PunctuationToken);
        assert_eq!(&source[tokens[1].range().as_range()], "?");
    }
}
