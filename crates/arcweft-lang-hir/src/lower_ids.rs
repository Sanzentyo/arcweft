use crate::lower_context::LowerContext;
use crate::model::HirLowerError;
use arcweft_lang_syntax::{
    ChoiceAction, EntityRef, EntityRefSyntax, Flow, FlowKind, IdRef, RelativeId, TextRange,
};

// TODO(lint): Module paths and named `scope` blocks should both be available to
// the ID policy checker. Today lowering derives relative IDs from the current
// flow ID and named scopes only; a later lint pass should compare generated IDs
// against the source module path and report IDs that break the project hierarchy.

pub(crate) fn normalize_flow_decl_id(flow: &Flow) -> Result<Option<EntityRef>, HirLowerError> {
    let family = flow_decl_family(flow.kind());
    match flow.id() {
        Some(IdRef::Absolute(id)) => Ok(Some(id.clone())),
        Some(IdRef::Relative(relative)) => Ok(Some(EntityRef::new(
            format!("{family}.{}", relative.suffix()),
            false,
            *relative.range(),
        ))),
        Some(IdRef::FamilyRelative(relative)) => {
            if !flow_decl_family_matches(flow.kind(), relative.family()) {
                return Err(HirLowerError::new(
                    format!(
                        "{} declaration cannot use `{}` family-relative id",
                        flow_decl_family(flow.kind()),
                        relative.family()
                    ),
                    Some(*relative.range()),
                ));
            }
            Ok(Some(EntityRef::new(
                format!("{family}.{}", relative.relative().suffix()),
                false,
                *relative.range(),
            )))
        }
        None => Ok(flow
            .name()
            .map(|name| EntityRef::new(format!("{family}.{name}"), false, *flow.range()))),
    }
}

pub(crate) fn normalize_choice_action(
    action: &ChoiceAction,
    context: &LowerContext,
) -> Result<ChoiceAction, HirLowerError> {
    match action {
        ChoiceAction::Goto(target) => normalize_entity_ref_syntax(target, context)
            .map(EntityRefSyntax::absolute)
            .map(ChoiceAction::Goto),
        ChoiceAction::Out(expr) => Ok(ChoiceAction::Out(expr.clone())),
        ChoiceAction::SelectBlock(body) => Ok(ChoiceAction::SelectBlock(body.clone())),
        ChoiceAction::None => Ok(ChoiceAction::None),
    }
}

pub(crate) fn normalize_entity_ref_syntax(
    entity: &EntityRefSyntax,
    context: &LowerContext,
) -> Result<EntityRef, HirLowerError> {
    match entity {
        EntityRefSyntax::Absolute(entity) => Ok(entity.clone()),
        EntityRefSyntax::FamilyRelative(relative) => {
            let Some(flow_slug) = &context.flow_slug else {
                return Err(HirLowerError::new(
                    "relative entity reference requires a flow context",
                    Some(*relative.range()),
                ));
            };
            // Family-relative refs are the recommended spelling for reference
            // positions (`@flow:.next`, `@frag:.intro`) because the family keeps
            // lookup separate from ID-bearing `@.suffix` declaration contexts.
            let mut parts = vec![relative.family().to_owned(), flow_slug.clone()];
            parts.extend(relative_scopes(context, relative.relative())?);
            parts.push(relative.relative().suffix().to_owned());
            Ok(EntityRef::new(parts.join("."), false, *relative.range()))
        }
    }
}

pub(crate) fn normalize_choice_id(
    id: &IdRef,
    context: &LowerContext,
) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "choice", relative.range())?;
            relative.relative()
        }
    };
    let Some(flow_slug) = &context.flow_slug else {
        return Err(HirLowerError::new(
            "relative choice ID requires a flow context",
            Some(*id.range()),
        ));
    };
    let mut parts = vec!["choice".to_owned(), flow_slug.clone()];
    parts.extend(relative_scopes(context, relative)?);
    parts.push(relative.suffix().to_owned());
    Ok(EntityRef::new(parts.join("."), false, *id.range()))
}

