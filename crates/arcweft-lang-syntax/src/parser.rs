use crate::ast::{
    Attribute, AwaitBranch, AwaitBranchKind, AwaitWith, BlockStyle, BorrowBlock, CallableItem,
    CallableKind, CancelRuleSyntax, ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceOption,
    ChoicePlan, ChoicePlanItem, ChoiceUiField, ContentCall, ContractClause, DialogueContent,
    EntityRef, EnumItem, EnumVariant, Flow, FlowInit, FlowItem, FlowKind, ForBlock, FunctionInit,
    FunctionItem, HookItem, IfBlock, IfLetBlock, ImplItem, Item, LineOptions, LinePlan,
    LinePlanItem, LoopBlock, MatchArm, MatchBlock, MemoFn, ModuleDecl, ParserItem, Pattern,
    RawItem, RecordPatternField, ScenarioCommand, ScopeBlock, ScopeExprBlock, SelectBlock,
    SelectBranch, SelectBranchHead, SourceLocaleBlock, SpeakerLine, StateField, StateItem, Stmt,
    StructField, StructItem, SyntaxTree, TextRange, TraitItem, TraitMember, TypeAliasItem, UseItem,
    UseMode, Visibility, WhileBlock, WhileLetBlock, WikiLink,
};
use crate::expr::parse_expr;
use crate::text::parse_dialogue_tokens;
use crate::types::{parse_fn_signature, parse_type_ref};
use arcweft_source::{SourceAnchor, SourceName};
use core::fmt;

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
#[derive(Clone, Debug, Eq, PartialEq)]
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

            if let Some(item) = self.reject_old_memo_attribute(&trimmed, range) {
                items.push(item);
            } else if let Some(attribute) = parse_attribute(&trimmed, range) {
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
            } else if looks_like_function_item(&trimmed) {
                if let Some(function) = self.parse_function_item() {
                    items.push(Item::Function(function));
                }
            } else if looks_like_callable_item(&trimmed) {
                if let Some(callable) = self.parse_callable_item() {
                    items.push(Item::Callable(callable));
                }
            } else if looks_like_state_item(&trimmed) {
                if let Some(state) = self.parse_state_item() {
                    items.push(Item::State(state));
                }
            } else if looks_like_trait_item(&trimmed) {
                if let Some(trait_item) = self.parse_trait_item() {
                    items.push(Item::Trait(trait_item));
                }
            } else if looks_like_impl_item(&trimmed) {
                if let Some(impl_item) = self.parse_impl_item() {
                    items.push(Item::Impl(impl_item));
                }
            } else if looks_like_enum_item(&trimmed) {
                if let Some(enum_item) = self.parse_enum_item() {
                    items.push(Item::Enum(enum_item));
                }
            } else if looks_like_struct_item(&trimmed) {
                if let Some(struct_item) = self.parse_struct_item() {
                    items.push(Item::Struct(struct_item));
                }
            } else if looks_like_type_alias(&trimmed) {
                if let Some(type_alias) = self.parse_type_alias() {
                    items.push(Item::TypeAlias(type_alias));
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

    fn reject_old_memo_attribute(&mut self, trimmed: &str, range: TextRange) -> Option<Item> {
        if !trimmed.starts_with("@memo") {
            return None;
        }
        self.push_error(
            range,
            "`@memo`-style memo attributes are not valid Arcweft syntax",
            [
                "memo fn name(...) -> Type",
                "memo(scope=..., key=...) { ... }",
            ],
            Some(trimmed),
            ["remove `@` and use a `memo fn` item or `memo(...) { ... }` block"],
        );
        self.index += 1;
        Some(Item::Raw(RawItem::new(trimmed.to_owned(), None, range)))
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

    fn parse_function_item(&mut self) -> Option<FunctionItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing function",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the function body"],
            );
            return None;
        }

        let header_lines = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let first = header_lines.first().copied()?;
        let (visibility, signature_text) = parse_visibility_prefix(first);
        let signature_text = signature_text.trim().to_owned();
        let Ok(signature) = parse_fn_signature(&signature_text) else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "invalid function signature",
                ["fn name<'a>(...)"],
                Some(first),
                ["write the function item with a valid `fn` signature head"],
            );
            return None;
        };
        let contracts = header_lines
            .iter()
            .skip(1)
            .filter_map(|line| parse_contract_clause(line))
            .collect();
        let (body_statements, body_value) = parse_scope_expr_body(&body);

        Some(FunctionItem::new(FunctionInit {
            visibility,
            signature,
            signature_text,
            contracts,
            body,
            body_statements,
            body_value,
            range: TextRange::new(start_line.start, end),
        }))
    }

    fn parse_enum_item(&mut self) -> Option<EnumItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing enum",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the enum body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let name = rest.trim_start().strip_prefix("enum")?.trim();
        let (name, _) = parse_name_and_tail(name);
        Some(EnumItem::new(
            visibility,
            name.unwrap_or_default(),
            parse_enum_variants(&body),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_callable_item(&mut self) -> Option<CallableItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing function-like item",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the item body"],
            );
            return None;
        }
        let header_lines = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let first = header_lines.first().copied()?;
        let (visibility, rest) = parse_visibility_prefix(first);
        let (kind, after_kind) = parse_callable_kind(rest.trim_start())?;
        let (name, signature_tail) = parse_name_and_tail(after_kind);
        let contracts = header_lines
            .iter()
            .skip(1)
            .filter_map(|line| parse_contract_clause(line))
            .collect();

        Some(CallableItem::new(
            kind,
            visibility,
            name.unwrap_or_default(),
            signature_tail,
            contracts,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_state_item(&mut self) -> Option<StateItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing state",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the state body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let name = rest.trim_start().strip_prefix("state")?.trim();
        let (name, _) = parse_name_and_tail(name);
        Some(StateItem::new(
            visibility,
            name.unwrap_or_default(),
            parse_state_fields(&body),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_trait_item(&mut self) -> Option<TraitItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing trait",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the trait body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let rest = rest.trim_start().strip_prefix("trait")?.trim();
        let (name, supertraits) = rest
            .split_once(':')
            .map_or((rest, ""), |(name, traits)| (name.trim(), traits.trim()));
        Some(TraitItem::new(
            visibility,
            name.to_owned(),
            split_supertraits(supertraits),
            parse_trait_members(&body),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_impl_item(&mut self) -> Option<ImplItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing impl",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the impl body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let rest = rest.trim_start().strip_prefix("impl")?.trim();
        let (generics, rest) = parse_optional_angle_head(rest);
        let (trait_name, target) = rest
            .split_once(" for ")
            .map_or((None, rest.trim()), |(trait_name, target)| {
                (Some(trait_name.trim().to_owned()), target.trim())
            });
        Some(ImplItem::new(
            visibility,
            generics,
            trait_name,
            target.to_owned(),
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_struct_item(&mut self) -> Option<StructItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing struct",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the struct body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let name = rest.trim_start().strip_prefix("struct")?.trim();
        let (name, _) = parse_name_and_tail(name);
        Some(StructItem::new(
            visibility,
            name.unwrap_or_default(),
            parse_struct_fields(&body),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_type_alias(&mut self) -> Option<TypeAliasItem> {
        let start_line = self.current().clone();
        let mut raw = start_line.text.clone();
        let mut end = start_line.end;
        self.index += 1;
        while self.index < self.lines.len() {
            let line = self.current();
            let trimmed = line.text.trim();
            if !trimmed.starts_with("where ") {
                break;
            }
            raw.push('\n');
            raw.push_str(&line.text);
            end = line.end;
            self.index += 1;
        }

        let mut lines = raw.lines().map(str::trim).filter(|line| !line.is_empty());
        let first = lines.next()?;
        let (visibility, rest) = parse_visibility_prefix(first);
        let rest = rest.trim_start().strip_prefix("type")?.trim();
        let (name, target) = rest.split_once('=')?;
        let target = parse_type_ref(target.trim()).ok()?;
        let where_clauses = lines
            .filter_map(|line| line.strip_prefix("where "))
            .map(str::trim)
            .map(parse_expr_lossy)
            .collect();

        Some(TypeAliasItem::new(
            visibility,
            name.trim().to_owned(),
            target,
            where_clauses,
            TextRange::new(start_line.start, end),
        ))
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
        self.reject_old_hook_header_syntax(&header_lines, start_line.start);
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
        Some(MemoFn::new(
            visibility,
            signature,
            options,
            body,
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
            return Some(FlowItem::Raw(self.reject_old_choice_syntax()));
        }
        if let Some(item) = self.parse_structured_flow_block(trimmed) {
            return Some(item);
        }
        if let Some(command) = parse_scenario_command(trimmed, TextRange::new(line.start, line.end))
        {
            self.index += 1;
            return Some(FlowItem::ScenarioCommand(command));
        }
        if let Some(command) =
            parse_word_scenario_command(trimmed, TextRange::new(line.start, line.end))
        {
            self.index += 1;
            return Some(FlowItem::ScenarioCommand(command));
        }
        if let Some(rest) = trimmed.strip_prefix("include ") {
            let entity = parse_required_entity_ref(rest.trim(), line.start, &mut self.errors)?.0;
            self.index += 1;
            return Some(FlowItem::Include(entity));
        }
        if is_await_with_head(trimmed) {
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
        }
        // `choice` owns a brace block, so parse `let x = choice ... { ... }`
        // before generic `let` handling can collapse it into a raw expression.
        if is_let_choice_head(trimmed) {
            return self.parse_let_choice().map(FlowItem::Stmt);
        }
        if is_let_scope_head(trimmed) {
            return self.parse_let_scope().map(FlowItem::Stmt);
        }
        if is_let_block_head(trimmed) {
            return self.parse_let_block().map(FlowItem::Stmt);
        }
        if is_let_loop_head(trimmed) {
            return self.parse_let_loop().map(FlowItem::Stmt);
        }
        if is_let_if_let_head(trimmed) {
            return self.parse_let_if_let().map(FlowItem::Stmt);
        }
        if is_let_if_head(trimmed) {
            return self.parse_let_if().map(FlowItem::Stmt);
        }
        if is_let_match_head(trimmed) {
            return self.parse_let_match().map(FlowItem::Stmt);
        }
        if is_let_else_head(trimmed) {
            return self.parse_let_else().map(FlowItem::Stmt);
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

    fn parse_structured_flow_block(&mut self, trimmed: &str) -> Option<FlowItem> {
        if trimmed.starts_with("choice ") {
            return self.parse_choice().map(FlowItem::Choice);
        }
        if trimmed.starts_with("if let ") {
            return self.parse_if_let_block().map(FlowItem::IfLet);
        }
        if trimmed.starts_with("if ") {
            return self.parse_if_block().map(FlowItem::If);
        }
        if trimmed.starts_with("match ") {
            return self.parse_match_block().map(FlowItem::Match);
        }
        if trimmed == "loop" || trimmed.starts_with("loop ") {
            return self.parse_loop_block().map(FlowItem::Loop);
        }
        if trimmed.starts_with("while let ") {
            return self.parse_while_let_block().map(FlowItem::WhileLet);
        }
        if trimmed.starts_with("while ") {
            return self.parse_while_block().map(FlowItem::While);
        }
        if trimmed.starts_with("for ") {
            return self.parse_for_block().map(FlowItem::For);
        }
        if trimmed.starts_with("select") {
            return self.parse_select_block().map(FlowItem::Select);
        }
        if trimmed.starts_with("borrow ") {
            return self.parse_borrow_block().map(FlowItem::BorrowBlock);
        }
        if trimmed.starts_with("source locale ") {
            return self.parse_source_locale_block().map(FlowItem::SourceLocale);
        }
        if trimmed.starts_with("scope ") {
            return self.parse_scope_block().map(FlowItem::Scope);
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
        let (pattern, choice_head) = rest.split_once('=')?;
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
        let (pattern, scope_head) = rest.split_once('=')?;
        let name = scope_head.trim().strip_prefix("scope")?.trim();
        let (statements, value) = parse_scope_expr_body(&body);

        Some(Stmt::LetScope {
            pattern: parse_pattern(pattern.trim()),
            scope: ScopeExprBlock::new(
                name.to_owned(),
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
        let (pattern, block_head) = rest.split_once('=')?;
        if !block_head.trim().is_empty() {
            return None;
        }

        Some(Stmt::Let {
            pattern: parse_pattern(pattern.trim()),
            expr: parse_block_expr(&body),
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
        let (pattern, loop_head) = rest.split_once('=')?;
        if loop_head.trim() != "loop" {
            return None;
        }

        Some(Stmt::LetLoop {
            pattern: parse_pattern(pattern.trim()),
            block: LoopBlock::new(
                self.parse_flow_body(&body, start_line.start + head.len()),
                TextRange::new(start_line.start, end),
            ),
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
        let (pattern, if_head) = rest.split_once('=')?;
        let condition = if_head.trim().strip_prefix("if")?.trim();

        Some(Stmt::Let {
            pattern: parse_pattern(pattern.trim()),
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
        let (target_pattern, if_head) = rest.split_once('=')?;
        let if_let_head = if_head.trim().strip_prefix("if let")?.trim();
        let (binding_pattern, value_and_guard) = if_let_head.split_once('=')?;
        let (value, guard) = split_if_let_guard(value_and_guard);

        Some(Stmt::Let {
            pattern: parse_pattern(target_pattern.trim()),
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
        let (pattern, match_head) = rest.split_once('=')?;
        let scrutinee = match_head.trim().strip_prefix("match")?.trim();

        Some(Stmt::Let {
            pattern: parse_pattern(pattern.trim()),
            expr: crate::expr::Expr::Match {
                scrutinee: Box::new(parse_expr_lossy(scrutinee)),
                arms: parse_match_expr_arms(&body),
            },
        })
    }

    fn take_optional_else_block(&mut self, base: usize) -> Option<String> {
        self.skip_blank_and_comments();
        if self.index >= self.lines.len() {
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
        let (pattern, rhs) = rest.split_once('=')?;
        let expr = rhs.trim().strip_suffix("else")?.trim();
        Some(Stmt::LetElse {
            pattern: parse_pattern(pattern.trim()),
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
        let name = head.trim().strip_prefix("scope")?.trim().to_owned();
        let body = self.parse_flow_body(&body, start_line.start + head.len());
        Some(ScopeBlock::new(
            name,
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    fn take_choice_plan_after_current(&mut self, base: usize) -> Option<ChoicePlan> {
        self.skip_blank_and_comments();
        if self.index >= self.lines.len() {
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

    fn reject_old_choice_syntax(&mut self) -> String {
        let start_line = self.current().clone();
        let raw = if start_line.text.contains('{') {
            let (head, body, _, _) = self.take_brace_block();
            if body.is_empty() {
                head
            } else {
                format!("{head} {{ ... }}")
            }
        } else {
            self.index += 1;
            start_line.text.trim().to_owned()
        };
        self.push_error(
            TextRange::new(start_line.start, start_line.end),
            "`@choice` is not valid Arcweft syntax",
            ["choice #choice.id { ... }"],
            Some(start_line.text.trim()),
            ["remove `@` and write `choice #choice.id { ... }`"],
        );
        raw
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
        let (pattern, expr_and_guard) = rest.split_once('=')?;
        let (expr, guard) = expr_and_guard
            .split_once(" when ")
            .map_or((expr_and_guard.trim(), None), |(expr, guard)| {
                (expr.trim(), Some(parse_expr_lossy(guard.trim())))
            });
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
        let Some((source, binding)) = rest.split_once(" as ") else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "borrow block must bind a typed alias",
                ["borrow expr as name: Type { ... }"],
                Some(head.trim()),
                ["write the borrow block as `borrow source as name: Type { ... }`"],
            );
            return None;
        };
        let Some((name, ty)) = binding.split_once(':') else {
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
        if head.trim() != "loop" {
            return None;
        }
        Some(LoopBlock::new(
            self.parse_flow_body(&body, start_line.start + head.len()),
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
        let (pattern, source) = rest.split_once(" in ")?;
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
        let (pattern, expr_and_guard) = rest.split_once('=')?;
        let (expr, guard) = expr_and_guard
            .split_once(" when ")
            .map_or((expr_and_guard.trim(), None), |(expr, guard)| {
                (expr.trim(), Some(parse_expr_lossy(guard.trim())))
            });
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
                args.clone(),
                parse_line_options(args.as_deref(), line.start, &mut self.errors),
                content,
                plan,
                TextRange::new(line.start, self.previous_end()),
            )));
        }

        if let Some((callee, args, content, consumed_end)) = self.try_take_content_call() {
            let plan = self.take_optional_line_plan();
            return Some(FlowItem::ContentCall(ContentCall::new(
                callee,
                args.clone(),
                parse_line_options(args.as_deref(), line.start, &mut self.errors),
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

    fn take_indented_await_body(&mut self, min_indent: usize) -> String {
        let mut raw = String::new();
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
            self.index += 1;
        }
        raw
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

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

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

fn looks_like_function_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("fn ")
}

fn looks_like_callable_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    let rest = rest.trim_start();
    rest.starts_with("reducer ") || rest.starts_with("view ")
}

fn looks_like_state_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("state ")
}

fn looks_like_trait_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("trait ")
}

fn looks_like_impl_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("impl")
}

fn looks_like_enum_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("enum ")
}

fn looks_like_struct_item(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("struct ")
}

fn looks_like_type_alias(trimmed: &str) -> bool {
    let (_, rest) = parse_visibility_prefix(trimmed);
    rest.trim_start().starts_with("type ")
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

fn parse_optional_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<EntityRef>, &'a str) {
    let trimmed = input.trim_start();
    if trimmed.starts_with('#') {
        parse_optional_entity_ref(trimmed, base, errors)
    } else if trimmed.starts_with('.') {
        match parse_required_id_ref(trimmed, base, errors) {
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

fn parse_required_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if input.starts_with('#') {
        return parse_required_entity_ref(input, base, errors);
    }
    let Some(rest) = input.strip_prefix('.') else {
        errors.push(simple_error(
            base,
            input.len(),
            "expected entity reference or relative id",
            "#domain.path",
        ));
        return None;
    };
    let len = rest
        .char_indices()
        .take_while(|(_, ch)| ch.is_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if len == 0 {
        errors.push(simple_error(
            base,
            input.len(),
            "relative id is missing a suffix",
            ".suffix",
        ));
        return None;
    }
    Some((
        EntityRef::new_relative(rest[..len].to_owned(), TextRange::new(base, base + len + 1)),
        &rest[len..],
    ))
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

fn parse_word_scenario_command(trimmed: &str, range: TextRange) -> Option<ScenarioCommand> {
    let (name, args) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    if !matches!(
        name,
        "option" | "log" | "scene" | "text" | "progress" | "meter"
    ) {
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

fn parse_line_options(
    args: Option<&str>,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> LineOptions {
    let Some(args) = args else {
        return LineOptions::default();
    };
    let mut id = None;
    let mut text_key = None;
    let mut source_locale = None;
    for arg in split_comma_args(args) {
        let Some((name, value)) = split_top_level_equals(arg) else {
            continue;
        };
        match name.trim() {
            "id" => {
                id = parse_required_id_ref(value.trim(), base, errors).map(|(entity, _)| entity);
            }
            "text_key" => {
                text_key =
                    parse_required_id_ref(value.trim(), base, errors).map(|(entity, _)| entity);
            }
            "source_locale" => {
                source_locale = Some(value.trim().to_owned());
            }
            _ => {}
        }
    }
    LineOptions::new(id, text_key, source_locale)
}

fn split_comma_args(source: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            ',' if depth == 0 && !in_string => {
                let arg = source[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg);
                }
                start = index + 1;
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

fn split_top_level_equals(source: &str) -> Option<(&str, &str)> {
    let mut depth = 0_i32;
    let mut in_string = false;
    for (index, ch) in source.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '(' | '[' | '{' if !in_string => depth += 1,
            ')' | ']' | '}' if !in_string => depth -= 1,
            '=' if depth == 0 && !in_string => {
                return Some((source[..index].trim(), source[index + 1..].trim()));
            }
            _ => {}
        }
    }
    None
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

fn parse_enum_variants(body: &str) -> Vec<EnumVariant> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_end_matches(',').trim())
        .filter_map(|line| {
            let name_len = line
                .char_indices()
                .take_while(|(_, ch)| ch.is_alphanumeric() || *ch == '_')
                .map(|(index, ch)| index + ch.len_utf8())
                .last()?;
            let name = line[..name_len].to_owned();
            let payload = line[name_len..].trim();
            Some(EnumVariant::new(
                name,
                (!payload.is_empty()).then(|| payload.to_owned()),
            ))
        })
        .collect()
}

fn parse_struct_fields(body: &str) -> Vec<StructField> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_end_matches(',').trim())
        .filter_map(|line| {
            let (name, ty) = line.split_once(':')?;
            parse_type_ref(ty.trim())
                .ok()
                .map(|ty| StructField::new(name.trim().to_owned(), ty))
        })
        .collect()
}

fn parse_state_fields(body: &str) -> Vec<StateField> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_end_matches(',').trim())
        .filter_map(|line| {
            let (visibility, rest) = parse_visibility_prefix(line);
            let (left, default) = rest.split_once('=')?;
            let (name, ty) = left.split_once(':')?;
            parse_type_ref(ty.trim()).ok().map(|ty| {
                StateField::new(
                    visibility,
                    name.trim().to_owned(),
                    ty,
                    parse_expr_lossy(default.trim()),
                )
            })
        })
        .collect()
}

fn split_supertraits(source: &str) -> Vec<String> {
    source
        .split('+')
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
    let mut depth = 0_i32;
    for (index, ch) in source.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return (
                        Some(source[..=index].to_owned()),
                        source[index + ch.len_utf8()..].trim_start(),
                    );
                }
            }
            _ => {}
        }
    }
    (None, source)
}

fn parse_trait_members(body: &str) -> Vec<TraitMember> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_trait_member)
        .collect()
}

fn parse_trait_member(line: &str) -> TraitMember {
    let line = line.trim_end_matches(';').trim();
    if let Some(rest) = line.strip_prefix("type ") {
        let (name, value) = rest.split_once('=').map_or((rest, None), |(name, value)| {
            (name.trim(), parse_type_ref(value.trim()).ok())
        });
        return TraitMember::AssociatedType {
            name: name.trim().to_owned(),
            value,
        };
    }
    if line.starts_with("fn ") {
        return TraitMember::Function {
            signature: line.to_owned(),
        };
    }
    TraitMember::Raw(line.to_owned())
}

fn parse_choice_items(body: &str, base: usize, errors: &mut Vec<ParseError>) -> Vec<ChoiceItem> {
    collect_logical_choice_lines(body)
        .into_iter()
        .filter_map(|line| parse_choice_item(line.trim(), base, errors))
        .collect()
}

fn collect_logical_choice_lines(body: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;

    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        for ch in line.chars() {
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

fn parse_choice_item(
    trimmed: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceItem> {
    if trimmed.starts_with("let ") {
        return Some(match parse_stmt(trimmed) {
            Stmt::Let { pattern, expr } => ChoiceItem::Let { pattern, expr },
            _ => ChoiceItem::Raw(trimmed.to_owned()),
        });
    }
    if let Some((head, body)) = split_brace_item(trimmed) {
        if let Some(rest) = head.strip_prefix("option ") {
            if let Some((pattern, source)) = rest.split_once(" in ") {
                let option_head = format!("option {}", pattern.trim());
                let Some(option) = parse_choice_option_block(&option_head, body, base, errors)
                else {
                    return Some(ChoiceItem::Raw(trimmed.to_owned()));
                };
                return Some(ChoiceItem::For {
                    pattern: parse_pattern(pattern.trim()),
                    source: parse_expr_lossy(source.trim()),
                    items: vec![ChoiceItem::Option(Box::new(option))],
                });
            }
        }
        if let Some(condition) = head.strip_prefix("if ") {
            return Some(ChoiceItem::If {
                condition: parse_expr_lossy(condition.trim()),
                items: parse_choice_items(body, base, errors),
            });
        }
        if let Some(rest) = head.strip_prefix("for ") {
            if let Some((pattern, source)) = rest.split_once(" in ") {
                return Some(ChoiceItem::For {
                    pattern: parse_pattern(pattern.trim()),
                    source: parse_expr_lossy(source.trim()),
                    items: parse_choice_items(body, base, errors),
                });
            }
        }
        if head.starts_with("option ") {
            return parse_choice_option_block(head, body, base, errors)
                .map(Box::new)
                .map(ChoiceItem::Option);
        }
    }
    parse_choice_arm_sugar(trimmed, base, errors)
        .map(Box::new)
        .map(ChoiceItem::Option)
}

fn split_brace_item(source: &str) -> Option<(&str, &str)> {
    let open = source.find('{')?;
    let close = source.rfind('}')?;
    (open < close).then(|| (source[..open].trim(), source[open + 1..close].trim()))
}

fn parse_choice_plan_items(body: &str) -> Vec<ChoicePlanItem> {
    collect_logical_choice_lines(body)
        .into_iter()
        .map(|line| {
            let trimmed = line.trim();
            if let Some((head, block_body)) = split_brace_item(trimmed) {
                if let Some(duration) = head.strip_prefix("timeout ") {
                    return ChoicePlanItem::Timeout {
                        duration: parse_expr_lossy(duration.trim()),
                        body: block_body.trim().to_owned(),
                    };
                }
                if let Some(trigger) = head.strip_prefix("cancel on ") {
                    return ChoicePlanItem::Cancel {
                        trigger: trigger.trim().to_owned(),
                        body: block_body.trim().to_owned(),
                    };
                }
                if let Some(pattern) = head.strip_prefix("on select ") {
                    return ChoicePlanItem::OnSelect {
                        pattern: parse_pattern(pattern.trim()),
                        body: block_body.trim().to_owned(),
                    };
                }
            }
            trimmed.split_once('=').map_or_else(
                || ChoicePlanItem::Raw(trimmed.to_owned()),
                |(name, value)| ChoicePlanItem::Option {
                    name: name.trim().to_owned(),
                    value: parse_expr_lossy(value.trim()),
                },
            )
        })
        .collect()
}

fn parse_choice_arm_sugar(
    trimmed: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceOption> {
    if trimmed.is_empty() {
        return None;
    }
    let (id, rest) = parse_optional_id_ref(trimmed, base, errors);
    let rest = rest.trim();
    let quote_start = rest.find('"')?;
    let quote_end = rest[quote_start + 1..].find('"')? + quote_start + 1;
    let label = rest[quote_start + 1..quote_end].to_owned();
    let after_label = rest[quote_end + 1..].trim();
    let (enabled, action) = if let Some(condition_body) = after_label.strip_prefix("if ") {
        let (condition, action) = split_choice_condition_action(condition_body, base, errors)?;
        (
            Some(
                parse_expr(condition.trim())
                    .unwrap_or_else(|_| crate::expr::Expr::Raw(condition.trim().to_owned())),
            ),
            action,
        )
    } else {
        let action = after_label
            .strip_prefix("->")
            .map(|target| format!("->{}", target.trim()))
            .or_else(|| {
                after_label
                    .strip_prefix("=>")
                    .map(|expr| format!("=>{}", expr.trim()))
            })?;
        (None, parse_choice_action(&action, base, errors)?)
    };
    let mut option = ChoiceOption::new(
        id,
        label,
        action,
        TextRange::new(base, base + trimmed.len()),
    );
    if let Some(enabled) = enabled {
        option = option.with_enabled(enabled);
    }
    Some(option)
}

fn split_choice_condition_action<'a>(
    source: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(&'a str, ChoiceAction)> {
    if let Some((condition, target)) = source.split_once("->") {
        let target = parse_required_entity_ref(target.trim(), base, errors)?.0;
        return Some((condition, ChoiceAction::Goto(target)));
    }
    source
        .split_once("=>")
        .map(|(condition, expr)| (condition, ChoiceAction::Out(parse_expr_lossy(expr.trim()))))
}

fn parse_choice_action(
    source: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceAction> {
    if let Some(target) = source.strip_prefix("->") {
        return parse_required_entity_ref(target.trim(), base, errors)
            .map(|(entity, _)| ChoiceAction::Goto(entity));
    }
    source
        .strip_prefix("=>")
        .map(|expr| ChoiceAction::Out(parse_expr_lossy(expr.trim())))
}

fn parse_choice_option_block(
    head: &str,
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<ChoiceOption> {
    let option_id = head.strip_prefix("option")?.trim();
    let (id, rest) = parse_optional_id_ref(option_id, base, errors);
    let mut id_expr =
        (id.is_none() && !rest.trim().is_empty()).then(|| parse_expr_lossy(rest.trim()));
    let mut label = String::new();
    let mut label_text_key = None;
    let mut value = None;
    let mut enabled = None;
    let mut visible = None;
    let mut order = None;
    let mut hotkey = None;
    let mut ui_fields = Vec::new();
    let mut action = ChoiceAction::None;

    for line in collect_logical_choice_lines(body) {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("label =") {
            label = trim_string_literal(value.trim()).unwrap_or_else(|| value.trim().to_owned());
        } else if let Some(value_expr) = trimmed.strip_prefix("id =") {
            id_expr = Some(parse_expr_lossy(value_expr.trim()));
        } else if let Some(value_part) = trimmed.strip_prefix("label(") {
            if let Some((key_part, expr_part)) = value_part.split_once(')') {
                if let Some(text_key) = key_part.trim().strip_prefix("id=") {
                    label_text_key = parse_required_id_ref(text_key.trim(), base, errors)
                        .map(|(entity, _)| entity);
                }
                let expr_part = expr_part
                    .trim()
                    .strip_prefix('=')
                    .unwrap_or(expr_part)
                    .trim();
                label = trim_string_literal(expr_part).unwrap_or_else(|| expr_part.to_owned());
            }
        } else if let Some(value_expr) = trimmed.strip_prefix("value =") {
            value = Some(parse_expr_lossy(value_expr.trim()));
        } else if let Some(value) = trimmed.strip_prefix("enabled =") {
            enabled = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("visible =") {
            visible = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("order =") {
            order = Some(parse_expr_lossy(value.trim()));
        } else if let Some(value) = trimmed.strip_prefix("hotkey =") {
            hotkey = Some(parse_expr_lossy(value.trim()));
        } else if let Some((head, ui_body)) = split_brace_item(trimmed) {
            if head == "ui" {
                ui_fields = parse_choice_ui_fields(ui_body);
            } else if head == "select" {
                action = parse_choice_select_action(ui_body, base, errors);
            }
        }
    }

    let mut option = ChoiceOption::new(id, label, action, TextRange::new(base, base + head.len()));
    if let Some(id_expr) = id_expr {
        option = option.with_id_expr(id_expr);
    }
    if let Some(label_text_key) = label_text_key {
        option = option.with_label_text_key(label_text_key);
    }
    if let Some(value) = value {
        option = option.with_value(value);
    }
    if let Some(enabled) = enabled {
        option = option.with_enabled(enabled);
    }
    if let Some(visible) = visible {
        option = option.with_visible(visible);
    }
    if let Some(order) = order {
        option = option.with_order(order);
    }
    if let Some(hotkey) = hotkey {
        option = option.with_hotkey(hotkey);
    }
    Some(option.with_ui_fields(ui_fields))
}

fn parse_choice_ui_fields(body: &str) -> Vec<ChoiceUiField> {
    body.lines()
        .map(str::trim)
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            Some(ChoiceUiField::new(
                name.trim().to_owned(),
                parse_expr_lossy(value.trim()),
            ))
        })
        .collect()
}

fn parse_choice_select_action(
    body: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> ChoiceAction {
    body.lines()
        .map(str::trim)
        .find_map(|line| {
            if let Some(target) = line.strip_prefix("goto ") {
                let target = target.trim();
                return target
                    .starts_with('#')
                    .then(|| {
                        parse_required_entity_ref(target, base, errors)
                            .map(|(entity, _)| ChoiceAction::Goto(entity))
                    })
                    .flatten()
                    .or_else(|| Some(ChoiceAction::SelectBlock(body.trim().to_owned())));
            }
            line.strip_prefix("out ")
                .map(|expr| ChoiceAction::Out(parse_expr_lossy(expr.trim())))
        })
        .unwrap_or_else(|| ChoiceAction::SelectBlock(body.trim().to_owned()))
}

fn trim_string_literal(source: &str) -> Option<String> {
    source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
}

fn parse_match_arms(body: &str, base: usize, errors: &mut Vec<ParseError>) -> Vec<MatchArm> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let (head, item) = line.split_once("=>")?;
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
            let (head, value) = line.split_once("=>")?;
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
    source
        .split_once(" when ")
        .map_or((source, None), |(pattern, guard)| (pattern, Some(guard)))
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
    if let Some((name, source)) = source.split_once('=') {
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
    if let Some(rest) = line.strip_prefix("out ") {
        return LinePlanItem::Out(parse_expr_lossy(rest.trim()));
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

fn is_await_with_head(trimmed: &str) -> bool {
    (trimmed.starts_with("await ")
        || trimmed.starts_with("try await ")
        || trimmed.starts_with("await? "))
        && (trimmed.contains(" with ") || trimmed.ends_with("with:"))
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
    if let Some((expr, branches)) = source.split_once(" with:") {
        return (expr, branches);
    }
    if let Some((expr, branches)) = source.split_once(" with ") {
        return (expr, branches);
    }
    (source, "")
}

fn parse_await_branches(source: &str) -> Vec<AwaitBranch> {
    let body = source
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(source)
        .trim();
    if body
        .lines()
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

    for line in source.lines() {
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
        parse_await_branch_body(body.trim()),
    ))
}

fn parse_await_branch_body(body: &str) -> Vec<FlowItem> {
    if let Some(command) = parse_scene_command(body.trim()) {
        return vec![FlowItem::ScenarioCommand(command)];
    }

    let mut nested = Parser::new(body.to_owned());
    let mut items = Vec::new();
    while nested.index < nested.lines.len() {
        nested.skip_blank_and_comments();
        if nested.index >= nested.lines.len() {
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
        Some(
            "let"
                | "match"
                | "if"
                | "return"
                | "out"
                | "goto"
                | "spawn"
                | "defer"
                | "yield"
                | "panic"
                | "fail"
                | "signal"
                | "close"
                | "break"
                | "continue"
        )
    )
}

fn is_let_choice_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("choice "))
}

fn is_let_scope_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("scope "))
}

fn is_let_block_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim().starts_with('{'))
}

fn is_let_loop_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("loop"))
}

fn is_let_if_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("if "))
}

fn is_let_if_let_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("if let "))
}

fn is_let_match_head(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix("let ") else {
        return false;
    };
    rest.split_once('=')
        .is_some_and(|(_, expr)| expr.trim_start().starts_with("match "))
}

fn split_if_let_guard(source: &str) -> (&str, Option<&str>) {
    source
        .split_once(" when ")
        .map_or((source, None), |(value, guard)| (value, Some(guard)))
}

fn is_let_else_head(trimmed: &str) -> bool {
    trimmed.starts_with("let ") && trimmed.contains(" else") && trimmed.contains('{')
}

fn parse_scope_expr_body(body: &str) -> (Vec<Stmt>, Option<crate::expr::Expr>) {
    let lines = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some((last, statements)) = lines.split_last() else {
        return (Vec::new(), None);
    };
    let parsed_statements = statements
        .iter()
        .map(|line| parse_stmt(line))
        .collect::<Vec<_>>();
    if is_typed_stmt(last) {
        let mut parsed_statements = parsed_statements;
        parsed_statements.push(parse_stmt(last));
        (parsed_statements, None)
    } else {
        (parsed_statements, Some(parse_expr_lossy(last)))
    }
}

fn parse_block_expr(body: &str) -> crate::expr::Expr {
    let (statements, value) = parse_scope_expr_body(body);
    crate::expr::Expr::Block {
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
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_stmt)
        .collect()
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
    if let Some(expr) = trimmed.strip_prefix("out ") {
        return Stmt::Out(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("goto ") {
        return Stmt::Goto(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("spawn ") {
        return Stmt::Spawn(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("defer ") {
        return Stmt::Defer(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("yield ") {
        return Stmt::Yield(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("panic ") {
        return Stmt::Panic(parse_expr_lossy(expr.trim()));
    }
    if let Some(expr) = trimmed.strip_prefix("fail ") {
        return Stmt::Fail(parse_expr_lossy(expr.trim()));
    }
    if let Some(rest) = trimmed.strip_prefix("signal ") {
        if let Some((target, value)) = rest.split_once("<-") {
            return Stmt::Signal {
                target: parse_expr_lossy(target.trim()),
                value: parse_expr_lossy(value.trim()),
            };
        }
        return Stmt::Raw(trimmed.to_owned());
    }
    if let Some(expr) = trimmed.strip_prefix("close ") {
        return Stmt::Close(parse_expr_lossy(expr.trim()));
    }
    if trimmed == "break" {
        return Stmt::Break(None);
    }
    if let Some(expr) = trimmed.strip_prefix("break ") {
        return Stmt::Break(Some(parse_expr_lossy(expr.trim())));
    }
    if trimmed == "continue" {
        return Stmt::Continue;
    }
    if matches!(trimmed.split_whitespace().next(), Some("match" | "if")) {
        return Stmt::Raw(trimmed.to_owned());
    }
    Stmt::Expr(parse_expr_lossy(trimmed))
}

fn parse_pattern(source: &str) -> Pattern {
    let source = source.trim();
    if source == "_" {
        return Pattern::Discard;
    }
    if let Some(name) = source
        .strip_prefix("mut ")
        .map(str::trim)
        .filter(|name| is_pattern_ident(name))
    {
        return Pattern::MutIdent(name.to_owned());
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
    if source.starts_with('.') {
        return Pattern::Variant(source.to_owned());
    }
    let mut entity_errors = Vec::new();
    if let Some((entity, rest)) = parse_required_entity_ref(source, 0, &mut entity_errors)
        && rest.trim().is_empty()
    {
        return Pattern::Entity(entity);
    }
    if let Ok(expr @ (crate::expr::Expr::Literal(_) | crate::expr::Expr::EntityRef(_))) =
        parse_expr(source)
    {
        return match expr {
            crate::expr::Expr::EntityRef(entity) => Pattern::Entity(entity),
            literal => Pattern::Literal(literal),
        };
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
    if let Some(inner) = source
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return parse_list_pattern(inner);
    }
    if let Some(pattern) = parse_record_pattern(source) {
        return pattern;
    }
    if let Some((name, rest)) = split_whole_pattern(source) {
        return Pattern::Whole {
            name: name.to_owned(),
            pattern: Box::new(parse_pattern(rest)),
        };
    }
    if is_pattern_ident(source) {
        return Pattern::Ident(source.to_owned());
    }
    Pattern::Raw(source.to_owned())
}

fn split_whole_pattern(source: &str) -> Option<(&str, &str)> {
    let (name, rest) = source.split_once(' ')?;
    let name = name.trim();
    let rest = rest.trim();
    (is_pattern_ident(name)
        && !matches!(name, "mut")
        && !rest.is_empty()
        && !is_pattern_ident(rest))
    .then_some((name, rest))
}

fn parse_list_pattern(inner: &str) -> Pattern {
    let mut rest = None;
    let items = split_pattern_items(inner)
        .into_iter()
        .filter_map(|item| {
            if item == ".." {
                rest = Some(String::new());
                None
            } else if let Some(name) = item.strip_prefix("..") {
                rest = Some(name.trim().to_owned());
                None
            } else {
                Some(parse_pattern(item))
            }
        })
        .collect();
    Pattern::List { items, rest }
}

fn parse_record_pattern(source: &str) -> Option<Pattern> {
    let (head, body) = split_brace_item(source)?;
    if head.split_whitespace().count() > 1 {
        return None;
    }
    if head.trim().is_empty() && !body.contains(':') {
        return None;
    }
    let mut rest = false;
    let fields = split_pattern_items(body)
        .into_iter()
        .filter_map(|field| {
            if field == ".." {
                rest = true;
                return None;
            }
            let (name, pattern) = field
                .split_once(':')
                .map_or((field.trim(), field.trim()), |(name, pattern)| {
                    (name.trim(), pattern.trim())
                });
            is_pattern_ident(name).then(|| RecordPatternField::new(name, parse_pattern(pattern)))
        })
        .collect();
    Some(Pattern::Record {
        path: (!head.trim().is_empty()).then(|| head.trim().to_owned()),
        fields,
        rest,
    })
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
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
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
