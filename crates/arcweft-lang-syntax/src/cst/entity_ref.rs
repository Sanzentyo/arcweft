//! Entity reference and relative-ID marker splitting.

use super::lexer::{is_ident_start, lex_cst, take_while};
use super::{CstEntityRef, CstRelativeEntityRef, CstRelativeId, CstRelativeIdSpelling, SyntaxKind};

/// Splits a leading entity reference and exposes its marker-normalized parts.
pub(crate) fn split_leading_entity_ref_parts(source: &str) -> Option<CstEntityRef<'_>> {
    let token = lex_cst(source).into_iter().next()?;
    if token.kind() != SyntaxKind::EntityRef || token.start() != 0 {
        return None;
    }
    let raw = token.text();
    if starts_family_relative_entity_ref(raw) || raw.starts_with("@.") || raw.starts_with("@super.")
    {
        return None;
    }
    let rest = &source[token.end()..];
    let delimited = raw.starts_with("@<");
    let body = if delimited {
        raw.strip_prefix("@<")
            .map_or("", |inner| inner.strip_suffix('>').unwrap_or(inner))
    } else {
        &raw[1..]
    };
    Some(CstEntityRef {
        raw,
        body,
        delimited,
        closed: !delimited || raw.ends_with('>'),
        rest,
    })
}

/// Returns true when a fragment begins with an entity reference token.
pub(crate) fn starts_leading_entity_ref(source: &str) -> bool {
    split_leading_entity_ref_parts(source).is_some()
}

/// Splits a leading family-qualified relative entity reference.
pub(crate) fn split_leading_relative_entity_ref(source: &str) -> Option<CstRelativeEntityRef<'_>> {
    let at = source.strip_prefix('@')?;
    let family_len = take_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    if family_len == 0 || !at.get(family_len..)?.starts_with(":.") {
        return None;
    }
    let family = &at[..family_len];
    let relative_source = &at[family_len + ':'.len_utf8()..];
    let dots = take_while(relative_source, |ch| ch == '.');
    if dots == 0 {
        return None;
    }
    let body_source = &relative_source[dots..];
    let body_len = take_relative_id_body(body_source);
    if body_len == 0 {
        return None;
    }
    let relative = CstRelativeId {
        body: &body_source[..body_len],
        parent_depth: dots.saturating_sub(1),
        spelling: CstRelativeIdSpelling::DotRun,
        marker_len: dots,
        rest: &body_source[body_len..],
    };
    let raw_len =
        '@'.len_utf8() + family_len + ':'.len_utf8() + relative.marker_len + relative.body.len();
    Some(CstRelativeEntityRef {
        raw: &source[..raw_len],
        family,
        relative,
        rest: &source[raw_len..],
    })
}

fn starts_family_relative_entity_ref(raw: &str) -> bool {
    let Some(at) = raw.strip_prefix('@') else {
        return false;
    };
    let family_len = take_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    family_len > 0
        && at
            .get(family_len..)
            .is_some_and(|tail| tail.starts_with(":."))
}

/// Returns true when a fragment begins with a family-qualified relative entity reference.
pub(crate) fn starts_leading_relative_entity_ref(source: &str) -> bool {
    split_leading_relative_entity_ref(source).is_some()
}

/// Returns true when a fragment begins with an ID-context relative ID marker.
pub(crate) fn starts_leading_relative_id(source: &str) -> bool {
    source.starts_with("@.") || source.starts_with("@super.")
}

/// Splits a leading relative ID marker in an ID-bearing context.
///
/// The current grammar uses `@.id`, parent-dot forms such as `@..id`, and
/// explicit `@super.id` forms.
pub(crate) fn split_leading_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    if let Some(relative) = split_dot_relative_id(source) {
        return Some(relative);
    }
    split_super_relative_id(source)
}

fn split_dot_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    let rest = source.strip_prefix('@')?;
    let dots = take_while(rest, |ch| ch == '.');
    let (dot_run, marker_len) = (dots, 1 + dots);
    if dot_run == 0 {
        return None;
    }
    let body_source = &source[marker_len..];
    let body_len = take_relative_id_body(body_source);
    (body_len > 0).then(|| CstRelativeId {
        body: &body_source[..body_len],
        parent_depth: dot_run.saturating_sub(1),
        spelling: CstRelativeIdSpelling::DotRun,
        marker_len,
        rest: &body_source[body_len..],
    })
}

fn split_super_relative_id(source: &str) -> Option<CstRelativeId<'_>> {
    if !source.starts_with('@') {
        return None;
    }
    let mut cursor = "@".len();
    let mut parents = 0usize;
    while cursor <= source.len() && source[cursor..].starts_with("super.") {
        parents += 1;
        cursor += "super.".len();
    }
    if parents == 0 {
        return None;
    }
    let body_len = take_relative_id_body(&source[cursor..]);
    (body_len > 0).then(|| CstRelativeId {
        body: &source[cursor..cursor + body_len],
        parent_depth: parents,
        spelling: CstRelativeIdSpelling::SuperChain,
        marker_len: cursor,
        rest: &source[cursor + body_len..],
    })
}

fn take_relative_id_body(source: &str) -> usize {
    let Some(first) = source.chars().next() else {
        return 0;
    };
    if !is_ident_start(first) {
        return 0;
    }
    take_while(source, |ch| {
        ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-')
    })
}
