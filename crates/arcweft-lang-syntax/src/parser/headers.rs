use crate::ast::common::{TextRange, Visibility};
use crate::ast::flow::{ContractClause, FlowKind};
use crate::ast::ids::{
    EntityRef, EntityRefSyntax, FamilyRelativeEntityRef, IdRef, RelativeId, RelativeIdSpelling,
};
use crate::ast::items::{CallableKind, EntityDeclKind, FunctionKind};
use crate::cst::{
    split_leading_entity_ref_parts, split_leading_ident, split_leading_relative_entity_ref,
    split_leading_relative_id, split_top_level_keyword_once, split_top_level_punctuation,
    starts_leading_entity_ref, starts_leading_relative_entity_ref, starts_leading_relative_id,
};
use crate::types::parse_fn_signature;
use arcweft_source::{SourceAnchor, SourceName};

use super::parse_expr_lossy;
use super::recovery::{ParseError, RecoverySuggestion};

pub(super) type EntityDeclHead = (
    EntityDeclKind,
    Option<Visibility>,
    EntityRef,
    Option<String>,
    Option<String>,
    String,
);
pub(super) fn parse_function_kind_and_signature(source: &str) -> (FunctionKind, &str) {
    [
        ("task ", FunctionKind::Task),
        ("dialogue ", FunctionKind::Dialogue),
        ("stream ", FunctionKind::Stream),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| {
        source
            .strip_prefix(prefix)
            .map(|signature| (kind, signature.trim_start()))
    })
    .unwrap_or((FunctionKind::Function, source))
}

pub(super) fn split_function_header_lines<'a>(
    lines: &'a [&'a str],
) -> Option<(String, Vec<&'a str>)> {
    let mut signature = Vec::new();
    let mut depth = 0_i32;
    let mut end_index = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if index > 0 && depth == 0 && parse_contract_clause(trimmed).is_some() {
            end_index = Some(index);
            break;
        }
        signature.push(trimmed);
        for ch in trimmed.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
    }
    let end_index = end_index.unwrap_or(signature.len());
    (!signature.is_empty()).then(|| (signature.join("\n"), lines[end_index..].to_vec()))
}

pub(super) fn parse_extern_mod_head(head: &str) -> Option<(String, String, Option<String>)> {
    let rest = head.trim_start().strip_prefix("extern")?.trim_start();
    let (abi, Some(rest)) = split_top_level_keyword_once(rest, "mod") else {
        return None;
    };
    let (path, source) = split_top_level_keyword_once(rest, "from");
    let source = source.map(|source| source.trim().to_owned());
    Some((abi.trim().to_owned(), path.trim().to_owned(), source))
}

pub(super) fn entity_decl_kind(input: &str) -> Option<(EntityDeclKind, &str)> {
    [
        ("audio bus", EntityDeclKind::AudioBus),
        ("mixer snapshot", EntityDeclKind::MixerSnapshot),
        ("asset", EntityDeclKind::Asset),
        ("image", EntityDeclKind::Image),
        ("character", EntityDeclKind::Character),
        ("component", EntityDeclKind::Component),
        ("action", EntityDeclKind::Action),
        ("activity", EntityDeclKind::Activity),
        ("content", EntityDeclKind::Content),
        ("metric counter", EntityDeclKind::Metric),
        ("metric gauge", EntityDeclKind::Metric),
        ("metric", EntityDeclKind::Metric),
        ("signal", EntityDeclKind::Signal),
        ("layer", EntityDeclKind::Layer),
        ("textbox", EntityDeclKind::Textbox),
        ("voice profile", EntityDeclKind::Voice),
        ("voice", EntityDeclKind::Voice),
        ("se", EntityDeclKind::Se),
        ("bgm", EntityDeclKind::Bgm),
        ("ducking", EntityDeclKind::Ducking),
        ("motion", EntityDeclKind::Motion),
        ("rig", EntityDeclKind::Rig),
    ]
    .into_iter()
    .find_map(|(keyword, kind)| {
        input
            .strip_prefix(keyword)
            .filter(|rest| rest.starts_with(char::is_whitespace))
            .map(|rest| (kind, rest.trim_start()))
    })
}

