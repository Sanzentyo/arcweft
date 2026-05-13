use crate::ast::{
    Attribute, AwaitBranch, AwaitBranchKind, AwaitWith, BlockStyle, CancelRuleSyntax, ChoiceBlock,
    ChoiceOption, ContentCall, ContractClause, DialogueContent, EntityRef, Flow, FlowInit,
    FlowItem, FlowKind, HookItem, IfBlock, Item, LinePlan, LinePlanItem, MatchArm, MatchBlock,
    MemoFn, ModuleDecl, ParserItem, Pattern, RawItem, ScenarioCommand, SpeakerLine, Stmt,
    SyntaxTree, TextRange, UseItem, UseMode, Visibility, WikiLink,
};
use crate::expr::parse_expr;
use crate::text::parse_dialogue_tokens;
use crate::types::parse_type_ref;
use arcweft_source::{SourceAnchor, SourceName};
use thiserror::Error;

/// Parses an Arcweft source string.
pub fn parse_source(source: impl Into<String>) -> Result<SyntaxTree, Vec<ParseError>> {
    let source = source.into();
    let mut parser = Parser::new(source);
    parser.parse()
}

/// Compatibility entry point kept as a direct alias to the real parser.
pub fn parse_stub(source: impl Into<String>) -> Result<SyntaxTree, Vec<ParseError>> {
    parse_source(source)
}

/// Syntax-level parse error with expected tokens and recovery suggestions.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ParseError {
    range: TextRange,
    expected: Vec<String>,
    found: Option<String>,
    message: String,
    recovery: Vec<RecoverySuggestion>,
    anchor: SourceAnchor,
}

/// Suggested local edit or strategy for recovering from an error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoverySuggestion {
    message: String,
}

#[derive(Clone, Debug)]
struct SourceLine {
    text: String,
    start: usize,
    end: usize,
}

struct Parser {
    source: String,
    lines: Vec<SourceLine>,
    index: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    fn new(source: String) -> Self {
        let lines = split_lines(&source);
        Self {
            source,
            lines,
            index: 0,
            errors: Vec::new(),
        }
    }

    fn parse(&mut self) -> Result<SyntaxTree, Vec<ParseError>> {
        let mut module = None;
        let mut uses = Vec::new();
        let mut items = Vec::new();
        let wiki_links = collect_wiki_links(&self.source);

        while self.index < self.lines.len() {
            self.skip_blank_and_comments();
            if self.index >= self.lines.len() {
                break;
            }

            let line = self.current().clone();
            let trimmed = line.text.trim().to_owned();
            let range = TextRange::new(line.start, line.end);

            if let Some(attribute) = parse_attribute(&trimmed, range.clone()) {
                items.push(Item::Attribute(attribute));
                self.index += 1;
            } else if let Some(path) = trimmed.strip_prefix("mod ") {
                module = Some(ModuleDecl::new(path.trim().to_owned(), range));
                self.index += 1;
            } else if is_use_line(&trimmed) {
                if let Some(use_item) = parse_use_line(&trimmed, range) {
                    uses.push(use_item);
                }
                self.index += 1;
            } else if looks_like_flow(&trimmed) {
                if let Some(flow) = self.parse_flow() {
                    items.push(Item::Flow(flow));
                }
            } else if looks_like_hook(&trimmed) {
                if let Some(hook) = self.parse_hook() {
                    items.push(Item::Hook(hook));
                }
            } else if looks_like_memo_fn(&trimmed) {
                if let Some(memo) = self.parse_memo_fn() {
                    items.push(Item::MemoFn(memo));
                }
            } else if looks_like_parser_item(&trimmed) {
                if let Some(parser) = self.parse_parser_item() {
                    items.push(Item::Parser(parser));
                }
            } else if let Some(flow_item) = self.parse_flow_item_until_indent(0) {
                items.push(Item::FlowItem(flow_item));
            } else {
                items.push(Item::Raw(RawItem::new(trimmed, None, range)));
                self.index += 1;
            }
        }

        if self.errors.is_empty() {
            Ok(SyntaxTree::new(
                source_take(self),
                module,
                uses,
                items,
                wiki_links,
            ))
        } else {
            Err(core::mem::take(&mut self.errors))
        }
    }

