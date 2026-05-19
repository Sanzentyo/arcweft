use crate::ast::choice::{ChoiceBlock, ChoicePlan};
use crate::ast::common::{DocBlock, TextRange, Visibility};
use crate::ast::dialogue::{
    ContentCall, DialogueContent, DialogueDefaultOption, DialogueDefaultsItem, LineArg,
    LineOptions, LineOptionsInit, ScenarioCommand, SpeakerLine,
};
use crate::ast::flow::{
    AwaitBranch, AwaitBranchKind, AwaitWith, BorrowBlock, ContractClause, Flow, FlowInit, FlowItem,
    FlowKind, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm, MatchBlock, ScopeBlock,
    ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead, SourceLocaleBlock, Stmt,
    StmtMatchArm, WaitTarget, WhileBlock, WhileLetBlock,
};
use crate::ast::ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, IdRef, RelativeId, RelativeIdSpelling,
    WikiLink,
};
use crate::ast::items::{
    CallableKind, EntityDeclKind, FunctionKind, HookInit, HookItem, MemoFn, ParserItem, RawSyntax,
    TypedSyntaxTree,
};
use crate::ast::line_plan::{BlockStyle, DeferOutcome, LinePlan};
use crate::ast::pattern::Pattern;
use crate::cst::{
    CstBlockOpenRule, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineEvents, CstStmtKind,
    CstStructuredFlowBlockKind, CstTopLevelItemKind, CstTopLevelLineKind, SyntaxNode,
    classify_stmt, collect_wiki_link_ranges, cst_lines, find_matching_punctuation,
    find_top_level_punctuation, nonempty_trimmed_source_lines, punctuation_delta,
    source_line_count, source_lines, split_leading_entity_ref_parts, split_leading_ident,
    split_leading_relative_entity_ref, split_leading_relative_id, split_top_level_keyword_once,
    split_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once, split_top_level_whitespace,
    starts_leading_entity_ref, starts_leading_relative_entity_ref, starts_leading_relative_id,
};
use crate::expr::{ComputationBlockKind, Expr, parse_expr};
use crate::pattern::parse_pattern;
use crate::source::ParsedSource;
use crate::text::parse_dialogue_tokens;
use crate::types::{parse_fn_signature, parse_type_ref};
use arcweft_source::{SourceAnchor, SourceName};

pub mod choice;
pub mod dialogue;
pub mod flow;
pub mod helpers;
pub mod items;
pub mod line_plan;
pub mod proof;
pub mod recovery;
pub mod source;
pub mod top_level;
use choice::{parse_choice_items, parse_choice_plan_items};
use line_plan::{
    nonempty_string, parse_defer_outcome, parse_line_plan_body, parse_thread_block,
    parse_trigger_pattern,
};
use recovery::{ParseError, RecoverySuggestion};

/// Parses an Arcweft source string.
#[must_use]
pub fn parse_source(source: impl Into<String>) -> ParsedSource {
    let source = source.into();
    let syntax = crate::cst::parse_cst(&source);
    let mut parser = Parser::from_syntax(source.clone(), &syntax);
    let (tree, errors) = parser.parse();
    ParsedSource::new(source, syntax, tree, errors)
}

enum OptionalLabel {
    None,
    Some(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FlatFence<'a> {
    kind: &'a str,
    head: &'a str,
    close: bool,
}

#[derive(Default)]
struct PendingDocLines {
    start_line: Option<usize>,
    lines: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopLevelDispatch {
    line: CstTopLevelLineKind,
    item: CstTopLevelItemKind,
}

impl OptionalLabel {
    fn into_option(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Some(label) => Some(label),
        }
    }
}

impl From<&CstLine> for TopLevelDispatch {
    fn from(line: &CstLine) -> Self {
        Self {
            line: line.top_level_line_kind(),
            item: line.top_level_item_kind(),
        }
    }
}

type EntityDeclHead = (
    EntityDeclKind,
    Option<Visibility>,
    EntityRef,
    Option<String>,
    Option<String>,
    String,
);
type ContentCallParse = (
    String,
    Option<String>,
    DialogueContent,
    usize,
    Option<LinePlan>,
    Option<ScopeBlock>,
);

struct Parser {
    source: String,
    events: CstLineEvents,
    index: usize,
    errors: Vec<ParseError>,
    pending_flow_items: Vec<FlowItem>,
    pending_doc: Option<DocBlock>,
}

impl PendingDocLines {
    fn push_if_doc(&mut self, line: &str, line_index: usize) -> bool {
        let Some(text) = line.strip_prefix("///") else {
            return false;
        };
        if self.start_line.is_none() {
            self.start_line = Some(line_index);
        }
        self.lines
            .push(text.strip_prefix(' ').unwrap_or(text).to_owned());
        true
    }

    fn take(&mut self) -> Option<DocBlock> {
        if self.lines.is_empty() {
            return None;
        }
        let start = self.start_line.take().unwrap_or(0);
        let end = start + self.lines.len();
        let text = core::mem::take(&mut self.lines).join("\n");
        Some(DocBlock::new(text, TextRange::new(start, end)))
    }
}

impl Parser {
    fn new(source: String) -> Self {
        let syntax = crate::cst::parse_cst(&source);
        Self::from_syntax(source, &syntax)
    }

    fn from_syntax(source: String, syntax: &SyntaxNode) -> Self {
        let events = cst_lines(syntax);
        Self {
            source,
            events,
            index: 0,
            errors: Vec::new(),
            pending_flow_items: Vec::new(),
            pending_doc: None,
        }
    }

    fn parse(&mut self) -> (TypedSyntaxTree, Vec<ParseError>) {
        let mut module = None;
        let mut uses = Vec::new();
        let mut items = Vec::new();
        let wiki_links = collect_wiki_links(&self.source);

        while self.index < self.events.len() {
            self.skip_blank_and_comments();
            if self.index >= self.events.len() {
                break;
            }
            if let Some(doc) = self.take_doc_block() {
                if self.pending_doc.is_some() {
                    self.push_error(
                        *doc.range(),
                        "documentation comment is not attached to an item",
                        ["item declaration"],
                        Some(doc.text()),
                        ["move the `///` block directly before the item it documents"],
                    );
                }
                self.pending_doc = Some(doc);
                continue;
            }

            let line = self.current().clone();
            let trimmed = line.trimmed().to_owned();
            let range = TextRange::new(line.start, line.end);
            let dispatch = TopLevelDispatch::from(&line);

            self.parse_top_level_line(
                dispatch,
                &trimmed,
                range,
                &mut module,
                &mut uses,
                &mut items,
            );
        }

        let tree = TypedSyntaxTree::new(source_take(self), module, uses, items, wiki_links);
        (tree, core::mem::take(&mut self.errors))
    }

    fn take_flow_block(&mut self) -> (String, String, usize, bool) {
        let event = self.events.collect_flow_block(self.index);
        self.index = event.next_index;
        (event.head, event.body, event.end, event.ok)
    }

    fn take_function_block(&mut self) -> (String, String, usize, bool) {
        self.take_block_event(CstBlockOpenRule::FunctionBodyOpen)
    }

    fn next_nonblank_line_is_brace(&self) -> bool {
        for line in self.events.iter().skip(self.index + 1) {
            if line.is_trivia() {
                continue;
            }
            let trimmed = line.trimmed();
            if !trimmed.starts_with('#') {
                return trimmed == "{";
            }
        }
        false
    }

