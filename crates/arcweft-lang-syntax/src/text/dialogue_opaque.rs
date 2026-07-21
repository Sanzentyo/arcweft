//! Bounded classification of dialogue surfaces whose interior markup is opaque
//! to the private attached `RichText` grammar.

use super::{
    MAX_RICH_TEXT_CONTENT_ARGUMENTS, ScannedTagArgValueSurface, ScannedTagArgument,
    find_dialogue_tag_boundary, is_valid_compact_ruby_base, parse_inline_style_head,
    scan_tag_arg_value, scan_tag_arguments, slice_offset, split_once_top_level,
    split_tag_name_attrs, trim_rich_text_whitespace,
};

/// One dialogue surface whose interior brackets are not standalone `RichText`
/// tags in the public text grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DialogueOpaqueSurface {
    end: usize,
    rich_text_tags: usize,
    rich_text_arguments: usize,
}

impl DialogueOpaqueSurface {
    pub(crate) const fn end(self) -> usize {
        self.end
    }

    pub(crate) const fn rich_text_tags(self) -> usize {
        self.rich_text_tags
    }

    pub(crate) const fn rich_text_arguments(self) -> usize {
        self.rich_text_arguments
    }
}

/// Classifies public dialogue surfaces that the private tag grammar must
/// consume opaquely instead of inventing identities for their inner brackets.
///
/// This carrier contains only a bounded source end and the public content-limit
/// charges. It constructs no dialogue AST and applies no semantic defaults.
pub(crate) fn scan_dialogue_opaque_surface(
    source: &str,
    start: usize,
    end: usize,
) -> Option<DialogueOpaqueSurface> {
    let source = source.get(..end)?;
    let tail = source.get(start..)?;
    let first = tail.chars().next()?;
    let opaque = |end, rich_text_tags, rich_text_arguments| DialogueOpaqueSurface {
        end,
        rich_text_tags,
        rich_text_arguments,
    };

    match first {
        '\\' => {
            let escaped = tail['\\'.len_utf8()..].chars().next()?;
            Some(opaque(start + '\\'.len_utf8() + escaped.len_utf8(), 0, 0))
        }
        '|' => scan_ascii_explicit_ruby_end(source, start)
            .or_else(|| scan_ascii_compact_ruby_end(source, start))
            .map(|end| opaque(end, 0, 0)),
        '｜' => scan_natural_ruby_end(source, start).map(|end| opaque(end, 0, 0)),
        '#' if tail.starts_with("#[") => Some(opaque(
            scan_balanced_end(source, start + "#[".len(), '[', ']').unwrap_or(start + "#[".len()),
            0,
            0,
        )),
        '$' if tail.starts_with("$(") => Some(opaque(
            scan_balanced_end(source, start + "$(".len(), '(', ')').unwrap_or(start + "$(".len()),
            0,
            0,
        )),
        '[' => scan_bracket_ruby_end(source, start)
            .map(|end| opaque(end, 1, 0))
            .or_else(|| scan_raw_span_end(source, start).map(|end| opaque(end, 1, 0)))
            .or_else(|| scan_inline_raw_end(source, start).map(|end| opaque(end, 1, 0)))
            .or_else(|| {
                scan_inline_style_end(source, start)
                    .map(|(end, arguments)| opaque(end, 2, arguments))
            }),
        _ => None,
    }
}

fn scan_natural_ruby_end(source: &str, start: usize) -> Option<usize> {
    let after_marker = start + '｜'.len_utf8();
    let tail = source.get(after_marker..)?;
    let open_relative = tail.find('《')?;
    (!tail[..open_relative].is_empty()).then_some(())?;
    let ruby_start = after_marker + open_relative + '《'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('》')?;
    (!ruby_tail[..close_relative].is_empty()).then_some(())?;
    Some(ruby_start + close_relative + '》'.len_utf8())
}