pub(crate) fn normalize_option_id(
    id: &IdRef,
    context: &LowerContext,
) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "choice", relative.range())?;
            relative.relative()
        }
    };
    let Some(choice) = context.choice_stack.last() else {
        return Err(HirLowerError::new(
            "relative option ID requires an enclosing choice",
            Some(*id.range()),
        ));
    };
    Ok(EntityRef::new(
        append_relative_suffix(choice, relative.suffix(), relative.parent_depth())?,
        false,
        *id.range(),
    ))
}

pub(crate) fn normalize_text_key_id(
    id: &IdRef,
    context: &LowerContext,
) -> Result<EntityRef, HirLowerError> {
    let relative = match id {
        IdRef::Absolute(id) => return Ok(id.clone()),
        IdRef::Relative(relative) => relative,
        IdRef::FamilyRelative(relative) => {
            ensure_id_family(relative.family(), "text", relative.range())?;
            relative.relative()
        }
    };
    let Some(choice) = context.choice_stack.last() else {
        return Err(HirLowerError::new(
            "relative choice text key requires an enclosing choice",
            Some(*id.range()),
        ));
    };
    let normalized_choice = append_relative_suffix(choice, "", relative.parent_depth())?;
    let choice_path = normalized_choice
        .trim_end_matches('.')
        .strip_prefix("choice.")
        .unwrap_or(normalized_choice.trim_end_matches('.'));
    Ok(EntityRef::new(
        format!("text.choice.{choice_path}.{}", relative.suffix()),
        false,
        *id.range(),
    ))
}

pub(crate) fn normalize_line_id(
    id: Option<&IdRef>,
    speaker: String,
    context: &mut LowerContext,
    range: TextRange,
) -> Result<Option<EntityRef>, HirLowerError> {
    if context.flow_slug.is_none() && !matches!(id, Some(IdRef::Absolute(_))) {
        return Ok(None);
    }
    match id {
        Some(IdRef::Absolute(id)) => Ok(Some(id.clone())),
        Some(IdRef::Relative(relative)) => Ok(Some(build_line_entity_ref(
            speaker,
            Some(relative),
            context,
            *relative.range(),
        )?)),
        Some(IdRef::FamilyRelative(relative)) => {
            ensure_id_family(relative.family(), "say", relative.range())?;
            Ok(Some(build_line_entity_ref(
                speaker,
                Some(relative.relative()),
                context,
                *relative.range(),
            )?))
        }
        None => Ok(Some(build_line_entity_ref(speaker, None, context, range)?)),
    }
}

pub(crate) fn normalize_line_text_key(
    text_key: Option<&IdRef>,
    line_id: Option<&EntityRef>,
    speaker: String,
    context: &LowerContext,
) -> Result<Option<EntityRef>, HirLowerError> {
    if let Some(text_key) = text_key {
        let relative = match text_key {
            IdRef::Absolute(text_key) => return Ok(Some(text_key.clone())),
            IdRef::Relative(relative) => relative,
            IdRef::FamilyRelative(relative) => {
                ensure_id_family(relative.family(), "text", relative.range())?;
                relative.relative()
            }
        };
        let Some(flow_slug) = &context.flow_slug else {
            return Err(HirLowerError::new(
                "relative text key requires a flow context",
                Some(*text_key.range()),
            ));
        };
        let mut parts = vec!["text".to_owned(), flow_slug.clone(), speaker];
        parts.extend(relative_scopes(context, relative)?);
        parts.push(relative.suffix().to_owned());
        return Ok(Some(EntityRef::new(
            parts.join("."),
            false,
            *text_key.range(),
        )));
    }
    Ok(line_id.map(|id| EntityRef::new(line_id_to_text_key(id.body()), false, *id.range())))
}