    fn parse_hook(&mut self) -> Option<HookItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing hook",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the hook body"],
            );
            return None;
        }
        let header_lines: Vec<_> = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        self.reject_old_hook_header_syntax(&header_lines, start_line.start);
        let first = header_lines.first()?;
        let (visibility, after_visibility) = parse_visibility_prefix(first);
        let after_hook = after_visibility
            .trim_start()
            .strip_prefix("hook")?
            .trim_start();
        let (id, _) = parse_required_decl_entity_ref_without_name_marker(
            after_hook,
            "hook",
            "hook declaration marker needs an explicit hook name suffix",
            start_line.start,
            &mut self.errors,
        )?;
        let target = find_header_value(&header_lines, "on ");
        let phase = find_header_value(&header_lines, "phase ");
        let mut when = None;
        let mut priority = None;
        let mut once = false;
        let mut effects = Vec::new();
        for line in header_lines.iter().skip(1) {
            if let Some(expr) = line.strip_prefix("when ").map(str::trim) {
                if when.replace(parse_expr_lossy(expr)).is_some() {
                    self.push_duplicate_hook_header_error("when", start_line.start, line);
                }
            } else if let Some(value) = line.strip_prefix("priority ").map(str::trim) {
                match value.parse::<i64>() {
                    Ok(value) if priority.replace(value).is_none() => {}
                    Ok(_) => {
                        self.push_duplicate_hook_header_error("priority", start_line.start, line);
                    }
                    Err(_) => self.push_error(
                        TextRange::new(start_line.start, start_line.start + line.len()),
                        "hook priority must be an integer",
                        ["priority 0"],
                        Some(line),
                        ["write priority as a signed integer"],
                    ),
                }
            } else if let Some(value) = line.strip_prefix("once").map(str::trim) {
                if once {
                    self.push_duplicate_hook_header_error("once", start_line.start, line);
                }
                once = true;
                if !value.is_empty() && value != "true" {
                    self.push_error(
                        TextRange::new(start_line.start, start_line.start + line.len()),
                        "hook `once` does not take a value in this grammar",
                        ["once"],
                        Some(line),
                        ["remove the value after `once`"],
                    );
                }
            } else if let Some(values) = line.strip_prefix("effects ").map(str::trim) {
                effects.extend(split_comma_args(values).into_iter().map(parse_expr_lossy));
            } else if line.starts_with("check ") {
                self.push_error(
                    TextRange::new(start_line.start, start_line.start + line.len()),
                    "`check` is not valid hook condition syntax",
                    ["when expr"],
                    Some(line),
                    ["write hook conditions with `when`"],
                );
            }
        }
        let body_statements = parse_stmt_lines(&body);

        Some(HookItem::new(HookInit {
            visibility,
            id,
            target,
            phase,
            when,
            priority,
            once,
            effects,
            body,
            body_statements,
            range: TextRange::new(start_line.start, end),
        }))
    }

    fn push_duplicate_hook_header_error(&mut self, name: &str, base: usize, line: &str) {
        self.push_error(
            TextRange::new(base, base + line.len()),
            &format!("duplicate hook `{name}` header"),
            [name],
            Some(line),
            ["keep only one header with this name"],
        );
    }

    fn reject_old_hook_header_syntax(&mut self, header_lines: &[&str], base: usize) {
        for line in header_lines {
            if line.starts_with("for ") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`for` is not valid hook target syntax",
                    ["on #target"],
                    Some(line),
                    ["write the hook target as `on #target`"],
                );
            } else if line.starts_with("phase =") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`phase =` is not valid hook phase syntax",
                    ["phase PhaseName"],
                    Some(line),
                    ["write the hook phase without `=`"],
                );
            } else if line.starts_with("on input target") {
                self.push_error(
                    TextRange::new(base, base + line.len()),
                    "`on input target` is not valid hook input syntax",
                    ["phase InputTarget", "check on input EventKind"],
                    Some(line),
                    ["split input hooks into `phase InputTarget` and `check on input EventKind`"],
                );
            }
        }
    }

    fn parse_dialogue_defaults(&mut self) -> Option<DialogueDefaultsItem> {
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
        let options = body
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    return None;
                }
                let (name, value) =
                    split_top_level_punctuation_once(trimmed, '=').unwrap_or_else(|| {
                        self.push_error(
                            TextRange::new(start_line.start, start_line.start + trimmed.len()),
                            "expected dialogue default assignment",
                            ["name = expr"],
                            Some(trimmed),
                            ["write defaults as `name = value`"],
                        );
                        ("", "")
                    });
                (!name.trim().is_empty()).then(|| {
                    DialogueDefaultOption::new(
                        name.trim().to_owned(),
                        parse_expr_lossy(value.trim()),
                        TextRange::new(start_line.start, start_line.start + trimmed.len()),
                    )
                })
            })
            .collect();
        Some(DialogueDefaultsItem::new(
            visibility,
            id,
            options,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_memo_fn(&mut self) -> Option<MemoFn> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing memo fn",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the memo function body"],
            );
            return None;
        }
        let mut lines = head.lines().map(str::trim).filter(|line| !line.is_empty());
        let first = lines.next()?;
        let (visibility, after_visibility) = parse_visibility_prefix(first);
        let signature = after_visibility
            .trim_start()
            .strip_prefix("memo fn")?
            .trim()
            .to_owned();
        let options = lines
            .inspect(|line| self.reject_old_memo_option(line, start_line.start))
            .map(str::to_owned)
            .collect();
        let (body_statements, body_value) = parse_scope_expr_body(&body);
        Some(MemoFn::new(
            visibility,
            signature,
            options,
            body,
            body_statements,
            body_value,
            TextRange::new(start_line.start, end),
        ))
    }

    fn reject_old_memo_option(&mut self, line: &str, base: usize) {
        if line.starts_with("cache ") {
            self.push_error(
                TextRange::new(base, base + line.len()),
                "`cache` is not valid memo option syntax",
                ["scope = MemoScope"],
                Some(line),
                ["replace `cache session` with `scope = session`"],
            );
        }
    }

    fn parse_parser_item(&mut self) -> Option<ParserItem> {
        if !self.current().text.contains('{') && !self.next_nonblank_line_is_brace() {
            return self.parse_parser_item_line();
        }
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing parser item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the parser body"],
            );
            return None;
        }
        let (visibility, after_visibility) = parse_visibility_prefix(head.trim());
        let after_parser = after_visibility
            .trim_start()
            .strip_prefix("parser")?
            .trim_start();
        let (name, tail) = parse_name_and_tail(after_parser);
        let (body_statements, body_value) = parse_scope_expr_body(&body);
        Some(ParserItem::new(
            visibility,
            name.unwrap_or_default(),
            tail,
            body,
            body_statements,
            body_value,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_parser_item_line(&mut self) -> Option<ParserItem> {
        let line = self.current().clone();
        self.index += 1;
        let (visibility, after_visibility) = parse_visibility_prefix(line.text.trim());
        let after_parser = after_visibility
            .trim_start()
            .strip_prefix("parser")?
            .trim_start();
        let (name, tail) = parse_name_and_tail(after_parser);
        Some(ParserItem::new(
            visibility,
            name.unwrap_or_default(),
            tail,
            String::new(),
            Vec::new(),
            None,
            TextRange::new(line.start, line.end),
        ))
    }

    fn parse_choice(&mut self) -> Option<ChoiceBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing choice",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the choice block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("choice")?.trim();
        let (id, _) = parse_optional_id_ref(rest, start_line.start, &mut self.errors);
        let items = parse_choice_items(&body, start_line.start, &mut self.errors);
        let plan = self.take_choice_plan_after_current(start_line.start);
        Some(ChoiceBlock::new(
            id,
            items,
            plan,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_let_choice(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing choice expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the choice expression block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, choice_head) = split_top_level_binding(rest)?;
        let choice_rest = choice_head.trim().strip_prefix("choice")?.trim();
        let (id, _) = parse_optional_id_ref(choice_rest, start_line.start, &mut self.errors);
        let items = parse_choice_items(&body, start_line.start, &mut self.errors);
        let plan = self.take_choice_plan_after_current(start_line.start);

        Some(Stmt::LetChoice {
            pattern: parse_pattern(pattern.trim()),
            choice: ChoiceBlock::new(id, items, plan, TextRange::new(start_line.start, end)),
        })
    }

    fn parse_let_scope(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing scope expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the scope expression block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, scope_head) = split_top_level_binding(rest)?;
        let name = parse_scope_head(scope_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        Some(Stmt::LetScope {
            pattern: parse_pattern(pattern.trim()),
            scope: ScopeExprBlock::new(
                name.as_option().map(str::to_owned),
                statements,
                value,
                TextRange::new(start_line.start, end),
            ),
        })
    }

    fn parse_let_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the block expression"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        if !block_head.trim().is_empty() {
            return None;
        }

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: parse_block_expr(&body),
        })
    }

    fn parse_let_computation_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing computation block expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the computation block expression"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let kind = parse_computation_block_kind(block_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::ComputationBlock {
                kind,
                statements,
                value: value.map(Box::new),
            },
        })
    }

    fn parse_let_memo_block(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing memo expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the memo expression block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, block_head) = split_top_level_binding(rest)?;
        let options = parse_memo_block_options(block_head.trim())?;
        let (statements, value) = parse_scope_expr_body(&body);

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: Expr::MemoBlock {
                options,
                statements,
                value: value.map(Box::new),
            },
        })
    }

    fn parse_let_loop(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop expression block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, loop_head) = split_top_level_binding(rest)?;
        let (label, loop_head) = split_optional_block_label(loop_head.trim());
        if loop_head != "loop" {
            return None;
        }

        Some(Stmt::LetLoop {
            pattern: parse_pattern(pattern.trim()),
            block: LoopBlock::new(
                label,
                self.parse_flow_body(&body, start_line.start + head.len()),
                TextRange::new(start_line.start, end),
            ),
        })
    }

    fn parse_let_await_with(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let trimmed = start_line.text.trim();
        let range = TextRange::new(start_line.start, start_line.end);

        let (head, body) = if has_inline_brace_await_with(trimmed) {
            let (head, body, _, ok) = self.take_brace_block();
            if !ok {
                self.push_error(
                    range,
                    "unclosed block while parsing await expression binding",
                    ["}"],
                    Some(trimmed),
                    ["insert a closing `}` for the await wait-view block"],
                );
                return None;
            }
            (head, Some(format!("{{ {body} }}")))
        } else if trimmed.ends_with("with:") {
            self.index += 1;
            let body = self.take_indented_await_body(indentation(&start_line.text) + 1);
            let closing = self.take_parenthesized_await_closing();
            (
                format!("{}{}", trimmed, closing.unwrap_or_default()),
                Some(body),
            )
        } else {
            self.take_multiline_let_await_head(&start_line)
        };

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, await_head) = split_top_level_binding(rest)?;
        let await_source = body.map_or_else(
            || normalize_let_await_source(await_head.trim(), None),
            |body| {
                let head = normalize_let_await_source(await_head.trim(), None);
                if body.trim_start().starts_with('{') {
                    format!("{head} {body}")
                } else {
                    format!("{head}\n{body}")
                }
            },
        );

        Some(Stmt::LetAwait {
            pattern: parse_pattern(pattern.trim()),
            await_with: parse_await_with(&await_source, range, &mut self.errors),
        })
    }

    fn take_multiline_let_await_head(&mut self, start_line: &CstLine) -> (String, Option<String>) {
        let base_indent = indentation(&start_line.text);
        let mut head = start_line.text.trim().to_owned();
        self.index += 1;

        while self.index < self.events.len() {
            let line = self.current().clone();
            let trimmed = line.text.trim();
            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }
            if trimmed == "with:" {
                self.index += 1;
                let body = self.take_indented_await_body(base_indent + 1);
                let closing = self.take_parenthesized_await_closing();
                return (
                    format!("{head} with:{}", closing.unwrap_or_default()),
                    Some(body),
                );
            }
            if has_standalone_brace_with(trimmed) {
                let (with_head, body, _, ok) = self.take_brace_block();
                if ok {
                    let closing = self.take_parenthesized_await_closing();
                    return (
                        format!("{head} {with_head}{}", closing.unwrap_or_default()),
                        Some(format!("{{ {body} }}")),
                    );
                }
            }
            if indentation(&line.text) > base_indent || trimmed.starts_with('.') {
                append_await_head_continuation(&mut head, trimmed);
                self.index += 1;
                continue;
            }
            break;
        }

        (head, None)
    }

    fn take_parenthesized_await_closing(&mut self) -> Option<String> {
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim();
        if !trimmed.starts_with(')') {
            return None;
        }
        self.index += 1;
        Some(trimmed.to_owned())
    }

    fn has_multiline_await_with(&self, base_indent: usize) -> bool {
        self.events
            .iter()
            .skip(self.index + 1)
            .take_while(|line| {
                let trimmed = line.text.trim();
                trimmed.is_empty()
                    || indentation(&line.text) > base_indent
                    || trimmed.starts_with('.')
                    || trimmed.starts_with("with")
            })
            .any(|line| {
                let trimmed = line.text.trim();
                trimmed == "with:" || has_standalone_brace_with(trimmed)
            })
    }

    fn parse_let_if(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if expression block"],
            );
            return None;
        }
        let (then_body, else_body) = split_embedded_else_body(&body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body, else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, if_head) = split_top_level_binding(rest)?;
        let condition = if_head.trim().strip_prefix("if")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::If {
                condition: Box::new(parse_expr_lossy(condition)),
                then_branch: Box::new(parse_block_expr(&then_body)),
                else_branch: Some(Box::new(parse_block_expr(&else_body))),
            },
        })
    }

    fn parse_let_if_let(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let expression block"],
            );
            return None;
        }
        let (then_body, else_body) = split_embedded_else_body(&body).map_or_else(
            || {
                self.take_optional_else_block(start_line.start)
                    .map(|else_body| (body, else_body))
            },
            Some,
        )?;
        let rest = head.trim().strip_prefix("let")?.trim();
        let (target_pattern, if_head) = split_top_level_binding(rest)?;
        let if_let_head = if_head.trim().strip_prefix("if let")?.trim();
        let (binding_pattern, value_and_guard) = split_top_level_binding(if_let_head)?;
        let (value, guard) = split_if_let_guard(value_and_guard);

        let (target_pattern, ty) = parse_binding_pattern(target_pattern);
        Some(Stmt::Let {
            pattern: target_pattern,
            ty,
            expr: crate::expr::Expr::IfLet {
                pattern: Box::new(parse_pattern(binding_pattern.trim())),
                expr: Box::new(parse_expr_lossy(value.trim())),
                guard: guard.map(|guard| Box::new(parse_expr_lossy(guard.trim()))),
                then_branch: Box::new(parse_block_expr(&then_body)),
                else_branch: Some(Box::new(parse_block_expr(&else_body))),
            },
        })
    }

    fn parse_let_match(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match expression",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match expression block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, match_head) = split_top_level_binding(rest)?;
        let scrutinee = match_head.trim().strip_prefix("match")?.trim();

        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::Let {
            pattern,
            ty,
            expr: crate::expr::Expr::Match {
                scrutinee: Box::new(parse_expr_lossy(scrutinee)),
                arms: parse_match_expr_arms(&body),
            },
        })
    }

    fn take_optional_else_block(&mut self, base: usize) -> Option<String> {
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            self.push_error(
                TextRange::new(base, self.previous_end()),
                "value-producing if expression requires else",
                ["else { ... }"],
                None,
                ["add an else block or use statement-style if"],
            );
            return None;
        }
        let line = self.current().clone();
        if !line.text.trim_start().starts_with("else") {
            self.push_error(
                TextRange::new(line.start, line.end),
                "value-producing if expression requires else",
                ["else { ... }"],
                Some(line.text.trim()),
                ["add an else block before the next statement"],
            );
            return None;
        }
        let (_, body, _, ok) = self.take_brace_block();
        if ok {
            Some(body)
        } else {
            self.push_error(
                TextRange::new(line.start, line.end),
                "unclosed else block while parsing if expression",
                ["}"],
                Some(line.text.trim()),
                ["insert a closing `}` for the else block"],
            );
            None
        }
    }

    fn parse_let_else(&mut self) -> Option<Stmt> {
        let start_line = self.current().clone();
        let (head, body, _end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing let-else",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the let-else block"],
            );
            return None;
        }

        let rest = head.trim().strip_prefix("let")?.trim();
        let (pattern, rhs) = split_top_level_binding(rest)?;
        let expr = rhs.trim().strip_suffix("else")?.trim();
        let (pattern, ty) = parse_binding_pattern(pattern);
        Some(Stmt::LetElse {
            pattern,
            ty,
            expr: parse_expr_lossy(expr),
            else_body: parse_stmt_lines(&body),
        })
    }

    fn parse_source_locale_block(&mut self) -> Option<SourceLocaleBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing source locale",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the source locale block"],
            );
            return None;
        }
        let locale = head.trim().strip_prefix("source locale")?.trim().to_owned();
        let body = self.parse_flow_body(&body, start_line.start + head.len());
        Some(SourceLocaleBlock::new(
            locale,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_scope_block(&mut self) -> Option<ScopeBlock> {
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

    fn parse_bare_scope_block(&mut self) -> Option<ScopeBlock> {
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

    fn take_choice_plan_after_current(&mut self, base: usize) -> Option<ChoicePlan> {
        self.skip_blank_and_comments();
        if self.index >= self.events.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim();
        if trimmed == "with" || trimmed.starts_with("with ") {
            let (head, body, end, ok) = self.take_brace_block();
            if ok && head.trim() == "with" {
                return Some(ChoicePlan::new(
                    BlockStyle::Brace,
                    parse_choice_plan_items(&body),
                    TextRange::new(line.start, end),
                ));
            }
        }
        if trimmed == "with:" {
            self.index += 1;
            let body = self.take_indented_await_body(indentation(&line.text) + 1);
            return Some(ChoicePlan::new(
                BlockStyle::Indent,
                parse_choice_plan_items(&body),
                TextRange::new(line.start, base + body.len()),
            ));
        }
        None
    }

    fn parse_if_block(&mut self) -> Option<IfBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if body"],
            );
            return None;
        }
        let condition = head.strip_prefix("if")?.trim();
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());
        Some(IfBlock::new(
            parse_expr_lossy(condition),
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_if_let_block(&mut self) -> Option<IfLetBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing if-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the if-let body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("if let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let guard = guard.map(|guard| parse_expr_lossy(guard.trim()));
        Some(IfLetBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(expr),
            guard,
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_borrow_block(&mut self) -> Option<BorrowBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing borrow",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the borrow block"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("borrow")?.trim();
        let (source, Some(binding)) = split_top_level_keyword_once(rest, "as") else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "borrow block must bind a typed alias",
                ["borrow expr as name: Type { ... }"],
                Some(head.trim()),
                ["write the borrow block as `borrow source as name: Type { ... }`"],
            );
            return None;
        };
        let Some((name, ty)) = split_top_level_punctuation_once(binding, ':') else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "borrow binding must declare a type",
                ["name: Type"],
                Some(binding.trim()),
                ["add the borrowed reference type after the alias name"],
            );
            return None;
        };
        let binding = parse_pattern(&format!("{}: {}", name.trim(), ty.trim()));
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());

        Some(BorrowBlock::new(
            parse_expr_lossy(source.trim()),
            binding,
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_match_block(&mut self) -> Option<MatchBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing match",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the match body"],
            );
            return None;
        }
        let expr = head.strip_prefix("match")?.trim();
        Some(MatchBlock::new(
            parse_expr_lossy(expr),
            parse_match_arms(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_loop_block(&mut self) -> Option<LoopBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing loop",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the loop body"],
            );
            return None;
        }
        let body_base = start_line.start + head.len();
        let (label, head) = split_optional_block_label(head.trim());
        if head != "loop" {
            return None;
        }
        Some(LoopBlock::new(
            label,
            self.parse_flow_body(&body, body_base),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_for_block(&mut self) -> Option<ForBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing for",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the for body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("for")?.trim();
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return None;
        };
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());
        Some(ForBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(source.trim()),
            body_items,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_while_block(&mut self) -> Option<WhileBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while body"],
            );
            return None;
        }
        let condition = head.trim().strip_prefix("while")?.trim();
        Some(WhileBlock::new(
            parse_expr_lossy(condition),
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_while_let_block(&mut self) -> Option<WhileLetBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing while-let",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the while-let body"],
            );
            return None;
        }
        let rest = head.trim().strip_prefix("while let")?.trim();
        let (pattern, expr_and_guard) = split_top_level_binding(rest)?;
        let (expr, guard) = split_top_level_keyword_once(expr_and_guard, "when");
        let guard = guard.map(|guard| parse_expr_lossy(guard.trim()));
        Some(WhileLetBlock::new(
            parse_pattern(pattern.trim()),
            parse_expr_lossy(expr),
            guard,
            self.parse_flow_body(&body, start_line.start + head.len()),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_select_block(&mut self) -> Option<SelectBlock> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing select",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the select body"],
            );
            return None;
        }
        if !head.trim().starts_with("select") {
            return None;
        }
        Some(SelectBlock::new(
            parse_select_branches(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    fn take_indented_await_body(&mut self, min_indent: usize) -> String {
        let mut raw = String::new();
        while self.index < self.events.len() {
            let line = self.current();
            if line.text.trim().is_empty() {
                raw.push('\n');
                self.index += 1;
                continue;
            }
            if indentation(&line.text) < min_indent {
                break;
            }
            if !raw.is_empty() {
                raw.push('\n');
            }
            raw.push_str(&line.text);
            self.index += 1;
        }
        raw
    }

    fn take_brace_block(&mut self) -> (String, String, usize, bool) {
        self.take_block_event(CstBlockOpenRule::FirstTopLevelOpen)
    }

    fn take_block_event(&mut self, rule: CstBlockOpenRule) -> (String, String, usize, bool) {
        let event = self.events.collect_brace_block(self.index, rule);
        self.index = event.next_index;
        (event.head, event.body, event.end, event.ok)
    }

    fn current(&self) -> &CstLine {
        &self.events[self.index]
    }

    fn previous_end(&self) -> usize {
        self.index
            .checked_sub(1)
            .and_then(|index| self.events.get(index))
            .map_or(0, |line| line.end)
    }

    fn skip_blank_and_comments(&mut self) {
        while self.index < self.events.len() {
            if self.current().is_trivia() {
                self.index += 1;
            } else {
                break;
            }
        }
    }

    fn take_doc_block(&mut self) -> Option<DocBlock> {
        let first = self.events.get(self.index)?;
        first.doc_comment_text()?;
        let start = first.start;
        let mut end = first.end;
        let mut lines = Vec::new();
        while self.index < self.events.len() {
            let line = self.current();
            let Some(text) = line.doc_comment_text() else {
                break;
            };
            lines.push(text.to_owned());
            end = line.end;
            self.index += 1;
        }
        Some(DocBlock::new(lines.join("\n"), TextRange::new(start, end)))
    }

    fn take_pending_doc(&mut self) -> Option<DocBlock> {
        self.pending_doc.take()
    }

    fn reject_pending_doc(&mut self, fallback_range: TextRange) {
        if let Some(doc) = self.pending_doc.take() {
            self.push_error(
                *doc.range(),
                "documentation comment is not attached to a documentable item",
                ["function or flow declaration"],
                Some(doc.text()),
                ["move the `///` block directly before a supported declaration"],
            );
        } else {
            let _ = fallback_range;
        }
    }

    fn push_error<const E: usize, const R: usize>(
        &mut self,
        range: TextRange,
        message: &str,
        expected: [&str; E],
        found: Option<&str>,
        recovery: [&str; R],
    ) {
        self.errors.push(ParseError::new(
            range,
            expected.into_iter().map(str::to_owned).collect(),
            found.map(str::to_owned),
            message.to_owned(),
            recovery
                .into_iter()
                .map(|message| RecoverySuggestion {
                    message: message.to_owned(),
                })
                .collect(),
            SourceAnchor::new(SourceName::path("<memory>"), 0..0),
        ));
    }
}

fn source_take(parser: &mut Parser) -> String {
    core::mem::take(&mut parser.source)
}

fn collect_wiki_links(source: &str) -> Vec<WikiLink> {
    collect_wiki_link_ranges(source)
        .into_iter()
        .map(|(body, start, end)| WikiLink::new(body.to_owned(), TextRange::new(start, end)))
        .collect()
}

fn parse_function_kind_and_signature(source: &str) -> (FunctionKind, &str) {
    [
        ("task ", FunctionKind::Task),
        ("dialogue ", FunctionKind::Dialogue),
        ("stream ", FunctionKind::Stream),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| {
        source
            .strip_prefix(prefix)
            .map(|signature| (kind, signature.trim_start()))
    })
    .unwrap_or((FunctionKind::Function, source))
}

fn split_function_header_lines<'a>(lines: &'a [&'a str]) -> Option<(String, Vec<&'a str>)> {
    let mut signature = Vec::new();
    let mut depth = 0_i32;
    let mut end_index = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if index > 0 && depth == 0 && parse_contract_clause(trimmed).is_some() {
            end_index = Some(index);
            break;
        }
        signature.push(trimmed);
        for ch in trimmed.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        if depth <= 0 && trimmed.contains(')') {
            end_index = Some(index + 1);
            break;
        }
    }
    let end_index = end_index.unwrap_or(signature.len());
    (!signature.is_empty()).then(|| (signature.join("\n"), lines[end_index..].to_vec()))
}

fn parse_extern_mod_head(head: &str) -> Option<(String, String, Option<String>)> {
    let rest = head.trim_start().strip_prefix("extern")?.trim_start();
    let (abi, Some(rest)) = split_top_level_keyword_once(rest, "mod") else {
        return None;
    };
    let (path, source) = split_top_level_keyword_once(rest, "from");
    let source = source.map(|source| source.trim().to_owned());
    Some((abi.trim().to_owned(), path.trim().to_owned(), source))
}

fn entity_decl_kind(input: &str) -> Option<(EntityDeclKind, &str)> {
    [
        ("audio bus", EntityDeclKind::AudioBus),
        ("mixer snapshot", EntityDeclKind::MixerSnapshot),
        ("character", EntityDeclKind::Character),
        ("component", EntityDeclKind::Component),
        ("activity", EntityDeclKind::Activity),
        ("metric counter", EntityDeclKind::Metric),
        ("metric gauge", EntityDeclKind::Metric),
        ("metric", EntityDeclKind::Metric),
        ("signal", EntityDeclKind::Signal),
        ("layer", EntityDeclKind::Layer),
        ("textbox", EntityDeclKind::Textbox),
        ("voice profile", EntityDeclKind::Voice),
        ("voice", EntityDeclKind::Voice),
        ("se", EntityDeclKind::Se),
        ("bgm", EntityDeclKind::Bgm),
        ("ducking", EntityDeclKind::Ducking),
        ("motion", EntityDeclKind::Motion),
        ("rig", EntityDeclKind::Rig),
    ]
    .into_iter()
    .find_map(|(keyword, kind)| {
        input
            .strip_prefix(keyword)
            .filter(|rest| rest.starts_with(char::is_whitespace))
            .map(|rest| (kind, rest.trim_start()))
    })
}

fn entity_decl_family(kind: EntityDeclKind) -> &'static str {
    match kind {
        EntityDeclKind::Character => "character",
        EntityDeclKind::Component => "component",
        EntityDeclKind::Activity => "activity",
        EntityDeclKind::Signal => "signal",
        EntityDeclKind::Metric => "metric",
        EntityDeclKind::Layer => "layer",
        EntityDeclKind::Textbox => "textbox",
        EntityDeclKind::Voice => "voice",
        EntityDeclKind::Se => "se",
        EntityDeclKind::Bgm => "bgm",
        EntityDeclKind::AudioBus => "bus",
        EntityDeclKind::MixerSnapshot => "mix",
        EntityDeclKind::Ducking => "duck",
        EntityDeclKind::Motion => "motion",
        EntityDeclKind::Rig => "rig",
    }
}