pub(super) fn entity_decl_family(kind: EntityDeclKind) -> &'static str {
    match kind {
        EntityDeclKind::Asset => "asset",
        EntityDeclKind::Image => "image",
        EntityDeclKind::Character => "character",
        EntityDeclKind::Component => "ui",
        EntityDeclKind::Action => "action",
        EntityDeclKind::Activity => "activity",
        EntityDeclKind::Content => "content",
        EntityDeclKind::Signal => "signal",
        EntityDeclKind::Metric => "metric",
        EntityDeclKind::Layer => "layer",
        EntityDeclKind::Textbox => "textbox",
        EntityDeclKind::Voice => "voice",
        EntityDeclKind::Se => "se",
        EntityDeclKind::Bgm => "bgm",
        EntityDeclKind::AudioBus => "bus",
        EntityDeclKind::MixerSnapshot => "mix",
        EntityDeclKind::Ducking => "duck",
        EntityDeclKind::Motion => "motion",
        EntityDeclKind::Rig => "rig",
    }
}

pub(super) fn parse_entity_decl_head(
    head: &str,
    base: usize,
    module_path: Option<&str>,
    errors: &mut Vec<ParseError>,
) -> Option<EntityDeclHead> {
    let (visibility, rest) = parse_visibility_prefix(head);
    let rest = rest
        .trim_start()
        .strip_prefix("surface ")
        .unwrap_or(rest.trim_start());
    let (kind, rest) = entity_decl_kind(rest.trim_start())?;
    let family = entity_decl_family(kind);
    let rest = rest.trim_start();
    let (id, name, signature_tail) = if rest.starts_with('@') {
        let id_source = rest;
        let id_base = base + slice_offset(head, rest);
        let (parsed_id, rest) =
            parse_required_decl_entity_ref_or_marker(rest, family, id_base, errors)?;
        let (id, rest) = match parsed_id {
            DeclEntityId::Entity(id) => {
                let (id, rest) = normalize_trailing_colon_id(id, rest);
                (
                    rebase_relative_declaration_entity(id, id_source, family, module_path),
                    rest,
                )
            }
            DeclEntityId::NameMarker(marker) => {
                let rest = rest.trim();
                let (name, _) = parse_name_and_tail(rest);
                let Some(name) = name.as_deref() else {
                    errors.push(simple_error(
                        marker.range.start(),
                        marker.range.end() - marker.range.start(),
                        "relative declaration marker needs a following declaration name",
                        &format!("@{family}:. name"),
                    ));
                    return None;
                };
                (
                    EntityRef::module_scoped_declaration(family, name, module_path, marker.range),
                    rest.to_owned(),
                )
            }
        };
        let rest = rest.trim();
        let (name, signature_tail) = parse_name_and_tail(rest);
        (id, name, signature_tail)
    } else {
        let (name, signature_tail) = parse_dotted_decl_name_and_tail(rest);
        let Some(name) = name else {
            errors.push(simple_error(
                base,
                head.len(),
                "entity declaration needs an id or canonical declaration name",
                &format!("{family} name"),
            ));
            return None;
        };
        let range = entity_bare_name_range(head, base, &name);
        (
            EntityRef::module_scoped_declaration(family, &name, module_path, range),
            None,
            signature_tail,
        )
    };
    let (signature_tail, surface_alias) = split_surface_alias(signature_tail);
    Some((kind, visibility, id, name, surface_alias, signature_tail))
}

pub(super) fn rebase_relative_declaration_entity(
    entity: EntityRef,
    source: &str,
    family: &str,
    module_path: Option<&str>,
) -> EntityRef {
    let source = source.trim_start();
    if !(source.starts_with("@.") || source.starts_with(&format!("@{family}:."))) {
        return entity;
    }
    let Some(suffix) = entity.body().strip_prefix(&format!("{family}.")) else {
        return entity;
    };
    EntityRef::module_scoped_declaration(family, suffix, module_path, *entity.range())
}

fn entity_bare_name_range(head: &str, base: usize, name: &str) -> TextRange {
    let start = head
        .find(name)
        .map_or(base, |offset| base.saturating_add(offset));
    TextRange::new(start, start.saturating_add(name.len()))
}

