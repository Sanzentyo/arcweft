//! Top-level parser dispatch for modules, imports, and item families.

use super::{
    Parser, TopLevelDispatch, TopLevelSinks,
    headers::{parse_required_entity_ref, parse_visibility_prefix, simple_error, slice_offset},
    helpers::{
        is_relative_id_path, normalize_module_path, parse_inner_attribute, parse_outer_attribute,
        parse_use_line,
    },
    parse_expr_lossy, split_top_level_binding,
};
use crate::ast::{
    common::{ModuleDecl, TextRange},
    ids::EntityRef,
    items::{Item, RawItem, UiTextInputItem, UiTextInputKind},
};
use crate::cst::{CstTopLevelItemKind, CstTopLevelLineKind};
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
                    *sinks.module =
                        Some(ModuleDecl::new(normalize_module_path(path.trim()), range));
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
                                ["use self::path", "use super::path", "use crate::path"],
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
        } else {
            true
        }
    }

    fn use_line_has_removed_execution_mode(trimmed: &str) -> bool {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("surface ").unwrap_or(rest);
        rest.starts_with("lazy use ") || rest.starts_with("eager use ")
    }

    fn use_tree_source(trimmed: &str) -> Option<&str> {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        rest.trim_start().strip_prefix("use ").map(str::trim)
    }

    fn top_level_item_has_removed_asset_set(trimmed: &str) -> bool {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("surface ").unwrap_or(rest);
        rest.starts_with("asset set ")
    }

    fn top_level_item_has_removed_hot_checkpoint(trimmed: &str) -> bool {
        let (_, rest) = super::headers::parse_visibility_prefix(trimmed);
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("surface ").unwrap_or(rest);
        rest == "hot checkpoint"
            || rest.starts_with("hot checkpoint ")
            || rest.starts_with("hot checkpoint{")
    }

    fn parse_top_level_item_line(
        &mut self,
        kind: CstTopLevelItemKind,
        trimmed: &str,
        range: TextRange,
        sinks: &mut TopLevelSinks<'_>,
    ) {
        *sinks.source_attrs_open = false;
        if Self::top_level_item_has_removed_asset_set(trimmed) {
            self.reject_removed_asset_set_decl(range);
            return;
        }
        if Self::top_level_item_has_removed_hot_checkpoint(trimmed) {
            self.reject_removed_hot_checkpoint_decl(range);
            return;
        }
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

    fn reject_removed_asset_set_decl(&mut self, range: TextRange) {
        let line = self.current().clone();
        self.push_error(
            range,
            "`asset set` is not part of the v1 Arcweft source grammar",
            ["linker finite sets in the manifest"],
            Some(line.text.trim()),
            [
                "use direct typed entity references in source and manifest-backed finite sets for extern/reflection boundaries",
            ],
        );
        self.reject_pending_doc(range);
        self.reject_pending_attrs(range);
        if line.text.contains('{') || self.next_nonblank_line_is_brace() {
            let _ = self.take_flow_block_event();
        } else {
            self.index += 1;
        }
    }

    fn reject_removed_hot_checkpoint_decl(&mut self, range: TextRange) {
        let line = self.current().clone();
        self.push_error(
            range,
            "`hot checkpoint` is not part of the v1 Arcweft source grammar",
            ["use runtime generation pins and packaging hot-reload policy"],
            Some(line.text.trim()),
            ["keep checkpoint policy in the runtime/manifest layer instead of source declarations"],
        );
        self.reject_pending_doc(range);
        self.reject_pending_attrs(range);
        if line.text.contains('{') || self.next_nonblank_line_is_brace() {
            let _ = self.take_flow_block_event();
        } else {
            self.index += 1;
        }
    }

    pub(super) fn validate_use_tree(&mut self, tree: &str, range: TextRange) -> bool {
        if is_relative_id_path(tree) {
            self.push_error(
                range,
                "use paths cannot use relative ID syntax",
                ["use self::path", "use super::path", "use crate::path"],
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
                        | CstTopLevelItemKind::UiTextInput
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
            CstTopLevelItemKind::UiTextInput => self.parse_ui_text_input().map(Item::UiTextInput),
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
            items.push(Item::Raw(RawItem::new(trimmed.to_owned(), None, range)));
            self.index += 1;
        }
    }

    fn parse_ui_text_input(&mut self) -> Option<UiTextInputItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing UI text input declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the UI text input body"],
            );
            return None;
        }
        let head = head.trim();
        let (visibility, rest) = parse_visibility_prefix(head);
        let rest = rest.trim_start().strip_prefix("ui")?.trim_start();
        let (kind, rest) = parse_ui_text_input_kind(rest)?;
        let id_base = start_line.start + slice_offset(head, rest);
        let (id, trailing) =
            parse_required_entity_ref(rest.trim_start(), id_base, &mut self.errors)?;
        if !trailing.trim().is_empty() {
            self.push_error(
                TextRange::new(id.range().end(), start_line.end),
                "unexpected text after UI text input id",
                ["{"],
                Some(trailing.trim()),
                ["move properties into the UI text input body"],
            );
        }
        let fields = UiTextInputFields::parse(&body, start_line.start, &mut self.errors);
        Some(
            UiTextInputItem::new(
                attrs,
                visibility,
                id,
                kind,
                TextRange::new(start_line.start, end),
            )
            .with_label(fields.label)
            .with_value(fields.value)
            .with_placeholder(fields.placeholder)
            .with_purpose(fields.purpose)
            .with_enter_key(fields.enter_key)
            .with_submit(fields.submit)
            .with_change(fields.change),
        )
    }
}