fn parse_entity_decl_head(
    head: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<EntityDeclHead> {
    let (visibility, rest) = parse_visibility_prefix(head);
    let rest = rest
        .trim_start()
        .strip_prefix("surface ")
        .unwrap_or(rest.trim_start());
    let (kind, rest) = entity_decl_kind(rest.trim_start())?;
    let family = entity_decl_family(kind);
    let (parsed_id, rest) = parse_required_decl_entity_ref_or_marker(rest, family, base, errors)?;
    let (id, rest) = match parsed_id {
        DeclEntityId::Entity(id) => normalize_trailing_colon_id(id, rest),
        DeclEntityId::NameMarker(marker) => {
            let rest = rest.trim();
            let (name, _) = parse_name_and_tail(rest);
            let Some(name) = name.as_deref() else {
                errors.push(simple_error(
                    marker.range.start(),
                    marker.range.end() - marker.range.start(),
                    "relative declaration marker needs a following declaration name",
                    &format!("@{family}:. name"),
                ));
                return None;
            };
            (
                EntityRef::new(format!("{family}.{name}"), false, marker.range),
                rest.to_owned(),
            )
        }
    };
    let rest = rest.trim();
    let (name, signature_tail) = parse_name_and_tail(rest);
    let (signature_tail, surface_alias) = split_surface_alias(signature_tail);
    Some((kind, visibility, id, name, surface_alias, signature_tail))
}

fn split_surface_alias(signature_tail: String) -> (String, Option<String>) {
    let (before, after) = split_top_level_keyword_once(&signature_tail, "as");
    if let Some(after) = after {
        let alias = after
            .split_whitespace()
            .next()
            .filter(|value| is_simple_identifier(value))
            .map(str::to_owned);
        return (before.trim().to_owned(), alias);
    }
    (signature_tail, None)
}

fn is_simple_identifier(source: &str) -> bool {
    let mut chars = source.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_alphanumeric() || ch == '_')
}

