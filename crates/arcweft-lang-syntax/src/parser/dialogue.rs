use crate::ast::dialogue::DialogueContentSourceMap;
use crate::expr::{Expr, collect_dialogue_call_content_ranges};

use super::headers::simple_error;
use super::{
    BlockStyle, ContentCall, ContentCallParse, CstLine, DialogueContent, FlowItem, LinePlan,
    MappedDialogueSource, MappedDialogueSourceBuilder, Parser, RawSyntax, ScopeBlock, SpeakerLine,
    SpeakerLineSurface, Stmt, TextRange, attach_plan_to_dialogue_expr, contains_dialogue_expr,
    find_content_bracket, find_matching_punctuation, find_top_level_punctuation, flat_block_head,
    indentation, is_with_brace_head, parse_binding_pattern, parse_dialogue_call_expr_source,
    parse_expr_lossy, parse_flat_fence, parse_inline_with_colon_plan, parse_line_options,
    parse_line_plan_attachment, parse_line_plan_attachment_with_body_base, parse_with_brace_label,
    parse_with_indent_label, split_brace_item, split_brace_item_with_scan, split_call_head,
    split_leading_ident, split_speaker_line, split_top_level_binding,
};
use crate::cst::CstPunctuationScan;

impl Parser<'_> {
    pub(super) fn parse_let_dialogue_call(&mut self) -> Option<Stmt> {
        let start = self.current().clone();
        let mut text = start.text.trim().to_owned();
        let mut cursor = self.index;
        let mut bracket_delta = start.punctuation_deltas().bracket;

        while bracket_delta > 0 && cursor + 1 < self.events.len() {
            cursor += 1;
            bracket_delta += self.events[cursor].punctuation_deltas().bracket;
            text.push('\n');
            text.push_str(self.events[cursor].text.trim_end());
        }

        let trimmed = text.trim();
        let (_, rest) = split_leading_ident(trimmed).filter(|(kw, _)| *kw == "let")?;
        let (pattern, expr_text) = split_top_level_binding(rest)?;
        let text_offset = text.len() - text.trim_start().len();
        let after_let = &trimmed["let".len()..];
        let rest_offset =
            text_offset + "let".len() + after_let.len() - after_let.trim_start().len();
        let expr_offset = rest_offset + find_top_level_punctuation(rest, '=')? + 1;
        let open = find_content_bracket(expr_text).map(|offset| expr_offset + offset)?;
        let close = find_matching_punctuation(&text, open, '[', ']')?;
        let expr_untrimmed = &text[expr_offset..=close];
        let expr_leading = expr_untrimmed.len() - expr_untrimmed.trim_start().len();
        let expr_source = expr_untrimmed.trim();
        let trailing_untrimmed = &text[close + 1..];
        let trailing_leading = trailing_untrimmed.len() - trailing_untrimmed.trim_start().len();
        let trailing = trailing_untrimmed.trim();
        let line_leading = start.text.len() - start.text.trim_start().len();
        let final_line = &self.events[cursor];
        let final_line_trailing = final_line.text.len() - final_line.text.trim_end().len();
        let close_end = final_line
            .end
            .checked_sub(final_line_trailing + trailing_untrimmed.len())?;
        let trailing_start = close_end + trailing_leading;
        let inline_plan = self.take_trailing_line_plan(trailing, trailing_start, &mut cursor);

        self.index = cursor + 1;
        let plan = inline_plan.or_else(|| self.take_optional_line_plan());
        let plan_end = plan.as_ref().map(|plan| plan.range().end());
        let mut expr = parse_dialogue_call_expr_source(expr_source)
            .unwrap_or_else(|| parse_expr_lossy(expr_source));
        let has_dialogue = if let Some(plan) = plan {
            attach_plan_to_dialogue_expr(&mut expr, plan)
        } else {
            contains_dialogue_expr(&expr)
        };
        if !has_dialogue {
            return None;
        }

        let (pattern, ty) = parse_binding_pattern(
            pattern,
            start.start + line_leading + text.find(pattern).unwrap_or_default(),
        );
        let expr_start = start.start + line_leading + expr_offset + expr_leading;
        let (expr_source, expr_range) = plan_end
            .and_then(|end| {
                self.source_text_in_range(expr_start, end).map(|source| {
                    (
                        source.trim_end().to_owned(),
                        TextRange::new(expr_start, end),
                    )
                })
            })
            .unwrap_or_else(|| {
                (
                    expr_source.to_owned(),
                    TextRange::new(expr_start, close_end),
                )
            });
        self.attach_dialogue_expr_content_source_map(&mut expr, &expr_source, expr_range);
        Some(Stmt::Let {
            pattern,
            ty,
            expr,
            expr_source: Some(expr_source),
            expr_range: Some(expr_range),
        })
    }

    fn source_text_in_range(&self, start: usize, end: usize) -> Option<String> {
        self.mapped_source_in_range(start, end)
            .map(|mapped| mapped.raw)
    }

    fn mapped_source_in_range(&self, start: usize, end: usize) -> Option<MappedDialogueSource> {
        if start >= end {
            return None;
        }
        let mut mapped = MappedDialogueSourceBuilder::new(start);
        let mut found = false;
        for line in self.events.iter() {
            if line.end() <= start || line.start() >= end {
                continue;
            }
            let line_start = start.saturating_sub(line.start());
            let line_end = end.min(line.end()).saturating_sub(line.start());
            let text = line.text().get(line_start..line_end)?;
            mapped.push_line(
                text,
                TextRange::new(line.start() + line_start, line.start() + line_end),
            );
            found = true;
        }
        found.then(|| mapped.finish())
    }

    fn attach_dialogue_expr_content_source_map(
        &self,
        expr: &mut Expr,
        expr_source: &str,
        expr_range: TextRange,
    ) {
        let Some(mapped_expr) = self
            .mapped_source_in_range(expr_range.start(), expr_range.end())
            .and_then(|mapped| mapped.trim())
        else {
            return;
        };
        if mapped_expr.raw != expr_source {
            return;
        }
        let Some(content_range) = collect_dialogue_call_content_ranges(
            expr,
            &mapped_expr.raw,
            TextRange::new(0, mapped_expr.raw.len()),
        )
        .into_iter()
        .next() else {
            return;
        };
        let Some(mapped_content) = mapped_expr.slice(content_range) else {
            return;
        };
        replace_dialogue_call_content_source_map(expr, mapped_content.source_map);
    }

    pub(super) fn parse_content_call_or_speaker_line(&mut self) -> Option<FlowItem> {
        let line = self.current().clone();
        let trimmed = line.text.trim();
        let line_leading = line.text.len() - line.text.trim_start().len();

        if let Some(parts) = split_speaker_line(trimmed) {
            self.index += 1;
            let has_inline_content = !parts.inline_content.is_empty();
            let content = if has_inline_content {
                let content_start = line.start + line_leading + parts.inline_content_range.start;
                let inline_content =
                    self.take_inline_dialogue_content(parts.inline_content, content_start);
                self.dialogue_content(inline_content)
            } else {
                self.take_indented_dialogue(indentation(&line.text) + 1, line.start)
            };
            let plan = self.take_optional_line_plan();
            let option_args = parts.arguments.as_ref().map(|(args, relative)| {
                (args.as_str(), line.start + line_leading + relative.start)
            });
            let absolute_range = |relative: &std::ops::Range<usize>| {
                TextRange::new(
                    line.start + line_leading + relative.start,
                    line.start + line_leading + relative.end,
                )
            };
            let surface = SpeakerLineSurface::new(
                TextRange::new(line.start, line.end),
                absolute_range(&parts.head_range),
                parts
                    .arguments
                    .as_ref()
                    .map(|(_, range)| absolute_range(range)),
                has_inline_content.then(|| absolute_range(&parts.inline_content_range)),
            );
            return Some(FlowItem::SpeakerLine(SpeakerLine::new(
                parts.speaker,
                parse_line_options(option_args, &mut self.errors),
                content,
                plan,
                surface,
                TextRange::new(line.start, self.previous_end()),
            )));
        }

        let content_call_start = self.index;
        if let Some((callee, args, content, consumed_end, inline_plan, trailing_block)) =
            self.try_take_content_call()
        {
            let plan = inline_plan.or_else(|| self.take_optional_line_plan());
            if let Some(block) = trailing_block {
                self.pending_flow_items.push(FlowItem::Scope(block));
            }
            let option_args = args.as_ref().map(|(args, base)| (args.as_str(), *base));
            return Some(FlowItem::ContentCall(ContentCall::new(
                callee,
                parse_line_options(option_args, &mut self.errors),
                content,
                plan,
                TextRange::new(line.start, consumed_end),
            )));
        }

        if self.index != content_call_start {
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, self.previous_end())),
            )));
        }

        None
    }

    fn try_take_content_call(&mut self) -> Option<ContentCallParse> {
        let start = self.current().clone();
        let line_leading = start.text.len() - start.text.trim_start().len();
        let first_text = start.text.trim();
        let first_start = start.start + line_leading;
        let mut mapped_text = MappedDialogueSourceBuilder::new(first_start);
        mapped_text.push_line(
            first_text,
            TextRange::new(first_start, first_start + first_text.len()),
        );
        let mut end = start.end;
        let mut cursor = self.index;
        let mut bracket_delta = start.punctuation_deltas().bracket;

        while bracket_delta > 0 && cursor + 1 < self.events.len() {
            cursor += 1;
            bracket_delta += self.events[cursor].punctuation_deltas().bracket;
            let line = &self.events[cursor];
            let text = line.text.trim_end();
            mapped_text.push_line(text, TextRange::new(line.start, line.start + text.len()));
            end = line.end;
        }
        let mapped_text = mapped_text.finish();
        let text = &mapped_text.raw;

        let open = find_content_bracket(text)?;
        let Some(close) = find_matching_punctuation(text, open, '[', ']') else {
            self.index = cursor + 1;
            let diagnostic_range = mapped_text
                .source_map
                .source_range(TextRange::new(open, text.len()))
                .unwrap_or_else(|| TextRange::new(start.start + open, end));
            self.push_error(
                diagnostic_range,
                "unclosed dialogue content block",
                ["]"],
                Some(&text[open..]),
                ["insert a closing `]` for the dialogue content block"],
            );
            return None;
        };
        let before = text[..open].trim();
        if before.is_empty() || before.starts_with('@') && !before.starts_with("@<") {
            return None;
        }
        let before_start = text[..open].find(before).unwrap_or(0);
        let (callee, args) = split_call_head(before);
        let args = args
            .map(|(args, relative)| (args, start.start + line_leading + before_start + relative));
        let trailing_untrimmed = &text[close + 1..];
        let trailing_leading = trailing_untrimmed.len() - trailing_untrimmed.trim_start().len();
        let trailing = trailing_untrimmed.trim();
        let trailing_start = start.start + line_leading + close + 1 + trailing_leading;
        let inline_plan = self.take_trailing_line_plan(trailing, trailing_start, &mut cursor);
        let trailing_block = inline_plan
            .is_none()
            .then(|| self.take_trailing_bare_scope(text, close, &mut cursor, start.start))
            .flatten();
        self.index = cursor + 1;
        let content_source = mapped_text.slice(TextRange::new(open + 1, close))?.trim()?;
        let content = self.dialogue_content(content_source);
        let consumed_end = trailing_block
            .as_ref()
            .map_or(end, |block| block.range().end());
        Some((
            callee,
            args,
            content,
            consumed_end,
            inline_plan,
            trailing_block,
        ))
    }

    fn take_trailing_line_plan(
        &mut self,
        trailing: &str,
        trailing_start: usize,
        cursor: &mut usize,
    ) -> Option<LinePlan> {
        if !trailing.starts_with("with") {
            return None;
        }
        if is_with_brace_head(trailing) {
            let punctuation = CstPunctuationScan::new(trailing);
            let mut brace_delta = punctuation.deltas().brace;
            if brace_delta <= 0 {
                let source_base = trailing_start;
                let (head, body, body_base) =
                    split_brace_item_with_body_base(trailing, source_base, &punctuation)?;
                let range = TextRange::new(trailing_start, self.events[*cursor].end);
                return Some(parse_line_plan_attachment_with_body_base(
                    BlockStyle::Brace,
                    body,
                    body_base,
                    range,
                    parse_with_brace_label(head.trim()),
                    &mut self.errors,
                ));
            }
            drop(punctuation);
            let mut block_text = trailing.to_owned();
            while brace_delta > 0 && *cursor + 1 < self.events.len() {
                *cursor += 1;
                brace_delta += self.events[*cursor].punctuation_deltas().brace;
                block_text.push('\n');
                block_text.push_str(self.events[*cursor].text.trim_end());
            }
            self.syntax_stats.block_owned_bytes += block_text.len();
            let source_base = trailing_start;
            let punctuation = CstPunctuationScan::new(&block_text);
            let (head, body, body_base) =
                split_brace_item_with_body_base(&block_text, source_base, &punctuation)?;
            let range = TextRange::new(trailing_start, self.events[*cursor].end);
            return Some(parse_line_plan_attachment_with_body_base(
                BlockStyle::Brace,
                body,
                body_base,
                range,
                parse_with_brace_label(head.trim()),
                &mut self.errors,
            ));
        }
        parse_inline_with_colon_plan(trailing).map(|(label, body)| {
            parse_line_plan_attachment(
                BlockStyle::Indent,
                body,
                TextRange::new(trailing_start, trailing_start + trailing.len()),
                label,
                &mut self.errors,
            )
        })
    }

    fn take_trailing_bare_scope(
        &mut self,
        text: &str,
        close_bracket: usize,
        cursor: &mut usize,
        base: usize,
    ) -> Option<ScopeBlock> {
        let block_text = text[close_bracket + 1..].trim();
        if !block_text.starts_with('{') {
            return None;
        }
        let punctuation = CstPunctuationScan::new(block_text);
        let mut brace_delta = punctuation.deltas().brace;
        if brace_delta <= 0 {
            let (head, body) = split_brace_item_with_scan(block_text, &punctuation)?;
            if !head.trim().is_empty() {
                return None;
            }
            return Some(ScopeBlock::new(
                None,
                self.parse_flow_body(body, base + close_bracket + 1),
                TextRange::new(base + close_bracket + 1, self.events[*cursor].end),
            ));
        }
        drop(punctuation);
        let mut block_text = block_text.to_owned();
        while brace_delta > 0 && *cursor + 1 < self.events.len() {
            *cursor += 1;
            brace_delta += self.events[*cursor].punctuation_deltas().brace;
            block_text.push('\n');
            block_text.push_str(self.events[*cursor].text.trim_end());
        }
        self.syntax_stats.block_owned_bytes += block_text.len();
        let (head, body) = split_brace_item(&block_text)?;
        if !head.trim().is_empty() {
            return None;
        }
        Some(ScopeBlock::new(
            None,
            self.parse_flow_body(body, base + close_bracket + 1),
            TextRange::new(base + close_bracket + 1, self.events[*cursor].end),
        ))
    }

    fn take_optional_line_plan(&mut self) -> Option<LinePlan> {
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim();
        if let Some(fence) = parse_flat_fence(trimmed)
            && !fence.close
            && fence.kind == "with"
        {
            let body = self.take_flat_block_body("with", line.start);
            return Some(parse_line_plan_attachment(
                BlockStyle::Flat,
                &body,
                TextRange::new(line.start, self.previous_end()),
                parse_with_brace_label(&flat_block_head("with", fence.head)),
                &mut self.errors,
            ));
        }
        if let Some(label) = parse_with_indent_label(trimmed) {
            self.index += 1;
            return Some(self.take_indented_line_plan(
                indentation(&line.text) + 1,
                line.start,
                label.into_option(),
            ));
        }
        if is_with_brace_head(trimmed) {
            let (head, body, end, ok) = self.take_brace_block();
            if !ok {
                self.push_error(
                    TextRange::new(line.start, line.end),
                    "unclosed block while parsing line plan",
                    ["}"],
                    Some(head.trim()),
                    ["insert a closing `}` for the line plan"],
                );
                return None;
            }
            return Some(parse_line_plan_attachment(
                BlockStyle::Brace,
                &body,
                TextRange::new(line.start, end),
                parse_with_brace_label(head.trim()),
                &mut self.errors,
            ));
        }
        None
    }

    pub(super) fn take_flat_block_body(&mut self, expected_kind: &str, start: usize) -> String {
        // Consume the opening fence. Nested flat blocks are immediately converted
        // to brace-form source so the existing block parsers remain the single
        // source of AST construction rules.
        self.index += 1;
        let mut body = String::new();
        while self.index < self.events.len() {
            let line = self.current().clone();
            let trimmed = line.text.trim();
            if let Some(fence) = parse_flat_fence(trimmed) {
                if fence.close {
                    if fence.kind != expected_kind {
                        self.push_flat_close_mismatch(expected_kind, &line);
                    }
                    self.index += 1;
                    return body;
                }
                let nested_kind = fence.kind.to_owned();
                let nested_head = fence.head.trim().to_owned();
                let nested_body = self.take_flat_block_body(&nested_kind, line.start);
                if !body.is_empty() {
                    body.push('\n');
                }
                let canonical_head = if nested_head.is_empty() {
                    nested_kind
                } else {
                    format!("{nested_kind} {nested_head}")
                };
                body.push_str(&canonical_head);
                body.push_str(" {\n");
                body.push_str(&nested_body);
                body.push_str("\n}");
                continue;
            }
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line.text.trim_end());
            self.index += 1;
        }
        self.errors.push(simple_error(
            start,
            expected_kind.len(),
            &format!("missing close fence `=== /{expected_kind} ===`"),
            &format!("=== /{expected_kind} ==="),
        ));
        body
    }

    pub(super) fn push_flat_close_mismatch(&mut self, expected_kind: &str, line: &CstLine) {
        self.errors.push(simple_error(
            line.start,
            line.end.saturating_sub(line.start),
            &format!("flat fence close mismatch; expected `=== /{expected_kind} ===`"),
            &format!("=== /{expected_kind} ==="),
        ));
    }

    fn take_indented_dialogue(&mut self, min_indent: usize, start: usize) -> DialogueContent {
        let mut content = MappedDialogueSourceBuilder::new(start);
        while self.index < self.events.len() {
            let line = self.current().clone();
            if line.text.trim().is_empty() {
                content.push_line("", TextRange::new(line.end, line.end));
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent || line.text.trim_start().starts_with("with") {
                break;
            }
            let trimmed = line.text.trim();
            let leading = line.text.len() - line.text.trim_start().len();
            let source_start = line.start + leading;
            content.push_line(
                trimmed,
                TextRange::new(source_start, source_start + trimmed.len()),
            );
            self.index += 1;
        }
        self.dialogue_content(content.finish())
    }

    fn take_inline_dialogue_content(
        &mut self,
        first_line: &str,
        source_start: usize,
    ) -> MappedDialogueSource {
        let mut content = MappedDialogueSourceBuilder::new(source_start);
        content.push_line(
            first_line,
            TextRange::new(source_start, source_start + first_line.len()),
        );
        let mut expr_bracket_depth = dialogue_expr_bracket_depth(first_line, 0);
        while expr_bracket_depth > 0 && self.index < self.events.len() {
            let line = self.current().clone();
            let trimmed = line.text.trim();
            let leading = line.text.len() - line.text.trim_start().len();
            let trimmed_start = line.start + leading;
            content.push_line(
                trimmed,
                TextRange::new(trimmed_start, trimmed_start + trimmed.len()),
            );
            expr_bracket_depth = dialogue_expr_bracket_depth(trimmed, expr_bracket_depth);
            self.index += 1;
        }
        content.finish()
    }

    fn take_indented_line_plan(
        &mut self,
        min_indent: usize,
        start: usize,
        label: Option<String>,
    ) -> LinePlan {
        let mut raw = String::new();
        let mut end = start;
        let mut body_start = None;
        while self.index < self.events.len() {
            let line = self.current();
            let trimmed = line.text.trim();
            if trimmed.is_empty() {
                raw.push('\n');
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent || trimmed.starts_with(')') {
                break;
            }
            if !raw.is_empty() {
                raw.push('\n');
            }
            body_start.get_or_insert(line.start);
            raw.push_str(&line.text);
            end = line.end;
            self.index += 1;
        }
        let body_base = body_start.unwrap_or(end);
        let body = self.source.get(body_base..end).unwrap_or(raw.as_str());
        parse_line_plan_attachment_with_body_base(
            BlockStyle::Indent,
            body,
            body_base,
            TextRange::new(start, end),
            label,
            &mut self.errors,
        )
    }
}

fn split_brace_item_with_body_base<'a>(
    source: &'a str,
    source_base: usize,
    punctuation: &CstPunctuationScan<'a>,
) -> Option<(&'a str, &'a str, usize)> {
    let open = punctuation.find_top_level_punctuation('{')?;
    let close = punctuation.find_matching_punctuation(open, '{', '}')?;
    if !source[close + '}'.len_utf8()..].trim().is_empty() {
        return None;
    }
    let raw_body = &source[open + '{'.len_utf8()..close];
    let body = raw_body.trim();
    let body_leading = raw_body.len() - raw_body.trim_start().len();
    Some((
        source[..open].trim(),
        body,
        source_base + open + '{'.len_utf8() + body_leading,
    ))
}

fn dialogue_expr_bracket_depth(source: &str, mut depth: usize) -> usize {
    let mut chars = source.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if depth == 0 {
            if ch == '#' && chars.peek().is_some_and(|(_, next)| *next == '[') {
                let _ = chars.next();
                depth = 1;
            }
            continue;
        }
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

fn replace_dialogue_call_content_source_map(expr: &mut Expr, source_map: DialogueContentSourceMap) {
    match expr {
        Expr::DialogueCall { content, .. } => content.replace_source_map(source_map),
        Expr::Try(try_expr) => {
            replace_dialogue_call_content_source_map(try_expr.operand_mut(), source_map);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclosed_brace_line_plan_reports_the_line_plan_owner() {
        let mut parser = Parser::new("with {\n    at(0.42s) { alice.stage.face(worried)\n");

        assert!(parser.take_optional_line_plan().is_none());
        let [error] = parser.errors.as_slice() else {
            panic!("expected one line-plan recovery diagnostic");
        };
        assert_eq!(error.message(), "unclosed block while parsing line plan");
        assert_eq!(error.expected(), &["}"]);
        assert!(!error.recovery().is_empty());
    }
}