fn parse_ui_text_input_kind(input: &str) -> Option<(UiTextInputKind, &str)> {
    [
        ("text_input", UiTextInputKind::TextField),
        ("text_area", UiTextInputKind::TextArea),
        ("secure_field", UiTextInputKind::SecureField),
    ]
    .into_iter()
    .find_map(|(keyword, kind)| {
        input
            .strip_prefix(keyword)
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
            .map(|rest| (kind, rest.trim_start()))
    })
}

#[derive(Default)]
struct UiTextInputFields {
    label: Option<String>,
    value: Option<String>,
    placeholder: Option<String>,
    purpose: Option<String>,
    enter_key: Option<String>,
    submit: Option<EntityRef>,
    change: Option<EntityRef>,
}

impl UiTextInputFields {
    fn parse(body: &str, base: usize, errors: &mut Vec<super::ParseError>) -> Self {
        let mut fields = Self::default();
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            let Some((name, value)) = split_top_level_binding(trimmed) else {
                errors.push(simple_error(
                    base,
                    trimmed.len(),
                    &format!("invalid UI text input field `{trimmed}`"),
                    "name = value",
                ));
                continue;
            };
            fields.set(name.trim(), value.trim(), base, errors);
        }
        fields
    }

    fn set(&mut self, name: &str, value: &str, base: usize, errors: &mut Vec<super::ParseError>) {
        match name {
            "label" => self.label = ui_field_string(value),
            "value" => self.value = ui_field_string(value),
            "placeholder" => self.placeholder = ui_field_string(value),
            "purpose" => self.purpose = ui_field_symbol(value),
            "enter_key" => self.enter_key = ui_field_symbol(value),
            "submit" => self.submit = ui_field_entity(value, base, errors),
            "change" => self.change = ui_field_entity(value, base, errors),
            _ => errors.push(simple_error(
                base,
                name.len(),
                &format!("unknown UI text input field `{name}`"),
                "label | value | placeholder | purpose | enter_key | submit | change",
            )),
        }
    }
}

fn ui_field_string(value: &str) -> Option<String> {
    match parse_expr_lossy(value) {
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => Some(value),
        _ => None,
    }
}

fn ui_field_symbol(value: &str) -> Option<String> {
    match parse_expr_lossy(value) {
        Expr::Literal(Literal::String(value)) | Expr::Path(value) => Some(value),
        _ => None,
    }
}

fn ui_field_entity(
    value: &str,
    base: usize,
    errors: &mut Vec<super::ParseError>,
) -> Option<EntityRef> {
    parse_required_entity_ref(value, base, errors).map(|(entity, _)| entity)
}
