//! Top-level parser dispatch for modules, imports, and item families.

use super::{
    Parser, TopLevelDispatch, TopLevelSinks,
    headers::{
        DeclEntityId, parse_required_decl_entity_ref_or_marker, parse_visibility_prefix,
        simple_error, slice_offset,
    },
    helpers::{
        is_relative_id_path, normalize_module_path, parse_inner_attribute, parse_outer_attribute,
        parse_use_line,
    },
    parse_expr_lossy, split_top_level_binding,
};
use crate::ast::{
    common::{ModuleDecl, TextRange},
    ids::EntityRef,
    items::{
        Item, RawItem, StyleItem, StyleItemInit, ViewStyleAssignOpDecl, ViewStyleDeclarationDecl,
        ViewStyleEnvironmentPredicateDecl, ViewStyleRuleDecl, ViewStyleSelectorPartDecl,
        ViewStyleTokenDecl, ViewStyleValueDecl,
    },
    module_path::ModulePath,
    style::StyleSyntax,
};
use crate::cst::{CstTopLevelItemKind, CstTopLevelLineKind, split_top_level_punctuation};
use crate::expr::{Expr, Literal};
use crate::parser::SourceDialect;

impl Parser<'_> {
    pub(super) fn parse_top_level_line(
        &mut self,
        dispatch: TopLevelDispatch,
        trimmed: &str,
        range: TextRange,
        sinks: &mut TopLevelSinks<'_>,
    ) {
        match dispatch.line {
            CstTopLevelLineKind::Attribute => {
                if let Some(attribute) = parse_outer_attribute(trimmed, range) {
                    *sinks.source_attrs_open = false;
                    self.push_pending_attr(attribute);
                    self.index += 1;
                } else if let Some(attribute) = self.take_multiline_outer_attribute() {
                    *sinks.source_attrs_open = false;
                    self.push_pending_attr(attribute);
                } else if let Some(attribute) = parse_inner_attribute(trimmed, range) {
                    if *sinks.source_attrs_open
                        && self.pending_doc.is_none()
                        && self.pending_attrs.is_empty()
                    {
                        sinks.attrs.push(attribute);
                    } else {
                        self.push_error(
                            range,
                            "inner source attribute must appear before documentation comments, outer attributes, module/use declarations, and items",
                            ["#![generated(...)] at the start of the source file"],
                            Some(trimmed),
                            ["move source-level `#![...]` attributes to the source header"],
                        );
                        self.reject_pending_doc(range);
                        self.reject_pending_attrs(range);
                        *sinks.source_attrs_open = false;
                    }
                    self.index += 1;
                } else {
                    *sinks.source_attrs_open = false;
                    self.parse_top_level_item(dispatch.item, trimmed, range, sinks.items);
                }
            }
            CstTopLevelLineKind::Module => {
                *sinks.source_attrs_open = false;
                let path = trimmed.strip_prefix("mod ").unwrap_or_default();
                self.reject_pending_doc(range);
                self.reject_pending_attrs(range);
                if self.validate_module_path(path, range) {
                    let module_path = normalize_module_path(path.trim());
                    self.current_module_path = Some(module_path.clone());
                    *sinks.module = Some(ModuleDecl::new(module_path, range));
                }
                self.index += 1;
            }
            CstTopLevelLineKind::Use => {
                *sinks.source_attrs_open = false;
                self.reject_pending_doc(range);
                self.reject_pending_attrs(range);
                if Self::use_line_has_removed_execution_mode(trimmed) {
                    self.push_error(
                        range,
                        "`lazy use` and `eager use` were removed from Arcweft import syntax",
                        ["use module::path"],
                        Some(trimmed),
                        ["remove the import qualifier; use compiler build settings and content availability declarations for demand policy"],
                    );
                } else if let Some(tree) = Self::use_tree_source(trimmed)
                    && self.validate_use_tree(tree, range)
                {
                    match parse_use_line(trimmed, range) {
                        Ok(Some(use_item)) => sinks.uses.push(use_item),
                        Ok(None) => {}
                        Err(error) => {
                            let message = format!("invalid use tree: {error}");
                            self.push_error(
                                range,
                                &message,
                                ["use self.path", "use super.path", "use crate.path"],
                                Some(trimmed),
                                ["use a valid module path or grouped import tree"],
                            );
                        }
                    }
                }
                self.index += 1;
            }
            CstTopLevelLineKind::Item => {
                self.parse_top_level_item_line(dispatch.item, trimmed, range, sinks);
            }
        }
    }

    pub(super) fn validate_module_path(&mut self, path: &str, range: TextRange) -> bool {
        if is_relative_id_path(path) {
            self.push_error(
                range,
                "module paths cannot use relative ID syntax",
                ["self::path", "super::path", "crate::path"],
                Some(path.trim()),
                ["use `self::`, `super::`, or `crate::` for module-relative paths"],
            );
            false
        } else if path.contains("::") {
            self.push_error(
                range,
                "module paths use `.` separators; `::` is not Arcweft module syntax",
                ["mod game.routes.opening"],
                Some(path.trim()),
                ["replace each `::` with `.`"],
            );
            false
        } else {
            let normalized = normalize_module_path(path.trim());
            match normalized.parse::<ModulePath>() {
                Ok(_) => true,
                Err(error) => {
                    self.push_error(
                        range,
                        &format!("invalid module path: {error}"),
                        ["mod game.routes.opening"],
                        Some(path.trim()),
                        ["use identifiers separated by `.`"],
                    );
                    false
                }
            }
        }
    }

    fn use_line_has_removed_execution_mode(trimmed: &str) -> bool {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        let rest = rest.trim_start();
        rest.starts_with("lazy use ") || rest.starts_with("eager use ")
    }

    fn use_tree_source(trimmed: &str) -> Option<&str> {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        rest.trim_start().strip_prefix("use ").map(str::trim)
    }

    fn parse_top_level_item_line(
        &mut self,
        kind: CstTopLevelItemKind,
        trimmed: &str,
        range: TextRange,
        sinks: &mut TopLevelSinks<'_>,
    ) {
        *sinks.source_attrs_open = false;
        if trimmed.starts_with('@') {
            let message = if trimmed.starts_with("@memo") {
                "`@memo` is not valid Arcweft syntax"
            } else {
                "`@` does not start a top-level item"
            };
            self.push_error(
                range,
                message,
                ["fn name(...) { ... }", "#[attribute]"],
                Some(trimmed),
                ["use `#[...]` for attributes or an ordinary item keyword"],
            );
        }
        self.parse_top_level_item(kind, trimmed, range, sinks.items);
    }

    pub(super) fn validate_use_tree(&mut self, tree: &str, range: TextRange) -> bool {
        if is_relative_id_path(tree) {
            self.push_error(
                range,
                "use paths cannot use relative ID syntax",
                ["use self.path", "use super.path", "use crate.path"],
                Some(tree.trim()),
                ["use `self::`, `super::`, or `crate::` for module-relative imports"],
            );
            false
        } else {
            true
        }
    }

    pub(super) fn parse_top_level_item(
        &mut self,
        kind: CstTopLevelItemKind,
        trimmed: &str,
        range: TextRange,
        items: &mut Vec<Item>,
    ) {
        match kind {
            CstTopLevelItemKind::Flow => {
                if let Some(flow) = self.parse_flow() {
                    items.push(Item::Flow(flow));
                }
            }
            CstTopLevelItemKind::Function => {
                if let Some(function) = self.parse_function_item() {
                    items.push(Item::Function(function));
                }
            }
            CstTopLevelItemKind::Agent => {
                if let Some(agent) = self.parse_agent_item() {
                    items.push(Item::Agent(agent));
                }
            }
            CstTopLevelItemKind::FlowBodyItemOrRaw => {
                self.parse_top_level_flow_item_or_raw(trimmed, range, items);
            }
            kind => {
                self.reject_pending_doc(range);
                if !matches!(
                    kind,
                    CstTopLevelItemKind::State
                        | CstTopLevelItemKind::Trait
                        | CstTopLevelItemKind::Enum
                        | CstTopLevelItemKind::Struct
                        | CstTopLevelItemKind::TypeAlias
                        | CstTopLevelItemKind::EntityDecl
                        | CstTopLevelItemKind::ExternCapability
                        | CstTopLevelItemKind::DialogueDefaults
                        | CstTopLevelItemKind::Source
                        | CstTopLevelItemKind::Style
                ) {
                    self.reject_pending_attrs(range);
                }
                if let Some(item) = self.parse_classified_top_level_item(kind) {
                    items.push(item);
                }
            }
        }
    }

    pub(super) fn parse_classified_top_level_item(
        &mut self,
        kind: CstTopLevelItemKind,
    ) -> Option<Item> {
        match kind {
            CstTopLevelItemKind::Callable => self.parse_callable_item().map(Item::Callable),
            CstTopLevelItemKind::State => self.parse_state_item().map(Item::State),
            CstTopLevelItemKind::Trait => self.parse_trait_item().map(Item::Trait),
            CstTopLevelItemKind::Impl => self.parse_impl_item().map(Item::Impl),
            CstTopLevelItemKind::Enum => self.parse_enum_item().map(Item::Enum),
            CstTopLevelItemKind::Struct => self.parse_struct_item().map(Item::Struct),
            CstTopLevelItemKind::TypeAlias => self.parse_type_alias().map(Item::TypeAlias),
            CstTopLevelItemKind::EntityDecl => self.parse_entity_decl_item().map(Item::EntityDecl),
            CstTopLevelItemKind::Entry => self.parse_entry_item().map(Item::Entry),
            CstTopLevelItemKind::ExternCapability => self
                .parse_extern_capability_item()
                .map(Item::ExternCapability),
            CstTopLevelItemKind::ExternMod => self.parse_extern_mod_item().map(Item::ExternMod),
            CstTopLevelItemKind::Hook => self.parse_hook().map(Item::Hook),
            CstTopLevelItemKind::DialogueDefaults => {
                self.parse_dialogue_defaults().map(Item::DialogueDefaults)
            }
            CstTopLevelItemKind::MemoFn => self.parse_memo_fn().map(Item::MemoFn),
            CstTopLevelItemKind::Proof => self.parse_proof_item().map(Item::Proof),
            CstTopLevelItemKind::TrustedAxiom => {
                self.parse_trusted_axiom_item().map(Item::TrustedAxiom)
            }
            CstTopLevelItemKind::Test => self.parse_test_item().map(Item::Test),
            CstTopLevelItemKind::Bench => self.parse_bench_item().map(Item::Bench),
            CstTopLevelItemKind::Parser => self.parse_parser_item().map(Item::Parser),
            CstTopLevelItemKind::Source => self.parse_source_item().map(Item::Source),
            CstTopLevelItemKind::Style => self.parse_style().map(Item::Style),
            CstTopLevelItemKind::Flow
            | CstTopLevelItemKind::Agent
            | CstTopLevelItemKind::Function
            | CstTopLevelItemKind::FlowBodyItemOrRaw => None,
        }
    }

    pub(super) fn parse_top_level_flow_item_or_raw(
        &mut self,
        trimmed: &str,
        range: TextRange,
        items: &mut Vec<Item>,
    ) {
        self.reject_pending_doc(range);
        self.reject_pending_attrs(range);
        if self.source_dialect == SourceDialect::Agent {
            self.push_error(
                range,
                "unsupported top-level item in Agent dialect",
                ["agent @agent.id name() { ... }"],
                Some(trimmed),
                ["wrap Agent work in a top-level `agent` item"],
            );
            self.index += 1;
            return;
        }
        if let Some(flow_item) = self.parse_flow_item_until_indent(0) {
            items.push(Item::FlowItem(Box::new(flow_item)));
        } else {
            self.push_error(
                range,
                "unexpected top-level item",
                ["a declaration", "a flow item"],
                Some(trimmed),
                ["use a current Arcweft declaration or flow-item form"],
            );
            items.push(Item::Raw(RawItem::new(trimmed.to_owned(), None, range)));
            if self.current().text.contains('{') || self.next_nonblank_line_is_brace() {
                let _ = self.take_flow_block_event();
            } else {
                self.index += 1;
            }
        }
    }

    fn parse_style(&mut self) -> Option<StyleItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing style declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the style body"],
            );
            return None;
        }
        let head = head.trim();
        let (visibility, rest) = parse_visibility_prefix(head);
        let rest = rest.trim_start().strip_prefix("style")?.trim_start();
        let id_base = start_line.start + slice_offset(head, rest);
        let module_path = self.current_module_path.as_deref();
        let (id, syntax, trailing) =
            parse_style_decl_head(rest, id_base, module_path, &mut self.errors)?;
        if !trailing.trim().is_empty() {
            self.push_error(
                TextRange::new(id.range().end(), start_line.end),
                "unexpected text after style declaration head",
                ["{", ": .Css {"],
                Some(trailing.trim()),
                ["move properties into the style body or place `: .Css` before the body"],
            );
        }
        let inline_source = Some(body.to_string());
        let fields = match syntax {
            StyleSyntax::Arcweft => {
                ViewStyleFields::parse(&body, start_line.start, &mut self.errors)
            }
            StyleSyntax::Css => ViewStyleFields::default(),
        };
        Some(StyleItem::new(StyleItemInit {
            attrs,
            visibility,
            id,
            syntax,
            inline_source,
            tokens: fields.tokens,
            rules: fields.rules,
            environment_predicates: fields.environment_predicates,
            range: TextRange::new(start_line.start, end),
        }))
    }
}

