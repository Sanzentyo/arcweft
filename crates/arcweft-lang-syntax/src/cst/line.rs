//! CST line projection, line events, and brace-block collection.

use super::classify::{
    classify_flow_item, classify_line, classify_top_level_item, classify_top_level_line,
    flow_line_starts_body, function_body_line_starts_body,
};
use super::{
    CstBlockEvent, CstBlockOpenRule, CstFlowItemKind, CstLine, CstLineEvents, CstLineKind,
    CstLinePunctuationSummary, CstPunctuationDeltas, CstTopLevelItemKind, CstTopLevelLineKind,
    SyntaxKind, SyntaxNode, SyntaxParseStats,
};
use std::borrow::Cow;
use std::ops::{Index, Range};

impl From<&SyntaxNode> for CstLineEvents<'static> {
    fn from(root: &SyntaxNode) -> Self {
        let lines = root
            .children()
            .filter(|node| node.kind() == SyntaxKind::Line)
            .map(|node| CstLine::from_node(&node))
            .collect::<Vec<_>>();
        let line_owned_bytes = lines.iter().map(|line| line.text.len()).sum();
        let punctuation_scan_bytes = line_owned_bytes;
        Self {
            stats: SyntaxParseStats {
                cst_lex_passes: 1,
                punctuation_scans: lines.len(),
                punctuation_scan_bytes,
                line_owned_bytes,
                ..SyntaxParseStats::default()
            },
            lines,
            source: None,
        }
    }
}

impl<'a> CstLineEvents<'a> {
    pub(crate) fn from_root_and_source(root: &SyntaxNode, source: &'a str) -> Self {
        let lines = root
            .children()
            .filter(|node| node.kind() == SyntaxKind::Line)
            .map(|node| CstLine::from_node_and_source(&node, source))
            .collect::<Vec<_>>();
        Self::from_borrowed_lines(lines, source)
    }

    fn from_borrowed_lines(lines: Vec<CstLine<'a>>, source: &'a str) -> Self {
        let punctuation_scan_bytes = lines.iter().map(|line| line.text.len()).sum();
        Self {
            stats: SyntaxParseStats {
                cst_lex_passes: 1,
                punctuation_scans: lines.len(),
                punctuation_scan_bytes,
                line_owned_bytes: 0,
                ..SyntaxParseStats::default()
            },
            lines,
            source: Some(source),
        }
    }

