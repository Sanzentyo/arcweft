use crate::ast::items::{EnumVariant, ImplMember, StateField, StructField, TraitMember};
use crate::cst::{
    find_matching_angle_group, find_top_level_punctuation, split_leading_ident,
    split_top_level_punctuation, split_top_level_punctuation_once,
};
use crate::types::{parse_fn_signature, parse_type_ref};

use super::{
    PendingDocLines, collect_logical_block_items, parse_expr_lossy, parse_scope_expr_body,
    parse_visibility_prefix, split_brace_item, split_top_level_binding,
};

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
