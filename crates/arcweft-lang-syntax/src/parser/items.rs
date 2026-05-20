use crate::ast::common::TextRange;
use crate::ast::items::{
    CallableItem, CapabilityFn, EntityDeclItem, EntryDeclItem, EntryItem, EntryKind, EnumItem,
    EnumVariant, ExternCapabilityItem, ExternModItem, FunctionInit, FunctionItem, ImplItem,
    ImplMember, MemoFn, ParserItem, StateField, StateItem, StructField, StructItem, TraitItem,
    TraitMember, TypeAliasItem,
};
use crate::cst::{
    find_matching_angle_group, find_top_level_punctuation, split_first_string_literal,
    split_leading_ident, split_top_level_punctuation, split_top_level_punctuation_once,
    split_top_level_punctuation_sequence_once,
};
use crate::types::{parse_fn_signature, parse_type_ref};

use super::headers::{
    parse_callable_kind, parse_contract_clause, parse_contract_expr_list, parse_entity_decl_head,
    parse_extern_mod_head, parse_function_kind_and_signature, parse_name_and_tail,
    parse_optional_angle_head, parse_required_decl_entity_ref_without_name_marker,
    parse_required_entity_ref, parse_visibility_prefix, split_function_header_lines,
    split_supertraits,
};
use super::{
    Parser, PendingDocLines, collect_logical_block_items, parse_expr_lossy, parse_scope_expr_body,
    split_brace_item, split_top_level_binding,
};