fn normalize_trailing_colon_id(entity: EntityRef, rest: &str) -> (EntityRef, String) {
    if entity.is_delimited() || !entity.body().ends_with(':') {
        return (entity, rest.to_owned());
    }
    let body = entity.body().trim_end_matches(':').to_owned();
    let range = TextRange::new(entity.range().start(), entity.range().end() - 1);
    (
        EntityRef::new(body, false, range),
        format!(": {}", rest.trim_start()),
    )
}

fn parse_callable_kind(input: &str) -> Option<(CallableKind, &str)> {
    if let Some(rest) = input.strip_prefix("reducer") {
        return Some((CallableKind::Reducer, rest.trim_start()));
    }
    input
        .strip_prefix("view")
        .map(|rest| (CallableKind::View, rest.trim_start()))
}

fn parse_flow_kind(input: &str) -> Option<(FlowKind, &str)> {
    if let Some(rest) = input.strip_prefix("flow") {
        return Some((FlowKind::Flow, rest.trim_start()));
    }
    input
        .strip_prefix("fragment")
        .map(|rest| (FlowKind::Fragment, rest.trim_start()))
}

fn flow_decl_family(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Flow => "flow",
        FlowKind::Fragment => "fragment",
    }
}

fn find_header_value(lines: &[&str], prefix: &str) -> String {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .unwrap_or_default()
        .to_owned()
}

fn parse_flow_signature(
    name: Option<&str>,
    signature_tail: &str,
) -> Option<crate::types::FnSignature> {
    let tail = signature_tail.trim();
    if !(tail.starts_with('(') || tail.starts_with('<')) {
        return None;
    }
    parse_fn_signature(&format!("fn {}{}", name.unwrap_or("flow"), tail)).ok()
}

fn implicit_flow_name_from_id(id: Option<&IdRef>) -> Option<String> {
    match id? {
        IdRef::Relative(relative) => Some(relative.suffix().to_owned()),
        IdRef::FamilyRelative(relative) => Some(relative.relative().suffix().to_owned()),
        IdRef::Absolute(_) => None,
    }
}

fn parse_visibility_prefix(input: &str) -> (Option<Visibility>, &str) {
    let trimmed = input.trim_start();
    if let Some(rest) = trimmed.strip_prefix("pub(crate)") {
        (Some(Visibility::Crate), rest)
    } else if let Some(rest) = trimmed.strip_prefix("pub(super)") {
        (Some(Visibility::Super), rest)
    } else if let Some(rest) = trimmed.strip_prefix("pub ") {
        (Some(Visibility::Public), rest)
    } else {
        (None, input)
    }
}

fn parse_optional_entity_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<EntityRef>, &'a str) {
    let trimmed = input.trim_start();
    if starts_leading_entity_ref(trimmed) {
        match parse_required_entity_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else {
        (None, input)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EmptyDeclRelativeMarker {
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DeclEntityId {
    Entity(EntityRef),
    NameMarker(EmptyDeclRelativeMarker),
}

fn parse_optional_decl_id_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<IdRef>, &'a str) {
    let trimmed = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(trimmed) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
        }
        return (None, rest);
    }
    if starts_leading_relative_id(trimmed) || starts_leading_relative_entity_ref(trimmed) {
        return match parse_required_id_ref(trimmed, base, errors) {
            Some((id, rest)) => {
                if let IdRef::FamilyRelative(relative) = &id {
                    if !decl_family_matches(family, relative.family()) {
                        errors.push(simple_error(
                            relative.range().start(),
                            relative.range().end() - relative.range().start(),
                            "family-relative declaration id uses the wrong family",
                            &format!("@{family}:.suffix"),
                        ));
                    }
                }
                (Some(id), rest)
            }
            None => (None, input),
        };
    }
    let (id, rest) = parse_optional_id_ref(input, base, errors);
    let Some(id) = id else {
        return (None, rest);
    };
    match &id {
        IdRef::FamilyRelative(relative) if !decl_family_matches(family, relative.family()) => {
            errors.push(simple_error(
                relative.range().start(),
                relative.range().end() - relative.range().start(),
                "family-relative declaration id uses the wrong family",
                &format!("@{family}:.suffix"),
            ));
        }
        _ => {}
    }
    (Some(id), rest)
}

fn parse_optional_decl_entity_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<EntityRef>, &'a str) {
    let trimmed = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(trimmed) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
        }
        return (None, rest);
    }
    if starts_leading_relative_id(trimmed) || starts_leading_relative_entity_ref(trimmed) {
        match parse_required_id_ref(trimmed, base, errors)
            .and_then(|(id, rest)| normalize_decl_id_ref(id, family, errors).map(|id| (id, rest)))
        {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else {
        parse_optional_entity_ref(input, base, errors)
    }
}

fn parse_optional_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<IdRef>, &'a str) {
    let trimmed = input.trim_start();
    if starts_leading_relative_id(trimmed) {
        match parse_required_id_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else if trimmed.starts_with('.') {
        let _ = parse_required_id_ref(trimmed, base, errors);
        (None, input)
    } else if starts_leading_entity_ref(trimmed) {
        match parse_required_entity_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(IdRef::absolute(entity)), rest),
            None => (None, input),
        }
    } else {
        (None, input)
    }
}

fn parse_required_decl_entity_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_id(input) || starts_leading_relative_entity_ref(input) {
        let (id, rest) = parse_required_id_ref(input, base, errors)?;
        let entity = normalize_decl_id_ref(id, family, errors)?;
        Some((entity, rest))
    } else {
        parse_required_entity_ref(input, base, errors)
    }
}

fn parse_required_decl_entity_ref_or_marker<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(DeclEntityId, &'a str)> {
    let input = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(input) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
            return None;
        }
        return Some((
            DeclEntityId::NameMarker(EmptyDeclRelativeMarker {
                range: TextRange::new(base, base + marker_len),
            }),
            rest,
        ));
    }
    parse_required_decl_entity_ref(input, family, base, errors)
        .map(|(entity, rest)| (DeclEntityId::Entity(entity), rest))
}

fn parse_required_decl_entity_ref_without_name_marker<'a>(
    input: &'a str,
    family: &str,
    marker_message: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    match parse_required_decl_entity_ref_or_marker(input, family, base, errors)? {
        (DeclEntityId::Entity(id), rest) => Some((id, rest)),
        (DeclEntityId::NameMarker(marker), _) => {
            errors.push(simple_error(
                marker.range.start(),
                marker.range.end() - marker.range.start(),
                marker_message,
                &format!("@{family}:.suffix"),
            ));
            None
        }
    }
}

fn normalize_decl_id_ref(
    id: IdRef,
    family: &str,
    errors: &mut Vec<ParseError>,
) -> Option<EntityRef> {
    match id {
        IdRef::Absolute(entity) => Some(entity),
        IdRef::Relative(relative) => Some(EntityRef::new(
            format!("{family}.{}", relative.suffix()),
            false,
            *relative.range(),
        )),
        IdRef::FamilyRelative(relative) => {
            if !decl_family_matches(family, relative.family()) {
                errors.push(simple_error(
                    relative.range().start(),
                    relative.range().end() - relative.range().start(),
                    "family-relative declaration id uses the wrong family",
                    &format!("@{family}:.suffix"),
                ));
                return None;
            }
            Some(EntityRef::new(
                format!("{family}.{}", relative.relative().suffix()),
                false,
                *relative.range(),
            ))
        }
    }
}

fn decl_family_matches(expected: &str, actual: &str) -> bool {
    expected == actual || expected == "fragment" && actual == "frag"
}

fn split_empty_decl_relative_marker(source: &str) -> Option<(Option<&str>, usize, &str)> {
    if let Some(rest) = source.strip_prefix("@.") {
        return (!rest.starts_with(is_decl_relative_suffix_start)).then_some((
            None,
            "@.".len(),
            rest,
        ));
    }
    let at = source.strip_prefix('@')?;
    let family_len = take_decl_marker_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    if family_len == 0 {
        return None;
    }
    let marker = at.get(family_len..)?.strip_prefix(":.")?;
    (!marker.starts_with(is_decl_relative_suffix_start)).then_some((
        Some(&at[..family_len]),
        '@'.len_utf8() + family_len + ":.".len(),
        marker,
    ))
}

fn take_decl_marker_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| predicate(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

fn is_decl_relative_suffix_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn parse_required_entity_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if input.starts_with("@<") {
        let Some(entity_ref) = split_leading_entity_ref_parts(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "unclosed delimited entity reference",
                "@<...>",
            ));
            return None;
        };
        if !entity_ref.closed {
            errors.push(simple_error(
                base,
                input.len(),
                "unclosed delimited entity reference",
                "@<...>",
            ));
            return None;
        }
        if entity_ref.body.trim().is_empty() {
            errors.push(simple_error(
                base,
                input.len(),
                "empty entity reference",
                "@foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(
                entity_ref.body.to_owned(),
                true,
                TextRange::new(base, base + entity_ref.raw.len()),
            ),
            entity_ref.rest,
        ));
    }
    if starts_leading_entity_ref(input) {
        let Some(entity_ref) = split_leading_entity_ref_parts(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid entity reference",
                "@foo.bar",
            ));
            return None;
        };
        if entity_ref.body.is_empty() {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid entity reference",
                "@foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(
                entity_ref.body.to_owned(),
                false,
                TextRange::new(base, base + entity_ref.raw.len()),
            ),
            entity_ref.rest,
        ));
    }
    None
}

