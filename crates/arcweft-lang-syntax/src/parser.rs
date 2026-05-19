use crate::ast::common::{DocBlock, TextRange};
use crate::ast::dialogue::{
    ContentCall, DialogueContent, LineArg, LineOptions, LineOptionsInit, ScenarioCommand,
    SpeakerLine,
};
use crate::ast::flow::{
    BorrowBlock, Flow, FlowInit, FlowItem, ForBlock, IfBlock, IfLetBlock, LoopBlock, MatchArm,
    MatchBlock, ScopeBlock, ScopeExprBlock, SelectBlock, SelectBranch, SelectBranchHead, Stmt,
    StmtMatchArm, WaitTarget, WhileBlock, WhileLetBlock,
};
use crate::ast::ids::{EntityRefSyntax, IdRef, RelativeId, RelativeIdSpelling, WikiLink};
use crate::ast::items::{RawSyntax, TypedSyntaxTree};
use crate::ast::line_plan::{BlockStyle, DeferOutcome, LinePlan};
use crate::ast::pattern::Pattern;
use crate::cst::{
    CstBlockOpenRule, CstFlowItemKind, CstLetFlowItemKind, CstLine, CstLineEvents, CstStmtKind,
    CstStructuredFlowBlockKind, CstTopLevelItemKind, CstTopLevelLineKind, SyntaxNode,
    classify_stmt, collect_wiki_link_ranges, cst_lines, find_matching_punctuation,
    find_top_level_punctuation, punctuation_delta, source_lines, split_leading_ident,
    split_top_level_keyword_once, split_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once, split_top_level_whitespace,
};
use crate::expr::{ComputationBlockKind, Expr, parse_expr};
use crate::pattern::parse_pattern;
use crate::source::ParsedSource;
use crate::text::parse_dialogue_tokens;
use crate::types::parse_type_ref;
use arcweft_source::{SourceAnchor, SourceName};

pub mod await_;
pub mod choice;
pub mod control_flow;
pub mod dialogue;
pub mod flow;
pub mod headers;
pub mod helpers;
pub mod hooks;
pub mod items;
pub mod line_plan;
pub mod proof;
pub mod recovery;
pub mod source;
pub mod statements;
pub mod top_level;
use await_::{is_await_with_head, parse_await_with};
use control_flow::{
    parse_block_expr, parse_braced_while_let_stmt, parse_named_block_expr, parse_scope_expr_body,
    parse_stmt_lines, parse_stmt_match_arms, split_pattern_guard,
};
use headers::{parse_required_entity_ref_syntax, parse_required_id_ref, simple_error};
use line_plan::{
    nonempty_string, parse_defer_outcome, parse_line_plan_body, parse_thread_block,
    parse_trigger_pattern,
};
use recovery::{ParseError, RecoverySuggestion};
use statements::{
    parse_label_ref, parse_scope_head, parse_stmt, parse_unsafe_lifetime_block, raw_stmt,
};

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

fn indentation(text: &str) -> usize {
    text.chars().take_while(|ch| ch.is_whitespace()).count()
}