pub(super) fn slice_offset(source: &str, slice: &str) -> usize {
    (slice.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

pub(super) fn split_surface_alias(signature_tail: String) -> (String, Option<String>) {
    let (before, after) = split_top_level_keyword_once(&signature_tail, "as");
    if let Some(after) = after {
        let alias = after
            .split_whitespace()
            .next()
            .filter(|value| is_simple_identifier(value))
            .map(str::to_owned);
        return (before.trim().to_owned(), alias);
    }
    (signature_tail, None)
}

pub(super) fn is_simple_identifier(source: &str) -> bool {
    let mut chars = source.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_alphanumeric() || ch == '_')
}

pub(super) fn normalize_trailing_colon_id(entity: EntityRef, rest: &str) -> (EntityRef, String) {
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

pub(super) fn parse_callable_kind(input: &str) -> Option<(CallableKind, &str)> {
    if let Some(rest) = input.strip_prefix("reducer") {
        return Some((CallableKind::Reducer, rest.trim_start()));
    }
    input
        .strip_prefix("view")
        .map(|rest| (CallableKind::View, rest.trim_start()))
}

pub(super) fn parse_flow_kind(input: &str) -> Option<(FlowKind, &str)> {
    if let Some(rest) = input.strip_prefix("flow") {
        return Some((FlowKind::Flow, rest.trim_start()));
    }
    input
        .strip_prefix("fragment")
        .map(|rest| (FlowKind::Fragment, rest.trim_start()))
}

pub(super) fn flow_decl_family(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Flow => "flow",
        FlowKind::Fragment => "fragment",
    }
}

pub(super) fn find_header_value(lines: &[&str], prefix: &str) -> String {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .unwrap_or_default()
        .to_owned()
}

pub(super) fn parse_flow_signature(
    name: Option<&str>,
    signature_tail: &str,
) -> Option<crate::types::FnSignature> {
    let tail = signature_tail.trim();
    if !(tail.starts_with('(') || tail.starts_with('<')) {
        return None;
    }
    parse_fn_signature(&format!("fn {}{}", name.unwrap_or("flow"), tail)).ok()
}

pub(super) fn implicit_flow_name_from_id(id: Option<&IdRef>) -> Option<String> {
    match id? {
        IdRef::Relative(relative) => Some(relative.suffix().to_owned()),
        IdRef::FamilyRelative(relative) => Some(relative.relative().suffix().to_owned()),
        IdRef::Absolute(_) => None,
    }
}

pub(super) fn parse_visibility_prefix(input: &str) -> (Option<Visibility>, &str) {
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

pub(super) fn parse_optional_entity_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<EntityRef>, &'a str) {
    let trimmed = input.trim_start();
    if starts_leading_entity_ref(trimmed) {
        match parse_required_entity_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else {
        (None, input)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EmptyDeclRelativeMarker {
    pub(super) range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DeclEntityId {
    Entity(EntityRef),
    NameMarker(EmptyDeclRelativeMarker),
}

pub(super) fn parse_optional_decl_id_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<IdRef>, &'a str) {
    let trimmed = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(trimmed) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
        }
        return (None, rest);
    }
    if starts_leading_relative_id(trimmed) || starts_leading_relative_entity_ref(trimmed) {
        return match parse_required_id_ref(trimmed, base, errors) {
            Some((id, rest)) => {
                if let IdRef::FamilyRelative(relative) = &id
                    && !decl_family_matches(family, relative.family())
                {
                    errors.push(simple_error(
                        relative.range().start(),
                        relative.range().end() - relative.range().start(),
                        "family-relative declaration id uses the wrong family",
                        &format!("@{family}:.suffix"),
                    ));
                }
                (Some(id), rest)
            }
            None => (None, input),
        };
    }
    let (id, rest) = parse_optional_id_ref(input, base, errors);
    let Some(id) = id else {
        return (None, rest);
    };
    match &id {
        IdRef::FamilyRelative(relative) if !decl_family_matches(family, relative.family()) => {
            errors.push(simple_error(
                relative.range().start(),
                relative.range().end() - relative.range().start(),
                "family-relative declaration id uses the wrong family",
                &format!("@{family}:.suffix"),
            ));
        }
        _ => {}
    }
    (Some(id), rest)
}

pub(super) fn parse_optional_decl_entity_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<EntityRef>, &'a str) {
    let trimmed = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(trimmed) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
        }
        return (None, rest);
    }
    if starts_leading_relative_id(trimmed) || starts_leading_relative_entity_ref(trimmed) {
        match parse_required_id_ref(trimmed, base, errors)
            .and_then(|(id, rest)| normalize_decl_id_ref(id, family, errors).map(|id| (id, rest)))
        {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else {
        parse_optional_entity_ref(input, base, errors)
    }
}

pub(super) fn parse_optional_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> (Option<IdRef>, &'a str) {
    let trimmed = input.trim_start();
    if starts_leading_relative_id(trimmed) {
        match parse_required_id_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(entity), rest),
            None => (None, input),
        }
    } else if trimmed.starts_with('.') {
        let _ = parse_required_id_ref(trimmed, base, errors);
        (None, input)
    } else if starts_leading_entity_ref(trimmed) {
        match parse_required_entity_ref(trimmed, base, errors) {
            Some((entity, rest)) => (Some(IdRef::absolute(entity)), rest),
            None => (None, input),
        }
    } else {
        (None, input)
    }
}