    /// Number of projected line events.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns true when the source has no non-empty CST line events.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Iterates over projected CST line events.
    pub fn iter(&self) -> impl Iterator<Item = &CstLine<'a>> {
        self.lines.iter()
    }

    /// Returns a line event by index.
    pub fn get(&self, index: usize) -> Option<&CstLine<'a>> {
        self.lines.get(index)
    }

    /// Path-free counters collected while projecting CST lines.
    pub const fn stats(&self) -> SyntaxParseStats {
        self.stats
    }

    pub(crate) fn with_absolute_offsets(mut self, base_offset: usize) -> Option<Self> {
        self.lines = self
            .lines
            .into_iter()
            .map(|line| line.with_absolute_offsets(base_offset))
            .collect::<Option<Vec<_>>>()?;
        Some(self)
    }

    pub(crate) fn line_slice(&self, range: Range<usize>) -> Option<CstLineEvents<'a>> {
        if range.start > range.end || range.end > self.lines.len() {
            return None;
        }
        Some(CstLineEvents {
            lines: self.lines[range].to_vec(),
            source: None,
            stats: SyntaxParseStats::default(),
        })
    }

    /// Reuses a complete line-event range as parser input with offsets rebased
    /// to a virtual fragment. This avoids reparsing nested flow bodies when the
    /// body is already represented by whole CST lines.
    pub(crate) fn relative_line_slice(
        &self,
        range: Range<usize>,
        base_offset: usize,
    ) -> Option<CstLineEvents<'a>> {
        if range.start > range.end || range.end > self.lines.len() {
            return None;
        }
        let mut lines = Vec::with_capacity(range.end - range.start);
        for line in &self.lines[range] {
            lines.push(line.with_relative_offsets(base_offset)?);
        }
        Some(CstLineEvents {
            lines,
            source: None,
            stats: SyntaxParseStats::default(),
        })
    }

    /// Collects a balanced brace block beginning at a line-event index.
    pub(crate) fn collect_brace_block(
        &self,
        start: usize,
        rule: CstBlockOpenRule,
    ) -> CstBlockEvent<'a> {
        let Some(first) = self.get(start) else {
            return CstBlockEvent::new(Cow::Borrowed(""), Cow::Borrowed(""), 0, false, start);
        };
        let mut end = first.end;
        let mut depth = 0_i32;
        let mut seen_open = false;
        let mut seen_body_open = false;
        let mut first_top_level_open = None;
        let mut last_top_level_open = None;
        let mut last_brace_close = None;
        let mut index = start;
        let mut virtual_len = 0usize;

        while let Some(line) = self.get(index) {
            if index > start {
                virtual_len += 1;
            }
            let line_offset = virtual_len;
            virtual_len += line.text.len();
            end = line.end;
            if depth == 0 {
                if first_top_level_open.is_none()
                    && let Some(open) = line.first_top_level_brace_open()
                {
                    first_top_level_open = Some(line_offset + open);
                }
                if let Some(open) = line.last_top_level_brace_open() {
                    last_top_level_open = Some(line_offset + open);
                }
            }
            if let Some(close) = line.last_brace_close() {
                last_brace_close = Some(line_offset + close);
            }
            if matches!(rule, CstBlockOpenRule::FunctionBody)
                && function_body_line_starts_body(line)
            {
                seen_body_open = true;
            }
            if line.has_top_level_brace_open() {
                seen_open = true;
            }
            depth += line.brace_delta();
            index += 1;
            if block_event_is_complete(rule, seen_open, seen_body_open, depth) {
                break;
            }
        }

        let open = match rule {
            CstBlockOpenRule::FirstTopLevel => first_top_level_open,
            CstBlockOpenRule::FlowBody | CstBlockOpenRule::FunctionBody => last_top_level_open,
        };
        let Some(open) = open else {
            return CstBlockEvent::new(
                self.collect_virtual_fragment(start, index, 0, virtual_len),
                Cow::Borrowed(""),
                end,
                false,
                start + 1,
            );
        };
        let Some(close) = last_brace_close else {
            return CstBlockEvent::new(
                self.collect_virtual_fragment(start, index, 0, virtual_len),
                Cow::Borrowed(""),
                end,
                false,
                index,
            );
        };
        if depth != 0 {
            return CstBlockEvent::new(
                self.collect_virtual_fragment(start, index, 0, virtual_len),
                Cow::Borrowed(""),
                end,
                false,
                index,
            );
        }
        let head = self.collect_virtual_fragment(start, index, 0, open);
        let body = self.collect_virtual_fragment(start, index, open + 1, close);
        CstBlockEvent::new(trim_cow(head), body, end, true, index)
            .with_body_range(self.source_range_for_virtual_fragment(start, index, open + 1, close))
            .with_body_line_range(self.full_line_range_for_virtual_fragment(
                start,
                index,
                open + 1,
                close,
            ))
    }

    fn source_range_for_virtual_fragment(
        &self,
        start: usize,
        end: usize,
        range_start: usize,
        range_end: usize,
    ) -> Option<Range<usize>> {
        let source_start = self.source_pos_for_virtual_offset(start, end, range_start)?;
        let source_end = self.source_pos_for_virtual_offset(start, end, range_end)?;
        Some(source_start..source_end)
    }

    fn full_line_range_for_virtual_fragment(
        &self,
        start: usize,
        end: usize,
        range_start: usize,
        range_end: usize,
    ) -> Option<Range<usize>> {
        if range_start > range_end {
            return None;
        }

        let line_ranges = self.virtual_line_ranges(start, end);
        let first = line_ranges
            .iter()
            .find(|(_, line_start, line_end)| *line_start >= range_start && *line_end <= range_end)
            .map(|(index, _, _)| *index);
        let Some(first) = first else {
            let fragment = self.collect_virtual_fragment(start, end, range_start, range_end);
            return fragment.trim().is_empty().then_some(start..start);
        };
        let last = line_ranges
            .iter()
            .rev()
            .find(|(_, line_start, line_end)| *line_start >= range_start && *line_end <= range_end)
            .map(|(index, _, _)| *index + 1)?;
        let prefix_end = line_ranges
            .iter()
            .find(|(index, _, _)| *index == first)
            .map(|(_, line_start, _)| *line_start)?;
        let suffix_start = line_ranges
            .iter()
            .find(|(index, _, _)| *index + 1 == last)
            .map(|(_, _, line_end)| *line_end)?;
        let prefix = self.collect_virtual_fragment(start, end, range_start, prefix_end);
        let suffix = self.collect_virtual_fragment(start, end, suffix_start, range_end);
        (prefix.trim().is_empty() && suffix.trim().is_empty()).then_some(first..last)
    }

    fn virtual_line_ranges(&self, start: usize, end: usize) -> Vec<(usize, usize, usize)> {
        let mut ranges = Vec::new();
        let mut virtual_offset = 0usize;
        for index in start..end {
            if index > start {
                virtual_offset += 1;
            }
            let Some(line) = self.get(index) else {
                break;
            };
            let line_start = virtual_offset;
            let line_end = line_start + line.text.len();
            ranges.push((index, line_start, line_end));
            virtual_offset = line_end;
        }
        ranges
    }

    fn collect_virtual_fragment(
        &self,
        start: usize,
        end: usize,
        range_start: usize,
        range_end: usize,
    ) -> Cow<'a, str> {
        if let Some(fragment) = self.borrow_virtual_fragment(start, end, range_start, range_end) {
            return Cow::Borrowed(fragment);
        }
        let mut fragment = String::new();
        let mut virtual_offset = 0usize;
        for index in start..end {
            if index > start {
                push_virtual_text_overlap(
                    "\n",
                    virtual_offset,
                    range_start,
                    range_end,
                    &mut fragment,
                );
                virtual_offset += 1;
            }
            let Some(line) = self.get(index) else {
                break;
            };
            push_virtual_text_overlap(
                &line.text,
                virtual_offset,
                range_start,
                range_end,
                &mut fragment,
            );
            virtual_offset += line.text.len();
            if virtual_offset >= range_end {
                break;
            }
        }
        Cow::Owned(fragment)
    }

    fn borrow_virtual_fragment(
        &self,
        start: usize,
        end: usize,
        range_start: usize,
        range_end: usize,
    ) -> Option<&'a str> {
        let source = self.source?;
        let source_start = self.source_pos_for_virtual_offset(start, end, range_start)?;
        let source_end = self.source_pos_for_virtual_offset(start, end, range_end)?;
        let fragment = source.get(source_start..source_end)?;
        (!fragment.contains('\r')).then_some(fragment)
    }

    fn source_pos_for_virtual_offset(
        &self,
        start: usize,
        end: usize,
        offset: usize,
    ) -> Option<usize> {
        let mut virtual_offset = 0usize;
        let mut previous_line_end = None;
        for index in start..end {
            if index > start {
                if offset == virtual_offset {
                    return previous_line_end;
                }
                virtual_offset += 1;
            }
            let line = self.get(index)?;
            let line_len = line.text.len();
            if offset <= virtual_offset + line_len {
                return Some(line.start + offset - virtual_offset);
            }
            virtual_offset += line_len;
            previous_line_end = Some(line.end);
        }
        (offset == virtual_offset).then_some(previous_line_end?)
    }

    /// Collects a flow-like header prelude followed by a balanced brace body.
    pub(crate) fn collect_flow_block(&self, start: usize) -> CstBlockEvent<'a> {
        let Some(first) = self.get(start) else {
            return CstBlockEvent::new(Cow::Borrowed(""), Cow::Borrowed(""), 0, false, start);
        };
        let header_start = first.start;
        let mut header_end = first.end;
        let mut header = String::new();
        let mut header_owned = false;
        let mut header_has_lines = false;
        let mut end = first.end;
        let mut index = start;

        while let Some(line) = self.get(index) {
            if flow_line_starts_body(line, index == start) {
                break;
            }
            header_has_lines = true;
            if header_owned {
                if !header.is_empty() {
                    header.push('\n');
                }
                header.push_str(&line.text);
            } else if self.source.is_none() || line.text.contains('\r') {
                header_owned = true;
                header.push_str(&line.text);
            }
            header_end = line.end;
            end = line.end;
            index += 1;
        }

        if index >= self.len() {
            let header = if header_owned {
                Cow::Owned(header)
            } else {
                self.source
                    .and_then(|source| source.get(header_start..header_end))
                    .map_or_else(|| Cow::Owned(header), Cow::Borrowed)
            };
            return CstBlockEvent::new(header, Cow::Borrowed(""), end, false, index);
        }

        let mut body = self.collect_brace_block(index, CstBlockOpenRule::FlowBody);
        body.head = merge_flow_header(
            header_owned,
            header_has_lines,
            header,
            self.source,
            header_start,
            header_end,
            body.head,
        );
        body
    }
}
fn trim_cow(source: Cow<'_, str>) -> Cow<'_, str> {
    match source {
        Cow::Borrowed(source) => Cow::Borrowed(source.trim()),
        Cow::Owned(source) => Cow::Owned(source.trim().to_owned()),
    }
}

fn merge_flow_header<'a>(
    header_owned: bool,
    header_has_lines: bool,
    mut header: String,
    source: Option<&'a str>,
    header_start: usize,
    header_end: usize,
    body_head: Cow<'a, str>,
) -> Cow<'a, str> {
    if body_head.is_empty() {
        if header_owned {
            Cow::Owned(header)
        } else if !header_has_lines {
            Cow::Borrowed("")
        } else {
            source
                .and_then(|source| source.get(header_start..header_end))
                .map_or_else(|| Cow::Owned(header), Cow::Borrowed)
        }
    } else {
        if !header_has_lines && !header_owned {
            return body_head;
        }
        if !header.is_empty() {
            header.push('\n');
        } else if !header_owned
            && header_has_lines
            && let Some(source) = source.and_then(|source| source.get(header_start..header_end))
        {
            header.push_str(source);
            header.push('\n');
        }
        header.push_str(&body_head);
        Cow::Owned(header)
    }
}

