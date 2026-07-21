use crate::ast::common::TextRange;
use crate::ast::ids::{EntityRef, EntityRefSyntax};
use crate::ast::items::{
    CapabilityFn, ContentDeclBody, EntityDeclBody, EntityDeclItem, EntityDeclKind, EntryDeclItem,
    EntryItem, EntryKind, EntryRoleKind, EntryRouteBinding, EntryRouteBindingSource, EnumItem,
    EnumItemInit, EnumVariant, ExternCapabilityItem, ExternModActivity, ExternModFunction,
    ExternModItem, ExternModMember, ExternModType, ExternModTypeKind, FunctionInit, FunctionItem,
    FunctionParameterSource, FunctionSignatureSource, ImageDeclBody, ImageDeclField, ImplItem,
    ImplItemInit, ImplMember, StructField, StructItem, StructItemInit, TraitItem, TraitMember,
    TypeAliasItem, TypeAliasItemInit, ViewDeclBody,
};
use crate::cst::{
    ArcweftPunctuation, find_matching_angle_group, find_matching_punctuation,
    find_top_level_punctuation, is_identifier, split_first_string_literal, split_leading_ident,
    split_top_level_arcweft_punctuation_once, split_top_level_keyword_once,
    split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::expr::DottedPath;
use crate::types::{parse_fn_signature_at, parse_generic_params_at, parse_where_clauses_at};
use std::collections::BTreeMap;

use super::headers::{
    parse_contract_clauses, parse_contract_expr_list, parse_entity_decl_head,
    parse_extern_mod_head, parse_function_kind_and_signature, parse_optional_angle_head,
    parse_required_decl_entity_ref_without_name_marker, parse_required_entity_ref,
    parse_required_entity_ref_syntax, parse_visibility_prefix, simple_error,
    split_function_header_lines,
};
use super::recovery::{ParseError, ParseErrorKind, RecoverySuggestion};
use super::view::parse_view_body;
use super::{
    Parser, PendingDocLines, collect_logical_block_items, collect_logical_block_items_with_base,
    parse_expr_lossy, parse_owned_expr_recovering, parse_scope_authored_expr_body,
    parse_scope_authored_expr_body_recovering_with_base, parse_type_ref_or_error,
    split_top_level_binding,
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
        let head_range = block.head_range.as_ref()?;
        let head_source = self.source.get(head_range.clone())?;
        let signature_base = head_range.start + find_fn_token(head_source)?;
        let signature = match parse_fn_signature_at(&signature_text, signature_base) {
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
        let signature_source = function_signature_source(
            head_source,
            head_range.start,
            contract_lines.first().copied(),
            &signature,
        )?;
        let (body_statements, body_value) = match block.body_range.as_ref() {
            Some(range) => parse_scope_authored_expr_body_recovering_with_base(
                body,
                range.start,
                &mut self.errors,
            ),
            None => parse_scope_authored_expr_body(body),
        };

        Some(FunctionItem::new(FunctionInit {
            attrs,
            doc,
            kind,
            visibility,
            signature,
            signature_text,
            signature_source,
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
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing enum",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the enum body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let end = block.end;
        let body_base = block
            .body_range
            .as_ref()
            .map_or(start_line.end, |range| range.start);
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let declaration = rest.trim_start().strip_prefix("enum")?.trim();
        let declaration_base = start_line.start + subslice_offset(head, declaration)?;
        let (name, tail) = split_leading_ident(declaration)?;
        let name_start = declaration_base + subslice_offset(declaration, name)?;
        let tail = tail.trim();
        let (generic_source, trailing) = parse_optional_angle_head(tail);
        let generic_base = generic_source.map_or(name_start + name.len(), |generic| {
            declaration_base
                + subslice_offset(declaration, generic)
                    .expect("enum generic group remains in the declaration source")
        });
        let generic_params =
            parse_nominal_generic_params(generic_source, generic_base, &mut self.errors)?;
        let generic_range = generic_source
            .map(|generic| TextRange::new(generic_base, generic_base + generic.len()));
        let where_clauses = parse_nominal_where_tail(
            trailing,
            declaration_base + subslice_offset(declaration, trailing)?,
            "enum",
            &mut self.errors,
        )?;
        Some(EnumItem::new(EnumItemInit {
            attrs,
            visibility,
            name: name.to_owned(),
            name_range: TextRange::new(name_start, name_start + name.len()),
            generic_params,
            generic_range,
            where_clauses,
            variants: parse_enum_variants(body, body_base, &mut self.errors),
            range: TextRange::new(start_line.start, end),
        }))
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
        let head_base = block
            .head_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        let mut parsed_supertraits = Vec::new();
        for supertrait in split_top_level_punctuation(supertraits, '+')
            .into_iter()
            .map(str::trim)
            .filter(|supertrait| !supertrait.is_empty())
        {
            parsed_supertraits.push(parse_type_ref_or_error(
                supertrait,
                head_base + subslice_offset(head, supertrait)?,
                &mut self.errors,
            ));
        }
        Some(TraitItem::new(
            attrs,
            visibility,
            name.to_owned(),
            parsed_supertraits,
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
        let head_base = block
            .head_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let rest = rest.trim_start().strip_prefix("impl")?.trim();
        let (generic_source, rest) = parse_optional_angle_head(rest);
        let generic_base = generic_source.map_or(head_base, |generics| {
            head_base
                + subslice_offset(head, generics)
                    .expect("impl generics remain in the declaration source")
        });
        let generics =
            parse_nominal_generic_params(generic_source, generic_base, &mut self.errors)?;
        let (impl_head, where_part) = crate::cst::split_top_level_keyword_once(rest, "where");
        let (maybe_trait, target) = crate::cst::split_top_level_keyword_once(impl_head, "for");
        let (trait_source, target_source) = target.map_or((None, impl_head.trim()), |target| {
            (Some(maybe_trait.trim()), target.trim())
        });
        let trait_ref = trait_source.map(|source| {
            parse_type_ref_or_error(
                source,
                head_base
                    + subslice_offset(head, source)
                        .expect("impl trait remains in the declaration source"),
                &mut self.errors,
            )
        });
        let target = parse_type_ref_or_error(
            target_source,
            head_base
                + subslice_offset(head, target_source)
                    .expect("impl target remains in the declaration source"),
            &mut self.errors,
        );
        let where_clauses = if let Some(where_source) = where_part {
            let where_source = where_source.trim();
            let where_base = head_base
                + subslice_offset(head, where_source)
                    .expect("impl predicate remains in the declaration source");
            match parse_where_clauses_at(where_source, where_base) {
                Ok(clauses) => clauses,
                Err(error) => {
                    self.push_error(
                        TextRange::new(where_base, where_base + where_source.len()),
                        &format!("invalid impl where clause: {error}"),
                        ["Type: Bound"],
                        Some(where_source),
                        ["write typed where predicates"],
                    );
                    return None;
                }
            }
        } else {
            Vec::new()
        };
        Some(ImplItem::new(ImplItemInit {
            visibility,
            generics,
            trait_ref,
            target,
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
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing struct",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the struct body"],
            );
            return None;
        }
        let head = &block.head;
        let body = &block.body;
        let end = block.end;
        let body_base = block
            .body_range
            .as_ref()
            .map_or(start_line.end, |range| range.start);
        let (visibility, rest) = parse_visibility_prefix(head.trim());
        let declaration = rest.trim_start().strip_prefix("struct")?.trim();
        let declaration_base = start_line.start + subslice_offset(head, declaration)?;
        let (name, tail) = split_leading_ident(declaration)?;
        let name_start = declaration_base + subslice_offset(declaration, name)?;
        let tail = tail.trim();
        let (generic_source, trailing) = parse_optional_angle_head(tail);
        let generic_base = generic_source.map_or(name_start + name.len(), |generic| {
            declaration_base
                + subslice_offset(declaration, generic)
                    .expect("struct generic group remains in the declaration source")
        });
        let generic_params =
            parse_nominal_generic_params(generic_source, generic_base, &mut self.errors)?;
        let generic_range = generic_source
            .map(|generic| TextRange::new(generic_base, generic_base + generic.len()));
        let where_clauses = parse_nominal_where_tail(
            trailing,
            declaration_base + subslice_offset(declaration, trailing)?,
            "struct",
            &mut self.errors,
        )?;
        Some(StructItem::new(StructItemInit {
            attrs,
            visibility,
            name: name.to_owned(),
            name_range: TextRange::new(name_start, name_start + name.len()),
            generic_params,
            generic_range,
            where_clauses,
            fields: parse_struct_fields(body, body_base, &mut self.errors),
            range: TextRange::new(start_line.start, end),
        }))
    }

    pub(super) fn parse_type_alias(&mut self) -> Option<TypeAliasItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let mut where_lines = Vec::new();
        let mut end = start_line.end;
        self.index += 1;
        while self.index < self.events.len() {
            let line = self.current();
            let trimmed = line.text.trim();
            if !trimmed.starts_with("where ") {
                break;
            }
            where_lines.push((line.text().to_owned(), line.start));
            end = line.end;
            self.index += 1;
        }

        let first_source = start_line.text();
        let first = first_source.trim();
        let first_base = start_line.start + subslice_offset(first_source, first)?;
        let (visibility, rest) = parse_visibility_prefix(first);
        let rest = rest.trim_start().strip_prefix("type")?.trim();
        let (name_source, target) = split_top_level_binding(rest)?;
        let name_source = name_source.trim();
        let (name, tail) = split_leading_ident(name_source)?;
        let declaration_base = first_base + subslice_offset(first, rest)?;
        let name_start = declaration_base + subslice_offset(rest, name)?;
        let tail = tail.trim();
        let (generic_source, trailing) = parse_optional_angle_head(tail);
        if !trailing.is_empty() {
            self.push_error(
                TextRange::new(first_base, first_base + first.len()),
                "unexpected tokens after type alias generic parameters",
                ["="],
                Some(trailing),
                ["remove the trailing declaration text"],
            );
            return None;
        }
        let generic_base = generic_source.map_or(name_start + name.len(), |generic| {
            declaration_base
                + subslice_offset(rest, generic)
                    .expect("alias generic group remains in the declaration source")
        });
        let generic_params =
            parse_nominal_generic_params(generic_source, generic_base, &mut self.errors)?;
        let generic_range = generic_source
            .map(|generic| TextRange::new(generic_base, generic_base + generic.len()));
        let target_source = target.trim();
        let target_base = first_base + subslice_offset(first, target_source)?;
        let target = parse_type_ref_or_error(target_source, target_base, &mut self.errors);
        let mut where_clauses = Vec::new();
        for (line_source, line_base) in where_lines {
            let trimmed = line_source.trim();
            let predicates = trimmed.strip_prefix("where ")?.trim();
            let predicates_base = line_base + subslice_offset(&line_source, predicates)?;
            match parse_where_clauses_at(predicates, predicates_base) {
                Ok(mut clauses) => where_clauses.append(&mut clauses),
                Err(error) => {
                    self.push_error(
                        TextRange::new(predicates_base, predicates_base + predicates.len()),
                        &format!("invalid type alias where clause: {error}"),
                        ["Type: Bound"],
                        Some(predicates),
                        ["write typed where predicates"],
                    );
                    return None;
                }
            }
        }

        Some(TypeAliasItem::new(TypeAliasItemInit {
            attrs,
            visibility,
            name: name.to_owned(),
            name_range: TextRange::new(name_start, name_start + name.len()),
            generic_params,
            generic_range,
            target,
            where_clauses,
            range: TextRange::new(start_line.start, end),
        }))
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
        let signature_name = name.as_deref().unwrap_or("view");
        let head_base = block
            .head_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        let signature_name_base = head_base + head.find(signature_name).unwrap_or_default();
        let structured_body = parse_structured_entity_decl_body(
            kind,
            &StructuredEntityBodyContext {
                signature_name,
                signature_name_base,
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
        let signature_name = name.as_deref().unwrap_or("view");
        let signature_name_base = line.start + line.text.find(signature_name).unwrap_or_default();
        let structured_body = (kind == EntityDeclKind::View)
            .then(|| {
                parse_structured_entity_decl_body(
                    kind,
                    &StructuredEntityBodyContext {
                        signature_name,
                        signature_name_base,
                        signature_tail: &signature_tail,
                        body: "",
                        base: line.start,
                        body_base: line.end,
                        module_path: self.current_module_path.as_deref(),
                        document: self.document,
                    },
                    &mut self.errors,
                )
            })
            .flatten();
        Some(EntityDeclItem::new(
            attrs,
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            None,
            structured_body,
            None,
            TextRange::new(line.start, line.end),
        ))
    }

    pub(super) fn parse_entry_item(&mut self) -> Option<EntryDeclItem> {
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing entry declaration",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the entry body"],
            );
            return None;
        }
        let head = block.head.trim();
        let head_base = block.head_range.as_ref().map_or_else(
            || start_line.start + start_line.text.find(head).unwrap_or_default(),
            |range| {
                range.start
                    + self
                        .source
                        .get(range.clone())
                        .and_then(|source| source.find(head))
                        .unwrap_or_default()
            },
        );
        let (kind, visibility, id) = parse_entry_head(head, head_base, &mut self.errors)?;
        let body_base = block
            .body_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        Some(EntryDeclItem::new(
            kind.clone(),
            visibility,
            id,
            parse_entry_body(&kind, &block.body, body_base, &mut self.errors),
            TextRange::new(start_line.start, block.end),
        ))
    }

    pub(super) fn parse_extern_capability_item(&mut self) -> Option<ExternCapabilityItem> {
        let attrs = self.take_pending_attrs();
        let start_line = self.current().clone();
        let block = self.take_brace_block_event();
        if !block.ok {
            self.push_error(
                TextRange::new(start_line.start, start_line.end),
                "unclosed block while parsing external capability",
                ["}"],
                Some(start_line.text.trim()),
                ["insert a closing `}` for the capability body"],
            );
            return None;
        }
        let body_base = block
            .body_range
            .as_ref()
            .map_or(start_line.start, |range| range.start);
        let (visibility, rest) = parse_visibility_prefix(block.head.trim());
        let id = rest
            .trim_start()
            .strip_prefix("extern capability")?
            .trim()
            .to_owned();
        Some(ExternCapabilityItem::new(
            attrs,
            visibility,
            id,
            parse_capability_fns(&block.body, body_base, &mut self.errors),
            block.body.into_owned(),
            TextRange::new(start_line.start, block.end),
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

pub(super) fn parse_enum_variants(
    body: &str,
    body_base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<EnumVariant> {
    let mut docs = PendingDocLines::default();
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, item)| {
            let item_source = item.source.trim();
            let item_base = item.base + subslice_offset(&item.source, item_source)?;
            let line = item_source;
            if line.is_empty() {
                return None;
            }
            if docs.push_if_doc(line, line_index) {
                return None;
            }
            let line = line.trim_end_matches(',').trim();
            let (name, payload) = split_leading_ident(line)?;
            let name_start = item_base + subslice_offset(item_source, name)?;
            let payload_source = payload.trim();
            let payload_range = (!payload_source.is_empty()).then(|| {
                let start = item_base
                    + subslice_offset(item_source, payload_source)
                        .expect("enum payload remains a source subslice");
                TextRange::new(start, start + payload_source.len())
            });
            let payload = payload_range
                .map(|range| parse_type_ref_or_error(payload_source, range.start(), errors));
            Some(EnumVariant::new(
                docs.take(),
                name.to_owned(),
                payload,
                TextRange::new(name_start, name_start + name.len()),
                payload_range,
                TextRange::new(item_base, item_base + line.len()),
            ))
        })
        .collect()
}

pub(super) fn parse_struct_fields(
    body: &str,
    body_base: usize,
    errors: &mut Vec<ParseError>,
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
            let part_base = item.base
                + subslice_offset(&item.source, part)
                    .expect("struct field fragment remains a source subslice");
            let line_base = part_base
                + subslice_offset(part, line).expect("trimmed field remains a source subslice");
            let name = name.trim();
            let name_start = line_base
                + subslice_offset(line, name).expect("field name remains a source subslice");
            let ty_source = ty.trim();
            let ty_base = line_base
                + subslice_offset(line, ty_source).expect("field type remains a source subslice");
            let ty = parse_type_ref_or_error(ty_source, ty_base, errors);
            fields.push(StructField::new(
                doc,
                name.to_owned(),
                ty,
                TextRange::new(name_start, name_start + name.len()),
                TextRange::new(line_base, line_base + line.len()),
            ));
        }
    }
    fields
}

struct StructuredEntityBodyContext<'a> {
    signature_name: &'a str,
    signature_name_base: usize,
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
            parse_image_decl_fields(context.body, context.body_base, errors),
        ))),
        EntityDeclKind::View => {
            let signature_source =
                format!("fn {}{}", context.signature_name, context.signature_tail);
            let signature_base = context
                .signature_name_base
                .checked_sub("fn ".len())
                .unwrap_or(context.base);
            let signature_error_count = errors.len();
            let signature = match parse_fn_signature_at(&signature_source, signature_base) {
                Ok(signature) => Some(signature),
                Err(error) => {
                    errors.push(simple_error(
                        context.base,
                        context.signature_tail.len(),
                        &error.to_string(),
                        "view Name(...) { ... }",
                    ));
                    None
                }
            };
            let signature_has_recovery = errors.len() != signature_error_count;
            if signature
                .as_ref()
                .and_then(crate::types::FnSignature::return_type)
                .is_some()
            {
                errors.push(simple_error(
                    context.base,
                    context.signature_tail.len(),
                    "invalid view declaration signature",
                    "view Name(...) { ... }",
                ));
            }
            let body_error_count = errors.len();
            let view = parse_view_body(
                context.body,
                context.body_base,
                context.module_path,
                context.document,
                errors,
            );
            let has_recovery = signature_has_recovery || errors.len() != body_error_count;
            Some(EntityDeclBody::View(Box::new(ViewDeclBody::new(
                signature,
                view,
                has_recovery,
            ))))
        }
        _ => None,
    }
}