fn parse_required_entity_ref_syntax<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRefSyntax, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_id(input) {
        errors.push(simple_error(
            base,
            input.len(),
            "relative entity references must include a family",
            "@flow:.suffix",
        ));
        return None;
    }
    if starts_leading_relative_entity_ref(input) {
        let Some(relative_ref) = split_leading_relative_entity_ref(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid relative entity reference",
                "@flow:.suffix",
            ));
            return None;
        };
        let relative = relative_id_from_cst(
            relative_ref.relative,
            TextRange::new(
                base + '@'.len_utf8() + relative_ref.family.len() + ':'.len_utf8(),
                base + relative_ref.raw.len(),
            ),
        );
        let entity = FamilyRelativeEntityRef::new(
            relative_ref.family.to_owned(),
            relative,
            TextRange::new(base, base + relative_ref.raw.len()),
        );
        return Some((EntityRefSyntax::family_relative(entity), relative_ref.rest));
    }
    parse_required_entity_ref(input, base, errors)
        .map(|(entity, rest)| (EntityRefSyntax::absolute(entity), rest))
}

fn parse_required_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(IdRef, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_entity_ref(input) {
        let Some(relative_ref) = split_leading_relative_entity_ref(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid family-relative id",
                "@family:.suffix",
            ));
            return None;
        };
        let relative = relative_id_from_cst(
            relative_ref.relative,
            TextRange::new(
                base + '@'.len_utf8() + relative_ref.family.len() + ':'.len_utf8(),
                base + relative_ref.raw.len(),
            ),
        );
        let entity = FamilyRelativeEntityRef::new(
            relative_ref.family.to_owned(),
            relative,
            TextRange::new(base, base + relative_ref.raw.len()),
        );
        return Some((IdRef::family_relative(entity), relative_ref.rest));
    }
    if starts_leading_relative_id(input) {
        let Some(relative) = split_leading_relative_id(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "relative id is missing a suffix",
                "@.suffix",
            ));
            return None;
        };
        let range = TextRange::new(base, base + relative.marker_len + relative.body.len());
        return Some((
            IdRef::relative(relative_id_from_cst(relative, range)),
            relative.rest,
        ));
    }
    if starts_leading_entity_ref(input) {
        return parse_required_entity_ref(input, base, errors)
            .map(|(entity, rest)| (IdRef::absolute(entity), rest));
    }
    if input.starts_with('.') {
        errors.push(simple_error(
            base,
            input.len(),
            "relative IDs must start with `@.`",
            "@.suffix",
        ));
        return None;
    }
    {
        errors.push(simple_error(
            base,
            input.len(),
            "expected entity reference or relative id",
            "@domain.path",
        ));
    }
    None
}

fn relative_id_from_cst(relative: crate::cst::CstRelativeId<'_>, range: TextRange) -> RelativeId {
    let spelling = match relative.spelling {
        crate::cst::CstRelativeIdSpelling::DotRun => RelativeIdSpelling::DotRun,
        crate::cst::CstRelativeIdSpelling::SuperChain => RelativeIdSpelling::SuperChain,
    };
    RelativeId::new(
        relative.body.to_owned(),
        relative.parent_depth,
        spelling,
        range,
    )
}

fn simple_error(base: usize, len: usize, message: &str, expected: &str) -> ParseError {
    ParseError::new(
        TextRange::new(base, base + len),
        vec![expected.to_owned()],
        None,
        message.to_owned(),
        vec![RecoverySuggestion {
            message: format!("use {expected} syntax"),
        }],
        SourceAnchor::new(SourceName::path("<memory>"), base..base + len),
    )
}

fn parse_name_and_tail(input: &str) -> (Option<String>, String) {
    let trimmed = input.trim_start();
    split_leading_ident(trimmed).map_or_else(
        || (None, trimmed.to_owned()),
        |(name, tail)| (Some(name.to_owned()), tail.trim().to_owned()),
    )
}

fn parse_word_scenario_command(trimmed: &str, range: TextRange) -> Option<ScenarioCommand> {
    let (name, args) = split_leading_ident(trimmed).unwrap_or((trimmed, ""));
    if name != "option" {
        return None;
    }
    Some(ScenarioCommand::new(
        name.to_owned(),
        parse_scenario_args(args.trim()),
        range,
    ))
}

fn parse_scenario_args(args: &str) -> Vec<crate::expr::Expr> {
    split_scenario_args(args)
        .into_iter()
        .map(parse_expr_lossy)
        .collect()
}

fn parse_binding_pattern(source: &str) -> (Pattern, Option<crate::types::TypeRef>) {
    split_top_level_punctuation_once(source, ':').map_or_else(
        || (parse_pattern(source.trim()), None),
        |(pattern, ty)| {
            let parsed_ty = parse_type_ref(ty.trim()).ok();
            (parse_pattern(pattern.trim()), parsed_ty)
        },
    )
}

fn split_scenario_args(source: &str) -> Vec<&str> {
    split_top_level_whitespace(source)
}

fn is_expression_statement_call(trimmed: &str) -> bool {
    if find_top_level_punctuation(trimmed, ':').is_some()
        || find_top_level_punctuation(trimmed, '[').is_some()
    {
        return false;
    }
    matches!(
        crate::expr::parse_expr(trimmed),
        Ok(Expr::Call { .. } | Expr::MethodCall { .. })
    )
}

fn parse_line_options(
    args: Option<&str>,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> LineOptions {
    let Some(args) = args else {
        return LineOptions::default();
    };
    let mut state = LineOptionsParseState::default();
    let mut consumed_positional_look = false;
    for arg in split_comma_args(args) {
        let Some((name, value)) = split_top_level_punctuation_once(arg, '=') else {
            if consumed_positional_look {
                errors.push(simple_error(
                    base,
                    arg.len(),
                    "only the first positional dialogue line option may be used as `look`",
                    "look = expr",
                ));
                continue;
            }
            consumed_positional_look = true;
            state.look = Some(parse_expr_lossy(arg.trim()));
            continue;
        };
        parse_named_line_option(
            &mut state,
            name.trim(),
            value.trim(),
            arg.len(),
            base,
            errors,
        );
    }
    LineOptions::new(LineOptionsInit {
        id: state.id,
        text_key: state.text_key,
        voice: state.voice,
        look: state.look,
        stage: state.stage,
        portrait: state.portrait,
        focus: state.focus,
        cleanup: state.cleanup,
        window: state.window,
        source_locale: state.source_locale,
        hooks: state.hooks,
        style: state.style,
        args: state.line_args,
    })
}

#[derive(Default)]
struct LineOptionsParseState {
    id: Option<IdRef>,
    text_key: Option<IdRef>,
    voice: Option<Expr>,
    look: Option<Expr>,
    stage: Option<Expr>,
    portrait: Option<Expr>,
    focus: Option<Expr>,
    cleanup: Option<Expr>,
    window: Option<EntityRefSyntax>,
    source_locale: Option<String>,
    hooks: Vec<Expr>,
    style: Option<Expr>,
    line_args: Vec<LineArg>,
}

fn parse_named_line_option(
    state: &mut LineOptionsParseState,
    name: &str,
    value: &str,
    arg_len: usize,
    base: usize,
    errors: &mut Vec<ParseError>,
) {
    match name {
        "id" => state.id = parse_required_id_ref(value, base, errors).map(|(entity, _)| entity),
        "text_key" => {
            state.text_key = parse_required_id_ref(value, base, errors).map(|(entity, _)| entity);
        }
        "voice" => state.voice = Some(parse_expr_lossy(value)),
        "look" => state.look = Some(parse_expr_lossy(value)),
        "face" => errors.push(simple_error(
            base,
            arg_len,
            "`face` is not a canonical dialogue line option",
            "use `look = expr` or the first positional look option",
        )),
        "stage" => state.stage = Some(parse_expr_lossy(value)),
        "portrait" => state.portrait = Some(parse_expr_lossy(value)),
        "focus" => state.focus = Some(parse_expr_lossy(value)),
        "cleanup" => state.cleanup = Some(parse_expr_lossy(value)),
        "window" => {
            state.window =
                parse_required_entity_ref_syntax(value, base, errors).map(|(entity, _)| entity);
        }
        "source_locale" => state.source_locale = Some(value.to_owned()),
        "hooks" => push_line_hooks(&mut state.hooks, parse_expr_lossy(value)),
        "style" => state.style = Some(parse_expr_lossy(value)),
        name => state
            .line_args
            .push(LineArg::new(name.to_owned(), parse_expr_lossy(value))),
    }
}

fn push_line_hooks(hooks: &mut Vec<Expr>, expr: Expr) {
    if let Expr::BracketSeq(items) = expr {
        hooks.extend(items);
    } else {
        hooks.push(expr);
    }
}

fn split_comma_args(source: &str) -> Vec<&str> {
    split_top_level_punctuation(source, ',')
}

fn parse_contract_clause(line: &str) -> Option<ContractClause> {
    if let Some(rest) = line.strip_prefix("requires ") {
        let (mode, expr) = split_contract_mode(rest);
        return Some(ContractClause::Requires {
            mode,
            expr: parse_expr_lossy(expr),
        });
    }
    if let Some(rest) = line.strip_prefix("ensures ") {
        let (mode, expr) = split_contract_mode(rest);
        if let Some(effect) = expr.strip_prefix("no_effect ") {
            return Some(ContractClause::NoEffect(parse_expr_lossy(effect.trim())));
        }
        return Some(ContractClause::Ensures {
            mode,
            expr: parse_expr_lossy(expr),
        });
    }
    if let Some(rest) = line.strip_prefix("invariant ") {
        let (mode, expr) = split_contract_mode(rest);
        return Some(ContractClause::Invariant {
            mode,
            expr: parse_expr_lossy(expr),
        });
    }
    if let Some(rest) = line.strip_prefix("assume ") {
        return Some(ContractClause::Assume {
            expr: parse_expr_lossy(rest.trim()),
        });
    }
    if let Some(rest) = line.strip_prefix("reads ") {
        return Some(ContractClause::Reads(parse_contract_expr_list(rest)));
    }
    if let Some(rest) = line.strip_prefix("effects ") {
        return Some(ContractClause::Effects(parse_contract_expr_list(rest)));
    }
    if let Some(rest) = line.strip_prefix("modifies ") {
        return Some(ContractClause::Modifies(parse_contract_expr_list(rest)));
    }
    line.strip_prefix("decreases ")
        .map(|expr| ContractClause::Decreases(parse_expr_lossy(expr.trim())))
}

fn split_contract_mode(source: &str) -> (Option<String>, &str) {
    let trimmed = source.trim();
    for mode in ["prove", "check", "debug"] {
        if let Some(rest) = trimmed.strip_prefix(mode) {
            return (Some(mode.to_owned()), rest.trim());
        }
    }
    (None, trimmed)
}

fn parse_contract_expr_list(source: &str) -> Vec<crate::expr::Expr> {
    let body = source
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source)
        .trim();
    body.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(parse_expr_lossy)
        .collect()
}

fn split_supertraits(source: &str) -> Vec<String> {
    split_top_level_punctuation(source, '+')
        .into_iter()
        .map(str::trim)
        .filter(|trait_name| !trait_name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_optional_angle_head(source: &str) -> (Option<String>, &str) {
    let source = source.trim_start();
    if !source.starts_with('<') {
        return (None, source);
    }
    if let Some(close) = crate::cst::find_matching_angle_group(source, 0) {
        return (
            Some(source[..=close].to_owned()),
            source[close + '>'.len_utf8()..].trim_start(),
        );
    }
    (None, source)
}

fn collect_logical_block_items(body: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;

    for raw_line in body.lines().filter(|line| !line.trim().is_empty()) {
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(raw_line);
        for ch in raw_line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth <= 0 {
            lines.push(core::mem::take(&mut current));
            depth = 0;
        }
    }
    if !current.trim().is_empty() {
        lines.push(current);
    }
    lines
}

fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let open = find_top_level_punctuation(source, '{')?;
    let close = find_matching_punctuation(source, open, '{', '}')?;
    (source[close + '}'.len_utf8()..].trim().is_empty())
        .then(|| (source[..open].trim(), source[open + 1..close].trim()))
}

fn parse_match_arms(body: &str, base: usize, errors: &mut Vec<ParseError>) -> Vec<MatchArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (head, item) = split_top_level_punctuation_sequence_once(line, &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head);
            let mut nested = Parser::new(item.trim().to_owned());
            let parsed = nested.parse_flow_item_until_indent(0).map_or_else(
                || vec![FlowItem::Stmt(parse_stmt(item.trim()))],
                |item| vec![item],
            );
            errors.extend(nested.errors.into_iter().map(|err| err.rebased(base)));
            Some(MatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| parse_expr_lossy(guard.trim())),
                parsed,
            ))
        })
        .collect()
}

