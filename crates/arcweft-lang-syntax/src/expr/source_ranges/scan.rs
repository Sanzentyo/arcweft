use super::super::{BinaryOp, is_ident_continue};
use crate::ast::common::TextRange;

pub(super) fn trim_source_with_base(source: &str, base: usize) -> (&str, usize) {
    let start_trim = source.len() - source.trim_start().len();
    let source = &source[start_trim..];
    let end = source.trim_end().len();
    (&source[..end], base + start_trim)
}

pub(super) fn absolute_source_slice(source: &str, base: usize, range: TextRange) -> Option<&str> {
    let start = range.start().checked_sub(base)?;
    let end = range.end().checked_sub(base)?;
    source.get(start..end)
}

pub(super) fn delimited_inner(
    source: &str,
    base: usize,
    open: char,
    close: char,
) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    source
        .strip_prefix(open)?
        .strip_suffix(close)
        .map(|inner| (inner, base + open.len_utf8()))
}

pub(super) fn braced_block_inner(source: &str, base: usize) -> Option<(&str, usize)> {
    let (source, base) = trim_source_with_base(source, base);
    if let Some(inner) = source
        .strip_prefix('{')
        .and_then(|source| source.strip_suffix('}'))
    {
        return Some((inner, base + '{'.len_utf8()));
    }
    if let Some(open) = find_top_level_char(source, '{')
        && source.ends_with('}')
    {
        return Some((
            &source[open + '{'.len_utf8()..source.len() - '}'.len_utf8()],
            base + open + '{'.len_utf8(),
        ));
    }
    if let Some(colon) = find_top_level_char(source, ':') {
        return Some((
            &source[colon + ':'.len_utf8()..],
            base + colon + ':'.len_utf8(),
        ));
    }
    None
}

pub(super) fn postfix_delimiter_bounds(
    source: &str,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    let close_start = source
        .char_indices()
        .last()
        .filter(|(_, ch)| *ch == close)
        .map(|(index, _)| index)?;
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source
        .char_indices()
        .take_while(|(index, _)| *index < close_start)
    {
        if state.is_top_level_before(ch) && ch == open {
            result = Some(index);
        }
        state.advance(ch);
    }
    result.map(|open_start| (open_start, close_start))
}

pub(super) fn split_top_level_segments(
    source: &str,
    base: usize,
    delimiter: char,
) -> Vec<(&str, usize)> {
    let mut state = SourceScanState::default();
    let mut start = 0;
    let mut segments = Vec::new();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == delimiter {
            push_trimmed_segment(source, base, start, index, &mut segments);
            start = index + ch.len_utf8();
        }
        state.advance(ch);
    }
    push_trimmed_segment(source, base, start, source.len(), &mut segments);
    segments
}

pub(super) fn split_top_level_lines(source: &str, base: usize) -> Vec<(&str, usize)> {
    let mut segments = Vec::new();
    let mut line_start = 0;
    for line in source.split_inclusive('\n') {
        let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
        push_trimmed_segment(
            source,
            base,
            line_start,
            line_start + line_without_newline.len(),
            &mut segments,
        );
        line_start += line.len();
    }
    if line_start < source.len() {
        push_trimmed_segment(source, base, line_start, source.len(), &mut segments);
    }
    segments
}

pub(super) fn push_trimmed_segment<'a>(
    source: &'a str,
    base: usize,
    start: usize,
    end: usize,
    segments: &mut Vec<(&'a str, usize)>,
) {
    let (segment, segment_base) = trim_source_with_base(&source[start..end], base + start);
    if !segment.is_empty() {
        segments.push((segment, segment_base));
    }
}

pub(super) fn find_top_level_char(source: &str, target: char) -> Option<usize> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == target {
            return Some(index);
        }
        state.advance(ch);
    }
    None
}

pub(super) fn find_last_top_level_char(source: &str, target: char) -> Option<usize> {
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch) && ch == target {
            result = Some(index);
        }
        state.advance(ch);
    }
    result
}

pub(super) fn find_top_level_operator(source: &str, operator: &str) -> Option<(usize, usize)> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(operator)
            && operator_boundaries_match(source, index, operator)
        {
            return Some((index, index + operator.len()));
        }
        state.advance(ch);
    }
    None
}

