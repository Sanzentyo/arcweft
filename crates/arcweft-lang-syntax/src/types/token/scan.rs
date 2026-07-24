//! Delimiter-aware operations over the authoritative type-token slice.

use super::{TypeToken, TypeTokenKind};
use crate::ast::common::TextRange;
use crate::types::{TypeParseError, TypeRefNodePath};

#[derive(Clone, Copy)]
pub(super) enum Delimiter {
    Paren,
    Bracket,
    Brace,
    Angle,
}

#[derive(Default)]
pub(super) struct DelimiterDepth {
    paren: usize,
    bracket: usize,
    brace: usize,
    angle: usize,
}

impl DelimiterDepth {
    pub(super) const fn is_top_level(&self) -> bool {
        self.paren == 0 && self.bracket == 0 && self.brace == 0 && self.angle == 0
    }

    pub(super) fn advance(&mut self, kind: TypeTokenKind<'_>) -> bool {
        match kind {
            TypeTokenKind::OpenParen => self.paren += 1,
            TypeTokenKind::CloseParen => {
                let Some(next) = self.paren.checked_sub(1) else {
                    return false;
                };
                self.paren = next;
            }
            TypeTokenKind::OpenBracket => self.bracket += 1,
            TypeTokenKind::CloseBracket => {
                let Some(next) = self.bracket.checked_sub(1) else {
                    return false;
                };
                self.bracket = next;
            }
            TypeTokenKind::OpenBrace => self.brace += 1,
            TypeTokenKind::CloseBrace => {
                let Some(next) = self.brace.checked_sub(1) else {
                    return false;
                };
                self.brace = next;
            }
            TypeTokenKind::OpenAngle => self.angle += 1,
            TypeTokenKind::CloseAngle => {
                let Some(next) = self.angle.checked_sub(1) else {
                    return false;
                };
                self.angle = next;
            }
            _ => {}
        }
        true
    }
}

pub(super) fn first_top_level(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    mut matches: impl FnMut(TypeTokenKind<'_>) -> bool,
) -> Option<usize> {
    let mut depth = DelimiterDepth::default();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if depth.is_top_level() && matches(token.kind) {
            return Some(index);
        }
        if !depth.advance(token.kind) {
            return None;
        }
    }
    None
}

pub(super) fn last_top_level(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    mut matches: impl FnMut(TypeTokenKind<'_>) -> bool,
) -> Option<usize> {
    let mut depth = DelimiterDepth::default();
    let mut found = None;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if depth.is_top_level() && matches(token.kind) {
            found = Some(index);
        }
        if !depth.advance(token.kind) {
            return None;
        }
    }
    found
}

pub(super) fn matching_close(
    tokens: &[TypeToken<'_>],
    open: usize,
    end: usize,
    delimiter: Delimiter,
) -> Option<usize> {
    let (open_kind, close_kind) = match delimiter {
        Delimiter::Paren => (TypeTokenKind::OpenParen, TypeTokenKind::CloseParen),
        Delimiter::Bracket => (TypeTokenKind::OpenBracket, TypeTokenKind::CloseBracket),
        Delimiter::Brace => (TypeTokenKind::OpenBrace, TypeTokenKind::CloseBrace),
        Delimiter::Angle => (TypeTokenKind::OpenAngle, TypeTokenKind::CloseAngle),
    };
    if tokens.get(open)?.kind != open_kind {
        return None;
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(end).skip(open) {
        if token.kind == open_kind {
            depth += 1;
        } else if token.kind == close_kind {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(super) fn split_top_level(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    matches: impl FnMut(TypeTokenKind<'_>) -> bool,
) -> Vec<(usize, usize)> {
    split_top_level_with_separators(tokens, start, end, matches)
        .into_iter()
        .map(|(part_start, part_end, _)| (part_start, part_end))
        .collect()
}

pub(super) fn split_top_level_with_separators(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
    mut matches: impl FnMut(TypeTokenKind<'_>) -> bool,
) -> Vec<(usize, usize, Option<usize>)> {
    let mut parts = Vec::new();
    let mut part_start = start;
    let mut preceding_separator = None;
    let mut depth = DelimiterDepth::default();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        if depth.is_top_level() && matches(token.kind) {
            parts.push((part_start, index, preceding_separator));
            part_start = index + 1;
            preceding_separator = Some(index);
        } else if !depth.advance(token.kind) {
            break;
        }
    }
    parts.push((part_start, end, preceding_separator));
    parts
}

pub(super) fn token_range(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<TextRange, TypeParseError> {
    require_nonempty(tokens, start, end)?;
    Ok(TextRange::new(
        tokens[start].range.start(),
        tokens[end - 1].range.end(),
    ))
}

pub(super) fn require_nonempty(
    tokens: &[TypeToken<'_>],
    start: usize,
    end: usize,
) -> Result<(), TypeParseError> {
    if start >= end || end > tokens.len() {
        return Err(TypeParseError::new("expected type"));
    }
    Ok(())
}

pub(super) fn index_u16(index: usize, path: &TypeRefNodePath) -> Result<u16, TypeParseError> {
    u16::try_from(index).map_err(|_| {
        TypeParseError::new_owned(format!(
            "type node at {:?} has too many indexed children",
            path.steps()
        ))
    })
}
