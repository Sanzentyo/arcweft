use super::headers::{
    flow_decl_family, implicit_flow_name_from_id, parse_contract_clause, parse_flow_kind,
    parse_flow_signature, parse_name_and_tail, parse_optional_decl_id_ref,
    parse_required_entity_ref_syntax, parse_visibility_prefix, simple_error,
};
use super::{
    BlockStyle, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstStructuredFlowBlockKind,
    DeferOutcome, DialogueContent, Flow, FlowInit, FlowItem, Parser, RawSyntax, ScopeBlock,
    SpeakerLine, Stmt, TextRange, indentation, is_await_with_head, is_expression_statement_call,
    is_typed_stmt, nonempty_string, parse_await_with, parse_defer_outcome, parse_dialogue_tokens,
    parse_expr_lossy, parse_flat_fence, parse_line_options, parse_line_plan_body,
    parse_presentation_special_call, parse_scope_head, parse_stmt, parse_stmt_lines,
    parse_thread_block, parse_unsafe_lifetime_block, parse_word_scenario_command, split_call_head,
    split_leading_ident,
};

impl Parser {
    pub(super) fn parse_flow(&mut self) -> Option<Flow> {
        let doc = self.take_pending_doc();
        let start_line = self.current().clone();
        let header = start_line.text.trim();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing flow",
                ["}"],
                Some(header),
                ["insert a closing `}` for the flow body"],
            );
            return None;
        }

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
        let signature = parse_flow_signature(name.as_deref(), &signature_tail);
        let contracts = header_lines
            .iter()
            .skip(1)
            .filter_map(|line| parse_contract_clause(line))
            .collect();
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());

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
            range: TextRange::new(start_line.start, end),
        }))
    }

    pub(super) fn parse_flow_body(&mut self, body: &str, base_offset: usize) -> Vec<FlowItem> {
        let mut nested = Parser::new(body.to_owned());
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
        self.errors.extend(
            nested
                .errors
                .into_iter()
                .map(|err| err.rebased(base_offset)),
        );
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
                    self.index += 1;
                    return Some(FlowItem::Stmt(parse_stmt(trimmed)));
                }
                if let Some(item) = self.parse_let_flow_item(kind, indent) {
                    return Some(item);
                }
            }
            CstFlowItemKind::TypedStmt => {
                self.index += 1;
                return Some(FlowItem::Stmt(parse_stmt(trimmed)));
            }
            CstFlowItemKind::Include | CstFlowItemKind::AwaitWith | CstFlowItemKind::Other => {}
        }

        // Keep typed statements from falling back to Raw when the coarse CST
        // classifier misses a surface form. This is especially important for
        // `let name: Array<T, N> = ...`: the colon belongs to the type
        // annotation and must not be reinterpreted as speaker-line sugar.
        if is_typed_stmt(trimmed) || trimmed.starts_with("let ") {
            self.index += 1;
            return Some(FlowItem::Stmt(parse_stmt(trimmed)));
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
        if self.reject_legacy_scenario_call(trimmed, TextRange::new(line.start, line.end)) {
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }
        if let Some(expr) = parse_presentation_special_call(trimmed) {
            self.index += 1;
            return Some(FlowItem::Stmt(Stmt::Expr(expr)));
        }
        if is_expression_statement_call(trimmed) {
            self.index += 1;
            return Some(FlowItem::Stmt(Stmt::Expr(parse_expr_lossy(trimmed))));
        }
        if let Some(command) =
            parse_word_scenario_command(trimmed, TextRange::new(line.start, line.end))
        {
            self.index += 1;
            return Some(FlowItem::ScenarioCommand(command));
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
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing named scope",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the scope block"],
            );
            return None;
        }
        let name = head.trim().strip_prefix("scope")?.trim();
        let name = (!name.is_empty()).then(|| name.to_owned());
        let body = self.parse_flow_body(&body, start_line.start + head.len());
        Some(ScopeBlock::new(
            name,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_thread_flow_stmt(&mut self) -> Option<FlowItem> {
        let start_line = self.current().clone();
        let trimmed = start_line.text.trim();
        if trimmed.ends_with(':') {
            self.index += 1;
            let body = self.take_indented_await_body(indentation(&start_line.text) + 1);
            let head = trimmed.trim_end_matches(':').trim();
            let thread = parse_thread_block(head, &body);
            return Some(FlowItem::Stmt(Stmt::Thread(thread)));
        }
        let (head, body, _, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing thread",
                ["}"],
                Some(trimmed),
                ["insert a closing `}` for the thread block"],
            );
            return None;
        }
        Some(FlowItem::Stmt(Stmt::Thread(parse_thread_block(
            head.trim(),
            &body,
        ))))
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
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing unnamed scope",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the unnamed scope block"],
            );
            return None;
        }
        if !head.trim().is_empty() {
            return None;
        }
        Some(ScopeBlock::new(
            None,
            self.parse_flow_body(&body, start_line.start),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_flat_flow_item(&mut self, line: &CstLine, trimmed: &str) -> Option<FlowItem> {
        let fence = parse_flat_fence(trimmed)?;
        if fence.close {
            self.push_error(
                TextRange::new(line.start, line.end),
                "flat close fence has no matching open fence",
                ["=== kind ==="],
                Some(trimmed),
                ["remove this close fence or add the missing open fence"],
            );
            self.index += 1;
            return Some(FlowItem::Raw(RawSyntax::flow_item(
                trimmed,
                Some(TextRange::new(line.start, line.end)),
            )));
        }
        match fence.kind {
            "line" => Some(self.parse_flat_line(line, fence.head)),
            "thread" => {
                let body = self.take_flat_block_body("thread", line.start);
                let head = format!("thread {}", fence.head).trim().to_owned();
                Some(FlowItem::Stmt(Stmt::Thread(parse_thread_block(
                    &head, &body,
                ))))
            }
            "defer" => {
                let body = self.take_flat_block_body("defer", line.start);
                Some(FlowItem::Stmt(Stmt::DeferBlock {
                    outcome: DeferOutcome::Always,
                    statements: parse_stmt_lines(&body),
                }))
            }
            "scope" => {
                let body = self.take_flat_block_body("scope", line.start);
                let name = nonempty_string(fence.head.trim());
                let items = self.parse_flow_body(&body, line.start);
                Some(FlowItem::Scope(ScopeBlock::new(
                    name,
                    items,
                    TextRange::new(line.start, self.previous_end()),
                )))
            }
            _ => {
                self.push_error(
                    TextRange::new(line.start, line.end),
                    "unsupported flat fence head in flow body",
                    ["line", "thread", "defer", "scope"],
                    Some(trimmed),
                    ["use a supported flat block head"],
                );
                self.index += 1;
                Some(FlowItem::Raw(RawSyntax::flow_item(
                    trimmed,
                    Some(TextRange::new(line.start, line.end)),
                )))
            }
        }
    }

    fn parse_flat_line(&mut self, line: &CstLine, head: &str) -> FlowItem {
        self.index += 1;
        let mut raw_content = String::new();
        let mut plan = None;
        let mut closed = false;
        while self.index < self.events.len() {
            let current = self.current().clone();
            let trimmed = current.text.trim();
            if let Some(fence) = parse_flat_fence(trimmed) {
                if fence.close {
                    if fence.kind != "line" {
                        self.push_flat_close_mismatch("line", &current);
                    }
                    self.index += 1;
                    closed = true;
                    break;
                }
                if fence.kind == "with" {
                    let body = self.take_flat_block_body("with", current.start);
                    plan = Some(parse_line_plan_body(
                        BlockStyle::Flat,
                        &body,
                        TextRange::new(current.start, self.previous_end()),
                    ));
                    continue;
                }
            }
            if !raw_content.is_empty() {
                raw_content.push('\n');
            }
            let text = current.text.trim_end();
            if let Some(rest) = current.text.trim_start().strip_prefix("\\===") {
                let leading = current.text.len() - current.text.trim_start().len();
                raw_content.push_str(&current.text[..leading]);
                raw_content.push_str("===");
                raw_content.push_str(rest.trim_end());
            } else {
                raw_content.push_str(text);
            }
            self.index += 1;
        }
        if !closed {
            self.errors.push(simple_error(
                line.start,
                line.end.saturating_sub(line.start),
                "missing close fence `=== /line ===`",
                "=== /line ===",
            ));
        }
        let (speaker, args) = split_call_head(head.trim());
        FlowItem::SpeakerLine(SpeakerLine::new(
            speaker,
            parse_line_options(args.as_deref(), line.start, &mut self.errors),
            DialogueContent::new(
                raw_content.clone(),
                parse_dialogue_tokens(&raw_content),
                TextRange::new(line.start, self.previous_end()),
            ),
            plan,
            TextRange::new(line.start, self.previous_end()),
        ))
    }

    fn reject_legacy_scenario_call(&mut self, trimmed: &str, range: TextRange) -> bool {
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
