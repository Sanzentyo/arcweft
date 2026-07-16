use crate::ast::common::TextRange;
use crate::ast::ids::{EntityRef, EntityRefSyntax};
use crate::ast::items::{
    AgentItem, AgentItemInit, CallableItem, CallableItemInit, CapabilityFn, ContentDeclBody,
    EntityDeclBody, EntityDeclItem, EntityDeclKind, EntryDeclItem, EntryItem, EntryKind,
    EntryRouteBinding, EntryRouteBindingSource, EnumItem, EnumVariant, ExternCapabilityItem,
    ExternModActivity, ExternModFunction, ExternModItem, ExternModMember, ExternModType,
    ExternModTypeKind, FunctionInit, FunctionItem, ImageDeclBody, ImageDeclField, ImplItem,
    ImplItemInit, ImplMember, StateField, StateItem, StructField, StructItem, TraitItem,
    TraitMember, TypeAliasItem, ViewDeclBody,
};
use crate::cst::{
    ArcweftPunctuation, find_matching_angle_group, find_matching_punctuation,
    find_top_level_punctuation, split_first_string_literal, split_leading_ident,
    split_top_level_arcweft_punctuation_once, split_top_level_punctuation,
    split_top_level_punctuation_once,
};
use crate::types::{parse_fn_signature, parse_where_clause_list};

use super::headers::{
    parse_callable_kind, parse_contract_clauses, parse_contract_expr_list, parse_entity_decl_head,
    parse_extern_mod_head, parse_function_kind_and_signature, parse_name_and_tail,
    parse_optional_angle_head, parse_required_decl_entity_ref_without_name_marker,
    parse_required_entity_ref, parse_required_entity_ref_syntax, parse_visibility_prefix,
    simple_error, split_function_header_lines, split_supertraits,
};
use super::view::parse_view_body;
use super::{
    Parser, PendingDocLines, SourceDialect, collect_logical_block_items, parse_expr_lossy,
    parse_scope_authored_expr_body, parse_scope_authored_expr_body_for_dialect,
    parse_scope_authored_expr_body_with_base, parse_scope_authored_expr_body_with_base_for_dialect,
    parse_scope_expr_body, parse_type_ref_or_error, split_top_level_binding,
};

