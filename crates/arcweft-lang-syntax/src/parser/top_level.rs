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
    module_path::ModulePath,
};
use crate::cst::{CstTopLevelItemKind, CstTopLevelLineKind};

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
                if let Some(tree) = Self::use_tree_source(trimmed)
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
            self.push_error(
                range,
                "`@` does not start a top-level item",
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
            CstTopLevelItemKind::FlowBodyItemOrRaw => {
                self.parse_top_level_flow_item_or_raw(trimmed, range, items);
            }
            kind => {
                self.reject_pending_doc(range);
                if !matches!(
                    kind,
                    CstTopLevelItemKind::Trait
                        | CstTopLevelItemKind::Enum
                        | CstTopLevelItemKind::Struct
                        | CstTopLevelItemKind::TypeAlias
                        | CstTopLevelItemKind::EntityDecl
                        | CstTopLevelItemKind::ExternCapability
                        | CstTopLevelItemKind::DialogueDefaults
                        | CstTopLevelItemKind::Proof
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
            CstTopLevelItemKind::DialogueDefaults => {
                self.parse_dialogue_defaults().map(Item::DialogueDefaults)
            }
            CstTopLevelItemKind::Proof => self.parse_proof_item().map(Item::Proof),
            CstTopLevelItemKind::Test => self.parse_test_item().map(Item::Test),
            CstTopLevelItemKind::Bench => self.parse_bench_item().map(Item::Bench),
            CstTopLevelItemKind::Source => self.parse_source_item().map(Item::Source),
            CstTopLevelItemKind::Style => self.parse_style().map(Item::Style),
            CstTopLevelItemKind::Flow
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
}