fn push_virtual_text_overlap(
    text: &str,
    text_start: usize,
    range_start: usize,
    range_end: usize,
    output: &mut String,
) {
    let text_end = text_start + text.len();
    let start = range_start.max(text_start);
    let end = range_end.min(text_end);
    if start < end {
        output.push_str(&text[start - text_start..end - text_start]);
    }
}
impl<'a> Index<usize> for CstLineEvents<'a> {
    type Output = CstLine<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.lines[index]
    }
}

impl<'a> CstLine<'a> {
    fn from_node(node: &SyntaxNode) -> Self {
        let start = usize::from(node.text_range().start());
        let mut end = usize::from(node.text_range().end());
        let mut text = node.text().to_string();
        if text.ends_with("\r\n") {
            text.truncate(text.len() - 2);
            end -= 2;
        } else if text.ends_with('\n') || text.ends_with('\r') {
            text.truncate(text.len() - 1);
            end -= 1;
        }
        let kind = classify_node_line(node, &text);
        let punctuation = CstLinePunctuationSummary::from_node(node);
        let trim = line_trim_ranges(&text);
        Self {
            text: Cow::Owned(text),
            start,
            end,
            trim_start: trim.trim_start,
            trim_end: trim.trim_end,
            leading_trim_start: trim.leading_trim_start,
            punctuation,
            kind,
        }
    }