pub(super) fn find_last_top_level_operator(source: &str, operator: &str) -> Option<(usize, usize)> {
    let mut state = SourceScanState::default();
    let mut result = None;
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(operator)
            && operator_boundaries_match(source, index, operator)
        {
            result = Some((index, index + operator.len()));
        }
        state.advance(ch);
    }
    result
}

pub(super) fn find_binary_operator(source: &str, op: BinaryOp) -> Option<(usize, usize)> {
    let operator = binary_op_source(op);
    if matches!(op, BinaryOp::Implies) {
        find_top_level_operator(source, operator)
    } else {
        find_last_top_level_operator(source, operator)
    }
}

fn binary_op_source(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Implies => "=>",
        BinaryOp::Or => "||",
        BinaryOp::And => "&&",
        BinaryOp::In => "in",
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        BinaryOp::Gte => ">=",
        BinaryOp::Lte => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Lt => "<",
        BinaryOp::Merge => "&",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
    }
}

fn operator_boundaries_match(source: &str, index: usize, operator: &str) -> bool {
    if operator == "in" {
        let before = source[..index].chars().next_back();
        let after = source[index + operator.len()..].chars().next();
        before.is_none_or(|ch| !is_ident_continue(ch))
            && after.is_none_or(|ch| !is_ident_continue(ch))
    } else {
        true
    }
}

pub(super) fn find_top_level_keyword(source: &str, keyword: &str) -> Option<usize> {
    let mut state = SourceScanState::default();
    for (index, ch) in source.char_indices() {
        if state.is_top_level_before(ch)
            && source[index..].starts_with(keyword)
            && operator_boundaries_match(source, index, keyword)
        {
            return Some(index);
        }
        state.advance(ch);
    }
    None
}

pub(super) fn matching_delimiter_end(
    source: &str,
    open_start: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut in_quoted_literal = false;
    let mut escaped = false;
    let mut depth = 0usize;
    for (index, ch) in source
        .char_indices()
        .skip_while(|(index, _)| *index < open_start)
    {
        if in_quoted_literal {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quoted_literal = false;
            }
            continue;
        }
        if ch == '"' {
            in_quoted_literal = true;
        } else if ch == open {
            depth = depth.checked_add(1)?;
        } else if ch == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return index.checked_add(ch.len_utf8());
            }
        }
    }
    None
}

/// Expression delimiter state. Arcweft char literals use the same double-quoted
/// payload as strings (`"x"c`); an apostrophe starts a lifetime, not a literal.
#[derive(Default)]
struct SourceScanState {
    paren: usize,
    bracket: usize,
    brace: usize,
    in_quoted_literal: bool,
    escaped: bool,
    overflowed: bool,
}

impl SourceScanState {
    fn is_top_level_before(&self, ch: char) -> bool {
        !self.in_quoted_literal
            && !self.overflowed
            && self.paren == 0
            && self.bracket == 0
            && self.brace == 0
            && !matches!(ch, ')' | ']' | '}')
    }

    fn advance(&mut self, ch: char) {
        if self.in_quoted_literal {
            if self.escaped {
                self.escaped = false;
            } else if ch == '\\' {
                self.escaped = true;
            } else if ch == '"' {
                self.in_quoted_literal = false;
            }
            return;
        }
        match ch {
            '"' => self.in_quoted_literal = true,
            '(' => {
                if let Some(depth) = self.paren.checked_add(1) {
                    self.paren = depth;
                } else {
                    self.overflowed = true;
                }
            }
            ')' => {
                if let Some(depth) = self.paren.checked_sub(1) {
                    self.paren = depth;
                }
            }
            '[' => {
                if let Some(depth) = self.bracket.checked_add(1) {
                    self.bracket = depth;
                } else {
                    self.overflowed = true;
                }
            }
            ']' => {
                if let Some(depth) = self.bracket.checked_sub(1) {
                    self.bracket = depth;
                }
            }
            '{' => {
                if let Some(depth) = self.brace.checked_add(1) {
                    self.brace = depth;
                } else {
                    self.overflowed = true;
                }
            }
            '}' => {
                if let Some(depth) = self.brace.checked_sub(1) {
                    self.brace = depth;
                }
            }
            _ => {}
        }
    }
}