pub(super) fn parse_required_decl_entity_ref<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_id(input) || starts_leading_relative_entity_ref(input) {
        let (id, rest) = parse_required_id_ref(input, base, errors)?;
        let entity = normalize_decl_id_ref(id, family, errors)?;
        Some((entity, rest))
    } else {
        parse_required_entity_ref(input, base, errors)
    }
}

pub(super) fn parse_required_decl_entity_ref_or_marker<'a>(
    input: &'a str,
    family: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(DeclEntityId, &'a str)> {
    let input = input.trim_start();
    if let Some((marker_family, marker_len, rest)) = split_empty_decl_relative_marker(input) {
        if marker_family.is_some_and(|actual| !decl_family_matches(family, actual)) {
            errors.push(simple_error(
                base,
                marker_len,
                "family-relative declaration marker uses the wrong family",
                &format!("@{family}:. name"),
            ));
            return None;
        }
        return Some((
            DeclEntityId::NameMarker(EmptyDeclRelativeMarker {
                range: TextRange::new(base, base + marker_len),
            }),
            rest,
        ));
    }
    parse_required_decl_entity_ref(input, family, base, errors)
        .map(|(entity, rest)| (DeclEntityId::Entity(entity), rest))
}

pub(super) fn parse_required_decl_entity_ref_without_name_marker<'a>(
    input: &'a str,
    family: &str,
    marker_message: &str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    match parse_required_decl_entity_ref_or_marker(input, family, base, errors)? {
        (DeclEntityId::Entity(id), rest) => Some((id, rest)),
        (DeclEntityId::NameMarker(marker), _) => {
            errors.push(simple_error(
                marker.range.start(),
                marker.range.end() - marker.range.start(),
                marker_message,
                &format!("@{family}:.suffix"),
            ));
            None
        }
    }
}

pub(super) fn normalize_decl_id_ref(
    id: IdRef,
    family: &str,
    errors: &mut Vec<ParseError>,
) -> Option<EntityRef> {
    match id {
        IdRef::Absolute(entity) => Some(entity),
        IdRef::Relative(relative) => Some(EntityRef::new(
            format!("{family}.{}", relative.suffix()),
            false,
            *relative.range(),
        )),
        IdRef::FamilyRelative(relative) => {
            if !decl_family_matches(family, relative.family()) {
                errors.push(simple_error(
                    relative.range().start(),
                    relative.range().end() - relative.range().start(),
                    "family-relative declaration id uses the wrong family",
                    &format!("@{family}:.suffix"),
                ));
                return None;
            }
            Some(EntityRef::new(
                format!("{family}.{}", relative.relative().suffix()),
                false,
                *relative.range(),
            ))
        }
    }
}

pub(super) fn decl_family_matches(expected: &str, actual: &str) -> bool {
    expected == actual || expected == "fragment" && actual == "frag"
}

pub(super) fn split_empty_decl_relative_marker(
    source: &str,
) -> Option<(Option<&str>, usize, &str)> {
    if let Some(rest) = source.strip_prefix("@.") {
        return (!rest.starts_with(is_decl_relative_suffix_start)).then_some((
            None,
            "@.".len(),
            rest,
        ));
    }
    let at = source.strip_prefix('@')?;
    let family_len = take_decl_marker_while(at, |ch| ch.is_ascii_alphanumeric() || ch == '_');
    if family_len == 0 {
        return None;
    }
    let marker = at.get(family_len..)?.strip_prefix(":.")?;
    (!marker.starts_with(is_decl_relative_suffix_start)).then_some((
        Some(&at[..family_len]),
        '@'.len_utf8() + family_len + ":.".len(),
        marker,
    ))
}

