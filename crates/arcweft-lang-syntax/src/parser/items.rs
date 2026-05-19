use crate::ast::common::TextRange;
use crate::ast::items::{
    CallableItem, EnumItem, EnumVariant, FunctionInit, FunctionItem, ImplItem, ImplMember,
    StateField, StateItem, StructField, StructItem, TraitItem, TraitMember, TypeAliasItem,
};
use crate::cst::{
    find_matching_angle_group, find_top_level_punctuation, split_leading_ident,
    split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::types::{parse_fn_signature, parse_type_ref};

use super::{
    Parser, PendingDocLines, collect_logical_block_items, parse_callable_kind,
    parse_contract_clause, parse_expr_lossy, parse_function_kind_and_signature,
    parse_name_and_tail, parse_optional_angle_head, parse_scope_expr_body, parse_visibility_prefix,
    split_brace_item, split_function_header_lines, split_supertraits, split_top_level_binding,
};

impl Parser {
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
            let (name, ty) = split_top_level_punctuation_once(line, ':')?;
            parse_type_ref(ty.trim())
                .ok()
                .map(|ty| StructField::new(docs.take(), name.trim().to_owned(), ty))
        })
        .collect()
}

pub(super) fn parse_state_fields(body: &str) -> Vec<StateField> {
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

pub(super) fn parse_trait_members(body: &str) -> Vec<TraitMember> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_trait_member)
        .collect()
}

fn parse_trait_member(line: &str) -> TraitMember {
    let line = line.trim_end_matches(';').trim();
    if let Some(rest) = line.strip_prefix("type ") {
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
    if line.starts_with("fn ") {
        return parse_fn_signature(line).map_or_else(
            |_| TraitMember::Raw(line.to_owned()),
            |signature| TraitMember::Function { signature },
        );
    }
    TraitMember::Raw(line.to_owned())
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