    fn parse_flow(&mut self) -> Option<Flow> {
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
        let (id, after_id) =
            parse_optional_entity_ref(after_flow, start_line.start, &mut self.errors);
        let (name, signature_tail) = parse_name_and_tail(after_id.trim());
        let contracts = header_lines
            .iter()
            .skip(1)
            .filter_map(|line| parse_contract_clause(line))
            .collect();
        let body_items = self.parse_flow_body(&body, start_line.start + head.len());

        Some(Flow::new(FlowInit {
            kind,
            visibility,
            id,
            name,
            signature_tail,
            contracts,
            body: body_items,
            range: TextRange::new(start_line.start, end),
        }))
    }

    fn take_flow_block(&mut self) -> (String, String, usize, bool) {
        let start = self.index;
        let mut header = String::new();
        let mut end = self.current().end;

        while self.index < self.lines.len() {
            let line = self.current();
            let trimmed = line.text.trim();
            let is_body_line = trimmed == "{"
                || (self.index == start
                    && trimmed.contains('{')
                    && !trimmed.starts_with("effects"));
            if is_body_line {
                break;
            }
            if !header.is_empty() {
                header.push('\n');
            }
            header.push_str(&line.text);
            end = line.end;
            self.index += 1;
        }
        if self.index >= self.lines.len() {
            return (header, String::new(), end, false);
        }
        let (body_head, body, end, ok) = self.take_brace_block();
        if !body_head.is_empty() {
            if !header.is_empty() {
                header.push('\n');
            }
            header.push_str(&body_head);
        }
        (header, body, end, ok)
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
        let first = header_lines.first()?;
        let (visibility, after_visibility) = parse_visibility_prefix(first);
        let after_hook = after_visibility
            .trim_start()
            .strip_prefix("hook")?
            .trim_start();
        let (id, _) = parse_required_entity_ref(after_hook, start_line.start, &mut self.errors)?;
        let target = header_lines
            .iter()
            .find_map(|line| line.strip_prefix("on ").map(str::trim))
            .unwrap_or_default()
            .to_owned();
        let phase = header_lines
            .iter()
            .find_map(|line| line.strip_prefix("phase ").map(str::trim))
            .unwrap_or_default()
            .to_owned();
        let check = header_lines
            .iter()
            .find_map(|line| line.strip_prefix("check ").map(str::trim))
            .map(str::to_owned);

        Some(HookItem::new(
            visibility,
            id,
            target,
            phase,
            check,
            body,
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
        let options = lines.map(str::to_owned).collect();
        Some(MemoFn::new(
            visibility,
            signature,
            options,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_parser_item(&mut self) -> Option<ParserItem> {
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
        Some(ParserItem::new(
            visibility,
            name.unwrap_or_default(),
            tail,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_flow_body(&mut self, body: &str, base_offset: usize) -> Vec<FlowItem> {
        let mut nested = Parser::new(body.to_owned());
        let mut items = Vec::new();
        while nested.index < nested.lines.len() {
            nested.skip_blank_and_comments();
            if nested.index >= nested.lines.len() {
                break;
            }
            if let Some(item) = nested.parse_flow_item_until_indent(0) {
                items.push(item);
            } else {
                let line = nested.current().text.trim().to_owned();
                items.push(FlowItem::Raw(line));
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

    fn parse_flow_item_until_indent(&mut self, min_indent: usize) -> Option<FlowItem> {
        self.skip_blank_and_comments();
        let line = self.current().clone();
        let indent = indentation(&line.text);
        if indent < min_indent {
            return None;
        }
        let trimmed = line.text.trim();

        if trimmed.starts_with("@choice") {
            return self.parse_choice().map(FlowItem::Choice);
        }
        if trimmed.starts_with("if ") {
            return self.parse_if_block().map(FlowItem::If);
        }
        if trimmed.starts_with("match ") {
            return self.parse_match_block().map(FlowItem::Match);
        }
        if let Some(command) = parse_scenario_command(trimmed, TextRange::new(line.start, line.end))
        {
            self.index += 1;
            return Some(FlowItem::ScenarioCommand(command));
        }
        if let Some(rest) = trimmed.strip_prefix("include ") {
            let entity = parse_required_entity_ref(rest.trim(), line.start, &mut self.errors)?.0;
            self.index += 1;
            return Some(FlowItem::Include(entity));
        }
        if trimmed.starts_with("await ") && trimmed.contains(" with ") {
            if trimmed.contains('{') {
                let (head, body, _, ok) = self.take_brace_block();
                if ok {
                    return Some(FlowItem::AwaitWith(parse_await_with(&format!(
                        "{head} {{ {body} }}"
                    ))));
                }
            } else {
                let await_with = parse_await_with(trimmed);
                self.index += 1;
                return Some(FlowItem::AwaitWith(await_with));
            }
        }
        if is_typed_stmt(trimmed) {
            self.index += 1;
            return Some(FlowItem::Stmt(parse_stmt(trimmed)));
        }
        if let Some(item) = self.parse_content_call_or_speaker_line() {
            return Some(item);
        }

        None
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
        let rest = head.trim().strip_prefix("@choice")?.trim();
        let (id, _) = parse_optional_entity_ref(rest, start_line.start, &mut self.errors);
        let options = body
            .lines()
            .filter_map(|line| parse_choice_option(line.trim(), start_line.start, &mut self.errors))
            .collect();
        Some(ChoiceBlock::new(
            id,
            options,
            TextRange::new(start_line.start, end),
        ))
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

    fn parse_content_call_or_speaker_line(&mut self) -> Option<FlowItem> {
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
                args,
                content,
                plan,
                TextRange::new(line.start, self.previous_end()),
            )));
        }

        if let Some((callee, args, content, consumed_end)) = self.try_take_content_call() {
            let plan = self.take_optional_line_plan();
            return Some(FlowItem::ContentCall(ContentCall::new(
                callee,
                args,
                content,
                plan,
                TextRange::new(line.start, consumed_end),
            )));
        }

        None
    }

    fn try_take_content_call(
        &mut self,
    ) -> Option<(String, Option<String>, DialogueContent, usize)> {
        let start = self.current().clone();
        let mut text = start.text.trim().to_owned();
        let mut end = start.end;
        let mut cursor = self.index;

        while bracket_delta(&text) > 0 && cursor + 1 < self.lines.len() {
            cursor += 1;
            text.push('\n');
            text.push_str(self.lines[cursor].text.trim_end());
            end = self.lines[cursor].end;
        }

        let open = find_content_bracket(&text)?;
        let Some(close) = find_matching_square(&text, open) else {
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
        if before.is_empty() || before.starts_with('@') {
            return None;
        }
        let (callee, args) = split_call_head(before);
        let raw_content = text[open + 1..close].trim().to_owned();
        self.index = cursor + 1;
        let content = DialogueContent::new(
            raw_content.clone(),
            parse_dialogue_tokens(&raw_content),
            TextRange::new(start.start + open + 1, start.start + close),
        );
        Some((callee, args, content, end))
    }

    fn take_optional_line_plan(&mut self) -> Option<LinePlan> {
        self.skip_blank_and_comments();
        if self.index >= self.lines.len() {
            return None;
        }
        let line = self.current().clone();
        let trimmed = line.text.trim();
        if trimmed == "with:" {
            self.index += 1;
            return Some(self.take_indented_line_plan(indentation(&line.text) + 1, line.start));
        }
        if trimmed.starts_with("with {") || trimmed == "with{" {
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
            return Some(parse_line_plan_body(
                BlockStyle::Brace,
                &body,
                TextRange::new(line.start, end),
            ));
        }
        None
    }

    fn take_indented_dialogue(&mut self, min_indent: usize, start: usize) -> DialogueContent {
        let mut raw = String::new();
        let mut end = start;
        while self.index < self.lines.len() {
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
        while self.index < self.lines.len() {
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
            end = line.end;
            self.index += 1;
        }
        parse_line_plan_body(BlockStyle::Indent, &raw, TextRange::new(start, end))
    }

    fn take_brace_block(&mut self) -> (String, String, usize, bool) {
        let start = self.index;
        let mut text = String::new();
        let mut end = self.current().end;
        let mut depth = 0_i32;
        let mut seen_open = false;

        while self.index < self.lines.len() {
            let line = self.current();
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&line.text);
            end = line.end;
            for ch in line.text.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        seen_open = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            self.index += 1;
            if seen_open && depth == 0 {
                break;
            }
        }

        let Some(open) = text.find('{') else {
            self.index = start + 1;
            return (text, String::new(), end, false);
        };
        let Some(close) = text.rfind('}') else {
            return (text, String::new(), end, false);
        };
        if depth != 0 {
            return (text, String::new(), end, false);
        }
        (
            text[..open].trim().to_owned(),
            text[open + 1..close].to_owned(),
            end,
            true,
        )
    }

    fn current(&self) -> &SourceLine {
        &self.lines[self.index]
    }

    fn previous_end(&self) -> usize {
        self.index
            .checked_sub(1)
            .and_then(|index| self.lines.get(index))
            .map_or(0, |line| line.end)
    }

    fn skip_blank_and_comments(&mut self) {
        while self.index < self.lines.len() {
            let trimmed = self.current().text.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///") {
                self.index += 1;
            } else {
                break;
            }
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

impl ParseError {
    fn new(
        range: TextRange,
        expected: Vec<String>,
        found: Option<String>,
        message: String,
        recovery: Vec<RecoverySuggestion>,
        anchor: SourceAnchor,
    ) -> Self {
        Self {
            range,
            expected,
            found,
            message,
            recovery,
            anchor,
        }
    }

    fn rebased(mut self, base_offset: usize) -> Self {
        self.range = TextRange::new(
            self.range.start() + base_offset,
            self.range.end() + base_offset,
        );
        self
    }

    /// Error byte range.
    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    /// Expected syntax fragments.
    pub fn expected(&self) -> &[String] {
        &self.expected
    }

    /// Found fragment, if known.
    pub fn found(&self) -> Option<&str> {
        self.found.as_deref()
    }

    /// Human-readable diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Recovery suggestions.
    pub fn recovery(&self) -> &[RecoverySuggestion] {
        &self.recovery
    }

    /// Source anchor for tooling integrations.
    pub const fn anchor(&self) -> &SourceAnchor {
        &self.anchor
    }
}

impl RecoverySuggestion {
    /// Recovery message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

fn source_take(parser: &mut Parser) -> String {
    core::mem::take(&mut parser.source)
}

fn split_lines(source: &str) -> Vec<SourceLine> {
    let mut lines = Vec::new();
    let mut start = 0;
    for segment in source.split_inclusive('\n') {
        let end = start + segment.len();
        lines.push(SourceLine {
            text: segment.trim_end_matches(['\r', '\n']).to_owned(),
            start,
            end,
        });
        start = end;
    }
    if !source.ends_with('\n') && lines.is_empty() {
        lines.push(SourceLine {
            text: source.to_owned(),
            start: 0,
            end: source.len(),
        });
    }
    lines
}

fn collect_wiki_links(source: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(start_relative) = source[cursor..].find("[[") {
        let start = cursor + start_relative;
        let body_start = start + 2;
        let Some(end_relative) = source[body_start..].find("]]") else {
            break;
        };
        let end = body_start + end_relative;
        links.push(WikiLink::new(
            source[body_start..end].to_owned(),
            TextRange::new(start, end + 2),
        ));
        cursor = end + 2;
    }
    links
}

fn is_use_line(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    rest.starts_with("use ") || rest.starts_with("lazy use ") || rest.starts_with("eager use ")
}

fn parse_use_line(trimmed: &str, range: TextRange) -> Option<UseItem> {
    let (visibility, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    let (mode, tree) = if let Some(tree) = rest.strip_prefix("lazy use ") {
        (Some(UseMode::Lazy), tree)
    } else if let Some(tree) = rest.strip_prefix("eager use ") {
        (Some(UseMode::Eager), tree)
    } else {
        (None, rest.strip_prefix("use ")?)
    };
    Some(UseItem::new(
        visibility,
        mode,
        tree.trim().to_owned(),
        range,
    ))
}

fn looks_like_flow(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    rest.starts_with("flow ") || rest.starts_with("fragment ")
}

fn parse_flow_kind(input: &str) -> Option<(FlowKind, &str)> {
    if let Some(rest) = input.strip_prefix("flow") {
        return Some((FlowKind::Flow, rest.trim_start()));
    }
    input
        .strip_prefix("fragment")
        .map(|rest| (FlowKind::Fragment, rest.trim_start()))
}

fn looks_like_hook(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("hook ")
}

fn looks_like_memo_fn(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("memo fn ")
}

fn looks_like_parser_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("parser ")
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
    if input.trim_start().starts_with('#') {
        match parse_required_entity_ref(input.trim_start(), base, errors) {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else {
        (None, input)
    }
}

fn parse_required_entity_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix("#<") {
        let Some(end) = rest.find('>') else {
            errors.push(simple_error(
                base,
                input.len(),
                "unclosed delimited entity reference",
                "#<...>",
            ));
            return None;
        };
        let body = &rest[..end];
        if body.trim().is_empty() {
            errors.push(simple_error(
                base,
                input.len(),
                "empty entity reference",
                "#foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(body.to_owned(), true, TextRange::new(base, base + end + 3)),
            &rest[end + 1..],
        ));
    }
    if let Some(rest) = input.strip_prefix('#') {
        let len = rest
            .char_indices()
            .take_while(|(_, ch)| ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':' | '/'))
            .map(|(index, ch)| index + ch.len_utf8())
            .last()
            .unwrap_or(0);
        if len == 0 {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid entity reference",
                "#foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(
                rest[..len].to_owned(),
                false,
                TextRange::new(base, base + len + 1),
            ),
            &rest[len..],
        ));
    }
    None
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
    let name_len = trimmed
        .char_indices()
        .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if name_len == 0 {
        (None, trimmed.to_owned())
    } else {
        (
            Some(trimmed[..name_len].to_owned()),
            trimmed[name_len..].trim().to_owned(),
        )
    }
}

fn parse_scenario_command(trimmed: &str, range: TextRange) -> Option<ScenarioCommand> {
    let rest = trimmed.strip_prefix('@')?;
    if rest.starts_with("choice") {
        return None;
    }
    let (name, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
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

fn split_scenario_args(source: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            ch if ch.is_whitespace() && depth == 0 && !in_string => {
                let arg = source[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        args.push(tail);
    }
    args
}

fn parse_attribute(trimmed: &str, range: TextRange) -> Option<Attribute> {
    let rest = trimmed.strip_prefix('@')?;
    if rest.starts_with("choice") {
        return None;
    }
    let open = rest.find('(')?;
    if !rest.ends_with(')') {
        return None;
    }
    let name = rest[..open].trim().to_owned();
    let args = rest[open + 1..rest.len() - 1].trim();
    Some(Attribute::new(
        name,
        (!args.is_empty()).then(|| args.to_owned()),
        range,
    ))
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
        return Some(ContractClause::Ensures {
            mode,
            expr: parse_expr_lossy(expr),
        });
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

fn parse_choice_option(
    trimmed: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceOption> {
    if trimmed.is_empty() {
        return None;
    }
    let (id, rest) = parse_optional_entity_ref(trimmed, base, errors);
    let rest = rest.trim();
    let quote_start = rest.find('"')?;
    let quote_end = rest[quote_start + 1..].find('"')? + quote_start + 1;
    let label = rest[quote_start + 1..quote_end].to_owned();
    let after_label = rest[quote_end + 1..].trim();
    let (condition, target_part) = if let Some(condition_body) = after_label.strip_prefix("if ") {
        let (condition, target) = condition_body.split_once("->")?;
        (
            Some(
                parse_expr(condition.trim())
                    .unwrap_or_else(|_| crate::expr::Expr::Raw(condition.trim().to_owned())),
            ),
            target.trim(),
        )
    } else {
        (None, after_label.strip_prefix("->")?.trim())
    };
    let target = parse_required_entity_ref(target_part, base, errors)?.0;
    Some(ChoiceOption::new(
        id,
        label,
        condition,
        target,
        TextRange::new(base, base + trimmed.len()),
    ))
}

fn parse_match_arms(body: &str, base: usize, errors: &mut Vec<ParseError>) -> Vec<MatchArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (pattern, item) = line.split_once("=>")?;
            let mut nested = Parser::new(item.trim().to_owned());
            let parsed = nested.parse_flow_item_until_indent(0).map_or_else(
                || vec![FlowItem::Stmt(parse_stmt(item.trim()))],
                |item| vec![item],
            );
            errors.extend(nested.errors.into_iter().map(|err| err.rebased(base)));
            Some(MatchArm::new(parse_pattern(pattern.trim()), parsed))
        })
        .collect()
}

fn split_speaker_line(trimmed: &str) -> Option<(String, Option<String>, &str)> {
    let colon = find_top_level_colon(trimmed)?;
    if trimmed[..colon].contains('[') || trimmed[..colon].contains("->") {
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

fn find_top_level_colon(input: &str) -> Option<usize> {
    let mut parens = 0_i32;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' => parens += 1,
            ')' => parens -= 1,
            ':' if parens == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn split_call_head(head: &str) -> (String, Option<String>) {
    let head = head.trim();
    if let Some(open) = head.find('(') {
        if head.ends_with(')') {
            return (
                head[..open].trim().to_owned(),
                Some(head[open + 1..head.len() - 1].trim().to_owned()),
            );
        }
    }
    (head.to_owned(), None)
}

fn bracket_delta(text: &str) -> i32 {
    text.chars().fold(0, |depth, ch| match ch {
        '[' => depth + 1,
        ']' => depth - 1,
        _ => depth,
    })
}

fn find_content_bracket(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(index, ch)| *ch == '[' && !text[..*index].trim_end().ends_with('#'))
        .map(|(index, _)| index)
}

fn find_matching_square(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0_i32;
    for (relative, ch) in text[open..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + relative);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_line_plan_body(style: BlockStyle, body: &str, range: TextRange) -> LinePlan {
    let lines = body.lines().collect::<Vec<_>>();
    let mut items = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if is_multiline_timed_cue_header(trimmed) {
            let cue_indent = indentation(line);
            let mut body_lines = Vec::new();
            index += 1;
            while index < lines.len() {
                let child = lines[index];
                let child_trimmed = child.trim();
                if !child_trimmed.is_empty() && indentation(child) <= cue_indent {
                    break;
                }
                if !child_trimmed.is_empty() {
                    body_lines.push(child_trimmed);
                }
                index += 1;
            }
            let body = body_lines.join(" ");
            items.push(parse_line_plan_item(&format!("{trimmed} {body}")));
            continue;
        }
        items.push(parse_line_plan_item(trimmed));
        index += 1;
    }
    LinePlan::new(style, items, range)
}

fn is_multiline_timed_cue_header(line: &str) -> bool {
    line.starts_with("at(") && line.ends_with(':')
}

fn parse_line_plan_item(line: &str) -> LinePlanItem {
    if let Some(rest) = line.strip_prefix("return ") {
        return LinePlanItem::Return(parse_expr_lossy(rest.trim()));
    }
    if let Some(rest) = line.strip_prefix("let ") {
        if let Some((pattern, expr)) = rest.split_once('=') {
            return LinePlanItem::Let {
                pattern: parse_pattern(pattern.trim()),
                expr: parse_expr_lossy(expr.trim()),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("cancel on ") {
        let (trigger, action) = rest
            .split_once("=>")
            .or_else(|| rest.split_once(':'))
            .unwrap_or((rest, ""));
        return LinePlanItem::CancelRule(CancelRuleSyntax::new(
            trigger.trim().to_owned(),
            action.trim().to_owned(),
        ));
    }
    if let Some(rest) = line.strip_prefix("at(") {
        if let Some((anchor, body)) = rest.split_once(')') {
            return LinePlanItem::TimedCue {
                anchor: parse_expr_lossy(anchor.trim()),
                body: parse_expr_lossy(normalize_timed_cue_body(body)),
            };
        }
    }
    if let Some(rest) = line.strip_prefix("start ") {
        return LinePlanItem::StartGroup(rest.trim().to_owned());
    }
    if let Some(rest) = line.strip_prefix("together ") {
        return LinePlanItem::TogetherGroup(rest.trim().to_owned());
    }
    if let Some(rest) = line.strip_prefix("memo ") {
        return LinePlanItem::Memo(rest.trim().to_owned());
    }
    if line.starts_with("assert") || line.starts_with("debug_assert") {
        return LinePlanItem::Assert(line.to_owned());
    }
    if let Some((name, value)) = line.split_once('=') {
        return LinePlanItem::Option {
            name: name.trim().to_owned(),
            value: parse_expr_lossy(value.trim()),
        };
    }
    LinePlanItem::Raw(line.to_owned())
}

fn parse_expr_lossy(source: &str) -> crate::expr::Expr {
    parse_expr(source).unwrap_or_else(|_| crate::expr::Expr::Raw(source.to_owned()))
}

fn normalize_timed_cue_body(source: &str) -> &str {
    let body = source
        .trim_start_matches([':', ' ', '{'])
        .trim_end_matches('}')
        .trim();
    body.strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(body)
        .trim()
}

fn parse_await_with(trimmed: &str) -> AwaitWith {
    let without_await = trimmed.trim_start_matches("await").trim();
    let (expr_part, branch_part) = without_await
        .split_once(" with ")
        .unwrap_or((without_await, ""));
    let propagates_error = expr_part.ends_with('?');
    let expr_text = expr_part.trim_end_matches('?').trim();
    AwaitWith::new(
        parse_expr_lossy(expr_text),
        propagates_error,
        parse_await_branches(branch_part.trim()),
    )
}

fn parse_await_branches(source: &str) -> Vec<AwaitBranch> {
    let body = source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source)
        .trim();
    split_await_branch_lines(body)
        .into_iter()
        .filter_map(parse_await_branch)
        .collect()
}

fn split_await_branch_lines(source: &str) -> Vec<&str> {
    if source.lines().count() > 1 {
        return source
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
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
    let (head, body) = line.split_once("=>")?;
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
        vec![parse_inline_await_branch_item(body.trim())],
    ))
}

fn parse_inline_await_branch_item(body: &str) -> FlowItem {
    if let Some(command) = parse_scene_command(body) {
        return FlowItem::ScenarioCommand(command);
    }
    let mut nested = Parser::new(body.to_owned());
    nested
        .parse_flow_item_until_indent(0)
        .unwrap_or_else(|| FlowItem::Stmt(parse_stmt(body)))
}

fn parse_scene_command(body: &str) -> Option<ScenarioCommand> {
    let rest = body.strip_prefix("scene ")?;
    let args = rest.split_once('{').map_or(rest, |(head, _)| head).trim();
    Some(ScenarioCommand::new(
        "scene".to_owned(),
        parse_scenario_args(args),
        TextRange::new(0, body.len()),
    ))
}

fn is_typed_stmt(trimmed: &str) -> bool {
    matches!(
        trimmed.split_whitespace().next(),
        Some("let" | "match" | "if" | "return" | "goto" | "spawn" | "defer")
    )
}

fn parse_stmt(trimmed: &str) -> Stmt {
    if let Some(rest) = trimmed.strip_prefix("let ") {
        if let Some((pattern, expr)) = rest.split_once('=') {
            return Stmt::Let {
                pattern: parse_pattern(pattern.trim()),
                expr: parse_expr_lossy(expr.trim()),
            };
        }
        return Stmt::Raw(trimmed.to_owned());
    }
    if let Some(expr) = trimmed.strip_prefix("return ") {
        return Stmt::Return(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("goto ") {
        return Stmt::Goto(parse_expr_lossy(expr.trim()));
    }
    if matches!(
        trimmed.split_whitespace().next(),
        Some("match" | "if" | "spawn" | "defer")
    ) {
        return Stmt::Raw(trimmed.to_owned());
    }
    Stmt::Expr(parse_expr_lossy(trimmed))
}

fn parse_pattern(source: &str) -> Pattern {
    let source = source.trim();
    if source == "_" {
        return Pattern::Discard;
    }
    if let Some((name, ty)) = source.split_once(':') {
        let name = name.trim();
        if is_pattern_ident(name) {
            if let Ok(ty) = parse_type_ref(ty.trim()) {
                return Pattern::Typed {
                    name: name.to_owned(),
                    ty,
                };
            }
        }
    }
    if let Some(inner) = source
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    {
        return Pattern::Tuple(
            split_pattern_items(inner)
                .into_iter()
                .map(parse_pattern)
                .collect(),
        );
    }
    if is_pattern_ident(source) {
        return Pattern::Ident(source.to_owned());
    }
    Pattern::Raw(source.to_owned())
}

fn is_pattern_ident(source: &str) -> bool {
    source
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_'))
        && source
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
}

fn split_pattern_items(source: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                items.push(source[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = source[start..].trim();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn indentation(text: &str) -> usize {
    text.chars().take_while(|ch| ch.is_whitespace()).count()
}