fn parse_match_expr_arms(body: &str) -> Vec<crate::expr::MatchExprArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (head, value) = split_top_level_punctuation_sequence_once(line, &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head);
            Some(crate::expr::MatchExprArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| Box::new(parse_expr_lossy(guard.trim()))),
                Box::new(parse_match_arm_value(value.trim())),
            ))
        })
        .collect()
}

fn split_pattern_guard(source: &str) -> (&str, Option<&str>) {
    split_top_level_keyword_once(source, "when")
}

fn parse_match_arm_value(source: &str) -> crate::expr::Expr {
    source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .map_or_else(|| parse_expr_lossy(source), parse_block_expr)
}

fn parse_select_branches(
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<SelectBranch> {
    let lines = body.lines().collect::<Vec<_>>();
    let mut branches = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        let Some(head) = trimmed
            .strip_suffix("=> {")
            .or_else(|| trimmed.strip_suffix("=>"))
        else {
            index += 1;
            continue;
        };
        let branch_indent = indentation(line);
        index += 1;
        let mut body_lines = Vec::new();
        while index < lines.len() {
            let child = lines[index];
            let child_trimmed = child.trim();
            if child_trimmed == "}" && indentation(child) <= branch_indent {
                index += 1;
                break;
            }
            body_lines.push(child);
            index += 1;
        }
        let mut nested = Parser::new(body_lines.join("\n"));
        let parsed = nested.parse_flow_body(&body_lines.join("\n"), base);
        errors.extend(nested.errors.into_iter().map(|err| err.rebased(base)));
        branches.push(SelectBranch::new(
            parse_select_branch_head(head.trim()),
            parsed,
        ));
    }
    branches
}

fn parse_select_branch_head(source: &str) -> SelectBranchHead {
    if let Some(rest) = source.strip_prefix("frame ") {
        return SelectBranchHead::Frame(parse_pattern(rest.trim()));
    }
    if let Some(rest) = source.strip_prefix("event ") {
        return SelectBranchHead::Event(parse_pattern(rest.trim()));
    }
    if let Some((name, source)) = split_top_level_binding(source) {
        let source = source.trim();
        let propagates_error = source.ends_with('?');
        return SelectBranchHead::Bind {
            name: name.trim().to_owned(),
            source: parse_expr_lossy(source.trim_end_matches('?').trim()),
            propagates_error,
        };
    }
    SelectBranchHead::Raw(source.to_owned())
}

fn split_speaker_line(trimmed: &str) -> Option<(String, Option<String>, &str)> {
    let colon = find_top_level_colon(trimmed)?;
    if has_top_level_square(&trimmed[..colon]) || trimmed[..colon].contains("->") {
        return None;
    }
    let head = trimmed[..colon].trim();
    let content = trimmed[colon + 1..].trim();
    if head.is_empty() || head.starts_with("cancel ") || head.starts_with("at(") {
        return None;
    }
    let (speaker, args) = split_call_head(head);
    Some((speaker, args, content))
}

fn has_top_level_square(input: &str) -> bool {
    let mut depth = 0_i32;
    let mut in_string = false;
    for ch in input.chars() {
        match ch {
            '"' => in_string = !in_string,
            '[' if depth == 0 && !in_string => return true,
            '(' | '{' | '[' if !in_string => depth += 1,
            ')' | '}' | ']' if !in_string => depth -= 1,
            _ => {}
        }
    }
    false
}

fn find_top_level_colon(input: &str) -> Option<usize> {
    find_top_level_punctuation(input, ':')
}

fn split_call_head(head: &str) -> (String, Option<String>) {
    let head = head.trim();
    if let Some(open) = find_top_level_punctuation(head, '(') {
        if let Some(close) = find_matching_punctuation(head, open, '(', ')')
            && head[close + ')'.len_utf8()..].trim().is_empty()
        {
            return (
                head[..open].trim().to_owned(),
                Some(head[open + 1..close].trim().to_owned()),
            );
        }
    }
    (head.to_owned(), None)
}

fn find_content_bracket(text: &str) -> Option<usize> {
    let open = find_top_level_punctuation(text, '[')?;
    (!text[..open].trim_end().ends_with('#')).then_some(open)
}

fn attach_line_plan_label(plan: LinePlan, label: Option<String>) -> LinePlan {
    if let Some(label) = label {
        plan.with_label(label)
    } else {
        plan
    }
}

fn parse_with_indent_label(trimmed: &str) -> Option<OptionalLabel> {
    if trimmed == "with:" {
        return Some(OptionalLabel::None);
    }
    let label = trimmed.strip_prefix("with ")?.strip_suffix(':')?.trim();
    parse_label_ref(label)
        .and_then(|(label, tail)| tail.trim().is_empty().then_some(OptionalLabel::Some(label)))
}

fn parse_inline_with_colon_plan(trimmed: &str) -> Option<(Option<String>, &str)> {
    let rest = trimmed.strip_prefix("with")?.trim_start();
    if let Some(body) = rest.strip_prefix(':') {
        let body = body.trim();
        return (!body.is_empty()).then_some((None, body));
    }
    let (label, tail) = parse_label_ref(rest)?;
    let body = tail.trim_start().strip_prefix(':')?.trim();
    (!body.is_empty()).then_some((Some(label), body))
}

fn is_with_brace_head(trimmed: &str) -> bool {
    trimmed.starts_with("with {")
        || trimmed == "with{"
        || trimmed.starts_with("with '")
        || trimmed.starts_with("with'")
}

fn parse_with_brace_label(head: &str) -> Option<String> {
    let label = head.strip_prefix("with")?.trim();
    parse_label_ref(label).and_then(|(label, tail)| tail.trim().is_empty().then_some(label))
}

fn split_optional_block_label(head: &str) -> (Option<String>, &str) {
    labeled_head_tail(head).map_or((None, head), |tail| {
        let label = head
            .trim_start()
            .strip_prefix('\'')
            .and_then(|rest| split_top_level_punctuation_once(rest, ':'))
            .map(|(label, _)| label.trim().to_owned())
            .unwrap_or_default();
        (Some(label), tail)
    })
}

fn labeled_head_tail(head: &str) -> Option<&str> {
    let rest = head.trim_start().strip_prefix('\'')?;
    let (_, tail) = split_top_level_punctuation_once(rest, ':')?;
    Some(tail.trim_start())
}

fn parse_expr_lossy(source: &str) -> crate::expr::Expr {
    if let Some(expr) = parse_presentation_special_call(source) {
        return expr;
    }
    if let Some((head, body)) = split_brace_item(source) {
        let name = head.trim();
        if is_plain_block_callee(name) {
            return parse_named_block_expr(name, body);
        }
    }
    parse_expr(source).unwrap_or_else(|_| crate::expr::Expr::Raw(source.to_owned()))
}

fn parse_presentation_special_call(source: &str) -> Option<crate::expr::Expr> {
    let trimmed = source.trim();
    let (callee, call_source) = if let Some(rest) = trimmed.strip_prefix("ref bg") {
        ("ref.bg", rest)
    } else if let Some(rest) = trimmed.strip_prefix("ref show") {
        ("ref.show", rest)
    } else if let Some(rest) = trimmed.strip_prefix("clear bg") {
        ("clear.bg", rest)
    } else {
        return None;
    };
    if !call_source.trim_start().starts_with('(') {
        return None;
    }
    let crate::expr::Expr::Call { args, .. } = parse_expr_lossy(&format!("_{call_source}")) else {
        return None;
    };
    Some(crate::expr::Expr::Call {
        callee: Box::new(crate::expr::Expr::Path(callee.to_owned())),
        args,
    })
}

fn is_plain_block_callee(source: &str) -> bool {
    !source.is_empty()
        && source
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':'))
        && source
            .chars()
            .next()
            .is_some_and(|ch| ch.is_lowercase() || ch == '_')
}

fn is_await_with_head(trimmed: &str) -> bool {
    (trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? "))
        && (trimmed.contains(" with ") || trimmed.ends_with("with:"))
}

fn has_inline_brace_await_with(trimmed: &str) -> bool {
    (trimmed.contains(" with {") || trimmed.contains(" with{")) && trimmed.contains('{')
}

fn has_standalone_brace_with(trimmed: &str) -> bool {
    trimmed.starts_with("with {") || trimmed.starts_with("with{")
}

fn append_await_head_continuation(head: &mut String, continuation: &str) {
    if continuation.starts_with('.') {
        head.push_str(continuation);
    } else {
        head.push(' ');
        head.push_str(continuation);
    }
}

fn normalize_let_await_source(head: &str, trailing: Option<&str>) -> String {
    let source = trailing.map_or_else(
        || head.trim().to_owned(),
        |trailing| format!("{head}{trailing}"),
    );
    normalize_parenthesized_await_source(&source).unwrap_or(source)
}

fn normalize_parenthesized_await_source(source: &str) -> Option<String> {
    let source = source.trim();
    let (await_part, postfix) = split_parenthesized_await_postfix(source)?;
    let postfix = postfix.trim();
    let applies_try = postfix.ends_with('?');
    let context_call = postfix.trim_end_matches('?').strip_prefix(".context(");
    let await_part = await_part.trim();
    let await_part = await_part.strip_prefix("await ")?;
    let await_head = context_call
        .and_then(|args| args.strip_suffix(')'))
        .map_or_else(
            || await_part.to_owned(),
            |args| insert_context_before_await_with(await_part, args),
        );
    Some(if applies_try {
        format!("try await {await_head}")
    } else {
        format!("await {await_head}")
    })
}

fn split_parenthesized_await_postfix(source: &str) -> Option<(&str, &str)> {
    source.strip_prefix('(')?;
    let close = find_matching_punctuation(source, 0, '(', ')')?;
    Some((&source[1..close], &source[close + ')'.len_utf8()..]))
}

fn insert_context_before_await_with(await_part: &str, args: &str) -> String {
    let (expr, branches) = split_await_head(await_part);
    if await_part.contains(" with:") {
        format!("{}.context({args}) with:{branches}", expr.trim())
    } else if await_part.contains(" with ") {
        format!("{}.context({args}) with {branches}", expr.trim())
    } else {
        format!("{}.context({args})", expr.trim())
    }
}

fn parse_await_with(trimmed: &str, range: TextRange, errors: &mut Vec<ParseError>) -> AwaitWith {
    let source = trimmed.trim();
    let (applies_try, after_keyword) = source
        .strip_prefix("try await")
        .map(|rest| (true, rest.trim()))
        .or_else(|| {
            source
                .strip_prefix("await?")
                .map(|rest| (true, rest.trim()))
        })
        .or_else(|| {
            source
                .strip_prefix("await")
                .map(|rest| (false, rest.trim()))
        })
        .unwrap_or((false, source));
    let (expr_part, branch_part) = split_await_head(after_keyword);

    // Postfix `?` remains the ordinary Rust-like propagation operator. The
    // rejected form is only `await expr? with:`, where pending handling must
    // group with the await before propagation is applied.
    if expr_part.trim_end().ends_with('?') {
        errors.push(ParseError::new(
            range,
            vec!["try await expr with:".to_owned()],
            Some(expr_part.trim().to_owned()),
            "`await expr? with` is ambiguous; use `try await expr with`".to_owned(),
            vec![RecoverySuggestion {
                message: "move `?` before `await` as `try await`".to_owned(),
            }],
            SourceAnchor::new(SourceName::path("<memory>"), range.start()..range.end()),
        ));
    }

    AwaitWith::new(
        parse_expr_lossy(expr_part.trim_end_matches('?').trim()),
        applies_try,
        parse_await_branches(branch_part.trim()),
    )
}

fn split_await_head(source: &str) -> (&str, &str) {
    let (expr, branches) = split_top_level_keyword_once(source, "with");
    if let Some(branches) = branches {
        return (expr, branches.trim_start_matches(':').trim_start());
    }
    (source, "")
}

fn parse_await_branches(source: &str) -> Vec<AwaitBranch> {
    let body = source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source)
        .trim();
    if source_lines(body)
        .into_iter()
        .any(|line| is_colon_await_branch_head(line.trim()))
    {
        return parse_colon_await_branches(body);
    }
    split_await_branch_lines(body)
        .into_iter()
        .filter_map(parse_await_branch)
        .collect()
}