impl Parser<'_> {
    pub(super) fn parse_function_item(&mut self) -> Option<FunctionItem> {
        let attrs = self.take_pending_attrs();
        let doc = self.take_pending_doc();
        let start_line = self.current().clone();
        let block = self.take_function_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing function",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the function body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;

        let header_lines = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let (signature_head, contract_lines) = split_function_header_lines(&header_lines)?;
        let (visibility, signature_text) = parse_visibility_prefix(&signature_head);
        let (kind, signature_text) = parse_function_kind_and_signature(signature_text.trim());
        let signature_text = signature_text.to_owned();
        let signature = match parse_fn_signature(&signature_text) {
            Ok(signature) => signature,
            Err(error) => {
                self.push_error(
                    TextRange::new(start_line.start, start_line.end),
                    &error.to_string(),
                    ["fn name<'a>(...)"],
                    Some(signature_head.as_str()),
                    ["write the function item with a valid `fn` signature head"],
                );
                return None;
            }
        };
        let contracts = parse_contract_clauses(&contract_lines);
        let (body_statements, body_value) = block.body_range.as_ref().map_or_else(
            || parse_scope_authored_expr_body(body),
            |range| parse_scope_authored_expr_body_with_base(body, range.start),
        );

        Some(FunctionItem::new(FunctionInit {
            attrs,
            doc,
            kind,
            visibility,
            signature,
            signature_text,
            contracts,
            body: body.clone().into_owned(),
            body_statements,
            body_value,
            range: TextRange::new(start_line.start, block.end),
        }))
    }

    pub(super) fn parse_agent_item(&mut self) -> Option<AgentItem> {
        let attrs = self.take_pending_attrs();
        let doc = self.take_pending_doc();
        let start_line = self.current().clone();
        let block = self.take_function_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing agent",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the agent body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;

        let header_lines = head
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let (agent_head, contract_lines) = split_function_header_lines(&header_lines)?;
        let (visibility, rest) = parse_visibility_prefix(&agent_head);
        let rest = rest.trim_start().strip_prefix("agent")?.trim_start();
        let (id, rest) = if rest.starts_with('@') {
            match parse_required_entity_ref(rest, start_line.start, &mut self.errors) {
                Some((id, rest)) => (Some(id), rest.trim_start()),
                None => (None, rest),
            }
        } else {
            (None, rest)
        };
        let (explicit_name, signature_tail) = parse_name_and_tail(rest);
        let name = explicit_name
            .or_else(|| id.as_ref().map(agent_name_from_id))
            .unwrap_or_else(|| "agent".to_owned());
        let signature_text = parse_agent_signature_text(&name, &signature_tail);
        let signature = match signature_text.as_deref() {
            Some(text) => match parse_fn_signature(text) {
                Ok(signature) => Some(signature),
                Err(error) => {
                    self.push_error(
                        TextRange::new(start_line.start, start_line.end),
                        &error.to_string(),
                        ["agent @agent.id name(...)"],
                        Some(agent_head.as_str()),
                        ["write the agent item with a valid function-style signature tail"],
                    );
                    return None;
                }
            },
            None => None,
        };
        let contracts = parse_contract_clauses(&contract_lines);
        let (body_statements, body_value) = block.body_range.as_ref().map_or_else(
            || parse_scope_authored_expr_body_for_dialect(body, SourceDialect::Agent),
            |range| {
                parse_scope_authored_expr_body_with_base_for_dialect(
                    body,
                    range.start,
                    SourceDialect::Agent,
                )
            },
        );

        Some(AgentItem::new(AgentItemInit {
            attrs,
            doc,
            visibility,
            id,
            name,
            signature,
            signature_text,
            contracts,
            body: body.clone().into_owned(),
            body_statements,
            body_value,
            range: TextRange::new(start_line.start, block.end),
        }))
    }

    pub(super) fn parse_enum_item(&mut self) -> Option<EnumItem> {
        let attrs = self.take_pending_attrs();
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
            attrs,
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
        let contracts = parse_contract_clauses(&header_lines[1..]);
        let (body_statements, body_value) = parse_scope_expr_body(&body);

        Some(CallableItem::new(CallableItemInit {
            kind,
            visibility,
            name: name.unwrap_or_default(),
            signature_tail: signature_tail.clone(),
            contracts,
            body: body.clone().into_owned(),
            body_statements,
            body_value,
            range: TextRange::new(start_line.start, end),
        }))
    }

    pub(super) fn parse_state_item(&mut self) -> Option<StateItem> {
        let attrs = self.take_pending_attrs();
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
            attrs,
            visibility,
            name.unwrap_or_default(),
            parse_state_fields(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_trait_item(&mut self) -> Option<TraitItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing trait",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the trait body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let rest = rest.trim_start().strip_prefix("trait")?.trim();
        let (name, supertraits) = split_top_level_punctuation_once(rest, ':')
            .map_or((rest, ""), |(name, traits)| (name.trim(), traits.trim()));
        Some(TraitItem::new(
            attrs,
            visibility,
            name.to_owned(),
            split_supertraits(supertraits),
            parse_trait_members(
                body,
                block.body_range.as_ref().map_or(0, |range| range.start),
                &mut self.errors,
            ),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_impl_item(&mut self) -> Option<ImplItem> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing impl",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the impl body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let rest = rest.trim_start().strip_prefix("impl")?.trim();
        let (generics, rest) = parse_optional_angle_head(rest);
        let (impl_head, where_part) = crate::cst::split_top_level_keyword_once(rest, "where");
        let (maybe_trait, target) = crate::cst::split_top_level_keyword_once(impl_head, "for");
        let (trait_name, target) = target.map_or((None, impl_head.trim()), |target| {
            (Some(maybe_trait.trim().to_owned()), target.trim())
        });
        let where_clauses = where_part
            .map(parse_where_clause_list)
            .and_then(Result::ok)
            .unwrap_or_default();
        Some(ImplItem::new(ImplItemInit {
            visibility,
            generics,
            trait_name,
            target: target.to_owned(),
            where_clauses,
            members: parse_impl_members(
                body,
                block.body_range.as_ref().map_or(0, |range| range.start),
                &mut self.errors,
            ),
            body: body.to_string(),
            range: TextRange::new(start_line.start, block.end),
        }))
    }

    pub(super) fn parse_struct_item(&mut self) -> Option<StructItem> {
        let attrs = self.take_pending_attrs();
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
            attrs,
            visibility,
            name.unwrap_or_default(),
            parse_struct_fields(&body, start_line.start, &mut self.errors),
            TextRange::new(start_line.start, end),
        ))
    }

    pub(super) fn parse_type_alias(&mut self) -> Option<TypeAliasItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let mut raw = start_line.text().to_owned();
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
        let target_source = target.trim();
        let target_base = start_line.start + raw.find(target_source).unwrap_or_default();
        let target = parse_type_ref_or_error(target_source, target_base, &mut self.errors)?;
        let where_clauses = lines
            .filter_map(|line| line.strip_prefix("where "))
            .map(str::trim)
            .map(parse_expr_lossy)
            .collect();

        Some(TypeAliasItem::new(
            attrs,
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
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let block = self.take_flow_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing entity declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the declaration body"],
            );
            return None;
        }
        let head = block.head;
        let body = block.body;
        let body_range = block
            .body_range
            .as_ref()
            .map(|range| TextRange::new(range.start, range.end));
        let body_base = block
            .body_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        let (kind, visibility, id, name, surface_alias, signature_tail) = parse_entity_decl_head(
            head.trim(),
            start_line.start,
            self.current_module_path.as_deref(),
            &mut self.errors,
        )?;
        let structured_body = parse_structured_entity_decl_body(
            kind,
            &StructuredEntityBodyContext {
                signature_tail: &signature_tail,
                body: &body,
                base: start_line.start,
                body_base,
                module_path: self.current_module_path.as_deref(),
                document: self.document,
            },
            &mut self.errors,
        );
        let raw_body = structured_body.is_none().then(|| body.into_owned());
        Some(EntityDeclItem::new(
            attrs,
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            raw_body,
            structured_body,
            body_range,
            TextRange::new(start_line.start, block.end),
        ))
    }

    fn parse_entity_decl_line(&mut self) -> Option<EntityDeclItem> {
        let attrs = self.take_pending_attrs();
        let line = self.current().clone();
        self.index += 1;
        let (kind, visibility, id, name, surface_alias, signature_tail) = parse_entity_decl_head(
            line.text.trim(),
            line.start,
            self.current_module_path.as_deref(),
            &mut self.errors,
        )?;
        Some(EntityDeclItem::new(
            attrs,
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            None,
            None,
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
        let attrs = self.take_pending_attrs();
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
            attrs,
            visibility,
            id,
            parse_capability_fns(&body, start_line.start, &mut self.errors),
            body.into_owned(),
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
            parse_extern_mod_members(&body, start_line.start, &mut self.errors),
            body.into_owned(),
            TextRange::new(start_line.start, end),
        ))
    }
}

