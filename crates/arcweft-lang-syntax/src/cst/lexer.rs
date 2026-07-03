//! CST lexer: tokenizes source into rowan-ready syntax kinds.

use super::SyntaxKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CstToken<'a> {
    kind: SyntaxKind,
    text: &'a str,
    start: usize,
    end: usize,
}

pub(crate) fn lex_cst(source: &str) -> Vec<CstToken<'_>> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    let mut in_block_comment = false;

    while cursor < source.len() {
        let rest = &source[cursor..];
        let (kind, len) = if in_block_comment || rest.starts_with("/*") {
            next_block_comment_token(rest, &mut in_block_comment)
        } else {
            next_token(rest)
        };
        tokens.push(CstToken {
            kind,
            text: &source[cursor..cursor + len],
            start: cursor,
            end: cursor + len,
        });
        cursor += len;
    }

    tokens
}

impl<'a> CstToken<'a> {
    pub(crate) const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    pub(crate) const fn text(&self) -> &'a str {
        self.text
    }

    pub(crate) const fn start(&self) -> usize {
        self.start
    }

    pub(crate) const fn end(&self) -> usize {
        self.end
    }

    pub(crate) fn text_starts_with(&self, value: char) -> bool {
        token_text_is(self.text, value)
    }
}
pub(super) fn token_text_is(text: &str, value: char) -> bool {
    let mut chars = text.chars();
    chars.next() == Some(value) && chars.next().is_none()
}
fn next_token(source: &str) -> (SyntaxKind, usize) {
    if source.starts_with("\r\n") {
        return (SyntaxKind::Newline, 2);
    }

    let mut chars = source.char_indices();
    let Some((_, first)) = chars.next() else {
        return (SyntaxKind::Text, 0);
    };

    if first == '\n' || first == '\r' {
        return (SyntaxKind::Newline, first.len_utf8());
    }

    if first.is_whitespace() {
        return (
            SyntaxKind::Whitespace,
            take_while(source, |ch| ch.is_whitespace() && ch != '\n' && ch != '\r'),
        );
    }

    if source.starts_with("///") {
        return (SyntaxKind::DocComment, take_until_newline(source));
    }

    if source.starts_with("//") {
        return (SyntaxKind::Comment, take_until_newline(source));
    }

    if first == '"' {
        return (SyntaxKind::String, take_string(source));
    }

    if first == '@' && at_starts_entity_ref(source) {
        return (SyntaxKind::EntityRef, take_entity_ref(source));
    }

    if is_ident_start(first) {
        return (SyntaxKind::Ident, take_while(source, is_ident_continue));
    }

    if first.is_ascii_digit() {
        return (
            SyntaxKind::Number,
            take_while(source, |ch| ch.is_ascii_digit() || ch == '.'),
        );
    }

    if is_punctuation(first) {
        return (SyntaxKind::Punctuation, first.len_utf8());
    }

    (SyntaxKind::Text, first.len_utf8())
}

fn next_block_comment_token(source: &str, in_block_comment: &mut bool) -> (SyntaxKind, usize) {
    if source.starts_with("\r\n") {
        return (SyntaxKind::Newline, 2);
    }
    if source.starts_with('\n') || source.starts_with('\r') {
        return (SyntaxKind::Newline, 1);
    }

    let close = source.find("*/");
    let newline = source.find(['\r', '\n']);
    match (close, newline) {
        (Some(close), Some(newline)) if close < newline => {
            *in_block_comment = false;
            (SyntaxKind::Comment, close + "*/".len())
        }
        (Some(close), None) => {
            *in_block_comment = false;
            (SyntaxKind::Comment, close + "*/".len())
        }
        (_, Some(newline)) => {
            *in_block_comment = true;
            (SyntaxKind::Comment, newline)
        }
        (None, None) => {
            *in_block_comment = true;
            (SyntaxKind::Comment, source.len())
        }
    }
}

fn take_until_newline(source: &str) -> usize {
    source.find(['\r', '\n']).unwrap_or(source.len())
}

pub(super) fn take_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| predicate(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn take_string(source: &str) -> usize {
    let mut escaped = false;
    for (index, ch) in source.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return index + ch.len_utf8();
        }
        if ch == '\r' || ch == '\n' {
            return index;
        }
    }
    source.len()
}

fn take_entity_ref(source: &str) -> usize {
    if let Some(after_at) = source.strip_prefix("@<") {
        return after_at
            .find('>')
            .map_or(source.len(), |index| index + "@<>".len());
    }

    1 + source[1..]
        .char_indices()
        .take_while(|(_, ch)| is_entity_ref_char(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn at_starts_entity_ref(source: &str) -> bool {
    source.starts_with("@<")
        || source
            .chars()
            .nth(1)
            .is_some_and(|ch| is_ident_start(ch) && !source.starts_with("@super."))
}

pub(super) fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

pub(super) fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

fn is_entity_ref_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '@' | '/')
}

fn is_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '{' | '}'
            | '('
            | ')'
            | '['
            | ']'
            | ','
            | ':'
            | ';'
            | '.'
            | '='
            | '+'
            | '-'
            | '*'
            | '/'
            | '?'
            | '!'
            | '<'
            | '>'
            | '|'
            | '&'
            | '@'
    )
}
