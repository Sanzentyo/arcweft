use super::{
    BlockStyle, ContentCall, ContentCallParse, CstLine, DialogueContent, FlowItem, LinePlan,
    Parser, ScopeBlock, SpeakerLine, Stmt, TextRange, attach_line_plan_label,
    attach_plan_to_dialogue_expr, contains_dialogue_expr, find_content_bracket,
    find_matching_punctuation, find_top_level_punctuation, indentation, is_with_brace_head,
    parse_binding_pattern, parse_dialogue_call_expr_source, parse_dialogue_tokens,
    parse_expr_lossy, parse_flat_fence, parse_inline_with_colon_plan, parse_line_options,
    parse_line_plan_body, parse_with_brace_label, parse_with_indent_label, punctuation_delta,
    simple_error, split_brace_item, split_call_head, split_leading_ident, split_speaker_line,
    split_top_level_binding,
};

impl Parser {
    pub(super) fn parse_let_dialogue_call(&mut self) -> Option<Stmt> {
        let start = self.current().clone();
        let mut text = start.text.trim().to_owned();
        let mut cursor = self.index;

        while punctuation_delta(&text, '[', ']') > 0 && cursor + 1 < self.events.len() {
            cursor += 1;
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

        if let Some((speaker, args, inline_content)) = split_speaker_line(trimmed) {
            self.index += 1;
            let content = if inline_content.is_empty() {
                self.take_indented_dialogue(indentation(&line.text) + 1, line.start)
            } else {
                DialogueContent::new(
                    inline_content.to_owned(),
                    parse_dialogue_tokens(inline_content),
                    TextRange::new(line.start, line.end),
                )
            };
            let plan = self.take_optional_line_plan();
            return Some(FlowItem::SpeakerLine(SpeakerLine::new(
                speaker,
                parse_line_options(args.as_deref(), line.start, &mut self.errors),
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
            return Some(FlowItem::ContentCall(ContentCall::new(
                callee,
                parse_line_options(args.as_deref(), line.start, &mut self.errors),
                content,
                plan,
                TextRange::new(line.start, consumed_end),
            )));
        }

        None
    }

    fn try_take_content_call(&mut self) -> Option<ContentCallParse> {
        let start = self.current().clone();
        let mut text = start.text.trim().to_owned();
        let mut end = start.end;
        let mut cursor = self.index;

        while punctuation_delta(&text, '[', ']') > 0 && cursor + 1 < self.events.len() {
            cursor += 1;
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
        let (callee, args) = split_call_head(before);
        let raw_content = text[open + 1..close].trim().to_owned();
        let trailing = text[close + 1..].trim();
        let inline_plan = self.take_trailing_line_plan(trailing, close, &mut cursor, start.start);
        let trailing_block = inline_plan
            .is_none()
            .then(|| self.take_trailing_bare_scope(&text, close, &mut cursor, start.start))
            .flatten();
        self.index = cursor + 1;
        let content = DialogueContent::new(
            raw_content.clone(),
            parse_dialogue_tokens(&raw_content),
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
            let mut block_text = trailing.to_owned();
            while punctuation_delta(&block_text, '{', '}') > 0 && *cursor + 1 < self.events.len() {
                *cursor += 1;
                block_text.push('\n');
                block_text.push_str(self.events[*cursor].text.trim_end());
            }
            let (head, body) = split_brace_item(&block_text)?;
            let range = TextRange::new(base + close_bracket + 1, self.events[*cursor].end);
            return Some(attach_line_plan_label(
                parse_line_plan_body(BlockStyle::Brace, body, range),
                parse_with_brace_label(head.trim()),
            ));
        }
        parse_inline_with_colon_plan(trailing).map(|(label, body)| {
            attach_line_plan_label(
                parse_line_plan_body(
                    BlockStyle::Indent,
                    body,
                    TextRange::new(
                        base + close_bracket + 1,
                        base + close_bracket + 1 + trailing.len(),
                    ),
                ),
                label,
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
        let mut block_text = text[close_bracket + 1..].trim().to_owned();
        if !block_text.starts_with('{') {
            return None;
        }
        while punctuation_delta(&block_text, '{', '}') > 0 && *cursor + 1 < self.events.len() {
            *cursor += 1;
            block_text.push('\n');
            block_text.push_str(self.events[*cursor].text.trim_end());
        }
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
            return Some(parse_line_plan_body(
                BlockStyle::Flat,
                &body,
                TextRange::new(line.start, self.previous_end()),
            ));
        }
        if let Some(label) = parse_with_indent_label(trimmed) {
            self.index += 1;
            let plan = self.take_indented_line_plan(indentation(&line.text) + 1, line.start);
            return Some(attach_line_plan_label(plan, label.into_option()));
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
            let plan =
                parse_line_plan_body(BlockStyle::Brace, &body, TextRange::new(line.start, end));
            return Some(attach_line_plan_label(
                plan,
                parse_with_brace_label(head.trim()),
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
        DialogueContent::new(
            raw.clone(),
            parse_dialogue_tokens(&raw),
            TextRange::new(start, end),
        )
    }

    fn take_indented_line_plan(&mut self, min_indent: usize, start: usize) -> LinePlan {
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
        parse_line_plan_body(BlockStyle::Indent, &raw, TextRange::new(start, end))
    }
}