fn scan_ascii_explicit_ruby_end(source: &str, start: usize) -> Option<usize> {
    let after_marker = start + '|'.len_utf8();
    let base_tail = source.get(after_marker..)?.strip_prefix('[')?;
    let base_end_relative = base_tail.find(']')?;
    (!base_tail[..base_end_relative].is_empty()).then_some(())?;
    let after_base = after_marker + '['.len_utf8() + base_end_relative + ']'.len_utf8();
    let ruby_tail = source.get(after_base..)?.strip_prefix('(')?;
    let ruby_end_relative = ruby_tail.find(')')?;
    (!ruby_tail[..ruby_end_relative].is_empty()).then_some(())?;
    Some(after_base + '('.len_utf8() + ruby_end_relative + ')'.len_utf8())
}

fn scan_ascii_compact_ruby_end(source: &str, start: usize) -> Option<usize> {
    let after_marker = start + '|'.len_utf8();
    let tail = source.get(after_marker..)?;
    if tail.starts_with('[') {
        return None;
    }
    let open_relative = tail.find('{')?;
    is_valid_compact_ruby_base(&tail[..open_relative]).then_some(())?;
    let ruby_start = after_marker + open_relative + '{'.len_utf8();
    let ruby_tail = source.get(ruby_start..)?;
    let close_relative = ruby_tail.find('}')?;
    (!ruby_tail[..close_relative].is_empty()).then_some(())?;
    Some(ruby_start + close_relative + '}'.len_utf8())
}

fn scan_bracket_ruby_end(source: &str, start: usize) -> Option<usize> {
    let boundary = find_dialogue_tag_boundary(source, start)?;
    if boundary.unterminated_quote_start().is_some() {
        return None;
    }
    let after_open = boundary.end();
    let inside = trim_rich_text_whitespace(source.get(start + 1..boundary.close())?);
    let (tag_name, attrs) = split_tag_name_attrs(inside);
    if !matches!(tag_name, "ruby" | "rb") {
        return None;
    }
    let attrs_start = slice_offset(source, attrs);
    let scanned = scan_tag_arguments(attrs, attrs_start, MAX_RICH_TEXT_CONTENT_ARGUMENTS);
    if !scanned.diagnostics().is_empty() {
        return None;
    }
    scanned.entries().iter().find_map(|argument| {
        let ScannedTagArgument::Named {
            name_range, value, ..
        } = argument
        else {
            return None;
        };
        let name = source.get(name_range.as_range())?;
        (name == "rt" && matches!(value, ScannedTagArgValueSurface::Present(_))).then_some(())
    })?;
    let tail = source.get(after_open..)?;
    let close_tag = format!("[/{tag_name}]");
    let close_relative = tail.find(&close_tag)?;
    let base_end = after_open + close_relative;
    (!trim_rich_text_whitespace(source.get(after_open..base_end)?).is_empty()).then_some(())?;
    Some(base_end + close_tag.len())
}

fn scan_raw_span_end(source: &str, start: usize) -> Option<usize> {
    let body_start = start + "[raw]".len();
    source.get(start..)?.starts_with("[raw]").then_some(())?;
    let close_relative = source.get(body_start..)?.find("[/raw]")?;
    Some(body_start + close_relative + "[/raw]".len())
}

fn scan_inline_raw_end(source: &str, start: usize) -> Option<usize> {
    source.get(start..)?.starts_with("[raw:").then_some(())?;
    scan_balanced_end(source, start + "[raw:".len(), '[', ']')
}

fn scan_inline_style_end(source: &str, start: usize) -> Option<(usize, usize)> {
    let boundary = find_dialogue_tag_boundary(source, start)?;
    if boundary.unterminated_quote_start().is_some() {
        return None;
    }
    let inside = trim_rich_text_whitespace(source.get(start + 1..boundary.close())?);
    let (tag_source, body) = split_once_top_level(inside, ':')?;
    (!body.is_empty()).then_some(())?;
    let tag_source = trim_rich_text_whitespace(tag_source);
    let (_, _, authored_value) = parse_inline_style_head(tag_source)?;
    if let Some(value) = authored_value {
        scan_tag_arg_value(value, slice_offset(source, value)).ok()?;
    }
    Some((boundary.end(), usize::from(authored_value.is_some())))
}

fn scan_balanced_end(source: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, character) in source.get(start..)?.char_indices() {
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + relative + close.len_utf8());
            }
        }
    }
    None
}