    fn from_node_and_source(node: &SyntaxNode, source: &'a str) -> Self {
        let start = usize::from(node.text_range().start());
        let mut end = usize::from(node.text_range().end());
        if source[start..end].ends_with("\r\n") {
            end -= 2;
        } else if source[start..end].ends_with('\n') || source[start..end].ends_with('\r') {
            end -= 1;
        }
        let text = &source[start..end];
        let kind = classify_node_line(node, text);
        let punctuation = CstLinePunctuationSummary::from_node(node);
        let trim = line_trim_ranges(text);
        Self {
            text: Cow::Borrowed(text),
            start,
            end,
            trim_start: trim.trim_start,
            trim_end: trim.trim_end,
            leading_trim_start: trim.leading_trim_start,
            punctuation,
            kind,
        }
    }

    /// Line text without a trailing newline.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Coarse line-event kind.
    pub const fn kind(&self) -> CstLineKind {
        self.kind
    }

    /// Trimmed line text.
    pub fn trimmed(&self) -> &str {
        &self.text[self.trim_start..self.trim_end]
    }

    /// Line text with leading whitespace removed.
    pub fn trim_start(&self) -> &str {
        &self.text[self.leading_trim_start..]
    }

    /// Returns true when the line should be skipped as trivia by grammar parsing.
    pub const fn is_trivia(&self) -> bool {
        matches!(self.kind, CstLineKind::Blank | CstLineKind::Comment)
    }