pub(super) fn parse_enum_variants(body: &str) -> Vec<EnumVariant> {
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
            let (name, payload) = split_leading_ident(line)?;
            Some(EnumVariant::new(
                docs.take(),
                name.to_owned(),
                (!payload.is_empty()).then(|| payload.to_owned()),
            ))
        })
        .collect()
}

pub(super) fn parse_struct_fields(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<StructField> {
    let mut docs = PendingDocLines::default();
    let mut fields = Vec::new();
    for (line_index, item) in super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .enumerate()
    {
        let item_source = item.source.trim();
        if item_source.is_empty() {
            continue;
        }
        if docs.push_if_doc(item_source, line_index) {
            continue;
        }
        let parts = split_top_level_punctuation(item_source, ',');
        for (part_index, part) in parts.into_iter().enumerate() {
            let line = part.trim();
            if line.is_empty() {
                continue;
            }
            let line = part.trim().trim_end_matches(',').trim();
            let Some((name, ty)) = split_top_level_punctuation_once(line, ':') else {
                continue;
            };
            let doc = if part_index == 0 { docs.take() } else { None };
            let ty_source = ty.trim();
            let ty_base = item.base + item.source.find(ty_source).unwrap_or_default();
            if let Some(ty) = parse_type_ref_or_error(ty_source, ty_base, errors) {
                fields.push(StructField::new(doc, name.trim().to_owned(), ty));
            }
        }
    }
    fields
}

pub(super) fn parse_state_fields(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<StateField> {
    let mut docs = PendingDocLines::default();
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, item)| {
            let line = item.source.trim();
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
            let ty_source = ty.trim();
            let ty_base = item.base + item.source.find(ty_source).unwrap_or_default();
            parse_type_ref_or_error(ty_source, ty_base, errors).map(|ty| {
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

struct StructuredEntityBodyContext<'a> {
    signature_tail: &'a str,
    body: &'a str,
    base: usize,
    body_base: usize,
    module_path: Option<&'a str>,
    document: Option<&'a arcweft_source::SourceDocument>,
}

fn parse_structured_entity_decl_body(
    kind: EntityDeclKind,
    context: &StructuredEntityBodyContext<'_>,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<EntityDeclBody> {
    match kind {
        EntityDeclKind::Content => Some(EntityDeclBody::Content(ContentDeclBody::new(
            parse_content_roots_field(context.body, context.base, errors),
        ))),
        EntityDeclKind::Image => Some(EntityDeclBody::Image(ImageDeclBody::new(
            parse_image_decl_fields(context.body, context.base, errors),
        ))),
        EntityDeclKind::View => {
            if entity_signature_has_return_type(context.signature_tail) {
                errors.push(simple_error(
                    context.base,
                    context.signature_tail.len(),
                    "invalid view declaration signature",
                    "view Name(...) { ... }",
                ));
            }
            Some(EntityDeclBody::View(Box::new(ViewDeclBody::new(
                parse_view_body(
                    context.body,
                    context.body_base,
                    context.module_path,
                    context.document,
                    errors,
                ),
            ))))
        }
        _ => None,
    }
}

fn entity_signature_has_return_type(signature_tail: &str) -> bool {
    split_top_level_arcweft_punctuation_once(signature_tail, ArcweftPunctuation::ThinArrow)
        .is_some()
}

fn parse_image_decl_fields(
    body: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<ImageDeclField> {
    collect_logical_block_items(body)
        .into_iter()
        .filter_map(|item| parse_image_decl_field(item.trim(), base, errors))
        .collect()
}

fn parse_image_decl_field(
    line: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<ImageDeclField> {
    if line.is_empty() {
        return None;
    }
    let line = line.trim_end_matches(',').trim();
    let Some((name, value)) = split_top_level_binding(line) else {
        errors.push(simple_error(
            base,
            line.len(),
            "image declaration body item must be a field assignment",
            "asset = @asset:.id",
        ));
        return None;
    };
    let name = name.trim();
    if name.is_empty() {
        errors.push(simple_error(
            base,
            line.len(),
            "image declaration field name cannot be empty",
            "field = value",
        ));
        return None;
    }
    let value_source = value.trim().to_owned();
    Some(ImageDeclField::new(
        name.to_owned(),
        value_source.clone(),
        parse_expr_lossy(&value_source),
    ))
}

fn parse_content_roots_field(
    body: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<EntityRef> {
    let mut roots = Vec::new();
    let mut found_roots = false;
    for item in collect_logical_block_items(body) {
        let line = item.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = split_top_level_binding(line) else {
            errors.push(simple_error(
                base,
                line.len(),
                "content declaration body item must be a field assignment",
                "roots = [@flow.id]",
            ));
            continue;
        };
        if name.trim() != "roots" {
            errors.push(simple_error(
                base,
                name.len(),
                "unsupported content declaration field",
                "roots = [@flow.id]",
            ));
            continue;
        }
        found_roots = true;
        let value = value.trim();
        let Some(list_body) = value
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            errors.push(simple_error(
                base,
                value.len(),
                "content roots must be an entity reference list",
                "roots = [@flow.id]",
            ));
            continue;
        };
        roots.extend(parse_entity_ref_list(list_body, base, errors));
    }
    if !found_roots {
        errors.push(simple_error(
            base,
            body.len().max(1),
            "content declaration requires roots",
            "roots = [@flow.id]",
        ));
    }
    roots
}

fn parse_entity_ref_list(
    body: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<EntityRef> {
    collect_logical_block_items(body)
        .into_iter()
        .flat_map(|item| {
            split_top_level_punctuation(item.trim(), ',')
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }
            parse_required_entity_ref_syntax(trimmed, base, errors).and_then(|(entity, rest)| {
                if !rest.trim().is_empty() {
                    errors.push(simple_error(
                        base,
                        trimmed.len(),
                        "entity list item has unsupported trailing syntax",
                        "@entity.id",
                    ));
                    return None;
                }
                normalize_top_level_content_root(entity, base, trimmed.len(), errors)
            })
        })
        .collect()
}

fn normalize_top_level_content_root(
    entity: EntityRefSyntax,
    base: usize,
    len: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<EntityRef> {
    match entity {
        EntityRefSyntax::Absolute(entity) => Some(entity),
        EntityRefSyntax::FamilyRelative(relative) => {
            if relative.relative().parent_depth() != 0 {
                errors.push(simple_error(
                    base,
                    len,
                    "content root references cannot use parent-relative family syntax",
                    "@asset:.id",
                ));
                return None;
            }
            Some(EntityRef::new(
                format!("{}.{}", relative.family(), relative.relative().suffix()),
                false,
                *relative.range(),
            ))
        }
    }
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
    if let Some(target) = parse_entry_target(item, "goto", base, errors) {
        return EntryItem::Goto(target);
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

fn parse_entry_target(
    item: &str,
    name: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<crate::ast::ids::EntityRef> {
    let rest = item.strip_prefix(name)?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| !ch.is_whitespace() && ch != '(')
    {
        return None;
    }
    let rest = rest.trim_start();
    let (target_source, trailing) = if let Some(args) = rest.strip_prefix('(') {
        (args.trim_end().strip_suffix(')')?.trim(), "")
    } else {
        rest.split_once(char::is_whitespace).unwrap_or((rest, ""))
    };
    let (target, rest) = parse_required_entity_ref(target_source, base, errors)?;
    if !rest.trim().is_empty() || !trailing.trim().is_empty() {
        return None;
    }
    Some(target)
}

fn parse_entry_route(
    source: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<EntryItem> {
    let (left, target_source) =
        split_top_level_arcweft_punctuation_once(source, ArcweftPunctuation::ThinArrow)?;
    let (method, rest) = split_leading_ident(left.trim())?;
    let (path, _) = split_first_string_literal(rest.trim())?;
    let (target, bindings) = parse_entry_route_target(target_source.trim(), base, errors)?;
    Some(EntryItem::Route {
        method: method.to_owned(),
        path: path.to_owned(),
        target,
        bindings,
    })
}

fn parse_entry_route_target(
    source: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<(crate::ast::ids::EntityRef, Vec<EntryRouteBinding>)> {
    let (target, rest) = parse_required_entity_ref(source, base, errors)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return Some((target, Vec::new()));
    }
    if !rest.starts_with('(') {
        errors.push(simple_error(
            base,
            rest.len(),
            "unexpected route target suffix",
            "(name = :path_param)",
        ));
        return None;
    }
    let Some(close) = find_matching_punctuation(rest, 0, '(', ')') else {
        errors.push(simple_error(
            base,
            rest.len(),
            "unclosed route target argument list",
            ")",
        ));
        return None;
    };
    if !rest[close + ')'.len_utf8()..].trim().is_empty() {
        errors.push(simple_error(
            base + close,
            rest.len() - close,
            "unexpected route target suffix",
            "end of route target",
        ));
        return None;
    }
    let bindings = split_top_level_punctuation(&rest['('.len_utf8()..close], ',')
        .into_iter()
        .filter_map(|binding| parse_entry_route_binding(binding.trim(), base, errors))
        .collect();
    Some((target, bindings))
}

fn parse_entry_route_binding(
    source: &str,
    base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<EntryRouteBinding> {
    if source.is_empty() {
        return None;
    }
    let Some((name, value)) = split_top_level_punctuation_once(source, '=') else {
        errors.push(simple_error(
            base,
            source.len(),
            "expected route argument binding",
            "name = :path_param",
        ));
        return None;
    };
    let (name, name_rest) = split_leading_ident(name.trim())?;
    if !name_rest.trim().is_empty() {
        errors.push(simple_error(
            base,
            source.len(),
            "invalid route argument name",
            "identifier",
        ));
        return None;
    }
    let value = value.trim();
    let Some(param) = value.strip_prefix(':') else {
        errors.push(simple_error(
            base,
            source.len(),
            "route arguments currently bind path parameters explicitly",
            ":path_param",
        ));
        return None;
    };
    let (param, param_rest) = split_leading_ident(param.trim())?;
    if !param_rest.trim().is_empty() {
        errors.push(simple_error(
            base,
            source.len(),
            "invalid route path parameter reference",
            ":path_param",
        ));
        return None;
    }
    Some(EntryRouteBinding::new(
        name,
        EntryRouteBindingSource::path_param(param),
    ))
}

fn parse_capability_fns(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<CapabilityFn> {
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
        .filter_map(|window| {
            parse_capability_fn(
                body[window[0]..window[1]].trim(),
                body_base + window[0],
                errors,
            )
        })
        .collect()
}

fn parse_capability_fn(
    item: &str,
    item_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Option<CapabilityFn> {
    let (signature_source, effects_source) =
        crate::cst::split_top_level_keyword_once(item, "effects");
    let signature_source = signature_source.trim();
    let signature = match parse_fn_signature(signature_source) {
        Ok(signature) => signature,
        Err(error) => {
            errors.push(simple_error(
                item_base,
                signature_source.len(),
                &error.to_string(),
                "a valid capability function signature",
            ));
            return None;
        }
    };
    let effects = effects_source
        .map(parse_contract_expr_list)
        .unwrap_or_default();
    Some(CapabilityFn::new(signature, effects))
}

fn parse_extern_mod_members(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<ExternModMember> {
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .map(|item| parse_extern_mod_member(item.source.trim(), item.base, errors))
        .collect()
}

fn parse_extern_mod_member(
    item: &str,
    item_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> ExternModMember {
    let item = item.trim_end_matches(';').trim();
    let (visibility, rest) = parse_visibility_prefix(item);
    let rest = rest.trim();
    if let Some(name) = rest.strip_prefix("type ").map(str::trim)
        && let Some((name, tail)) = split_leading_ident(name)
        && tail.trim().is_empty()
    {
        return ExternModMember::Type(ExternModType::new(
            visibility,
            ExternModTypeKind::Type,
            name,
        ));
    }
    if let Some(name) = rest.strip_prefix("event ").map(str::trim)
        && let Some((name, tail)) = split_leading_ident(name)
        && tail.trim().is_empty()
    {
        return ExternModMember::Type(ExternModType::new(
            visibility,
            ExternModTypeKind::Event,
            name,
        ));
    }
    if rest.starts_with("fn ") {
        return parse_fn_signature(rest).map_or_else(
            |error| {
                errors.push(simple_error(
                    item_base,
                    rest.len(),
                    &error.to_string(),
                    "a valid external function signature",
                ));
                ExternModMember::Raw(item.to_owned())
            },
            |signature| ExternModMember::Function(ExternModFunction::new(visibility, signature)),
        );
    }
    if let Some(activity) = rest.strip_prefix("activity ").map(str::trim)
        && let Some((name, ty)) = split_top_level_punctuation_once(activity, ':')
        && let Some((name, tail)) = split_leading_ident(name.trim())
        && tail.trim().is_empty()
    {
        let ty_source = ty.trim();
        let ty_base = item_base + item.find(ty_source).unwrap_or_default();
        if let Some(ty) = parse_type_ref_or_error(ty_source, ty_base, errors) {
            return ExternModMember::Activity(ExternModActivity::new(visibility, name, ty));
        }
    }
    ExternModMember::Raw(item.to_owned())
}

pub(super) fn parse_trait_members(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<TraitMember> {
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter(|item| !item.source.trim().is_empty())
        .map(|item| parse_trait_member(item.source.trim(), item.base, errors))
        .collect()
}

fn parse_trait_member(
    item: &str,
    item_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> TraitMember {
    let item = item.trim_end_matches(';').trim();
    if let Some(rest) = item.strip_prefix("type ") {
        let (name, value) = split_top_level_binding(rest).map_or((rest, None), |(name, value)| {
            let value_source = value.trim();
            let value_base = item_base + item.find(value_source).unwrap_or_default();
            (
                name,
                parse_type_ref_or_error(value_source, value_base, errors),
            )
        });
        let (name, params) = parse_associated_type_head(name.trim());
        return TraitMember::AssociatedType {
            name,
            params,
            value,
        };
    }
    if item.starts_with("fn ") {
        let (signature_source, body) = split_brace_item_with_body_base(item, item_base)
            .map_or((item, None), |(head, body, body_base)| {
                (head, Some((body, body_base)))
            });
        return parse_fn_signature(signature_source).map_or_else(
            |error| {
                errors.push(simple_error(
                    item_base,
                    signature_source.len(),
                    &error.to_string(),
                    "a valid trait function signature",
                ));
                TraitMember::Raw(item.to_owned())
            },
            |signature| {
                let (body, body_statements, body_value) = body.map_or_else(
                    || (None, Vec::new(), None),
                    |(body, body_base)| {
                        let (body_statements, body_value) =
                            parse_scope_authored_expr_body_with_base(body, body_base);
                        (Some(body.to_owned()), body_statements, body_value)
                    },
                );
                TraitMember::Function {
                    signature,
                    body,
                    body_statements,
                    body_value: body_value.map(Box::new),
                }
            },
        );
    }
    TraitMember::Raw(item.to_owned())
}

pub(super) fn parse_impl_members(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<ImplMember> {
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .map(|item| parse_impl_member(item.source.trim(), item.base, errors))
        .collect()
}

fn parse_impl_member(
    item: &str,
    item_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> ImplMember {
    let item = item.trim_end_matches(';').trim();
    if let Some(rest) = item.strip_prefix("type ") {
        if let Some((name, value)) = split_top_level_binding(rest) {
            let value_source = value.trim();
            let value_base = item_base + item.find(value_source).unwrap_or_default();
            let Some(value) = parse_type_ref_or_error(value_source, value_base, errors) else {
                return ImplMember::Raw(item.to_owned());
            };
            let (name, params) = parse_associated_type_head(name);
            return ImplMember::AssociatedType {
                name,
                params,
                value,
            };
        }
        return ImplMember::Raw(item.to_owned());
    }

    // Impl function bodies are kept as source text for later expression
    // lowering, but their signatures are parsed now so type/HIR passes do not
    // need to rediscover the member boundary.
    if let Some((head, body, body_base)) = split_brace_item_with_body_base(item, item_base)
        && head.starts_with("fn ")
    {
        return parse_fn_signature(head).map_or_else(
            |error| {
                errors.push(simple_error(
                    item_base,
                    head.len(),
                    &error.to_string(),
                    "a valid impl function signature",
                ));
                ImplMember::Raw(item.to_owned())
            },
            |signature| {
                let (body_statements, body_value) =
                    parse_scope_authored_expr_body_with_base(body, body_base);
                ImplMember::Function {
                    signature,
                    body: body.to_owned(),
                    body_statements,
                    body_value: body_value.map(Box::new),
                }
            },
        );
    }
    if item.starts_with("fn ") {
        return parse_fn_signature(item).map_or_else(
            |error| {
                errors.push(simple_error(
                    item_base,
                    item.len(),
                    &error.to_string(),
                    "a valid impl function signature",
                ));
                ImplMember::Raw(item.to_owned())
            },
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

fn split_brace_item_with_body_base(
    source: &str,
    source_base: usize,
) -> Option<(&str, &str, usize)> {
    let open = find_top_level_punctuation(source, '{')?;
    let close = find_matching_punctuation(source, open, '{', '}')?;
    Some((
        source[..open].trim_end(),
        &source[open + '{'.len_utf8()..close],
        source_base + open + '{'.len_utf8(),
    ))
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

fn parse_agent_signature_text(name: &str, signature_tail: &str) -> Option<String> {
    let tail = signature_tail.trim();
    (tail.starts_with('(') || tail.starts_with('<')).then(|| format!("fn {name}{tail}"))
}

fn agent_name_from_id(id: &EntityRef) -> String {
    id.body()
        .rsplit('.')
        .next()
        .filter(|suffix| !suffix.is_empty())
        .unwrap_or("agent")
        .to_owned()
}