fn parse_image_decl_fields(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<ImageDeclField> {
    collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .filter_map(|item| parse_image_decl_field(&item.source, item.base, errors))
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
    let leading = line.len().saturating_sub(line.trim_start().len());
    let line_base = base.saturating_add(leading);
    let line = line.trim().trim_end_matches(',').trim_end();
    let Some((name, value)) = split_top_level_binding(line) else {
        errors.push(simple_error(
            base,
            line.len(),
            "image declaration body item must be a field assignment",
            "asset = @asset:.id",
        ));
        return None;
    };
    let name_start = name.len().saturating_sub(name.trim_start().len());
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
    let value_untrimmed_start = line.len().saturating_sub(value.len());
    let value_leading = value.len().saturating_sub(value.trim_start().len());
    let value_source = value.trim();
    let name_range = TextRange::new(
        line_base.saturating_add(name_start),
        line_base
            .saturating_add(name_start)
            .saturating_add(name.len()),
    );
    let value_start = line_base
        .saturating_add(value_untrimmed_start)
        .saturating_add(value_leading);
    let value_range = TextRange::new(value_start, value_start.saturating_add(value_source.len()));
    Some(ImageDeclField::new(
        name.to_owned(),
        parse_owned_expr_recovering(value_source, value_start, None, errors),
        TextRange::new(line_base, line_base.saturating_add(line.len())),
        name_range,
        value_range,
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
    let Some((kind_source, id_source)) = split_leading_ident(rest) else {
        errors.push(entry_error(
            ParseErrorKind::EntryMissingKind,
            base + head.len().saturating_sub(rest.len()),
            rest.len(),
            "entry declarations require an explicit kind before their ID",
            "entry game @entry.game.main",
        ));
        return None;
    };
    let kind = EntryKind::parse(kind_source);
    let id_source = id_source.trim_start();
    if id_source.is_empty() {
        errors.push(entry_error(
            ParseErrorKind::EntryMissingId,
            base + head.len(),
            0,
            "entry declarations require an explicit canonical `@entry.*` ID",
            "@entry.game.main",
        ));
        return None;
    }
    let id_base = base + head.len().saturating_sub(id_source.len());
    let (id, trailing) = parse_required_decl_entity_ref_without_name_marker(
        id_source,
        "entry",
        "entry declaration markers must include a suffix",
        id_base,
        errors,
    )?;
    if id.body().strip_prefix("entry.").is_none_or(str::is_empty) {
        errors.push(entry_error(
            ParseErrorKind::EntryIdFamily,
            id.range().start(),
            id.range().end().saturating_sub(id.range().start()),
            "entry declaration IDs must use the `entry` family",
            "@entry.name",
        ));
        return None;
    }
    if !trailing.trim().is_empty() {
        let trailing_source = trailing.trim();
        let trailing_base = id_base
            + id_source.len().saturating_sub(trailing.len())
            + trailing.find(trailing_source).unwrap_or_default();
        errors.push(entry_error(
            ParseErrorKind::EntryTrailingHead,
            trailing_base,
            trailing_source.len(),
            "unexpected text after the entry ID",
            "the entry body",
        ));
        return None;
    }
    Some((kind, visibility, id))
}

fn entry_error(
    kind: ParseErrorKind,
    base: usize,
    len: usize,
    message: &str,
    expected: &str,
) -> ParseError {
    ParseError::new_with_kind(
        kind,
        TextRange::new(base, base + len),
        vec![expected.to_owned()],
        None,
        message.to_owned(),
        vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
    )
}

fn parse_entry_body(
    kind: &EntryKind,
    body: &str,
    body_base: usize,
    errors: &mut Vec<ParseError>,
) -> Vec<EntryItem> {
    let mut seen_roles = BTreeMap::new();
    let mut first_goto = None;
    let mut items = Vec::new();
    for logical in collect_logical_block_items_with_base(body, body_base) {
        let item = parse_entry_body_item(&logical.source, logical.base, errors);
        if let Some(role) = item.role()
            && let Some(range) = item.range().copied()
        {
            if let Some(first) = seen_roles.insert(role, range) {
                errors.push(
                    entry_error(
                        ParseErrorKind::EntryDuplicateRole,
                        range.start(),
                        range.end().saturating_sub(range.start()),
                        &format!("duplicate `{}` entry role", role.as_str()),
                        "each required role exactly once",
                    )
                    .with_related(first, Some("the first role binding is here".to_owned())),
                );
            }
            if !kind.allows_role(role) {
                errors.push(entry_error(
                    ParseErrorKind::EntryIncompatibleRole,
                    range.start(),
                    range.end().saturating_sub(range.start()),
                    &format!(
                        "entry kind `{}` cannot bind the `{}` role",
                        kind.as_str(),
                        role.as_str()
                    ),
                    "a role allowed by this entry kind",
                ));
            }
        }
        match &item {
            EntryItem::Goto(target) if kind.is_stateful() => {
                if let Some(first) = first_goto {
                    errors.push(
                        entry_error(
                            ParseErrorKind::EntryDuplicateGoto,
                            target.range().start(),
                            target.range().end().saturating_sub(target.range().start()),
                            "stateful entries require exactly one `goto` target",
                            "one initial flow target",
                        )
                        .with_related(first, Some("the first initial target is here".to_owned())),
                    );
                } else {
                    first_goto = Some(*target.range());
                }
            }
            EntryItem::Goto(target) if !kind.allows_goto() => errors.push(entry_error(
                ParseErrorKind::EntryIncompatibleGoto,
                target.range().start(),
                target.range().end().saturating_sub(target.range().start()),
                &format!("entry kind `{}` cannot declare `goto`", kind.as_str()),
                "controller = path",
            )),
            EntryItem::Route { target, .. } if !kind.allows_routes() => errors.push(entry_error(
                ParseErrorKind::EntryIncompatibleRoute,
                target.range().start(),
                target.range().end().saturating_sub(target.range().start()),
                &format!("entry kind `{}` cannot declare routes", kind.as_str()),
                "the entry kind's required members",
            )),
            _ => {}
        }
        items.push(item);
    }
    validate_required_entry_members(kind, body_base, &seen_roles, first_goto, errors);
    items
}

fn parse_nominal_where_tail(
    source: &str,
    base: usize,
    owner: &str,
    errors: &mut Vec<ParseError>,
) -> Option<Vec<crate::types::WhereClause>> {
    if source.is_empty() {
        return Some(Vec::new());
    }
    let predicates = source
        .strip_prefix("where")
        .filter(|_| {
            source
                .get("where".len()..)
                .is_some_and(|tail| tail.chars().next().is_none_or(char::is_whitespace))
        })
        .map(str::trim_start);
    let Some(predicates) = predicates else {
        errors.push(simple_error(
            base,
            source.len(),
            &format!("unexpected tokens after {owner} generic parameters"),
            "where Type: Bound",
        ));
        return None;
    };
    let predicate_base = base + subslice_offset(source, predicates)?;
    match parse_where_clauses_at(predicates, predicate_base) {
        Ok(clauses) => Some(clauses),
        Err(error) => {
            errors.push(simple_error(
                predicate_base,
                predicates.len(),
                &format!("invalid {owner} where clause: {error}"),
                "Type: Bound",
            ));
            None
        }
    }
}

fn parse_nominal_generic_params(
    source: Option<&str>,
    generic_base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<Vec<crate::types::GenericParam>> {
    let Some(source) = source else {
        return Some(Vec::new());
    };
    let contents = source
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .expect("the angle-head parser returns one complete angle group");
    match parse_generic_params_at(contents, generic_base + '<'.len_utf8()) {
        Ok(params) => Some(params),
        Err(error) => {
            errors.push(ParseError::new_with_kind(
                ParseErrorKind::NominalInvalidGenericParameters,
                TextRange::new(generic_base, generic_base + source.len()),
                vec!["<T>".to_owned()],
                None,
                format!("invalid nominal generic parameter list: {error}"),
                vec![RecoverySuggestion::new("use <T> syntax")],
            ));
            None
        }
    }
}

fn validate_required_entry_members(
    kind: &EntryKind,
    body_base: usize,
    seen_roles: &BTreeMap<EntryRoleKind, TextRange>,
    first_goto: Option<TextRange>,
    errors: &mut Vec<ParseError>,
) {
    for role in kind.required_roles() {
        if !seen_roles.contains_key(role) {
            errors.push(entry_error(
                ParseErrorKind::EntryMissingRole,
                body_base,
                0,
                &format!(
                    "entry kind `{}` requires exactly one `{}` role",
                    kind.as_str(),
                    role.as_str()
                ),
                &format!("{} = value", role.as_str()),
            ));
        }
    }
    if kind.is_stateful() && first_goto.is_none() {
        errors.push(entry_error(
            ParseErrorKind::EntryMissingGoto,
            body_base,
            0,
            "stateful entries require exactly one `goto` target",
            "goto @flow.initial",
        ));
    }
}

fn parse_entry_body_item(item: &str, base: usize, errors: &mut Vec<ParseError>) -> EntryItem {
    if let Some(role) = parse_entry_role_member(item, base, errors) {
        return role;
    }
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

fn parse_entry_role_member(
    item: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<EntryItem> {
    let (name, _) = split_leading_ident(item)?;
    let role = match name {
        "state" => EntryRoleKind::State,
        "initializer" => EntryRoleKind::Initializer,
        "event" => EntryRoleKind::Event,
        "reducer" => EntryRoleKind::Reducer,
        "controller" => EntryRoleKind::Controller,
        _ => return None,
    };
    let member_range = TextRange::new(base, base + item.len());
    let Some((binding_name, value_source)) = split_top_level_binding(item) else {
        errors.push(entry_error(
            ParseErrorKind::EntryRoleBinding,
            base,
            item.len(),
            &format!("entry role `{}` requires `=` and a value", role.as_str()),
            &format!("{} = value", role.as_str()),
        ));
        return Some(EntryItem::Raw(item.to_owned()));
    };
    if binding_name.trim() != role.as_str() {
        errors.push(entry_error(
            ParseErrorKind::EntryRoleBinding,
            base,
            binding_name.len(),
            &format!("malformed `{}` entry role name", role.as_str()),
            role.as_str(),
        ));
        return Some(EntryItem::Raw(item.to_owned()));
    }
    let value = value_source.trim();
    let value_offset =
        subslice_offset(item, value).expect("a trimmed binding value is a source subslice");
    let value_range = TextRange::new(base + value_offset, base + value_offset + value.len());
    if value.is_empty() {
        errors.push(entry_error(
            ParseErrorKind::EntryRoleValue,
            value_range.start(),
            0,
            &format!("entry role `{}` requires a value", role.as_str()),
            entry_role_expected_value(role),
        ));
        return Some(EntryItem::Raw(item.to_owned()));
    }
    match role {
        EntryRoleKind::State => Some(EntryItem::StateType {
            ty: parse_type_ref_or_error(value, value_range.start(), errors),
            value_range,
            range: member_range,
        }),
        EntryRoleKind::Event => Some(EntryItem::EventType {
            ty: parse_type_ref_or_error(value, value_range.start(), errors),
            value_range,
            range: member_range,
        }),
        EntryRoleKind::Initializer | EntryRoleKind::Reducer | EntryRoleKind::Controller => {
            let Some(path) = parse_entry_role_path(value, value_range, role, errors) else {
                return Some(EntryItem::Raw(item.to_owned()));
            };
            Some(match role {
                EntryRoleKind::Initializer => EntryItem::Initializer {
                    path,
                    value_range,
                    range: member_range,
                },
                EntryRoleKind::Reducer => EntryItem::Reducer {
                    path,
                    value_range,
                    range: member_range,
                },
                EntryRoleKind::Controller => EntryItem::Controller {
                    path,
                    value_range,
                    range: member_range,
                },
                EntryRoleKind::State | EntryRoleKind::Event => unreachable!(),
            })
        }
    }
}

fn parse_entry_role_path(
    value: &str,
    value_range: TextRange,
    role: EntryRoleKind,
    errors: &mut Vec<ParseError>,
) -> Option<DottedPath> {
    if value.split('.').all(is_identifier) {
        return Some(DottedPath::parse_dotted(value));
    }
    errors.push(entry_error(
        ParseErrorKind::EntryRolePath,
        value_range.start(),
        value_range.end().saturating_sub(value_range.start()),
        &format!(
            "entry role `{}` requires a dotted symbol path",
            role.as_str()
        ),
        "module.function",
    ));
    None
}

const fn entry_role_expected_value(role: EntryRoleKind) -> &'static str {
    match role {
        EntryRoleKind::State | EntryRoleKind::Event => "a canonical Arcweft type",
        EntryRoleKind::Initializer | EntryRoleKind::Reducer | EntryRoleKind::Controller => {
            "a dotted symbol path"
        }
    }
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
    let target_base = base
        + subslice_offset(item, target_source)
            .expect("entry target source is retained as a slice of its member");
    let (target, rest) = parse_required_entity_ref(target_source, target_base, errors)?;
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
            let fragment = &body[window[0]..window[1]];
            let item = fragment.trim();
            parse_capability_fn(
                item,
                body_base + window[0] + subslice_offset(fragment, item).unwrap_or_default(),
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
    let signature = match parse_fn_signature_at(
        signature_source,
        item_base
            + subslice_offset(item, signature_source)
                .expect("capability signature remains in the member source"),
    ) {
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
    let signature_source_ranges =
        function_signature_source(signature_source, item_base, None, &signature)?;
    let effects = effects_source
        .map(parse_contract_expr_list)
        .unwrap_or_default();
    Some(CapabilityFn::new(
        signature,
        signature_source_ranges,
        effects,
        TextRange::new(item_base, item_base + item.len()),
    ))
}

fn parse_extern_mod_members(
    body: &str,
    body_base: usize,
    errors: &mut Vec<super::recovery::ParseError>,
) -> Vec<ExternModMember> {
    super::collect_logical_block_items_with_base(body, body_base)
        .into_iter()
        .map(|item| {
            let source = item.source.trim();
            let base = item.base + subslice_offset(&item.source, source).unwrap_or_default();
            parse_extern_mod_member(source, base, errors)
        })
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
        return parse_fn_signature_at(
            rest,
            item_base
                + subslice_offset(item, rest)
                    .expect("external function remains in the member source"),
        )
        .map_or_else(
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
        let ty_base = item_base
            + subslice_offset(item, ty_source)
                .expect("external activity type remains in the member source");
        let ty = parse_type_ref_or_error(ty_source, ty_base, errors);
        return ExternModMember::Activity(ExternModActivity::new(visibility, name, ty));
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
        .map(|item| {
            let source = item.source.trim();
            let base = item.base + subslice_offset(&item.source, source).unwrap_or_default();
            parse_trait_member(source, base, errors)
        })
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
            let value_base = item_base
                + subslice_offset(item, value_source)
                    .expect("associated type value remains in the member source");
            (
                name,
                Some(parse_type_ref_or_error(value_source, value_base, errors)),
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
        return match parse_fn_signature_at(
            signature_source,
            item_base
                + subslice_offset(item, signature_source)
                    .expect("trait signature remains in the member source"),
        ) {
            Err(error) => {
                errors.push(simple_error(
                    item_base,
                    signature_source.len(),
                    &error.to_string(),
                    "a valid trait function signature",
                ));
                TraitMember::Raw(item.to_owned())
            }
            Ok(signature) => {
                let (body, body_statements, body_value) = body.map_or_else(
                    || (None, Vec::new(), None),
                    |(body, body_base)| {
                        let (body_statements, body_value) =
                            parse_scope_authored_expr_body_recovering_with_base(
                                body, body_base, errors,
                            );
                        (Some(body.to_owned()), body_statements, body_value)
                    },
                );
                TraitMember::Function {
                    signature,
                    body,
                    body_statements,
                    body_value: body_value.map(Box::new),
                }
            }
        };
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
        .map(|item| {
            let source = item.source.trim();
            let base = item.base + subslice_offset(&item.source, source).unwrap_or_default();
            parse_impl_member(source, base, errors)
        })
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
            let value_base = item_base
                + subslice_offset(item, value_source)
                    .expect("associated type value remains in the member source");
            let value = parse_type_ref_or_error(value_source, value_base, errors);
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
        return match parse_fn_signature_at(
            head,
            item_base
                + subslice_offset(item, head).expect("impl signature remains in the member source"),
        ) {
            Err(error) => {
                errors.push(simple_error(
                    item_base,
                    head.len(),
                    &error.to_string(),
                    "a valid impl function signature",
                ));
                ImplMember::Raw(item.to_owned())
            }
            Ok(signature) => {
                let (body_statements, body_value) =
                    parse_scope_authored_expr_body_recovering_with_base(body, body_base, errors);
                ImplMember::Function {
                    signature,
                    body: body.to_owned(),
                    body_statements,
                    body_value: body_value.map(Box::new),
                }
            }
        };
    }
    if item.starts_with("fn ") {
        return parse_fn_signature_at(item, item_base).map_or_else(
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

fn function_signature_source(
    head: &str,
    head_base: usize,
    first_contract_line: Option<&str>,
    signature: &crate::types::FnSignature,
) -> Option<FunctionSignatureSource> {
    let signature_start = find_fn_token(head)?;
    let signature_end = first_contract_line
        .and_then(|contract| find_exact_line_start(head, contract, signature_start))
        .unwrap_or(head.len());
    let signature_source = head.get(signature_start..signature_end)?.trim_end();
    let signature_end = signature_start + signature_source.len();
    let signature_source = head.get(signature_start..signature_end)?;

    let after_fn = signature_source.get(2..)?;
    let name_source = after_fn.trim_start();
    let name_offset = signature_source.len() - name_source.len();
    let (authored_name, _) = split_leading_ident(name_source)?;
    if authored_name != signature.name() {
        return None;
    }
    let name_start = signature_start + name_offset;
    let name_range = absolute_range(head_base, name_start, name_start + authored_name.len());

    let mut cursor = name_offset + authored_name.len();
    cursor = skip_whitespace(signature_source, cursor);
    if signature_source.get(cursor..)?.starts_with('<') {
        cursor = find_matching_angle_group(signature_source, cursor)? + 1;
    }

    let (mut cursor, parameters) = function_parameter_sources(
        signature_source,
        signature_start,
        head_base,
        signature,
        cursor,
    )?;

    cursor = skip_whitespace(signature_source, cursor);
    let result = signature.return_type().and_then(|_| {
        let rest = signature_source.get(cursor..)?.trim_start();
        let after_arrow = rest.strip_prefix("->")?.trim_start();
        let (result_source, _) = split_top_level_keyword_once(after_arrow, "where");
        let result_source = result_source.trim();
        let offset = subslice_offset(signature_source, result_source)?;
        Some(absolute_range(
            head_base,
            signature_start + offset,
            signature_start + offset + result_source.len(),
        ))
    });

    Some(FunctionSignatureSource::new(
        absolute_range(
            head_base,
            signature_start,
            signature_start + signature_source.len(),
        ),
        name_range,
        result,
        parameters,
    ))
}

fn function_parameter_sources(
    signature_source: &str,
    signature_start: usize,
    head_base: usize,
    signature: &crate::types::FnSignature,
    mut cursor: usize,
) -> Option<(usize, Vec<FunctionParameterSource>)> {
    let mut parameters = Vec::new();
    for (group_index, parameter_group) in signature.param_groups().iter().enumerate() {
        cursor = skip_whitespace(signature_source, cursor);
        if !signature_source.get(cursor..)?.starts_with('(') {
            return None;
        }
        let close = find_matching_punctuation(signature_source, cursor, '(', ')')?;
        let group_source = signature_source.get(cursor + 1..close)?;
        let parts = split_top_level_punctuation(group_source, ',')
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() != parameter_group.params().len() {
            return None;
        }
        for (parameter_index, (part, parameter)) in
            parts.into_iter().zip(parameter_group.params()).enumerate()
        {
            let group = u16::try_from(group_index).ok()?;
            let parameter_ordinal = u16::try_from(parameter_index).ok()?;
            let part_start = signature_start + cursor + 1 + subslice_offset(group_source, part)?;
            let whole = absolute_range(head_base, part_start, part_start + part.len());

            let (name, ty, default) = if parameter.receiver_kind().is_some() {
                let name = find_identifier_range(part, "self", head_base + part_start);
                (name, None, None)
            } else {
                let (pattern_source, type_source) = split_top_level_punctuation_once(part, ':')?;
                let name = parameter.pattern().simple_binding_name().and_then(|name| {
                    find_identifier_range(pattern_source, name, head_base + part_start)
                });
                let (type_source, default_source) =
                    split_top_level_punctuation_once(type_source, '=')
                        .map_or((type_source, None), |(ty, default)| (ty, Some(default)));
                let type_source = type_source
                    .trim()
                    .strip_prefix("...")
                    .map_or(type_source.trim(), str::trim_start);
                let type_offset = subslice_offset(part, type_source)?;
                let ty = Some(absolute_range(
                    head_base,
                    part_start + type_offset,
                    part_start + type_offset + type_source.len(),
                ));
                let default = default_source.map(|default| {
                    let offset = subslice_offset(part, default)
                        .expect("top-level split returns a source subslice");
                    absolute_range(
                        head_base,
                        part_start + offset,
                        part_start + offset + default.len(),
                    )
                });
                (name, ty, default)
            };
            parameters.push(FunctionParameterSource::new(
                group,
                parameter_ordinal,
                whole,
                name,
                ty,
                default,
            ));
        }
        cursor = close + 1;
    }
    Some((cursor, parameters))
}

fn find_fn_token(source: &str) -> Option<usize> {
    source.match_indices("fn").find_map(|(offset, _)| {
        let before = source[..offset].chars().next_back();
        let after = source[offset + 2..].chars().next();
        (before.is_none_or(|character| !is_identifier_character(character))
            && after.is_some_and(char::is_whitespace))
        .then_some(offset)
    })
}

fn find_exact_line_start(source: &str, expected: &str, after: usize) -> Option<usize> {
    let mut start = 0usize;
    for line in source.split_inclusive('\n') {
        if start >= after && line.trim() == expected {
            return Some(start + line.len() - line.trim_start().len());
        }
        start += line.len();
    }
    None
}

fn skip_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(character) = source.get(cursor..).and_then(|rest| rest.chars().next()) {
        if !character.is_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

fn subslice_offset(source: &str, fragment: &str) -> Option<usize> {
    let source_start = source.as_ptr() as usize;
    let fragment_start = fragment.as_ptr() as usize;
    fragment_start
        .checked_sub(source_start)
        .filter(|offset| offset.saturating_add(fragment.len()) <= source.len())
}

fn find_identifier_range(source: &str, name: &str, base: usize) -> Option<TextRange> {
    source.match_indices(name).find_map(|(offset, _)| {
        let before = source[..offset].chars().next_back();
        let after = source[offset + name.len()..].chars().next();
        (before.is_none_or(|character| !is_identifier_character(character))
            && after.is_none_or(|character| !is_identifier_character(character)))
        .then(|| TextRange::new(base + offset, base + offset + name.len()))
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

const fn absolute_range(base: usize, start: usize, end: usize) -> TextRange {
    TextRange::new(base + start, base + end)
}