    /// Extracts a documentation-comment payload from a doc-comment line.
    pub fn doc_comment_text(&self) -> Option<&str> {
        let text = self.trim_start().strip_prefix("///")?;
        Some(text.strip_prefix(' ').unwrap_or(text))
    }

    /// Classifies a top-level line before declaration-specific parsing.
    pub(crate) fn top_level_line_kind(&self) -> CstTopLevelLineKind {
        classify_top_level_line(self.trimmed())
    }

    /// Classifies a top-level declaration line before AST construction.
    pub(crate) fn top_level_item_kind(&self) -> CstTopLevelItemKind {
        classify_top_level_item(self.trimmed())
    }

    /// Classifies a flow-body line before AST construction.
    pub(crate) fn flow_item_kind(&self) -> CstFlowItemKind {
        classify_flow_item(self.trimmed())
    }

    /// Start byte offset in the original source.
    pub const fn start(&self) -> usize {
        self.start
    }

    /// End byte offset before the line terminator.
    pub const fn end(&self) -> usize {
        self.end
    }

    pub(crate) const fn brace_delta(&self) -> i32 {
        self.punctuation.brace_delta
    }

    pub(crate) const fn punctuation_deltas(&self) -> CstPunctuationDeltas {
        CstPunctuationDeltas {
            brace: self.punctuation.brace_delta,
            paren: self.punctuation.paren_delta,
            bracket: self.punctuation.bracket_delta,
        }
    }

    pub(crate) const fn has_top_level_brace_open(&self) -> bool {
        self.punctuation.first_top_level_brace_open.is_some()
    }

    pub(crate) const fn has_unclosed_top_level_brace_open(&self) -> bool {
        self.has_top_level_brace_open() && self.punctuation.brace_delta > 0
    }

    pub(crate) const fn first_top_level_brace_open(&self) -> Option<usize> {
        self.punctuation.first_top_level_brace_open
    }

    pub(crate) const fn last_top_level_brace_open(&self) -> Option<usize> {
        self.punctuation.last_top_level_brace_open
    }

    pub(crate) const fn last_brace_close(&self) -> Option<usize> {
        self.punctuation.last_brace_close
    }

    fn with_relative_offsets(&self, base_offset: usize) -> Option<Self> {
        Some(Self {
            text: self.text.clone(),
            start: self.start.checked_sub(base_offset)?,
            end: self.end.checked_sub(base_offset)?,
            trim_start: self.trim_start,
            trim_end: self.trim_end,
            leading_trim_start: self.leading_trim_start,
            punctuation: self.punctuation,
            kind: self.kind,
        })
    }

