//! Top-level parser dispatch for modules, imports, and item families.

use super::{
    Parser, TopLevelDispatch, TopLevelSinks,
    helpers::{
        is_relative_id_path, normalize_module_path, parse_inner_attribute, parse_outer_attribute,
        parse_use_line,
    },
};
use crate::ast::{
    common::{ModuleDecl, TextRange},
    items::{Item, RawItem},
};
use crate::cst::{CstTopLevelItemKind, CstTopLevelLineKind};
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
                if let Some(use_item) = parse_use_line(trimmed, range)
                    && self.validate_use_tree(use_item.tree(), range)
                {
                    sinks.uses.push(use_item);
                }
                self.index += 1;
            }
            CstTopLevelLineKind::Item => {
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
                self.parse_top_level_item(dispatch.item, trimmed, range, sinks.items);
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
}
