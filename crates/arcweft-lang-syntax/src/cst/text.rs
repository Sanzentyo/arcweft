//! Text-level CST helpers: source lines, doc prefixes, and flat fences.

use super::lexer::{is_ident_continue, lex_cst, take_while};
use super::{CstDocPrefix, FlatFence, SyntaxKind};

/// Iterates source lines without treating line splitting as parser grammar.
///
/// This helper is intentionally small and text-level. Parser modules use it
/// while they are still being migrated to rowan events so line handling is
/// centralized in the CST/text utility layer instead of open-coded at each
/// grammar site.
pub(crate) fn source_line_iter(source: &str) -> impl Iterator<Item = &str> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// Returns non-empty trimmed source lines for interim line-oriented parsing.
pub(crate) fn nonempty_trimmed_source_lines(source: &str) -> Vec<&str> {
    source_line_iter(source)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// Counts source lines using the same text policy as [`source_line_iter`].
pub(crate) fn source_line_count(source: &str) -> usize {
    source_line_iter(source).count()
}
/// Documentation prefix extracted from a text fragment.
impl CstDocPrefix {
    pub(crate) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(crate) const fn consumed(&self) -> usize {
        self.consumed
    }
}

/// Takes leading `///` lines from a parameter fragment.
///
/// Function parameters are parsed from signature fragments rather than full
/// rowan line nodes. Keeping the scan here preserves one source of truth for
/// doc-comment stripping until signatures are fully event-backed.
pub(crate) fn take_doc_comment_prefix(source: &str) -> Option<CstDocPrefix> {
    let mut lines = Vec::new();
    let mut consumed = 0;

    for segment in source.split_inclusive('\n') {
        let line = segment.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed = line.trim();
        let Some(text) = trimmed.strip_prefix("///") else {
            break;
        };
        lines.push(text.strip_prefix(' ').unwrap_or(text).to_owned());
        consumed += segment.len();
    }

    (!lines.is_empty()).then_some(CstDocPrefix { lines, consumed })
}
/// Parses a flat fence line while preserving the byte offset of the fence head.
pub(crate) fn parse_flat_fence(source: &str) -> Option<FlatFence<'_>> {
    let trimmed_offset = leading_byte_len(source);
    let trimmed = source.trim();
    let inner_source = trimmed.strip_prefix("===")?.strip_suffix("===")?;
    let inner_leading = leading_byte_len(inner_source);
    let inner = inner_source.trim();
    let inner_start = trimmed_offset + "===".len() + inner_leading;
    if inner.is_empty() {
        return Some(FlatFence {
            kind: "",
            head: "",
            close: false,
            head_start: inner_start,
        });
    }
    if let Some(close) = inner.strip_prefix('/') {
        let close_leading = leading_byte_len(close);
        let close = close.trim_start();
        let kind = close.split_whitespace().next().unwrap_or_default();
        return Some(FlatFence {
            kind,
            head: close.trim(),
            close: true,
            head_start: inner_start + '/'.len_utf8() + close_leading,
        });
    }
    let (kind, head) = split_leading_ident(inner).unwrap_or((inner, ""));
    let head_leading = leading_byte_len(head);
    Some(FlatFence {
        kind,
        head: head.trim(),
        close: false,
        head_start: inner_start + (inner.len() - head.len()) + head_leading,
    })
}

fn leading_byte_len(source: &str) -> usize {
    source.len() - source.trim_start().len()
}

/// Splits a leading identifier token from the rest of a source fragment.
pub(crate) fn split_leading_ident(source: &str) -> Option<(&str, &str)> {
    let token = lex_cst(source).into_iter().next()?;
    (token.kind() == SyntaxKind::Ident && token.start() == 0)
        .then(|| (token.text(), source[token.end()..].trim_start()))
}

/// Splits a leading lifetime name, including the leading apostrophe.
pub(crate) fn split_leading_lifetime(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix('\'')?;
    let len = take_while(rest, is_ident_continue);
    (len > 0).then(|| (&source[..'\''.len_utf8() + len], rest[len..].trim_start()))
}
