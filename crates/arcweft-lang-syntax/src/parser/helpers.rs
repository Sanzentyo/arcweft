//! Shared parser helpers that are not tied to a single grammar family.

use super::headers::parse_visibility_prefix;
use crate::ast::{
    common::{TextRange, UseItem, UseMode},
    items::Attribute,
};
use crate::cst::{find_matching_punctuation, find_top_level_punctuation};

pub(super) fn parse_use_line(trimmed: &str, range: TextRange) -> Option<UseItem> {
    let (visibility, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    let (mode, tree) = if let Some(tree) = rest.strip_prefix("lazy use ") {
        (Some(UseMode::Lazy), tree)
    } else if let Some(tree) = rest.strip_prefix("eager use ") {
        (Some(UseMode::Eager), tree)
    } else {
        (None, rest.strip_prefix("use ")?)
    };
    Some(UseItem::new(
        visibility,
        mode,
        normalize_module_path(tree.trim()),
        range,
    ))
}

pub(super) fn normalize_module_path(path: &str) -> String {
    path.strip_prefix("parent::")
        .map_or_else(|| path.to_owned(), |tail| format!("super::{tail}"))
}

pub(super) fn is_relative_id_path(path: &str) -> bool {
    let trimmed = path.trim_start();
    trimmed.starts_with('.') || trimmed.starts_with("@.") || trimmed.starts_with("@super.")
}

pub(super) fn parse_attribute(trimmed: &str, range: TextRange) -> Option<Attribute> {
    let rest = trimmed.strip_prefix("#[")?.strip_suffix(']')?.trim();
    let open = find_top_level_punctuation(rest, '(')?;
    let close = find_matching_punctuation(rest, open, '(', ')')?;
    (rest[close + ')'.len_utf8()..].trim().is_empty()).then_some(())?;
    let name = rest[..open].trim().to_owned();
    let args = rest[open + 1..close].trim();
    Some(Attribute::new(
        name,
        (!args.is_empty()).then(|| args.to_owned()),
        range,
    ))
}