fn parse_colon_await_branches(source: &str) -> Vec<AwaitBranch> {
    let mut branches = Vec::new();
    let mut current_head = None::<String>;
    let mut current_body = String::new();

    for line in source_lines(source) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_colon_await_branch_head(trimmed) {
            if let Some(head) = current_head.replace(trimmed.to_owned()) {
                if let Some(branch) = parse_colon_await_branch(&head, &current_body) {
                    branches.push(branch);
                }
                current_body.clear();
            }
        } else {
            if !current_body.is_empty() {
                current_body.push('\n');
            }
            current_body.push_str(line);
        }
    }

    if let Some(head) = current_head {
        if let Some(branch) = parse_colon_await_branch(&head, &current_body) {
            branches.push(branch);
        }
    }
    branches
}

fn is_colon_await_branch_head(trimmed: &str) -> bool {
    trimmed.ends_with(':')
        && matches!(
            trimmed.trim_end_matches(':').split_whitespace().next(),
            Some("pending" | "ready" | "error" | "denied")
        )
}

fn parse_colon_await_branch(head: &str, body: &str) -> Option<AwaitBranch> {
    let mut parts = head.trim_end_matches(':').split_whitespace();
    let kind = match parts.next()? {
        "pending" => AwaitBranchKind::Pending,
        "ready" => AwaitBranchKind::Ready,
        "error" => AwaitBranchKind::Error,
        "denied" => AwaitBranchKind::Denied,
        _ => return None,
    };
    let pattern = parse_pattern(parts.collect::<Vec<_>>().join(" ").trim());
    Some(AwaitBranch::new(
        kind,
        pattern,
        parse_await_branch_body(body),
    ))
}

fn split_await_branch_lines(source: &str) -> Vec<&str> {
    if source_line_count(source) > 1 {
        return nonempty_trimmed_source_lines(source);
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for keyword in [" pending ", " ready ", " error ", " denied "] {
        for (index, _) in source.match_indices(keyword) {
            let line = source[start..index].trim();
            if !line.is_empty() {
                lines.push(line);
            }
            start = index + 1;
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        lines.push(tail);
    }
    lines
}

fn parse_await_branch(line: &str) -> Option<AwaitBranch> {
    let (head, body) = split_top_level_punctuation_sequence_once(line, &["=", ">"])?;
    let mut parts = head.split_whitespace();
    let kind = match parts.next()? {
        "pending" => AwaitBranchKind::Pending,
        "ready" => AwaitBranchKind::Ready,
        "error" => AwaitBranchKind::Error,
        "denied" => AwaitBranchKind::Denied,
        _ => return None,
    };
    let pattern = parse_pattern(parts.collect::<Vec<_>>().join(" ").trim());
    Some(AwaitBranch::new(
        kind,
        pattern,
        parse_await_branch_body(body.trim()),
    ))
}

fn parse_await_branch_body(body: &str) -> Vec<FlowItem> {
    let mut nested = Parser::new(body.to_owned());
    let mut items = Vec::new();
    while nested.index < nested.events.len() {
        nested.skip_blank_and_comments();
        if nested.index >= nested.events.len() {
            break;
        }
        let before = nested.index;
        let item = nested.parse_flow_item_until_indent(0).unwrap_or_else(|| {
            let stmt = FlowItem::Stmt(parse_stmt(nested.current().text.trim()));
            nested.index += 1;
            stmt
        });
        items.push(item);
        if nested.index == before {
            nested.index += 1;
        }
    }
    if items.is_empty() && !body.trim().is_empty() {
        items.push(parse_inline_await_branch_item(body.trim()));
    }
    items
}

fn parse_inline_await_branch_item(body: &str) -> FlowItem {
    let mut nested = Parser::new(body.to_owned());
    nested
        .parse_flow_item_until_indent(0)
        .unwrap_or_else(|| FlowItem::Stmt(parse_stmt(body)))
}

fn is_typed_stmt(trimmed: &str) -> bool {
    matches!(
        trimmed.split_whitespace().next(),
        Some(
            "let"
                | "match"
                | "if"
                | "for"
                | "return"
                | "out"
                | "goto"
                | "thread"
                | "defer"
                | "yield"
                | "panic"
                | "fail"
                | "bail"
                | "ensure"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
    )
}

enum ParsedScopeName<'a> {
    Named(&'a str),
    Unnamed,
}

impl<'a> ParsedScopeName<'a> {
    const fn as_option(&self) -> Option<&'a str> {
        match self {
            Self::Named(name) => Some(name),
            Self::Unnamed => None,
        }
    }
}

fn parse_scope_head(source: &str) -> Option<ParsedScopeName<'_>> {
    let rest = source.strip_prefix("scope")?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !(ch.is_whitespace() || ch == '{'))
    {
        return None;
    }

    let rest = rest.trim_start();
    if rest.is_empty() || rest.starts_with('{') {
        return Some(ParsedScopeName::Unnamed);
    }

    let name = rest.trim();
    (!name.is_empty()).then_some(ParsedScopeName::Named(name))
}

fn parse_flat_fence(source: &str) -> Option<FlatFence<'_>> {
    let trimmed = source.trim();
    let inner = trimmed.strip_prefix("===")?.strip_suffix("===")?.trim();
    if inner.is_empty() {
        return Some(FlatFence {
            kind: "",
            head: "",
            close: false,
        });
    }
    if let Some(close) = inner.strip_prefix('/') {
        let kind = close.split_whitespace().next().unwrap_or_default();
        return Some(FlatFence {
            kind,
            head: close.trim(),
            close: true,
        });
    }
    let (kind, head) = split_leading_ident(inner).unwrap_or((inner, ""));
    Some(FlatFence {
        kind,
        head: head.trim(),
        close: false,
    })
}

fn parse_memo_block_options(source: &str) -> Option<Vec<(String, Expr)>> {
    let args = source
        .trim()
        .strip_prefix("memo(")?
        .trim_end()
        .strip_suffix(')')?;
    Some(
        split_comma_args(args)
            .into_iter()
            .filter_map(|part| {
                split_top_level_punctuation_once(part, '=')
                    .map(|(name, value)| (name.trim().to_owned(), parse_expr_lossy(value.trim())))
            })
            .collect(),
    )
}

fn parse_computation_block_kind(source: &str) -> Option<ComputationBlockKind> {
    match source {
        "result" => Some(ComputationBlockKind::Result),
        "task" => Some(ComputationBlockKind::Task),
        "seq" => Some(ComputationBlockKind::Seq),
        "stream" => Some(ComputationBlockKind::Stream),
        _ => None,
    }
}

fn split_top_level_binding(source: &str) -> Option<(&str, &str)> {
    split_top_level_punctuation_once(source, '=')
}

fn split_if_let_guard(source: &str) -> (&str, Option<&str>) {
    split_top_level_keyword_once(source, "when")
}

fn parse_scope_expr_body(body: &str) -> (Vec<Stmt>, Option<crate::expr::Expr>) {
    let lines = collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return (Vec::new(), None);
    };
    let parsed_statements = statements
        .iter()
        .map(|line| parse_stmt(line.as_str()))
        .collect::<Vec<_>>();
    if let Some(value) = parse_final_block_expr(last.as_str()) {
        return (parsed_statements, Some(value));
    }
    if is_typed_stmt(last) {
        let mut parsed_statements = parsed_statements;
        parsed_statements.push(parse_stmt(last.as_str()));
        (parsed_statements, None)
    } else {
        (parsed_statements, Some(parse_expr_lossy(last.as_str())))
    }
}

fn parse_final_block_expr(source: &str) -> Option<crate::expr::Expr> {
    let (head, body) = split_brace_item(source)?;
    head.strip_prefix("match ")
        .map(str::trim)
        .map(|scrutinee| crate::expr::Expr::Match {
            scrutinee: Box::new(parse_expr_lossy(scrutinee)),
            arms: parse_match_expr_arms(body),
        })
}

fn parse_block_expr(body: &str) -> crate::expr::Expr {
    let (statements, value) = parse_scope_expr_body(body);
    crate::expr::Expr::Block {
        statements,
        value: value.map(Box::new),
    }
}

fn parse_named_block_expr(name: &str, body: &str) -> crate::expr::Expr {
    let (statements, value) = parse_scope_expr_body(body);
    crate::expr::Expr::NamedBlock {
        name: name.to_owned(),
        statements,
        value: value.map(Box::new),
    }
}

fn split_embedded_else_body(body: &str) -> Option<(String, String)> {
    let mut then_lines = Vec::new();
    let mut else_lines = Vec::new();
    let mut in_else = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if matches!(trimmed, "} else {" | "} else{") {
            in_else = true;
            continue;
        }
        if in_else {
            else_lines.push(line);
        } else {
            then_lines.push(line);
        }
    }
    in_else.then(|| (then_lines.join("\n"), else_lines.join("\n")))
}

fn parse_stmt_lines(body: &str) -> Vec<Stmt> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .map(|line| parse_stmt(&line))
        .collect()
}

fn parse_expr_with_inline_line_plan(source: &str) -> Expr {
    let Some((expr_source, trailing_plan)) = split_inline_dialogue_line_plan(source) else {
        return parse_dialogue_call_expr_source(source).unwrap_or_else(|| parse_expr_lossy(source));
    };
    let mut expr = parse_dialogue_call_expr_source(expr_source.trim())
        .unwrap_or_else(|| parse_expr_lossy(expr_source.trim()));
    let Some(plan) = parse_inline_line_plan_source(trailing_plan) else {
        return parse_expr_lossy(source);
    };
    if attach_plan_to_dialogue_expr(&mut expr, plan) {
        expr
    } else {
        parse_expr_lossy(source)
    }
}

fn parse_dialogue_call_expr_source(source: &str) -> Option<Expr> {
    if let Some(rest) = source.trim().strip_prefix("try ") {
        return Some(Expr::Try {
            expr: Box::new(parse_dialogue_call_expr_source(rest.trim())?),
        });
    }
    let open = find_content_bracket(source)?;
    let close = find_matching_punctuation(source, open, '[', ']')?;
    if !source[close + 1..].trim().is_empty() {
        return None;
    }
    let callee = source[..open].trim();
    if callee.is_empty() {
        return None;
    }
    let content = source[open + 1..close].trim();
    if crate::expr::parse_expr(content).is_ok() {
        return None;
    }
    Some(Expr::DialogueCall {
        callee: Box::new(parse_expr_lossy(callee)),
        content: content.to_owned(),
        plan: None,
    })
}

fn attach_plan_to_dialogue_expr(expr: &mut Expr, line_plan: LinePlan) -> bool {
    match expr {
        Expr::DialogueCall { plan, .. } => {
            *plan = Some(line_plan);
            true
        }
        Expr::Try { expr } => attach_plan_to_dialogue_expr(expr, line_plan),
        _ => false,
    }
}