pub(super) fn take_decl_marker_while(source: &str, predicate: impl Fn(char) -> bool) -> usize {
    source
        .char_indices()
        .take_while(|(_, ch)| predicate(*ch))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

pub(super) fn is_decl_relative_suffix_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

pub(super) fn parse_required_entity_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRef, &'a str)> {
    let input = input.trim_start();
    if input.starts_with("@<") {
        let Some(entity_ref) = split_leading_entity_ref_parts(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "unclosed delimited entity reference",
                "@<...>",
            ));
            return None;
        };
        if !entity_ref.closed {
            errors.push(simple_error(
                base,
                input.len(),
                "unclosed delimited entity reference",
                "@<...>",
            ));
            return None;
        }
        if entity_ref.body.trim().is_empty() {
            errors.push(simple_error(
                base,
                input.len(),
                "empty entity reference",
                "@foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(
                entity_ref.body.to_owned(),
                true,
                TextRange::new(base, base + entity_ref.raw.len()),
            ),
            entity_ref.rest,
        ));
    }
    if starts_leading_entity_ref(input) {
        let Some(entity_ref) = split_leading_entity_ref_parts(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid entity reference",
                "@foo.bar",
            ));
            return None;
        };
        if entity_ref.body.is_empty() {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid entity reference",
                "@foo.bar",
            ));
            return None;
        }
        return Some((
            EntityRef::new(
                entity_ref.body.to_owned(),
                false,
                TextRange::new(base, base + entity_ref.raw.len()),
            ),
            entity_ref.rest,
        ));
    }
    None
}

pub(super) fn parse_required_entity_ref_syntax<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(EntityRefSyntax, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_id(input) {
        errors.push(simple_error(
            base,
            input.len(),
            "relative entity references must include a family",
            "@flow:.suffix",
        ));
        return None;
    }
    if starts_leading_relative_entity_ref(input) {
        let Some(relative_ref) = split_leading_relative_entity_ref(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid relative entity reference",
                "@flow:.suffix",
            ));
            return None;
        };
        let relative = relative_id_from_cst(
            relative_ref.relative,
            TextRange::new(
                base + '@'.len_utf8() + relative_ref.family.len() + ':'.len_utf8(),
                base + relative_ref.raw.len(),
            ),
        );
        let entity = FamilyRelativeEntityRef::new(
            relative_ref.family.to_owned(),
            relative,
            TextRange::new(base, base + relative_ref.raw.len()),
        );
        return Some((EntityRefSyntax::family_relative(entity), relative_ref.rest));
    }
    parse_required_entity_ref(input, base, errors)
        .map(|(entity, rest)| (EntityRefSyntax::absolute(entity), rest))
}

pub(super) fn parse_required_id_ref<'a>(
    input: &'a str,
    base: usize,
    errors: &mut Vec<ParseError>,
) -> Option<(IdRef, &'a str)> {
    let input = input.trim_start();
    if starts_leading_relative_entity_ref(input) {
        let Some(relative_ref) = split_leading_relative_entity_ref(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "invalid family-relative id",
                "@family:.suffix",
            ));
            return None;
        };
        let relative = relative_id_from_cst(
            relative_ref.relative,
            TextRange::new(
                base + '@'.len_utf8() + relative_ref.family.len() + ':'.len_utf8(),
                base + relative_ref.raw.len(),
            ),
        );
        let entity = FamilyRelativeEntityRef::new(
            relative_ref.family.to_owned(),
            relative,
            TextRange::new(base, base + relative_ref.raw.len()),
        );
        return Some((IdRef::family_relative(entity), relative_ref.rest));
    }
    if starts_leading_relative_id(input) {
        let Some(relative) = split_leading_relative_id(input) else {
            errors.push(simple_error(
                base,
                input.len(),
                "relative id is missing a suffix",
                "@.suffix",
            ));
            return None;
        };
        let range = TextRange::new(base, base + relative.marker_len + relative.body.len());
        return Some((
            IdRef::relative(relative_id_from_cst(relative, range)),
            relative.rest,
        ));
    }
    if starts_leading_entity_ref(input) {
        return parse_required_entity_ref(input, base, errors)
            .map(|(entity, rest)| (IdRef::absolute(entity), rest));
    }
    if input.starts_with('.') {
        errors.push(simple_error(
            base,
            input.len(),
            "relative IDs must start with `@.`",
            "@.suffix",
        ));
        return None;
    }
    {
        errors.push(simple_error(
            base,
            input.len(),
            "expected entity reference or relative id",
            "@domain.path",
        ));
    }
    None
}

