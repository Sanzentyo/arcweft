use super::headers::{
    flow_decl_family, implicit_flow_name_from_id, parse_contract_clause, parse_flow_kind,
    parse_flow_signature, parse_name_and_tail, parse_optional_decl_id_ref,
    parse_required_entity_ref_syntax, parse_visibility_prefix,
};
use super::{
    BlockStyle, ContentCall, CstBlockEvent, CstFlowItemKind, CstLetFlowItemKind, CstLine,
    CstLineEvents, CstStructuredFlowBlockKind, DeferOutcome, Flow, FlowInit, FlowItem, Parser,
    RawSyntax, ScopeBlock, Stmt, SyntaxParseStats, TextRange, flat_block_head, indentation,
    is_await_with_head, is_expression_statement_call, is_typed_stmt, is_with_brace_head,
    parse_await_with, parse_defer_outcome, parse_expr_lossy, parse_flat_fence, parse_line_options,
    parse_line_plan_attachment, parse_scope_head, parse_stmt, parse_stmt_lines,
    parse_stmt_with_stats, parse_thread_block, parse_unsafe_lifetime_block, parse_with_brace_label,
    split_call_head, split_leading_ident,
};
use std::borrow::Cow;
use std::ops::Range;

impl<'a> Parser<'a> {
    pub(super) fn parse_flow(&mut self) -> Option<Flow> {
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
        let (kind, after_flow) = parse_flow_kind(after_visibility.trim_start())?;
        let (id, after_id) = parse_optional_decl_id_ref(
            after_flow,
            flow_decl_family(kind),
            start_line.start,
            &mut self.errors,
        );
        let (name, signature_tail) = parse_name_and_tail(after_id.trim());
        let name = name.or_else(|| implicit_flow_name_from_id(id.as_ref()));
        let (signature_tail, inline_contracts) = split_inline_flow_contracts(&signature_tail);
        let signature = parse_flow_signature(name.as_deref(), &signature_tail);
        let mut contracts = inline_contracts;
        contracts.extend(
            header_lines
                .iter()
                .skip(1)
                .filter_map(|line| parse_contract_clause(line)),
        );
        let body_items = self.parse_flow_body_from_block(&block, start_line.start + head.len());

        Some(Flow::new(FlowInit {
            doc,
            kind,
            visibility,
            id,
            name,
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
        base_offset: usize,
    ) -> Option<Vec<FlowItem>> {
        let events = self.events.relative_line_slice(range, base_offset)?;
        Some(self.parse_flow_body_events(events, base_offset))
    }

    pub(super) fn parse_flow_body(&mut self, body: &str, base_offset: usize) -> Vec<FlowItem> {
        let mut nested = Parser::new(body);
        self.parse_nested_flow_body(&mut nested, base_offset)
    }

    fn parse_flow_body_events(
        &mut self,
        events: CstLineEvents<'a>,
        base_offset: usize,
    ) -> Vec<FlowItem> {
        let mut nested = Parser::from_line_events("", events, SyntaxParseStats::default());
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
        let kind = line.flow_item_kind();

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
                    return Some(FlowItem::Stmt(parse_stmt_with_stats(
                        stmt.trim(),
                        &mut self.syntax_stats,
                    )));
                }
                if let Some(item) = self.parse_let_flow_item(kind, indent) {
                    return Some(item);
                }
            }
            CstFlowItemKind::TypedStmt => {
                let stmt = self.consume_stmt_text_with_continuations(indent);
                return Some(FlowItem::Stmt(parse_stmt_with_stats(
                    stmt.trim(),
                    &mut self.syntax_stats,
                )));
            }
            CstFlowItemKind::Include | CstFlowItemKind::AwaitWith | CstFlowItemKind::Other => {}
        }

        // Keep typed statements from falling back to Raw when the coarse CST
        // classifier misses a surface form. This is especially important for
        // `let name: Array<T, N> = ...`: the colon belongs to the type
        // annotation and must not be reinterpreted as speaker-line sugar.
        if is_typed_stmt(trimmed) || trimmed.starts_with("let ") {
            let stmt = self.consume_stmt_text_with_continuations(indent);
            return Some(FlowItem::Stmt(parse_stmt_with_stats(
                stmt.trim(),
                &mut self.syntax_stats,
            )));
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

    fn consume_stmt_text_with_continuations(&mut self, indent: usize) -> Cow<'a, str> {
        let mut stmt = self.current().text;
        self.index += 1;
        while self.index < self.events.len() {
            let next = self.current();
            let next_trimmed = next.text.trim_start();
            if indentation(&next.text) <= indent || !next_trimmed.starts_with('.') {
                break;
            }
            // Dot-leading lines are expression continuations, not new flow
            // items. Preserve a newline so parser diagnostics can still point
            // back to the authored shape when the expression is malformed.
            let text = stmt.to_mut();
            text.push('\n');
            text.push_str(&next.text);
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
            CstLetFlowItemKind::MemoBlock => self.parse_let_memo_block().map(FlowItem::Stmt),
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
        if trimmed.starts_with("spawn ") {
            self.push_error(
                TextRange::new(line.start, line.end),
                "`spawn` was removed; use `thread` or `thread detached`",
                ["thread { ... }", "thread detached { ... }"],
                Some(trimmed),
                ["rewrite this unstructured task as a scoped thread"],
            );
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }
        if trimmed.starts_with('@') && !trimmed.contains('[') {
            let message = if trimmed.starts_with("@choice") {
                "`@choice` is not valid Arcweft syntax"
            } else {
                "`@` does not start a flow statement"
            };
            self.push_error(
                TextRange::new(line.start, line.end),
                message,
                ["choice @choice.id { ... }", "bg(@asset.id)"],
                Some(trimmed),
                ["use an ordinary statement or function-call style command"],
            );
        }
        if self
            .reject_unparenthesized_presentation_call(trimmed, TextRange::new(line.start, line.end))
        {
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }
        if is_expression_statement_call(trimmed) {
            self.index += 1;
            return Some(FlowItem::Stmt(Stmt::Expr(parse_expr_lossy(trimmed))));
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
            let body = self.take_indented_await_body(indentation(&start_line.text) + 1);
            let head = start_line.text.trim().trim_end_matches(':').trim();
            let name = parse_scope_head(head)?.as_option().map(str::to_owned);
            let body = self.parse_flow_body(&body, start_line.start + head.len());
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
            let body = self.take_indented_await_body(indentation(&start_line.text) + 1);
            let head = trimmed.trim_end_matches(':').trim();
            let body = self.parse_flow_body(&body, start_line.start + head.len());
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
            let body = self.take_indented_await_body(indentation(&start_line.text) + 1);
            return Some(FlowItem::Stmt(Stmt::DeferBlock {
                outcome: parse_defer_outcome(trimmed.trim_end_matches(':'))
                    .unwrap_or(DeferOutcome::Always),
                statements: parse_stmt_lines(&body),
            }));
        }
        if trimmed.starts_with("defer ") && !trimmed.contains('{') {
            self.index += 1;
            return Some(FlowItem::Stmt(parse_stmt(trimmed)));
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
        let (head, body, _, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing unsafe lifetime",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the unsafe lifetime block"],
            );
            return None;
        }
        Some(FlowItem::Stmt(parse_unsafe_lifetime_block(
            &head,
            &body,
            start_line.start,
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
        let body = self.take_flat_block_body("line", line.start);
        let (content_source, plan) = split_flat_line_content_and_plan(
            &body,
            TextRange::new(line.start, self.previous_end()),
        );
        let (callee, args) = split_call_head(head);
        FlowItem::ContentCall(ContentCall::new(
            callee,
            parse_line_options(args.as_deref(), line.start + head_start, &mut self.errors),
            self.dialogue_content(
                content_source.trim().to_owned(),
                TextRange::new(line.end, self.previous_end()),
            ),
            plan,
            TextRange::new(line.start, self.previous_end()),
        ))
    }

    fn reject_unparenthesized_presentation_call(
        &mut self,
        trimmed: &str,
        range: TextRange,
    ) -> bool {
        let Some((name, tail)) = split_leading_ident(trimmed) else {
            return false;
        };
        if !matches!(name, "bg" | "show") || tail.trim_start().starts_with('(') {
            return false;
        }
        self.push_error(
            range,
            "scenario staging uses canonical function-call syntax",
            [
                "bg(@asset.id, fade = 300ms)",
                "show(@character.alice, .normal)",
            ],
            Some(trimmed),
            ["rewrite this as an ordinary effectful call"],
        );
        true
    }

    fn parse_await_flow_item(&mut self, line: &CstLine, trimmed: &str) -> Option<FlowItem> {
        if trimmed.contains('{') {
            let (head, body, _, ok) = self.take_brace_block();
            if ok {
                let range = TextRange::new(line.start, line.end);
                let await_with =
                    parse_await_with(&format!("{head} {{ {body} }}"), range, &mut self.errors);
                return Some(FlowItem::AwaitWith(await_with));
            }
        } else if trimmed.ends_with("with:") {
            self.index += 1;
            let body = self.take_indented_await_body(indentation(&line.text) + 1);
            let range = TextRange::new(line.start, line.end);
            let await_with =
                parse_await_with(&format!("{trimmed}\n{body}"), range, &mut self.errors);
            return Some(FlowItem::AwaitWith(await_with));
        } else {
            let await_with = parse_await_with(
                trimmed,
                TextRange::new(line.start, line.end),
                &mut self.errors,
            );
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
            CstStructuredFlowBlockKind::Borrow => {
                self.parse_borrow_block().map(FlowItem::BorrowBlock)
            }
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
    if let Some(rest) = trimmed.strip_prefix("effects ") {
        return (
            String::new(),
            parse_contract_clause(&format!("effects {rest}"))
                .into_iter()
                .collect(),
        );
    }
    (signature_tail.to_owned(), Vec::new())
}