fn contains_dialogue_expr(expr: &Expr) -> bool {
    match expr {
        Expr::DialogueCall { .. } => true,
        Expr::Try { expr } => contains_dialogue_expr(expr),
        _ => false,
    }
}

fn split_inline_dialogue_line_plan(source: &str) -> Option<(&str, &str)> {
    let (head, tail) = split_top_level_keyword_once(source, "with");
    let tail = tail?;
    if matches!(tail.trim_start().chars().next(), Some(':' | '{' | '\'')) {
        let head_end = head.trim_end().len();
        Some((&source[..head_end], source[head_end..].trim_start()))
    } else {
        None
    }
}

fn parse_inline_line_plan_source(source: &str) -> Option<LinePlan> {
    if is_with_brace_head(source) {
        let (head, body) = split_brace_item(source)?;
        return Some(attach_line_plan_label(
            parse_line_plan_body(BlockStyle::Brace, body, TextRange::new(0, source.len())),
            parse_with_brace_label(head.trim()),
        ));
    }
    parse_inline_with_colon_plan(source).map(|(label, body)| {
        attach_line_plan_label(
            parse_line_plan_body(BlockStyle::Indent, body, TextRange::new(0, source.len())),
            label,
        )
    })
}

fn parse_stmt(trimmed: &str) -> Stmt {
    match classify_stmt(trimmed) {
        CstStmtKind::LifetimeSet => {
            let Some((target, expr)) =
                split_top_level_punctuation_sequence_once(trimmed, &["<", "-"])
            else {
                return raw_stmt(trimmed);
            };
            Stmt::LifetimeSet {
                target: parse_expr_lossy(target.trim()),
                expr: parse_expr_lossy(expr.trim()),
            }
        }
        CstStmtKind::Wait => trimmed
            .strip_prefix("wait ")
            .map(str::trim)
            .map_or_else(|| raw_stmt(trimmed), parse_wait_stmt),
        CstStmtKind::Let => parse_let_stmt(trimmed),
        CstStmtKind::DeferBlock | CstStmtKind::Braced | CstStmtKind::UnsafeLifetime => {
            parse_braced_stmt(trimmed).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::Defer => trimmed.strip_prefix("defer ").map_or_else(
            || raw_stmt(trimmed),
            |rest| Stmt::Defer {
                outcome: DeferOutcome::Always,
                expr: parse_expr_lossy(rest.trim()),
            },
        ),
        CstStmtKind::ControlTransfer => {
            parse_control_transfer_stmt(trimmed).unwrap_or_else(|| raw_stmt(trimmed))
        }
        CstStmtKind::Ensure => parse_ensure_stmt(trimmed),
        CstStmtKind::On => parse_on_stmt(trimmed),
        CstStmtKind::PresentationCall => {
            parse_presentation_special_call(trimmed).map_or_else(|| raw_stmt(trimmed), Stmt::Expr)
        }
        CstStmtKind::ScenarioCommand => {
            parse_word_scenario_command(trimmed, TextRange::new(0, trimmed.len()))
                .map_or_else(|| raw_stmt(trimmed), Stmt::Command)
        }
        CstStmtKind::AmbiguousBlockHead => raw_stmt(trimmed),
        CstStmtKind::Expr => Stmt::Expr(parse_expr_lossy(trimmed)),
    }
}

fn raw_stmt(source: &str) -> Stmt {
    Stmt::Raw(RawSyntax::stmt(
        source,
        Some(TextRange::new(0, source.len())),
    ))
}

fn parse_let_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return raw_stmt(trimmed);
    };
    if let Some((pattern, expr)) = split_top_level_binding(rest) {
        let (pattern, ty) = parse_binding_pattern(pattern);
        Stmt::Let {
            pattern,
            ty,
            expr: parse_expr_with_inline_line_plan(expr.trim()),
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_ensure_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("ensure ") else {
        return raw_stmt(trimmed);
    };
    if let Some((condition, message)) = split_top_level_punctuation_once(rest, ',') {
        Stmt::Ensure {
            condition: parse_expr_lossy(condition.trim()),
            message: parse_expr_lossy(message.trim()),
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_on_stmt(trimmed: &str) -> Stmt {
    let Some(rest) = trimmed.strip_prefix("on ") else {
        return raw_stmt(trimmed);
    };
    if let Some((head, action)) = split_top_level_punctuation_sequence_once(rest, &["=", ">"]) {
        Stmt::On {
            trigger: parse_trigger_pattern(head.trim()),
            body: vec![parse_stmt(action.trim())],
        }
    } else {
        raw_stmt(trimmed)
    }
}

fn parse_wait_stmt(rest: &str) -> Stmt {
    if let Some(name) = rest.strip_prefix("mark ") {
        return Stmt::Wait(WaitTarget::Mark(name.trim().to_owned()));
    }
    let expr = parse_expr_lossy(rest);
    match expr {
        Expr::Literal(crate::expr::Literal::Duration { .. }) => {
            Stmt::Wait(WaitTarget::Duration(expr))
        }
        _ => Stmt::Wait(WaitTarget::Expr(expr)),
    }
}

fn parse_braced_stmt(trimmed: &str) -> Option<Stmt> {
    let (head, body) = split_brace_item(trimmed)?;
    if head.starts_with("unsafe lifetime ") {
        let mut errors = Vec::new();
        return Some(parse_unsafe_lifetime_block(head, body, 0, &mut errors));
    }
    if head.starts_with("thread") {
        return Some(Stmt::Thread(parse_thread_block(head, body)));
    }
    if let Some(outcome) = parse_defer_outcome(head) {
        return Some(Stmt::DeferBlock {
            outcome,
            statements: parse_stmt_lines(body),
        });
    }
    if head.starts_with("scope") {
        return Some(Stmt::Expr(parse_named_block_expr(head, body)));
    }
    if let Some(condition) = head.strip_prefix("if ") {
        return Some(Stmt::If {
            condition: parse_expr_lossy(condition.trim()),
            body: parse_stmt_lines(body),
        });
    }
    if head == "loop" {
        return Some(Stmt::Loop {
            body: parse_stmt_lines(body),
        });
    }
    if let Some(stmt) = parse_braced_while_let_stmt(head, body) {
        return Some(stmt);
    }
    if let Some(condition) = head.strip_prefix("while ") {
        return Some(Stmt::While {
            condition: parse_expr_lossy(condition.trim()),
            body: parse_stmt_lines(body),
        });
    }
    if let Some(rest) = head.strip_prefix("for ") {
        let (pattern, Some(source)) = split_top_level_keyword_once(rest, "in") else {
            return Some(raw_stmt(trimmed));
        };
        return Some(Stmt::For {
            pattern: parse_pattern(pattern.trim()),
            source: parse_expr_lossy(source.trim()),
            body: parse_stmt_lines(body),
        });
    }
    head.strip_prefix("match ").map(|expr| Stmt::Match {
        expr: parse_expr_lossy(expr.trim()),
        arms: parse_stmt_match_arms(body),
    })
}

fn parse_unsafe_lifetime_block(
    head: &str,
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Stmt {
    let mut lines = head.lines().map(str::trim).filter(|line| !line.is_empty());
    let first = lines.next().unwrap_or(head.trim());
    let rest = first
        .trim_start()
        .strip_prefix("unsafe lifetime")
        .unwrap_or_default()
        .trim();
    let (id, trailing) = parse_required_id_ref(rest, base, errors).unwrap_or_else(|| {
        (
            IdRef::relative(RelativeId::new(
                "missing".to_owned(),
                0,
                RelativeIdSpelling::DotRun,
                TextRange::new(base, base),
            )),
            "",
        )
    });
    let inline_reason = split_top_level_keyword_once(trailing.trim(), "reason")
        .1
        .and_then(|tail| split_top_level_binding(tail.trim()).map(|(_, expr)| expr.trim()));
    let reason = inline_reason
        .or_else(|| {
            lines.find_map(|line| {
                line.strip_prefix("reason").and_then(|tail| {
                    split_top_level_binding(tail.trim()).map(|(_, expr)| expr.trim())
                })
            })
        })
        .map(parse_expr_lossy);
    let has_safety_doc = body
        .lines()
        .any(|line| line.trim_start().starts_with("/// SAFETY"));
    let executable_body = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("///"))
        .collect::<Vec<_>>()
        .join("\n");
    Stmt::UnsafeLifetime {
        id,
        reason,
        has_safety_doc,
        body: parse_stmt_lines(&executable_body),
    }
}

fn parse_braced_while_let_stmt(head: &str, body: &str) -> Option<Stmt> {
    let rest = head.strip_prefix("while let ")?;
    let Some((pattern, expr_and_guard)) = split_top_level_binding(rest) else {
        return Some(raw_stmt(&format!("{head} {{ {body} }}")));
    };
    let (expr, guard) = split_pattern_guard(expr_and_guard.trim());
    Some(Stmt::WhileLet {
        pattern: parse_pattern(pattern.trim()),
        expr: parse_expr_lossy(expr.trim()),
        guard: guard.map(|guard| parse_expr_lossy(guard.trim())),
        body: parse_stmt_lines(body),
    })
}

fn parse_control_transfer_stmt(trimmed: &str) -> Option<Stmt> {
    if trimmed == "break" {
        return Some(Stmt::Break {
            label: None,
            expr: None,
        });
    }
    if let Some(rest) = trimmed.strip_prefix("continue") {
        if rest.trim().is_empty() {
            return Some(Stmt::Continue { label: None });
        }
        let rest = rest.trim();
        return parse_label_ref(rest).and_then(|(label, tail)| {
            tail.trim()
                .is_empty()
                .then_some(Stmt::Continue { label: Some(label) })
        });
    }
    if let Some(rest) = trimmed.strip_prefix("out ") {
        let (label, expr) = split_optional_label_ref(rest.trim());
        return Some(Stmt::Out {
            label,
            expr: parse_expr_lossy(expr.trim()),
        });
    }
    if let Some(rest) = trimmed.strip_prefix("break ") {
        let (label, expr) = split_optional_label_ref(rest.trim());
        return Some(Stmt::Break {
            label,
            expr: (!expr.trim().is_empty()).then(|| parse_expr_lossy(expr.trim())),
        });
    }
    [
        ("return ", Stmt::Return as fn(Expr) -> Stmt),
        ("goto ", Stmt::Goto),
        ("yield ", Stmt::Yield),
        ("panic ", Stmt::Panic),
        ("fail ", Stmt::Fail),
        ("bail ", Stmt::Bail),
        ("close ", Stmt::Close),
        ("select ", Stmt::Select),
    ]
    .into_iter()
    .find_map(|(prefix, build)| {
        trimmed
            .strip_prefix(prefix)
            .map(str::trim)
            .map(parse_expr_lossy)
            .map(build)
    })
}

fn split_optional_label_ref(input: &str) -> (Option<String>, &str) {
    parse_label_ref(input).map_or((None, input), |(label, tail)| (Some(label), tail))
}

fn parse_label_ref(input: &str) -> Option<(String, &str)> {
    let (label, rest) = crate::cst::split_leading_lifetime(input)?;
    Some((label.trim_start_matches('\'').to_owned(), rest))
}

fn parse_stmt_match_arms(body: &str) -> Vec<StmtMatchArm> {
    collect_logical_block_items(body)
        .into_iter()
        .filter_map(|line| {
            let (head, value) =
                split_top_level_punctuation_sequence_once(line.trim(), &["=", ">"])?;
            let (pattern, guard) = split_pattern_guard(head.trim());
            let body = value
                .trim()
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .map_or_else(
                    || vec![parse_stmt(value.trim())],
                    |block| parse_stmt_lines(block.trim()),
                );
            Some(StmtMatchArm::new(
                parse_pattern(pattern.trim()),
                guard.map(|guard| parse_expr_lossy(guard.trim())),
                body,
            ))
        })
        .collect()
}

fn indentation(text: &str) -> usize {
    text.chars().take_while(|ch| ch.is_whitespace()).count()
}