pub(super) fn relative_id_from_cst(
    relative: crate::cst::CstRelativeId<'_>,
    range: TextRange,
) -> RelativeId {
    let spelling = match relative.spelling {
        crate::cst::CstRelativeIdSpelling::DotRun => RelativeIdSpelling::DotRun,
        crate::cst::CstRelativeIdSpelling::SuperChain => RelativeIdSpelling::SuperChain,
    };
    RelativeId::new(
        relative.body.to_owned(),
        relative.parent_depth,
        spelling,
        range,
    )
}

pub(super) fn simple_error(base: usize, len: usize, message: &str, expected: &str) -> ParseError {
    ParseError::new(
        TextRange::new(base, base + len),
        vec![expected.to_owned()],
        None,
        message.to_owned(),
        vec![RecoverySuggestion::new(format!("use {expected} syntax"))],
        SourceAnchor::new(SourceName::path("<memory>"), base..base + len),
    )
}

pub(super) fn parse_name_and_tail(input: &str) -> (Option<String>, String) {
    let trimmed = input.trim_start();
    split_leading_ident(trimmed).map_or_else(
        || (None, trimmed.to_owned()),
        |(name, tail)| (Some(name.to_owned()), tail.trim().to_owned()),
    )
}

fn parse_dotted_decl_name_and_tail(input: &str) -> (Option<String>, String) {
    let trimmed = input.trim_start();
    let Some((first, mut tail)) = split_leading_ident(trimmed) else {
        return (None, trimmed.to_owned());
    };
    let mut name = first.to_owned();
    while let Some(after_dot) = tail.strip_prefix('.') {
        let Some((segment, next_tail)) = split_leading_ident(after_dot) else {
            break;
        };
        name.push('.');
        name.push_str(segment);
        tail = next_tail;
    }
    (Some(name), tail.trim().to_owned())
}

pub(super) fn parse_contract_clause(line: &str) -> Option<ContractClause> {
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

pub(super) fn parse_contract_clauses(lines: &[&str]) -> Vec<ContractClause> {
    merge_contract_lines(lines)
        .iter()
        .filter_map(|line| parse_contract_clause(line))
        .collect()
}

fn merge_contract_lines(lines: &[&str]) -> Vec<String> {
    let mut merged = Vec::new();
    let mut current: Option<(String, i32)> = None;

    for line in lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
    {
        if let Some((text, depth)) = &mut current {
            text.push(' ');
            text.push_str(line);
            *depth += brace_delta(line);
            if *depth <= 0
                && let Some((text, _)) = current.take()
            {
                merged.push(text);
            }
            continue;
        }

        let depth = brace_delta(line);
        if starts_contract_list(line) && depth > 0 {
            current = Some((line.to_owned(), depth));
        } else {
            merged.push(line.to_owned());
        }
    }

    if let Some((text, _)) = current {
        merged.push(text);
    }

    merged
}

fn starts_contract_list(line: &str) -> bool {
    ["reads ", "effects ", "modifies "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

fn brace_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, ch| match ch {
        '{' => depth + 1,
        '}' => depth - 1,
        _ => depth,
    })
}

pub(super) fn split_contract_mode(source: &str) -> (Option<String>, &str) {
    let trimmed = source.trim();
    for mode in ["prove", "check", "debug"] {
        if let Some(rest) = trimmed.strip_prefix(mode) {
            return (Some(mode.to_owned()), rest.trim());
        }
    }
    (None, trimmed)
}

pub(super) fn parse_contract_expr_list(source: &str) -> Vec<crate::expr::Expr> {
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

pub(super) fn split_supertraits(source: &str) -> Vec<String> {
    split_top_level_punctuation(source, '+')
        .into_iter()
        .map(str::trim)
        .filter(|trait_name| !trait_name.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn parse_optional_angle_head(source: &str) -> (Option<String>, &str) {
    let source = source.trim_start();
    if !source.starts_with('<') {
        return (None, source);
    }
    if let Some(close) = crate::cst::find_matching_angle_group(source, 0) {
        return (
            Some(source[..=close].to_owned()),
            source[close + '>'.len_utf8()..].trim_start(),
        );
    }
    (None, source)
}
