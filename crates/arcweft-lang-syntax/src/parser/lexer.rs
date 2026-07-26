//! Exact one-pass tokenization for the private lossless document grammar.

use arcweft_source::SourceRange;

use crate::grammar::kinds::SyntaxKind;

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

    /// Lexes one already validated UTF-8 range as an independent fragment
    /// while retaining document-absolute token ranges.
    pub(super) const fn for_range(source: &'a str, range: SourceRange) -> Self {
        Self {
            source,
            cursor: range.start(),
            end: range.end(),
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
        if let Some(len) = raw_string_len(source) {
            return Some((SyntaxKind::RawStringToken, len));
        }
        if first == '"' {
            let (len, closed) = quoted_token(source, '"');
            return Some(if closed {
                (SyntaxKind::StringToken, len)
            } else {
                (
                    SyntaxKind::UnterminatedStringToken,
                    unescaped_recovery_square_close(&source[..len]).unwrap_or(len),
                )
            });
        }
        if first == '\'' {
            return Some(character_or_lifetime(source));
        }
        if first == '@'
            && let Some(len) = entity_reference_len(source)
        {
            return Some((SyntaxKind::EntityReferenceToken, len));
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

fn raw_string_len(source: &str) -> Option<usize> {
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
            return Some(suffix_end);
        }
        search = close_quote + 1;
    }
    Some(source.len())
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

fn character_or_lifetime(source: &str) -> (SyntaxKind, usize) {
    if let Some(len) = character_literal_len(source) {
        return (SyntaxKind::CharacterToken, len);
    }
    let rest = &source[1..];
    let Some(first) = rest.chars().next() else {
        return (SyntaxKind::PunctuationToken, 1);
    };
    if is_identifier_start(first) {
        return (
            SyntaxKind::LifetimeToken,
            1 + take_while(rest, is_identifier_continue),
        );
    }
    (SyntaxKind::PunctuationToken, 1)
}

fn character_literal_len(source: &str) -> Option<usize> {
    let rest = source.strip_prefix('\'')?;
    let first = rest.chars().next()?;
    let content_end = if first != '\\' {
        first.len_utf8()
    } else if let Some(body) = rest.strip_prefix("\\u{") {
        let close = body.find('}')?;
        let digits = &body[..close];
        if digits.is_empty()
            || !digits
                .chars()
                .all(|character| character == '_' || character.is_ascii_hexdigit())
        {
            return None;
        }
        "\\u{".len() + close + 1
    } else if let Some(body) = rest.strip_prefix("\\x") {
        let digits = body.get(..2)?;
        if !digits
            .chars()
            .all(|character| character.is_ascii_hexdigit())
        {
            return None;
        }
        4
    } else {
        '\\'.len_utf8() + rest['\\'.len_utf8()..].chars().next()?.len_utf8()
    };
    (rest.as_bytes().get(content_end) == Some(&b'\'')).then_some(content_end + 2)
}

fn entity_reference_len(source: &str) -> Option<usize> {
    let rest = source.strip_prefix('@')?;
    if let Some(delimited) = rest.strip_prefix('<') {
        return Some(delimited.find('>').map_or_else(
            || 2 + take_while(delimited, |character| !character.is_whitespace()),
            |close| close + 3,
        ));
    }
    let len = take_while(rest, |character| {
        is_identifier_continue(character) || matches!(character, '.' | ':' | '-' | '/')
    });
    (len > 0).then_some(len + 1)
}

fn number_len(source: &str) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = 1;

    if bytes.first() == Some(&b'0')
        && matches!(bytes.get(1), Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O'))
    {
        cursor = 2;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            cursor += 1;
        }
        return cursor;
    }

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
        let exponent = cursor;
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let digits = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            cursor += 1;
        }
        if !bytes[digits..cursor].iter().any(u8::is_ascii_digit) {
            cursor = exponent;
        }
    }
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

fn punctuation_len(source: &str) -> Option<usize> {
    const MULTI: &[&str] = &[
        "..=", "===", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "..", "+=",
        "-=", "*=", "/=", "%=", "**", "|>", "<-",
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

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
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
            | "dialogue"
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
}