pub(crate) fn speaker_slug(speaker: &str) -> String {
    match speaker.trim() {
        "地の文" | "地文" | "ナレーター" | "ナレータ" | "ナレーション" | "語り" | "語り手"
        | "narrator" | "Narrator" | "NARRATOR" | "VO" | "V.O." | "O.S." | "Offscreen"
        | "Script" | "StageDirection" | "ト書き" | "脚本" => "narrator".to_owned(),
        other => {
            let source = other
                .trim()
                .strip_prefix("@<")
                .and_then(|inner| inner.strip_suffix('>'))
                .or_else(|| other.trim().strip_prefix('@'))
                .unwrap_or(other)
                .trim_end_matches(".say");
            source
                .rsplit(['.', ':'])
                .next()
                .unwrap_or(source)
                .to_owned()
        }
    }
}

pub(crate) fn content_callee_slug(callee: &str) -> String {
    callee
        .strip_suffix(".say")
        .map_or_else(|| speaker_slug(callee), speaker_slug)
}

pub(crate) fn flow_slug_from_entity(id: &EntityRef) -> String {
    id.body()
        .strip_prefix("flow.")
        .or_else(|| id.body().strip_prefix("fragment."))
        .unwrap_or(id.body())
        .to_owned()
}

fn ensure_id_family(found: &str, expected: &str, range: &TextRange) -> Result<(), HirLowerError> {
    if found == expected {
        Ok(())
    } else {
        Err(HirLowerError::new(
            format!("relative ID family `{found}` is not valid here; expected `{expected}`"),
            Some(*range),
        ))
    }
}

fn build_line_entity_ref(
    speaker: String,
    explicit_id: Option<&RelativeId>,
    context: &mut LowerContext,
    range: TextRange,
) -> Result<EntityRef, HirLowerError> {
    let Some(flow_slug) = context.flow_slug.as_ref() else {
        return Err(HirLowerError::new(
            "dialogue line ID requires a flow context",
            Some(range),
        ));
    };
    let mut parts = vec!["say".to_owned(), flow_slug.clone(), speaker];
    if let Some(id) = explicit_id {
        parts.extend(relative_scopes(context, id)?);
    } else {
        parts.extend(context.scopes.iter().cloned());
    }
    let prefix = parts.join(".");
    let suffix = explicit_id.map_or_else(
        || {
            let next = context.line_counters.entry(prefix.clone()).or_insert(0);
            *next += 1;
            format!("{next:03}")
        },
        |id| id.suffix().to_owned(),
    );
    Ok(EntityRef::new(format!("{prefix}.{suffix}"), false, range))
}

fn relative_scopes(
    context: &LowerContext,
    relative: &RelativeId,
) -> Result<Vec<String>, HirLowerError> {
    // TODO(lint): `@...suffix` is accepted for machine output and compact
    // authoring, but hand-written source should be nudged toward explicit
    // `@super.super.suffix` once a lint/formatter layer exists.
    let Some(take_len) = context.scopes.len().checked_sub(relative.parent_depth()) else {
        return Err(HirLowerError::new(
            "relative ID walks past the available ID scopes",
            Some(*relative.range()),
        ));
    };
    Ok(context.scopes.iter().take(take_len).cloned().collect())
}

fn append_relative_suffix(
    base: &str,
    suffix: &str,
    parent_depth: usize,
) -> Result<String, HirLowerError> {
    let mut parts = base.split('.').map(str::to_owned).collect::<Vec<_>>();
    for _ in 0..parent_depth {
        if parts.len() <= 1 {
            return Err(HirLowerError::new(
                "relative ID walks past the available ID scopes",
                None,
            ));
        }
        parts.pop();
    }
    if !suffix.is_empty() {
        parts.push(suffix.to_owned());
    }
    Ok(parts.join("."))
}

fn line_id_to_text_key(line_id: &str) -> String {
    line_id
        .strip_prefix("say.")
        .map_or_else(|| format!("text.{line_id}"), |tail| format!("text.{tail}"))
}

fn flow_decl_family(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Flow => "flow",
        FlowKind::Fragment => "fragment",
    }
}

fn flow_decl_family_matches(kind: FlowKind, family: &str) -> bool {
    match kind {
        FlowKind::Flow => family == "flow",
        FlowKind::Fragment => matches!(family, "fragment" | "frag"),
    }
}