impl Parser {
    pub(super) fn parse_memo_fn(&mut self) -> Option<MemoFn> {
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

    pub(super) fn parse_parser_item(&mut self) -> Option<ParserItem> {
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

    pub(super) fn parse_function_item(&mut self) -> Option<FunctionItem> {
        let doc = self.take_pending_doc();
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_function_block();
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
        let (signature_head, contract_lines) = split_function_header_lines(&header_lines)?;
        let (visibility, signature_text) = parse_visibility_prefix(&signature_head);
        let (kind, signature_text) = parse_function_kind_and_signature(signature_text.trim());
        let signature_text = signature_text.to_owned();
        let Ok(signature) = parse_fn_signature(&signature_text) else {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "invalid function signature",
                ["fn name<'a>(...)"],
                Some(signature_head.as_str()),
                ["write the function item with a valid `fn` signature head"],
            );
            return None;
        };
        let contracts = contract_lines
            .iter()
            .filter_map(|line| parse_contract_clause(line))
            .collect();
        let (body_statements, body_value) = parse_scope_expr_body(&body);

        Some(FunctionItem::new(FunctionInit {
            doc,
            kind,
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

    pub(super) fn parse_enum_item(&mut self) -> Option<EnumItem> {
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

    pub(super) fn parse_callable_item(&mut self) -> Option<CallableItem> {
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

    pub(super) fn parse_state_item(&mut self) -> Option<StateItem> {
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

    pub(super) fn parse_trait_item(&mut self) -> Option<TraitItem> {
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
        let (name, supertraits) = split_top_level_punctuation_once(rest, ':')
            .map_or((rest, ""), |(name, traits)| (name.trim(), traits.trim()));
        Some(TraitItem::new(
            visibility,
            name.to_owned(),
            split_supertraits(supertraits),
            parse_trait_members(&body),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_impl_item(&mut self) -> Option<ImplItem> {
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
        let (maybe_trait, target) = crate::cst::split_top_level_keyword_once(rest, "for");
        let (trait_name, target) = target.map_or((None, rest.trim()), |target| {
            (Some(maybe_trait.trim().to_owned()), target.trim())
        });
        Some(ImplItem::new(
            visibility,
            generics,
            trait_name,
            target.to_owned(),
            parse_impl_members(&body),
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_struct_item(&mut self) -> Option<StructItem> {
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

    pub(super) fn parse_type_alias(&mut self) -> Option<TypeAliasItem> {
        let start_line = self.current().clone();
        let mut raw = start_line.text.clone();
        let mut end = start_line.end;
        self.index += 1;
        while self.index < self.events.len() {
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
        let (name, target) = split_top_level_binding(rest)?;
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

    pub(super) fn parse_entity_decl_item(&mut self) -> Option<EntityDeclItem> {
        if self.current().text.contains('{') || self.next_nonblank_line_is_brace() {
            self.parse_entity_decl_block()
        } else {
            self.parse_entity_decl_line()
        }
    }

    fn parse_entity_decl_block(&mut self) -> Option<EntityDeclItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_flow_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing entity declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the declaration body"],
            );
            return None;
        }
        let (kind, visibility, id, name, surface_alias, signature_tail) =
            parse_entity_decl_head(head.trim(), start_line.start, &mut self.errors)?;
        Some(EntityDeclItem::new(
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            Some(body),
            TextRange::new(start_line.start, end),
        ))
    }

    fn parse_entity_decl_line(&mut self) -> Option<EntityDeclItem> {
        let line = self.current().clone();
        self.index += 1;
        let (kind, visibility, id, name, surface_alias, signature_tail) =
            parse_entity_decl_head(line.text.trim(), line.start, &mut self.errors)?;
        Some(EntityDeclItem::new(
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            None,
            TextRange::new(line.start, line.end),
        ))
    }

    pub(super) fn parse_entry_item(&mut self) -> Option<EntryDeclItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing entry declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the entry body"],
            );
            return None;
        }
        let (kind, visibility, id) =
            parse_entry_head(head.trim(), start_line.start, &mut self.errors)?;
        Some(EntryDeclItem::new(
            kind,
            visibility,
            id,
            parse_entry_body(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_extern_capability_item(&mut self) -> Option<ExternCapabilityItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing external capability",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the capability body"],
            );
            return None;
        }
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let id = rest
            .trim_start()
            .strip_prefix("extern capability")?
            .trim()
            .to_owned();
        Some(ExternCapabilityItem::new(
            visibility,
            id,
            parse_capability_fns(&body),
            body,
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_extern_mod_item(&mut self) -> Option<ExternModItem> {
        let start_line = self.current().clone();
        let (head, body, end, ok) = self.take_brace_block();
        if !ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing external module",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the external module body"],
            );
            return None;
        }
        let (abi, path, source) = parse_extern_mod_head(head.trim())?;
        Some(ExternModItem::new(
            abi,
            path,
            source,
            body,
            TextRange::new(start_line.start, end),
        ))
    }
}

pub(super) fn parse_enum_variants(body: &str) -> Vec<EnumVariant> {
    let mut docs = PendingDocLines::default();
    body.lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            if docs.push_if_doc(line, line_index) {
                return None;
            }
            let line = line.trim_end_matches(',').trim();
            let (name, payload) = split_leading_ident(line)?;
            Some(EnumVariant::new(
                docs.take(),
                name.to_owned(),
                (!payload.is_empty()).then(|| payload.to_owned()),
            ))
        })
        .collect()
}

pub(super) fn parse_struct_fields(body: &str) -> Vec<StructField> {
    let mut docs = PendingDocLines::default();
    collect_logical_block_items(body)
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, item)| {
            let line = item.trim();
            if line.is_empty() {
                return None;
            }
            if docs.push_if_doc(line, line_index) {
                return None;
            }
            let line = line.trim_end_matches(',').trim();
            let (name, ty) = split_top_level_punctuation_once(line, ':')?;
            parse_type_ref(ty.trim())
                .ok()
                .map(|ty| StructField::new(docs.take(), name.trim().to_owned(), ty))
        })
        .collect()
}

pub(super) fn parse_state_fields(body: &str) -> Vec<StateField> {
    let mut docs = PendingDocLines::default();
    collect_logical_block_items(body)
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, item)| {
            let line = item.trim();
            if line.is_empty() {
                return None;
            }
            if docs.push_if_doc(line, line_index) {
                return None;
            }
            let line = line.trim_end_matches(',').trim();
            let (visibility, rest) = parse_visibility_prefix(line);
            let (left, default) = split_top_level_binding(rest)?;
            let (name, ty) = split_top_level_punctuation_once(left, ':')?;
            parse_type_ref(ty.trim()).ok().map(|ty| {
                StateField::new(
                    docs.take(),
                    visibility,
                    name.trim().to_owned(),
                    ty,
                    parse_expr_lossy(default.trim()),
                )
            })
        })
        .collect()
}

fn parse_entry_head(
    head: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<(
    EntryKind,
    Option<crate::ast::common::Visibility>,
    crate::ast::ids::EntityRef,
)> {
    let (visibility, rest) = parse_visibility_prefix(head);
    let rest = rest.trim_start().strip_prefix("entry")?.trim_start();
    let (kind, id_source) = if rest.starts_with('@') {
        (EntryKind::Game, rest)
    } else {
        let (kind, rest) = split_leading_ident(rest)
            .map(|(kind, rest)| (EntryKind::parse(kind), rest))
            .unwrap_or((EntryKind::Game, rest));
        (kind, rest.trim_start())
    };

    let id = if id_source.is_empty() {
        crate::ast::ids::EntityRef::new(
            format!("entry.{}", kind.as_str()),
            false,
            TextRange::new(base, base),
        )
    } else {
        parse_required_decl_entity_ref_without_name_marker(
            id_source,
            "entry",
            "entry declaration markers must include a suffix",
            base + head.len().saturating_sub(id_source.len()),
            errors,
        )?
        .0
    };
    Some((kind, visibility, id))
}

fn parse_entry_body(
    body: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<EntryItem> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|item| parse_entry_body_item(item.trim(), base, errors))
        .collect()
}

fn parse_entry_body_item(
    item: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> EntryItem {
    if let Some(rest) = item.strip_prefix("start ") {
        return parse_required_entity_ref(rest.trim(), base, errors).map_or_else(
            || EntryItem::Raw(item.to_owned()),
            |(target, _)| EntryItem::Start(target),
        );
    }
    if let Some(rest) = item.strip_prefix("run ") {
        return parse_required_entity_ref(rest.trim(), base, errors).map_or_else(
            || EntryItem::Raw(item.to_owned()),
            |(target, _)| EntryItem::Run(target),
        );
    }
    if let Some(rest) = item.strip_prefix("route ") {
        return parse_entry_route(rest, base, errors)
            .unwrap_or_else(|| EntryItem::Raw(item.to_owned()));
    }
    if let Some((name, value)) = split_top_level_binding(item) {
        return EntryItem::Option {
            name: name.trim().to_owned(),
            value: parse_expr_lossy(value.trim()),
        };
    }
    EntryItem::Raw(item.to_owned())
}

fn parse_entry_route(
    source: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<EntryItem> {
    let (left, target_source) = split_top_level_punctuation_sequence_once(source, &["-", ">"])?;
    let (method, rest) = split_leading_ident(left.trim())?;
    let (path, _) = split_first_string_literal(rest.trim())?;
    let (target, _) = parse_required_entity_ref(target_source.trim(), base, errors)?;
    Some(EntryItem::Route {
        method: method.to_owned(),
        path: path.to_owned(),
        target,
    })
}

fn parse_capability_fns(body: &str) -> Vec<CapabilityFn> {
    let mut starts = body
        .match_indices("fn ")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if starts.is_empty() {
        return Vec::new();
    }
    starts.push(body.len());
    starts
        .windows(2)
        .filter_map(|window| parse_capability_fn(body[window[0]..window[1]].trim()))
        .collect()
}

fn parse_capability_fn(item: &str) -> Option<CapabilityFn> {
    let (signature_source, effects_source) =
        crate::cst::split_top_level_keyword_once(item, "effects");
    let signature = parse_fn_signature(signature_source.trim()).ok()?;
    let effects = effects_source
        .map(parse_contract_expr_list)
        .unwrap_or_default();
    Some(CapabilityFn::new(signature, effects))
}

pub(super) fn parse_trait_members(body: &str) -> Vec<TraitMember> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .map(|item| parse_trait_member(&item))
        .collect()
}

fn parse_trait_member(item: &str) -> TraitMember {
    let item = item.trim_end_matches(';').trim();
    if let Some(rest) = item.strip_prefix("type ") {
        let (name, value) = split_top_level_binding(rest).map_or((rest, None), |(name, value)| {
            (name, parse_type_ref(value).ok())
        });
        let (name, params) = parse_associated_type_head(name.trim());
        return TraitMember::AssociatedType {
            name,
            params,
            value,
        };
    }
    if item.starts_with("fn ") {
        let signature_source = split_brace_item(item).map_or(item, |(head, _)| head);
        return parse_fn_signature(signature_source).map_or_else(
            |_| TraitMember::Raw(item.to_owned()),
            |signature| TraitMember::Function { signature },
        );
    }
    TraitMember::Raw(item.to_owned())
}

pub(super) fn parse_impl_members(body: &str) -> Vec<ImplMember> {
    collect_logical_block_items(body)
        .into_iter()
        .map(|item| parse_impl_member(item.trim()))
        .collect()
}

fn parse_impl_member(item: &str) -> ImplMember {
    let item = item.trim_end_matches(';').trim();
    if let Some(rest) = item.strip_prefix("type ") {
        if let Some((name, value)) = split_top_level_binding(rest) {
            if let Ok(value) = parse_type_ref(value) {
                let (name, params) = parse_associated_type_head(name);
                return ImplMember::AssociatedType {
                    name,
                    params,
                    value,
                };
            }
        }
        return ImplMember::Raw(item.to_owned());
    }

    // Impl function bodies are kept as source text for later expression
    // lowering, but their signatures are parsed now so type/HIR passes do not
    // need to rediscover the member boundary.
    if let Some((head, body)) = split_brace_item(item) {
        if head.starts_with("fn ") {
            return parse_fn_signature(head).map_or_else(
                |_| ImplMember::Raw(item.to_owned()),
                |signature| {
                    let (body_statements, body_value) = parse_scope_expr_body(body);
                    ImplMember::Function {
                        signature,
                        body: body.to_owned(),
                        body_statements,
                        body_value,
                    }
                },
            );
        }
    }
    if item.starts_with("fn ") {
        return parse_fn_signature(item).map_or_else(
            |_| ImplMember::Raw(item.to_owned()),
            |signature| ImplMember::Function {
                signature,
                body: String::new(),
                body_statements: Vec::new(),
                body_value: None,
            },
        );
    }
    ImplMember::Raw(item.to_owned())
}

fn parse_associated_type_head(source: &str) -> (String, Vec<String>) {
    let Some(open) = find_top_level_punctuation(source, '<') else {
        return (source.to_owned(), Vec::new());
    };
    let Some(close) = find_matching_angle_group(source, open) else {
        return (source.to_owned(), Vec::new());
    };
    let params = split_top_level_punctuation(&source[open + '<'.len_utf8()..close], ',')
        .into_iter()
        .map(str::trim)
        .filter(|param| !param.is_empty())
        .map(str::to_owned)
        .collect();
    (source[..open].trim().to_owned(), params)
}
