use crate::ast::dialogue::{
    DialogueDefaultAssignOp, DialogueDefaultAssignment, DialogueDefaultPath, DialogueDefaultsItem,
};

use super::headers::{parse_optional_decl_entity_ref, parse_visibility_prefix, simple_error};
use super::{
    BlockStyle, ContentCall, ContentCallParse, CstLine, DialogueContent, FlowItem, LinePlan,
    Parser, ScopeBlock, SpeakerLine, Stmt, TextRange, attach_plan_to_dialogue_expr,
    contains_dialogue_expr, find_content_bracket, find_matching_punctuation,
    find_top_level_punctuation, flat_block_head, indentation, is_with_brace_head,
    parse_binding_pattern, parse_dialogue_call_expr_source, parse_expr_lossy, parse_flat_fence,
    parse_inline_with_colon_plan, parse_line_options, parse_line_plan_attachment,
    parse_with_brace_label, parse_with_indent_label, split_brace_item, split_brace_item_with_scan,
    split_call_head, split_leading_ident, split_speaker_line, split_top_level_binding,
    split_top_level_punctuation_once,
};
use crate::cst::CstPunctuationScan;

impl Parser<'_> {
    pub(super) fn parse_dialogue_defaults(&mut self) -> Option<DialogueDefaultsItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing dialogue defaults",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the dialogue defaults body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let after_defaults = rest
            .trim_start()
            .strip_prefix("dialogue defaults")?
            .trim_start();
        if starts_dialogue_defaults_relative_id(after_defaults) {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "dialogue defaults profiles cannot use relative IDs",
                ["@dialogue.defaults", "@dialogue.defaults.mobile"],
                Some(after_defaults),
                ["write the full defaults profile ID"],
            );
        }
        let (id, tail) = parse_optional_decl_entity_ref(
            after_defaults,
            "dialogue",
            start_line.start,
            &mut self.errors,
        );
        if !tail.trim().is_empty() {
            self.push_error(
                TextRange::new(start_line.start, start_line.start + head.len()),
                "unexpected tokens after dialogue defaults header",
                ["{"],
                Some(tail.trim()),
                ["move defaults into the declaration body"],
            );
        }
        let body_base = start_line.start
            + start_line
                .text
                .find('{')
                .map_or_else(|| head.len() + "{".len(), |open| open + "{".len());
        let assignments = parse_dialogue_default_assignments(
            &body,
            body_base,
            TextRange::new(start_line.start, end),
            &mut self.errors,
        );
        Some(DialogueDefaultsItem::new(
            attrs,
            visibility,
            id,
            assignments,
            TextRange::new(start_line.start, end),
        ))
    }

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
        let expr_source = text[expr_offset..=close].trim();
        let trailing = text[close + 1..].trim();
        let inline_plan = self.take_trailing_line_plan(trailing, close, &mut cursor, start.start);

        self.index = cursor + 1;
        let plan = inline_plan.or_else(|| self.take_optional_line_plan());
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

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let { pattern, ty, expr })
    }

    pub(super) fn parse_content_call_or_speaker_line(&mut self) -> Option<FlowItem> {
        let line = self.current().clone();
        let trimmed = line.text.trim();
        let line_leading = line.text.len() - line.text.trim_start().len();

        if let Some((speaker, args, inline_content)) = split_speaker_line(trimmed) {
            self.index += 1;
            let content = if inline_content.is_empty() {
                self.take_indented_dialogue(indentation(&line.text) + 1, line.start)
            } else {
                self.dialogue_content(
                    inline_content.to_owned(),
                    TextRange::new(line.start, line.end),
                )
            };
            let plan = self.take_optional_line_plan();
            let option_args = args
                .as_ref()
                .map(|(args, relative)| (args.as_str(), line.start + line_leading + relative));
            return Some(FlowItem::SpeakerLine(SpeakerLine::new(
                speaker,
                parse_line_options(option_args, &mut self.errors),
                content,
                plan,
                TextRange::new(line.start, self.previous_end()),
            )));
        }

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

        None
    }

    fn try_take_content_call(&mut self) -> Option<ContentCallParse> {
        let start = self.current().clone();
        let line_leading = start.text.len() - start.text.trim_start().len();
        let mut text = start.text.trim().to_owned();
        let mut end = start.end;
        let mut cursor = self.index;
        let mut bracket_delta = start.punctuation_deltas().bracket;

        while bracket_delta > 0 && cursor + 1 < self.events.len() {
            cursor += 1;
            bracket_delta += self.events[cursor].punctuation_deltas().bracket;
            text.push('\n');
            text.push_str(self.events[cursor].text.trim_end());
            end = self.events[cursor].end;
        }

        let open = find_content_bracket(&text)?;
        let Some(close) = find_matching_punctuation(&text, open, '[', ']') else {
            self.index = cursor + 1;
            self.push_error(
                TextRange::new(start.start + open, end),
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
        let raw_content = text[open + 1..close].trim().to_owned();
        let trailing = text[close + 1..].trim();
        let inline_plan = self.take_trailing_line_plan(trailing, close, &mut cursor, start.start);
        let trailing_block = inline_plan
            .is_none()
            .then(|| self.take_trailing_bare_scope(&text, close, &mut cursor, start.start))
            .flatten();
        self.index = cursor + 1;
        let content = self.dialogue_content(
            raw_content.clone(),
            TextRange::new(start.start + open + 1, start.start + close),
        );
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
        close_bracket: usize,
        cursor: &mut usize,
        base: usize,
    ) -> Option<LinePlan> {
        if !trailing.starts_with("with") {
            return None;
        }
        if is_with_brace_head(trailing) {
            let punctuation = CstPunctuationScan::new(trailing);
            let mut brace_delta = punctuation.deltas().brace;
            if brace_delta <= 0 {
                let (head, body) = split_brace_item_with_scan(trailing, &punctuation)?;
                let range = TextRange::new(base + close_bracket + 1, self.events[*cursor].end);
                return Some(parse_line_plan_attachment(
                    BlockStyle::Brace,
                    body,
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
            let (head, body) = split_brace_item(&block_text)?;
            let range = TextRange::new(base + close_bracket + 1, self.events[*cursor].end);
            return Some(parse_line_plan_attachment(
                BlockStyle::Brace,
                body,
                range,
                parse_with_brace_label(head.trim()),
                &mut self.errors,
            ));
        }
        parse_inline_with_colon_plan(trailing).map(|(label, body)| {
            parse_line_plan_attachment(
                BlockStyle::Indent,
                body,
                TextRange::new(
                    base + close_bracket + 1,
                    base + close_bracket + 1 + trailing.len(),
                ),
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
        let mut raw = String::new();
        let mut end = start;
        while self.index < self.events.len() {
            let line = self.current();
            if line.text.trim().is_empty() {
                raw.push('\n');
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent || line.text.trim_start().starts_with("with") {
                break;
            }
            if !raw.is_empty() {
                raw.push('\n');
            }
            raw.push_str(line.text.trim());
            end = line.end;
            self.index += 1;
        }
        self.dialogue_content(raw.clone(), TextRange::new(start, end))
    }

    fn take_indented_line_plan(
        &mut self,
        min_indent: usize,
        start: usize,
        label: Option<String>,
    ) -> LinePlan {
        let mut raw = String::new();
        let mut end = start;
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
            raw.push_str(&line.text);
            end = line.end;
            self.index += 1;
        }
        parse_line_plan_attachment(
            BlockStyle::Indent,
            &raw,
            TextRange::new(start, end),
            label,
            &mut self.errors,
        )
    }
}

fn starts_dialogue_defaults_relative_id(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.starts_with("@.") || trimmed.starts_with("@..") || trimmed.starts_with("@dialogue:.")
}

fn parse_dialogue_default_assignments(
    body: &str,
    base: usize,
    fallback_range: TextRange,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<DialogueDefaultAssignment> {
    let mut assignments = Vec::new();
    let mut path_stack: Vec<String> = Vec::new();
    let mut line_start = 0usize;
    for line in body.split_inclusive('\n') {
        let line_without_eol = line.trim_end_matches(['\r', '\n']);
        let trimmed = line_without_eol.trim();
        let trimmed_start_in_line = line_without_eol
            .len()
            .saturating_sub(line_without_eol.trim_start().len());
        let trimmed_start = base + line_start + trimmed_start_in_line;
        let trimmed_range = TextRange::new(trimmed_start, trimmed_start + trimmed.len());
        line_start += line.len();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if trimmed == "}" {
            if path_stack.pop().is_none() {
                errors.push(simple_error(
                    trimmed_range.start(),
                    trimmed_range.end() - trimmed_range.start(),
                    "unexpected closing brace in dialogue defaults",
                    "nested defaults block",
                ));
            }
            continue;
        }
        if let Some(block_name) = trimmed.strip_suffix('{').map(str::trim) {
            if block_name.is_empty() || block_name.contains(char::is_whitespace) {
                errors.push(simple_error(
                    trimmed_range.start(),
                    trimmed_range.end() - trimmed_range.start(),
                    "expected dialogue defaults block path",
                    "rich_text {",
                ));
            } else {
                path_stack.extend(block_name.split('.').map(str::trim).map(str::to_owned));
            }
            continue;
        }
        if trimmed.contains('{') || trimmed.contains('}') {
            errors.push(simple_error(
                trimmed_range.start(),
                trimmed_range.end() - trimmed_range.start(),
                "one-line nested dialogue defaults blocks are not canonical",
                "write nested assignments on separate lines",
            ));
            continue;
        }
        let Some((name, op, value)) = split_dialogue_default_assignment(trimmed) else {
            errors.push(simple_error(
                trimmed_range.start(),
                trimmed_range.end() - trimmed_range.start(),
                "expected dialogue default assignment",
                "name = expr",
            ));
            continue;
        };
        let mut segments = path_stack.clone();
        segments.extend(name.split('.').map(str::trim).map(str::to_owned));
        let Some(path) = DialogueDefaultPath::from_dotted(&segments.join(".")) else {
            errors.push(simple_error(
                trimmed_range.start(),
                trimmed_range.end() - trimmed_range.start(),
                "expected dialogue default assignment path",
                "rich_text.ruby.size = expr",
            ));
            continue;
        };
        let value = value.trim();
        let name_offset = trimmed.find(name).unwrap_or_default();
        let value_offset = trimmed
            .rfind(value)
            .unwrap_or_else(|| trimmed.len().saturating_sub(value.len()));
        assignments.push(DialogueDefaultAssignment::new(
            path,
            op,
            parse_expr_lossy(value),
            value.to_owned(),
            trimmed_range,
            TextRange::new(
                trimmed_start + name_offset,
                trimmed_start + name_offset + name.len(),
            ),
            TextRange::new(
                trimmed_start + value_offset,
                trimmed_start + value_offset + value.len(),
            ),
        ));
    }
    if !path_stack.is_empty() {
        errors.push(simple_error(
            fallback_range.start(),
            fallback_range.end() - fallback_range.start(),
            "unclosed nested dialogue defaults block",
            "}",
        ));
    }
    assignments
}

fn split_dialogue_default_assignment(
    source: &str,
) -> Option<(&str, DialogueDefaultAssignOp, &str)> {
    if let Some((name, value)) = source.split_once("+=") {
        return Some((name.trim(), DialogueDefaultAssignOp::Append, value.trim()));
    }
    split_top_level_punctuation_once(source, '=')
        .map(|(name, value)| (name.trim(), DialogueDefaultAssignOp::Replace, value.trim()))
}
