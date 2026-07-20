use super::assertion::{assertion_statement_candidate, parse_assertion_statement};
use super::headers::{
    implicit_flow_name_from_id, parse_contract_clause, parse_contract_clauses, parse_flow_head,
    parse_flow_signature, parse_name_and_tail, parse_optional_decl_id_ref,
    parse_required_entity_ref_syntax, parse_visibility_prefix, slice_offset,
};
use super::{
    BlockStyle, ContentCall, CstBlockEvent, CstFlowItemKind, CstLetFlowItemKind, CstLine,
    CstLineEvents, CstPunctuationDeltas, CstStructuredFlowBlockKind, DeferOutcome, Flow, FlowInit,
    FlowItem, MappedDialogueSourceBuilder, ParseError, Parser, RawSyntax, ScopeBlock, Stmt,
    SyntaxParseStats, TextRange, UnsafeAuditInsertion, flat_block_head, indentation,
    is_await_with_head, is_expression_statement_call, is_typed_stmt, is_with_brace_head,
    parse_await_with, parse_defer_outcome, parse_flat_fence, parse_line_options,
    parse_line_plan_attachment, parse_owned_expr_recovering, parse_scope_head, parse_stmt_lines,
    parse_stmt_recovering_with_base, parse_thread_block, parse_unsafe_lifetime_block,
    parse_with_brace_label, retain_expr_recovery_diagnostic, split_call_head,
    split_top_level_keyword_once,
};
use std::borrow::Cow;
use std::ops::Range;