    fn with_absolute_offsets(self, base_offset: usize) -> Option<Self> {
        Some(Self {
            text: self.text,
            start: self.start.checked_add(base_offset)?,
            end: self.end.checked_add(base_offset)?,
            trim_start: self.trim_start,
            trim_end: self.trim_end,
            leading_trim_start: self.leading_trim_start,
            punctuation: self.punctuation,
            kind: self.kind,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CstLineTrimRanges {
    trim_start: usize,
    trim_end: usize,
    leading_trim_start: usize,
}

fn line_trim_ranges(text: &str) -> CstLineTrimRanges {
    let leading_trim_start = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    let trim_start = if trimmed.is_empty() {
        text.len()
    } else {
        text.find(trimmed)
            .expect("trimmed text must be a substring of the original line")
    };
    CstLineTrimRanges {
        trim_start,
        trim_end: trim_start + trimmed.len(),
        leading_trim_start,
    }
}

fn classify_node_line(node: &SyntaxNode, text: &str) -> CstLineKind {
    let text_kind = classify_line(text);
    if text_kind != CstLineKind::Code {
        return text_kind;
    }

    let mut has_comment = false;
    let mut has_code_token = false;
    for token in node
        .children_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
    {
        match token.kind() {
            SyntaxKind::Whitespace | SyntaxKind::Newline => {}
            SyntaxKind::Comment => has_comment = true,
            _ => has_code_token = true,
        }
    }
    if has_comment && !has_code_token {
        CstLineKind::Comment
    } else {
        CstLineKind::Code
    }
}

impl CstLinePunctuationSummary {
    fn from_node(node: &SyntaxNode) -> Self {
        let mut summary = Self::default();
        let node_start = usize::from(node.text_range().start());
        let mut paren = 0usize;
        let mut square = 0usize;
        let mut brace = 0usize;
        let mut angle = 0usize;

        for token in node
            .children_with_tokens()
            .filter_map(rowan::NodeOrToken::into_token)
        {
            if token.kind() != SyntaxKind::Punctuation {
                continue;
            }
            if token.text() == "{" && paren == 0 && square == 0 && brace == 0 && angle == 0 {
                let offset = usize::from(token.text_range().start()) - node_start;
                if summary.first_top_level_brace_open.is_none() {
                    summary.first_top_level_brace_open = Some(offset);
                }
                summary.last_top_level_brace_open = Some(offset);
            }
            match token.text() {
                "{" => {
                    summary.brace_delta += 1;
                    brace += 1;
                }
                "}" => {
                    summary.brace_delta -= 1;
                    brace = brace.saturating_sub(1);
                    summary.last_brace_close =
                        Some(usize::from(token.text_range().start()) - node_start);
                }
                "(" => {
                    summary.paren_delta += 1;
                    paren += 1;
                }
                ")" => {
                    summary.paren_delta -= 1;
                    paren = paren.saturating_sub(1);
                }
                "[" => {
                    summary.bracket_delta += 1;
                    square += 1;
                }
                "]" => {
                    summary.bracket_delta -= 1;
                    square = square.saturating_sub(1);
                }
                "<" => angle += 1,
                ">" => angle = angle.saturating_sub(1),
                _ => {}
            }
        }
        summary
    }
}
impl<'a> CstBlockEvent<'a> {
    fn new(
        head: Cow<'a, str>,
        body: Cow<'a, str>,
        end: usize,
        ok: bool,
        next_index: usize,
    ) -> Self {
        Self {
            head,
            body,
            body_range: None,
            end,
            ok,
            next_index,
            body_line_range: None,
        }
    }

    fn with_body_range(mut self, body_range: Option<Range<usize>>) -> Self {
        self.body_range = body_range;
        self
    }

    fn with_body_line_range(mut self, body_line_range: Option<Range<usize>>) -> Self {
        self.body_line_range = body_line_range;
        self
    }

    pub(crate) fn owned_bytes(&self) -> usize {
        let head = match &self.head {
            Cow::Owned(value) => value.len(),
            Cow::Borrowed(_) => 0,
        };
        let body = match &self.body {
            Cow::Owned(value) => value.len(),
            Cow::Borrowed(_) => 0,
        };
        head + body
    }
}

fn block_event_is_complete(
    rule: CstBlockOpenRule,
    seen_open: bool,
    seen_body_open: bool,
    depth: i32,
) -> bool {
    match rule {
        CstBlockOpenRule::FirstTopLevel | CstBlockOpenRule::FlowBody => seen_open && depth == 0,
        CstBlockOpenRule::FunctionBody => seen_open && seen_body_open && depth == 0,
    }
}