fn parse_style_decl_head(
    input: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<super::ParseError>,
) -> Option<(EntityRef, StyleSyntax, String)> {
    let input = input.trim_start();
    let (id, tail) = if input.starts_with('@') {
        let (parsed, rest) =
            parse_required_decl_entity_ref_or_marker(input, "style", base, errors)?;
        match parsed {
            DeclEntityId::Entity(entity) => {
                let (entity, rest) = normalize_style_decl_colon(entity, rest);
                (
                    rebase_relative_style_decl_entity(entity, input, module_path),
                    rest,
                )
            }
            DeclEntityId::NameMarker(marker) => {
                let rest = rest.trim_start();
                let (name, tail) = parse_style_name_and_tail(rest);
                let Some(name) = name else {
                    errors.push(simple_error(
                        marker.range.start(),
                        marker.range.end() - marker.range.start(),
                        "relative style declaration marker needs a following style name",
                        "@style:. primary_button",
                    ));
                    return None;
                };
                (
                    EntityRef::new(
                        style_decl_body(&name, module_path),
                        false,
                        TextRange::new(marker.range.end(), marker.range.end() + name.len()),
                    ),
                    tail,
                )
            }
        }
    } else {
        let (name, tail) = parse_style_name_and_tail(input);
        let Some(name) = name else {
            errors.push(simple_error(
                base,
                input.len(),
                "style declaration needs a canonical style name or declaration id",
                "style primary_button",
            ));
            return None;
        };
        let start = input
            .find(&name)
            .map_or(base, |offset| base.saturating_add(offset));
        (
            EntityRef::new(
                style_decl_body(&name, module_path),
                false,
                TextRange::new(start, start + name.len()),
            ),
            tail,
        )
    };
    let (syntax, tail) = parse_style_syntax_tail(&tail, id.range().end(), errors)?;
    Some((id, syntax, tail))
}