impl<'a> Parser<'a> {
    pub(super) fn parse_flow(&mut self) -> Option<Flow> {
        let attrs = self.take_pending_attrs();
        let doc = self.take_pending_doc();
        let start_line = self.current().clone();
        let header = start_line.text.trim();
        let block = self.take_flow_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing flow",
                ["}"],
                Some(header),
                ["insert a closing `}` for the flow body"],
            );
            return None;
        }

        let head = &block.head;
        let header_lines = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let first = header_lines.first().copied()?;
        let (visibility, after_visibility) = parse_visibility_prefix(first);
        let after_flow = parse_flow_head(after_visibility.trim_start())?;
        let after_flow_base = start_line.start + slice_offset(first, after_flow);
        let (id, after_id) =
            parse_optional_decl_id_ref(after_flow, "flow", after_flow_base, &mut self.errors);
        let (explicit_name, signature_tail) = parse_name_and_tail(after_id.trim());
        let has_explicit_name = explicit_name.is_some();
        let name = explicit_name.or_else(|| implicit_flow_name_from_id(id.as_ref()));
        let (signature_tail, inline_contracts) = split_inline_flow_contracts(&signature_tail);
        let mut signature = match parse_flow_signature(name.as_deref(), &signature_tail) {
            Ok(signature) => signature,
            Err(error) => {
                self.push_error(
                    TextRange::new(start_line.start, start_line.end),
                    &error.to_string(),
                    ["flow name(param: Type)"],
                    Some(header),
                    ["write the flow with a valid function-style signature"],
                );
                None
            }
        };
        if signature
            .as_ref()
            .is_some_and(|signature| signature.param_groups().len() > 1)
        {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "`flow` parameters cannot be curried; use one parameter group",
                ["flow name(param: Type)"],
                Some(header),
                ["move all flow parameters into a single `(...)` group"],
            );
            signature = None;
        }
        let mut contracts = inline_contracts;
        contracts.extend(parse_contract_clauses(&header_lines[1..]));
        let body_items = self.parse_flow_body_from_block(&block, start_line.start + head.len());

        Some(Flow::new(FlowInit {
            attrs,
            doc,
            visibility,
            id,
            name,
            explicit_name: has_explicit_name,
            signature_tail,
            signature,
            contracts,
            body: body_items,
            range: TextRange::new(start_line.start, block.end),
        }))
    }

    pub(super) fn parse_flow_body_from_block(
        &mut self,
        block: &CstBlockEvent<'a>,
        base_offset: usize,
    ) -> Vec<FlowItem> {
        if let Some(range) = block.body_line_range.clone()
            && let Some(items) = self.parse_flow_body_from_line_range(range, base_offset)
        {
            return items;
        }
        self.parse_flow_body(&block.body, base_offset)
    }

    pub(super) fn parse_flow_body_from_line_range(
        &mut self,
        range: Range<usize>,
        _base_offset: usize,
    ) -> Option<Vec<FlowItem>> {
        let events = self.events.line_slice(range)?;
        Some(self.parse_flow_body_events(events, 0))
    }

    pub(super) fn parse_flow_body(&mut self, body: &str, base_offset: usize) -> Vec<FlowItem> {
        let mut nested = Parser::new_with_base_offset(body, base_offset);
        self.parse_nested_flow_body(&mut nested, 0)
    }

    fn parse_flow_body_events(
        &mut self,
        events: CstLineEvents<'a>,
        base_offset: usize,
    ) -> Vec<FlowItem> {
        let mut nested = Parser::from_line_events(
            self.document,
            self.source,
            events,
            SyntaxParseStats::default(),
        );
        self.parse_nested_flow_body(&mut nested, base_offset)
    }

    fn parse_nested_flow_body(
        &mut self,
        nested: &mut Parser<'_>,
        base_offset: usize,
    ) -> Vec<FlowItem> {
        let mut items = Vec::new();
        while !nested.pending_flow_items.is_empty() || nested.index < nested.events.len() {
            if nested.pending_flow_items.is_empty() {
                nested.skip_blank_and_comments();
            }
            if nested.pending_flow_items.is_empty() && nested.index >= nested.events.len() {
                break;
            }
            if let Some(item) = nested.parse_flow_item_until_indent(0) {
                items.push(item);
            } else {
                let current = nested.current().clone();
                let line = current.text.trim().to_owned();
                nested.push_error(
                    TextRange::new(current.start, current.end),
                    "unsupported flow item",
                    ["an expression statement", "a structured flow item"],
                    Some(&line),
                    ["use a current Arcweft flow-item form"],
                );
                items.push(FlowItem::Raw(RawSyntax::flow_item(
                    line,
                    Some(TextRange::new(current.start, current.end)),
                )));
                nested.index += 1;
            }
        }
        self.errors
            .extend(nested.errors.drain(..).map(|err| err.rebased(base_offset)));
        self.syntax_stats.numeric_seq_summaries += nested.syntax_stats.numeric_seq_summaries;
        self.syntax_stats.prefix_depth_limit_failures +=
            nested.syntax_stats.prefix_depth_limit_failures;
        items
    }

    pub(super) fn parse_flow_item_until_indent(&mut self, min_indent: usize) -> Option<FlowItem> {
        if let Some(item) = self.pending_flow_items.pop() {
            return Some(item);
        }
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let indent = indentation(&line.text);
        if indent < min_indent {
            return None;
        }
        let trimmed = line.text.trim();
        if assertion_statement_candidate(trimmed) {
            return Some(self.parse_assertion_flow_item(&line, indent));
        }
        let kind = line.flow_item_kind();

        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            let message = if trimmed.starts_with("#![") {
                "inner attributes are not supported in flow bodies yet"
            } else {
                "outer attributes are not supported in flow bodies yet"
            };
            self.push_error(
                TextRange::new(line.start, line.end),
                message,
                ["attribute before a top-level declaration"],
                Some(trimmed),
                ["move the attribute to a supported declaration or source header"],
            );
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }

        match kind {
            CstFlowItemKind::StructuredBlock(kind) => {
                return self.parse_structured_flow_block(kind);
            }
            CstFlowItemKind::Let(kind) => {
                if matches!(kind, CstLetFlowItemKind::Plain)
                    || matches!(kind, CstLetFlowItemKind::AwaitStart)
                        && !self.has_multiline_await_with(indent)
                {
                    let stmt = self.consume_stmt_text_with_continuations(indent);
                    let base = line.start + line.text.len() - line.text.trim_start().len();
                    return Some(FlowItem::Stmt(
                        self.parse_authored_flow_stmt(stmt.trim(), base),
                    ));
                }
                if let Some(item) = self.parse_let_flow_item(kind, indent) {
                    return Some(item);
                }
            }
            CstFlowItemKind::TypedStmt => {
                let stmt = self.consume_stmt_text_with_continuations(indent);
                let base = line.start + line.text.len() - line.text.trim_start().len();
                return Some(FlowItem::Stmt(
                    self.parse_authored_flow_stmt(stmt.trim(), base),
                ));
            }
            CstFlowItemKind::Include | CstFlowItemKind::AwaitWith | CstFlowItemKind::Other => {}
        }

        // Keep typed statements from falling back to Raw when the coarse CST
        // classifier misses a surface form. This is especially important for
        // `let name: Array<T, N> = ...`: the colon belongs to the type
        // annotation and must not be reinterpreted as speaker-line sugar.
        if is_typed_stmt(trimmed) || trimmed.starts_with("let ") {
            let stmt = self.consume_stmt_text_with_continuations(indent);
            let base = line.start + line.text.len() - line.text.trim_start().len();
            return Some(FlowItem::Stmt(
                self.parse_authored_flow_stmt(stmt.trim(), base),
            ));
        }
        if let Some(item) = self.parse_line_flow_item(&line, trimmed) {
            return Some(item);
        }
        if let Some(item) = self.parse_flat_flow_item(&line, trimmed) {
            return Some(item);
        }
        if let Some(item) = self.parse_content_call_or_speaker_line() {
            return Some(item);
        }

        None
    }

    fn parse_assertion_flow_item(&mut self, line: &CstLine<'_>, indent: usize) -> FlowItem {
        let source = self.consume_stmt_text_with_continuations(indent);
        let source = source.trim();
        let base = line.start + line.text.len() - line.text.trim_start().len();
        let statement = match parse_assertion_statement(source, base) {
            Ok(assertion) => Stmt::Assertion(assertion),
            Err(error) => {
                self.errors.push(error.into());
                Stmt::Raw(RawSyntax::stmt(
                    source,
                    Some(TextRange::new(base, base + source.len())),
                ))
            }
        };
        FlowItem::Stmt(statement)
    }

    fn parse_authored_flow_stmt(&mut self, source: &str, base: usize) -> Stmt {
        match parse_stmt_recovering_with_base(source, &mut self.syntax_stats, base) {
            Ok(parsed) => {
                for diagnostic in &parsed.diagnostics {
                    retain_expr_recovery_diagnostic(diagnostic, &mut self.errors);
                }
                parsed.stmt
            }
            Err(error) => {
                self.errors.push(ParseError::from_expression(
                    &error,
                    vec!["expression".to_owned()],
                ));
                let range = base
                    .checked_add(source.len())
                    .map(|end| TextRange::new(base, end));
                Stmt::Raw(RawSyntax::stmt(source, range))
            }
        }
    }

    fn consume_stmt_text_with_continuations(&mut self, indent: usize) -> Cow<'a, str> {
        let mut stmt = self.current().text;
        let mut depth = self.current().punctuation_deltas();
        self.index += 1;
        while self.index < self.events.len() {
            let next = self.current();
            let next_trimmed = next.text.trim_start();
            let is_balanced = punctuation_depth_is_balanced(depth);
            let is_dot_continuation =
                indentation(&next.text) > indent && next_trimmed.starts_with('.');
            let is_value_continuation = is_balanced
                && statement_needs_value_continuation(stmt.as_ref())
                && indentation(&next.text) > indent;
            if is_balanced && !is_dot_continuation && !is_value_continuation {
                break;
            }
            // Dot-leading lines and value-required statement heads are
            // expression continuations, not new flow items. Unbalanced
            // punctuation also keeps multiline expression statements such as
            // return-typed closure literals together.
            // Preserve a newline so parser diagnostics can still point back
            // to the authored shape when the expression is malformed.
            let text = stmt.to_mut();
            text.push('\n');
            text.push_str(&next.text);
            add_punctuation_depth(&mut depth, next.punctuation_deltas());
            self.index += 1;
        }
        stmt
    }

    fn parse_let_flow_item(&mut self, kind: CstLetFlowItemKind, indent: usize) -> Option<FlowItem> {
        match kind {
            CstLetFlowItemKind::Choice => self.parse_let_choice().map(FlowItem::Stmt),
            CstLetFlowItemKind::DialogueCall => self.parse_let_dialogue_call().map(FlowItem::Stmt),
            CstLetFlowItemKind::Scope => self.parse_let_scope().map(FlowItem::Stmt),
            CstLetFlowItemKind::ComputationBlock => {
                self.parse_let_computation_block().map(FlowItem::Stmt)
            }
            CstLetFlowItemKind::Block => self.parse_let_block().map(FlowItem::Stmt),
            CstLetFlowItemKind::Loop => self.parse_let_loop().map(FlowItem::Stmt),
            CstLetFlowItemKind::AwaitWith => self.parse_let_await_with().map(FlowItem::Stmt),
            CstLetFlowItemKind::AwaitStart => self
                .has_multiline_await_with(indent)
                .then(|| self.parse_let_await_with().map(FlowItem::Stmt))
                .flatten(),
            CstLetFlowItemKind::IfLet => self.parse_let_if_let().map(FlowItem::Stmt),
            CstLetFlowItemKind::If => self.parse_let_if().map(FlowItem::Stmt),
            CstLetFlowItemKind::Match => self.parse_let_match().map(FlowItem::Stmt),
            CstLetFlowItemKind::LetElse => self.parse_let_else().map(FlowItem::Stmt),
            CstLetFlowItemKind::Plain => None,
        }
    }

    fn parse_line_flow_item(&mut self, line: &CstLine, trimmed: &str) -> Option<FlowItem> {
        if trimmed.starts_with('@') && !trimmed.contains('[') {
            self.push_error(
                TextRange::new(line.start, line.end),
                "`@` does not start a flow statement",
                ["choice @choice.id { ... }", "bg(@asset:.id)"],
                Some(trimmed),
                ["use an ordinary statement or function-call style command"],
            );
        }
        if is_expression_statement_call(trimmed) {
            self.index += 1;
            let base = line.start + line.text.len() - line.text.trim_start().len();
            return Some(FlowItem::Stmt(Stmt::Expr {
                expr: parse_owned_expr_recovering(
                    trimmed,
                    base,
                    Some(&mut self.syntax_stats),
                    &mut self.errors,
                ),
                expr_source: Some(trimmed.to_owned()),
                expr_range: base
                    .checked_add(trimmed.len())
                    .map(|end| TextRange::new(base, end)),
            }));
        }
        if let Some(rest) = trimmed.strip_prefix("include ") {
            let entity =
                parse_required_entity_ref_syntax(rest.trim(), line.start, &mut self.errors)?.0;
            self.index += 1;
            return Some(FlowItem::Include(entity));
        }
        is_await_with_head(trimmed)
            .then(|| self.parse_await_flow_item(line, trimmed))
            .flatten()
    }

    pub(super) fn parse_scope_block(&mut self) -> Option<ScopeBlock> {
        let start_line = self.current().clone();
        if start_line.text.trim().ends_with(':') {
            self.index += 1;
            let head = start_line.text.trim().trim_end_matches(':').trim();
            let name = parse_scope_head(head)?.as_option().map(str::to_owned);
            let body_range = self.take_indented_line_range(indentation(&start_line.text) + 1);
            let body = if let Some(body) = self
                .parse_flow_body_from_line_range(body_range.clone(), start_line.start + head.len())
            {
                body
            } else {
                let body_source = self.collect_line_range_source(body_range);
                self.parse_flow_body(&body_source, start_line.start + head.len())
            };
            return Some(ScopeBlock::new(
                name,
                body,
                TextRange::new(start_line.start, self.previous_end()),
            ));
        }
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing named scope",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the scope block"],
            );
            return None;
        }
        let head = &block.head;
        let name = head.trim().strip_prefix("scope")?.trim();
        let name = (!name.is_empty()).then(|| name.to_owned());
        let body = self.parse_flow_body_from_block(&block, start_line.start + head.len());
        Some(ScopeBlock::new(
            name,
            body,
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn parse_thread_flow_stmt(&mut self) -> Option<FlowItem> {
        let start_line = self.current().clone();
        let trimmed = start_line.text.trim();
        if trimmed.ends_with(':') {
            self.index += 1;
            let head = trimmed.trim_end_matches(':').trim();
            let body_range = self.take_indented_line_range(indentation(&start_line.text) + 1);
            let body = if let Some(body) = self
                .parse_flow_body_from_line_range(body_range.clone(), start_line.start + head.len())
            {
                body
            } else {
                let body_source = self.collect_line_range_source(body_range);
                self.parse_flow_body(&body_source, start_line.start + head.len())
            };
            let thread = super::parse_thread_block_items(head, body);
            return Some(FlowItem::Stmt(Stmt::Thread(thread)));
        }
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing thread",
                ["}"],
                Some(trimmed),
                ["insert a closing `}` for the thread block"],
            );
            return None;
        }
        let head = &block.head;
        let body = self.parse_flow_body_from_block(&block, start_line.start + head.len());
        Some(FlowItem::Stmt(Stmt::Thread(
            super::parse_thread_block_items(head.trim(), body),
        )))
    }

    fn parse_defer_flow_stmt(&mut self) -> Option<FlowItem> {
        let start_line = self.current().clone();
        let trimmed = start_line.text.trim();
        if trimmed.ends_with(':') || trimmed == "defer" {
            self.index += 1;
            let body_range = self.take_indented_line_range(indentation(&start_line.text) + 1);
            return Some(FlowItem::Stmt(Stmt::DeferBlock {
                outcome: parse_defer_outcome(trimmed.trim_end_matches(':'))
                    .unwrap_or(DeferOutcome::Always),
                statements: self.parse_stmt_line_range(body_range),
            }));
        }
        if trimmed.starts_with("defer ") && !trimmed.contains('{') {
            self.index += 1;
            let base =
                start_line.start + start_line.text.len() - start_line.text.trim_start().len();
            return Some(FlowItem::Stmt(self.parse_authored_flow_stmt(trimmed, base)));
        }
        let (head, body, _, ok) = self.take_brace_block();
        if ok && let Some(outcome) = parse_defer_outcome(head.trim()) {
            return Some(FlowItem::Stmt(Stmt::DeferBlock {
                outcome,
                statements: parse_stmt_lines(&body),
            }));
        }
        self.push_error(
            TextRange::new(start_line.start, start_line.end),
            "unclosed block while parsing defer",
            ["}"],
            Some(trimmed),
            ["insert a closing `}` for the defer block"],
        );
        None
    }

    fn parse_unsafe_lifetime_flow_stmt(&mut self) -> Option<FlowItem> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing unsafe lifetime",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the unsafe lifetime block"],
            );
            return None;
        }
        let audit_insertion = block
            .body_range
            .as_ref()
            .and_then(|range| range.start.checked_sub(1))
            .map(|open_brace| {
                UnsafeAuditInsertion::new(TextRange::new(open_brace, open_brace + 1))
            });
        Some(FlowItem::Stmt(parse_unsafe_lifetime_block(
            &block.head,
            &block.body,
            start_line.start,
            audit_insertion,
            &mut self.errors,
        )))
    }

    pub(super) fn parse_bare_scope_block(&mut self) -> Option<ScopeBlock> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing unnamed scope",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the unnamed scope block"],
            );
            return None;
        }
        let head = &block.head;
        if !head.trim().is_empty() {
            return None;
        }
        Some(ScopeBlock::new(
            None,
            self.parse_flow_body_from_block(&block, start_line.start),
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn parse_flat_flow_item(&mut self, line: &CstLine, trimmed: &str) -> Option<FlowItem> {
        let fence = parse_flat_fence(trimmed)?;
        if fence.close {
            self.push_flat_close_mismatch("flow item", line);
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }
        match fence.kind {
            "line" => Some(self.parse_flat_dialogue_line(line, fence.head, fence.head_start)),
            "scope" => {
                let body = self.take_flat_block_body("scope", line.start);
                let name = (!fence.head.is_empty()).then(|| fence.head.to_owned());
                Some(FlowItem::Scope(ScopeBlock::new(
                    name,
                    self.parse_flow_body(&body, line.start + fence.head_start),
                    TextRange::new(line.start, self.previous_end()),
                )))
            }
            "thread" => {
                let body = self.take_flat_block_body("thread", line.start);
                let head = flat_block_head("thread", fence.head);
                Some(FlowItem::Stmt(Stmt::Thread(parse_thread_block(
                    &head, &body,
                ))))
            }
            "defer" => {
                let body = self.take_flat_block_body("defer", line.start);
                Some(FlowItem::Stmt(Stmt::DeferBlock {
                    outcome: parse_defer_outcome(&flat_block_head("defer", fence.head))
                        .unwrap_or(DeferOutcome::Always),
                    statements: parse_stmt_lines(&body),
                }))
            }
            _ => {
                self.push_error(
                    TextRange::new(line.start, line.end),
                    "unknown flat fence kind",
                    [
                        "=== line ... ===",
                        "=== scope ... ===",
                        "=== thread ... ===",
                    ],
                    Some(trimmed),
                    ["use a supported flat fence kind"],
                );
                self.index += 1;
                Some(FlowItem::Raw(RawSyntax::flow_item(
                    trimmed,
                    Some(TextRange::new(line.start, line.end)),
                )))
            }
        }
    }

    fn parse_flat_dialogue_line(
        &mut self,
        line: &CstLine,
        head: &str,
        head_start: usize,
    ) -> FlowItem {
        let body_event_start = self.index + 1;
        let body = self.take_flat_block_body("line", line.start);
        let body_event_end = self.index.saturating_sub(1);
        let body_anchor = self
            .events
            .get(body_event_start)
            .map_or(line.end, |line| line.start);
        let mut mapped_body = MappedDialogueSourceBuilder::new(body_anchor);
        for body_line in self
            .events
            .iter()
            .take(body_event_end)
            .skip(body_event_start)
        {
            let text = body_line.text.trim_end();
            mapped_body.push_line(
                text,
                TextRange::new(body_line.start, body_line.start + text.len()),
            );
        }
        let mapped_body = mapped_body.finish();
        let (_content_source, plan) = split_flat_line_content_and_plan(
            &body,
            TextRange::new(line.start, self.previous_end()),
        );
        let mapped_content_len = split_flat_line_content_and_plan(
            &mapped_body.raw,
            TextRange::new(line.start, self.previous_end()),
        )
        .0
        .len();
        let mapped_content = mapped_body
            .slice(TextRange::new(0, mapped_content_len))
            .and_then(|content| content.trim())
            .expect("flat dialogue content must remain inside its mapped body");
        let (callee, args) = split_call_head(head);
        let option_args = args
            .as_ref()
            .map(|(args, relative)| (args.as_str(), line.start + head_start + relative));
        FlowItem::ContentCall(ContentCall::new(
            callee,
            parse_line_options(option_args, &mut self.errors),
            self.dialogue_content(mapped_content),
            plan,
            TextRange::new(line.start, self.previous_end()),
        ))
    }

    fn parse_await_flow_item(&mut self, line: &CstLine, trimmed: &str) -> Option<FlowItem> {
        let trimmed_start = line.start + slice_offset(&line.text, trimmed);
        let range = TextRange::new(trimmed_start, trimmed_start + trimmed.len());
        if trimmed.contains('{') {
            let (head, body, _, ok) = self.take_brace_block();
            if ok {
                let await_with =
                    parse_await_with(&format!("{head} {{ {body} }}"), range, &mut self.errors);
                return Some(FlowItem::AwaitWith(await_with));
            }
        } else if trimmed.ends_with("with:") {
            self.index += 1;
            let body_range = self.take_indented_line_range(indentation(&line.text) + 1);
            let await_with = self.parse_await_with_line_range(trimmed, range, body_range);
            return Some(FlowItem::AwaitWith(await_with));
        } else {
            let await_with = parse_await_with(trimmed, range, &mut self.errors);
            self.index += 1;
            return Some(FlowItem::AwaitWith(await_with));
        }
        None
    }

    fn parse_structured_flow_block(
        &mut self,
        kind: CstStructuredFlowBlockKind,
    ) -> Option<FlowItem> {
        match kind {
            CstStructuredFlowBlockKind::Choice => self.parse_choice().map(FlowItem::Choice),
            CstStructuredFlowBlockKind::IfLet => self.parse_if_let_block().map(FlowItem::IfLet),
            CstStructuredFlowBlockKind::If => self.parse_if_block().map(FlowItem::If),
            CstStructuredFlowBlockKind::Match => self.parse_match_block().map(FlowItem::Match),
            CstStructuredFlowBlockKind::Loop => self.parse_loop_block().map(FlowItem::Loop),
            CstStructuredFlowBlockKind::WhileLet => {
                self.parse_while_let_block().map(FlowItem::WhileLet)
            }
            CstStructuredFlowBlockKind::While => self.parse_while_block().map(FlowItem::While),
            CstStructuredFlowBlockKind::For => self.parse_for_block().map(FlowItem::For),
            CstStructuredFlowBlockKind::Select => self.parse_select_block().map(FlowItem::Select),
            CstStructuredFlowBlockKind::Thread => self.parse_thread_flow_stmt(),
            CstStructuredFlowBlockKind::Defer => self.parse_defer_flow_stmt(),
            CstStructuredFlowBlockKind::UnsafeLifetime => self.parse_unsafe_lifetime_flow_stmt(),
            CstStructuredFlowBlockKind::SourceLocale => {
                self.parse_source_locale_block().map(FlowItem::SourceLocale)
            }
            CstStructuredFlowBlockKind::BareScope => {
                self.parse_bare_scope_block().map(FlowItem::Scope)
            }
            CstStructuredFlowBlockKind::Scope => self.parse_scope_block().map(FlowItem::Scope),
        }
    }
}

fn add_punctuation_depth(depth: &mut CstPunctuationDeltas, delta: CstPunctuationDeltas) {
    depth.brace += delta.brace;
    depth.paren += delta.paren;
    depth.bracket += delta.bracket;
}

fn punctuation_depth_is_balanced(depth: CstPunctuationDeltas) -> bool {
    depth.brace <= 0 && depth.paren <= 0 && depth.bracket <= 0
}

fn statement_needs_value_continuation(source: &str) -> bool {
    source.trim_end().ends_with('=')
}

fn split_flat_line_content_and_plan(
    body: &str,
    range: TextRange,
) -> (String, Option<super::LinePlan>) {
    let Some(with_start) = body
        .rfind("with {")
        .filter(|index| *index == 0 || body[..*index].ends_with('\n'))
    else {
        return (body.to_owned(), None);
    };
    let plan_source = &body[with_start..];
    let Some((head, plan_body)) = super::split_brace_item(plan_source) else {
        return (body.to_owned(), None);
    };
    if !is_with_brace_head(head) {
        return (body.to_owned(), None);
    }
    (
        body[..with_start].trim_end().to_owned(),
        Some(parse_line_plan_attachment(
            BlockStyle::Brace,
            plan_body,
            range,
            parse_with_brace_label(head.trim()),
            &mut Vec::new(),
        )),
    )
}

fn split_inline_flow_contracts(
    signature_tail: &str,
) -> (String, Vec<crate::ast::flow::ContractClause>) {
    let trimmed = signature_tail.trim();
    let (signature, effects) = split_top_level_keyword_once(trimmed, "effects");
    if let Some(effects) = effects {
        return (
            signature.trim_end().to_owned(),
            parse_contract_clause(&format!("effects {effects}"))
                .into_iter()
                .collect(),
        );
    }
    (signature_tail.to_owned(), Vec::new())
}