fn normalize_style_decl_colon(entity: EntityRef, rest: &str) -> (EntityRef, String) {
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

fn style_decl_body(name: &str, module_path: Option<&str>) -> String {
    EntityRef::module_scoped_declaration_body("style", name, module_path)
}

fn rebase_relative_style_decl_entity(
    entity: EntityRef,
    source: &str,
    module_path: Option<&str>,
) -> EntityRef {
    if !(source.starts_with("@.") || source.starts_with("@style:.")) {
        return entity;
    }
    let Some(suffix) = entity.body().strip_prefix("style.") else {
        return entity;
    };
    EntityRef::new(style_decl_body(suffix, module_path), false, *entity.range())
}

fn parse_style_name_and_tail(input: &str) -> (Option<String>, String) {
    let trimmed = input.trim_start();
    let Some((first, mut tail)) = crate::cst::split_leading_ident(trimmed) else {
        return (None, trimmed.to_owned());
    };
    let mut name = first.to_owned();
    while let Some(after_dot) = tail.strip_prefix('.') {
        let Some((segment, next_tail)) = crate::cst::split_leading_ident(after_dot) else {
            break;
        };
        name.push('.');
        name.push_str(segment);
        tail = next_tail;
    }
    (Some(name), tail.trim().to_owned())
}

fn parse_style_syntax_tail(
    tail: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<(StyleSyntax, String)> {
    let tail = tail.trim_start();
    if tail.is_empty() {
        return Some((StyleSyntax::Arcweft, String::new()));
    }
    let Some(rest) = tail.strip_prefix(':') else {
        return Some((StyleSyntax::Arcweft, tail.to_owned()));
    };
    let syntax = rest.trim_start();
    if let Some(trailing) = syntax.strip_prefix(".Css") {
        Some((StyleSyntax::Css, trailing.to_owned()))
    } else if let Some(trailing) = syntax.strip_prefix(".Arcweft") {
        Some((StyleSyntax::Arcweft, trailing.to_owned()))
    } else {
        errors.push(simple_error(
            base,
            tail.len(),
            "style declaration syntax must be `.Css` or `.Arcweft`",
            ": .Css",
        ));
        None
    }
}

#[derive(Default)]
struct ViewStyleFields {
    tokens: Vec<ViewStyleTokenDecl>,
    rules: Vec<ViewStyleRuleDecl>,
    environment_predicates: Vec<ViewStyleEnvironmentPredicateDecl>,
}

#[derive(Debug)]
struct PendingViewStyleRule {
    selector: Vec<ViewStyleSelectorPartDecl>,
    declarations: Vec<ViewStyleDeclarationDecl>,
}

impl ViewStyleFields {
    fn parse(body: &str, base: usize, errors: &mut Vec<super::ParseError>) -> Self {
        let mut fields = Self::default();
        let mut pending_rule: Option<PendingViewStyleRule> = None;
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if trimmed == "}" {
                if let Some(rule) = pending_rule.take() {
                    fields
                        .rules
                        .push(ViewStyleRuleDecl::new(rule.selector, rule.declarations));
                } else {
                    errors.push(simple_error(
                        base,
                        trimmed.len(),
                        "unmatched style rule close",
                        "Button:hover { ... }",
                    ));
                }
                continue;
            }
            if trimmed.ends_with('{') {
                if let Some(rule) = pending_rule.take() {
                    fields
                        .rules
                        .push(ViewStyleRuleDecl::new(rule.selector, rule.declarations));
                }
                let selector = trimmed.trim_end_matches('{').trim();
                if selector.is_empty() {
                    errors.push(simple_error(
                        base,
                        trimmed.len(),
                        "style rule selector cannot be empty",
                        "Button:hover {",
                    ));
                    continue;
                }
                pending_rule = Some(PendingViewStyleRule {
                    selector: parse_view_style_selector(selector, base, errors),
                    declarations: Vec::new(),
                });
                continue;
            }
            if let Some(rule) = &mut pending_rule {
                if let Some(declaration) = parse_view_style_declaration(trimmed, base, errors) {
                    rule.declarations.push(declaration);
                }
                continue;
            }
            if let Some(token) = trimmed.strip_prefix("token ") {
                if let Some(token) = parse_view_style_token(token, base, errors) {
                    fields.tokens.push(token);
                }
                continue;
            }
            if let Some(predicate) = trimmed.strip_prefix("environment ") {
                if let Some(predicate) = parse_view_style_environment(predicate, base, errors) {
                    fields.environment_predicates.push(predicate);
                }
                continue;
            }
            errors.push(simple_error(
                base,
                trimmed.len(),
                &format!("invalid style declaration `{trimmed}`"),
                "token name = value | environment name = value | Button:hover { ... }",
            ));
        }
        if let Some(rule) = pending_rule {
            fields
                .rules
                .push(ViewStyleRuleDecl::new(rule.selector, rule.declarations));
        }
        fields
    }
}

fn parse_view_style_token(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleTokenDecl> {
    let Some((name, value)) = split_top_level_binding(source.trim()) else {
        errors.push(simple_error(
            base,
            source.len(),
            "invalid view style token",
            "token public.id = value",
        ));
        return None;
    };
    parse_view_style_value(value.trim(), base, errors)
        .map(|value| ViewStyleTokenDecl::new(name.trim().to_owned(), value))
}

fn parse_view_style_declaration(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleDeclarationDecl> {
    let (op, body) = if let Some(rest) = source.strip_prefix("append ") {
        (ViewStyleAssignOpDecl::Append, rest.trim())
    } else {
        (ViewStyleAssignOpDecl::Replace, source)
    };
    let Some((name, value)) = split_top_level_binding(body) else {
        errors.push(simple_error(
            base,
            source.len(),
            "invalid view style declaration",
            "property-name = value",
        ));
        return None;
    };
    parse_view_style_value(value.trim(), base, errors)
        .map(|value| ViewStyleDeclarationDecl::new(name.trim().to_owned(), value, op))
}

fn parse_view_style_environment(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleEnvironmentPredicateDecl> {
    let Some((name, value)) = split_top_level_binding(source.trim()) else {
        errors.push(simple_error(
            base,
            source.len(),
            "invalid view style environment predicate",
            "environment text_scale_at_least_milli = 1000",
        ));
        return None;
    };
    match name.trim() {
        "text_scale_at_least_milli" => parse_u32_literal(value.trim())
            .map(ViewStyleEnvironmentPredicateDecl::TextScaleAtLeastMilli),
        other => {
            errors.push(simple_error(
                base,
                other.len(),
                &format!("unknown view style environment predicate `{other}`"),
                "text_scale_at_least_milli",
            ));
            None
        }
    }
}

fn parse_view_style_selector(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Vec<ViewStyleSelectorPartDecl> {
    let mut parts = Vec::new();
    for token in source.split_whitespace() {
        if token == ">" {
            parts.push(ViewStyleSelectorPartDecl::Child);
        } else if token == "*" {
            parts.push(ViewStyleSelectorPartDecl::Descendant);
        } else if let Some(value) = call_arg(token, "part") {
            parts.push(ViewStyleSelectorPartDecl::Part(value.to_owned()));
        } else if let Some(value) = call_arg(token, "state") {
            parts.push(ViewStyleSelectorPartDecl::State(value.to_owned()));
        } else if let Some(value) = call_arg(token, "interaction") {
            parts.push(ViewStyleSelectorPartDecl::Interaction(value.to_owned()));
        } else {
            push_style_selector_compound(token, &mut parts);
        }
    }
    if parts.is_empty() {
        errors.push(simple_error(
            base,
            source.len(),
            "style rule selector cannot be empty",
            "Button:hover {",
        ));
    }
    parts
}

fn push_style_selector_compound(token: &str, parts: &mut Vec<ViewStyleSelectorPartDecl>) {
    let mut segments = token.split(':');
    if let Some(head) = segments
        .next()
        .map(str::trim)
        .filter(|head| !head.is_empty())
    {
        if let Some(part) = head.strip_prefix('.') {
            parts.push(ViewStyleSelectorPartDecl::Part(part.to_owned()));
        } else {
            parts.push(ViewStyleSelectorPartDecl::Element(canonical_style_element(
                head,
            )));
        }
    }
    parts.extend(
        segments
            .filter(|segment| !segment.trim().is_empty())
            .map(|segment| {
                let selector = canonical_style_selector_symbol(segment.trim());
                if is_interaction_selector(&selector) {
                    ViewStyleSelectorPartDecl::Interaction(selector)
                } else {
                    ViewStyleSelectorPartDecl::State(selector)
                }
            }),
    );
}

fn canonical_style_element(source: &str) -> String {
    match source {
        "Panel" | "panel" => "panel".to_owned(),
        "Box" | "box" => "box".to_owned(),
        "Scroll" | "scroll" => "scroll".to_owned(),
        "Row" | "row" => "row".to_owned(),
        "Column" | "column" => "column".to_owned(),
        "Stack" | "stack" => "stack".to_owned(),
        "Button" | "button" => "button".to_owned(),
        "TextField" | "text_field" | "text-field" => "text_field".to_owned(),
        "TextArea" | "text_area" | "text-area" => "text_area".to_owned(),
        "SecureField" | "secure_field" | "secure-field" => "secure_field".to_owned(),
        other => other.to_owned(),
    }
}

fn canonical_style_selector_symbol(source: &str) -> String {
    source.replace('-', "_")
}

fn is_interaction_selector(source: &str) -> bool {
    matches!(source, "hover" | "active" | "disabled")
}

fn parse_view_style_value(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleValueDecl> {
    if let Some(value) = call_arg(source, "token") {
        Some(ViewStyleValueDecl::Token(value.to_owned()))
    } else if let Some(value) = call_arg(source, "system_color") {
        Some(ViewStyleValueDecl::SystemColor(value.to_owned()))
    } else if let Some(value) = call_arg(source, "milli") {
        parse_i32_literal(value).map(ViewStyleValueDecl::Milli)
    } else if let Some(value) = source.trim().strip_suffix("milli") {
        parse_i32_literal(value.trim()).map(ViewStyleValueDecl::Milli)
    } else if source.trim().ends_with("px") {
        Some(ViewStyleValueDecl::Text(source.trim().to_owned()))
    } else if let Some(value) = call_arg(source, "text") {
        view_style_text_value(value).map(ViewStyleValueDecl::Text)
    } else if source.starts_with('[') && source.ends_with(']') {
        parse_view_style_list_value(source, base, errors)
    } else if let Some(value) = call_arg(source, "resource") {
        Some(ViewStyleValueDecl::Resource(value.to_owned()))
    } else if let Some(value) = call_arg(source, "rgba") {
        parse_rgba_value(value, base, errors)
    } else {
        errors.push(simple_error(
            base,
            source.len(),
            &format!("unknown view style value `{source}`"),
            "token(id) | system_color(name) | milli(1000) | 1000milli | 1px | text(\"value\") | [\"value\", ...] | resource(id) | rgba(r, g, b, a)",
        ));
        None
    }
}

fn parse_view_style_list_value(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleValueDecl> {
    let inner = source.trim().strip_prefix('[')?.strip_suffix(']')?.trim();
    let values = split_top_level_punctuation(inner, ',')
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_view_style_list_item(value, base, errors))
        .collect::<Option<Vec<_>>>()?;
    Some(ViewStyleValueDecl::List(values))
}

fn view_style_text_value(value: &str) -> Option<String> {
    match parse_expr_lossy(value) {
        Expr::Literal(Literal::String(value)) => Some(value),
        Expr::Path(value) => Some(value.as_label().to_owned()),
        Expr::ShortVariant(value) => Some(format!(".{value}")),
        _ => None,
    }
}

fn parse_view_style_list_item(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleValueDecl> {
    match parse_expr_lossy(source) {
        Expr::Literal(Literal::String(value)) => Some(ViewStyleValueDecl::Text(value)),
        Expr::Path(value) => Some(ViewStyleValueDecl::Text(value.as_label().to_owned())),
        Expr::ShortVariant(value) => Some(ViewStyleValueDecl::Text(format!(".{value}"))),
        Expr::Raw(value) if !value.trim().is_empty() && !value.contains(char::is_whitespace) => {
            Some(ViewStyleValueDecl::Text(value.trim().to_owned()))
        }
        _ => parse_view_style_value(source, base, errors),
    }
}

fn call_arg<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    source
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('('))
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
}

fn parse_rgba_value(
    source: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<ViewStyleValueDecl> {
    let channels = source
        .split(',')
        .map(str::trim)
        .map(parse_u8_literal)
        .collect::<Option<Vec<_>>>()?;
    let [red, green, blue, alpha] = channels.as_slice() else {
        errors.push(simple_error(
            base,
            source.len(),
            "rgba view style value needs four channels",
            "rgba(255, 255, 255, 255)",
        ));
        return None;
    };
    Some(ViewStyleValueDecl::Rgba {
        red: *red,
        green: *green,
        blue: *blue,
        alpha: *alpha,
    })
}

fn parse_i32_literal(value: &str) -> Option<i32> {
    value.replace('_', "").parse::<i32>().ok()
}

fn parse_u32_literal(value: &str) -> Option<u32> {
    value.replace('_', "").parse::<u32>().ok()
}

fn parse_u8_literal(value: &str) -> Option<u8> {
    value.replace('_', "").parse::<u8>().ok()
}
