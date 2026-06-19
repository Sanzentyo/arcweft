use super::{
    AwaitBranchKind, CallArg, ChoiceAction, EntityDeclKind, EntityKind, EntityRef, Expr,
    LifetimeScopeKind, Literal, MapKind, Pattern, Stmt, TypeCheckError, TypeKind, TypeRef,
    VariantPatternPayload,
};

pub(super) fn entity_kind(entity: &EntityRef) -> Option<EntityKind> {
    let head = entity.body().split(['.', '@', ':']).next()?;
    Some(match head {
        "agent" => EntityKind::Agent,
        "entry" => EntityKind::Entry,
        "flow" => EntityKind::Flow,
        "frag" | "fragment" => EntityKind::Fragment,
        "choice" => EntityKind::Choice,
        "character" => EntityKind::Character,
        "ui" => EntityKind::Component,
        "activity" => EntityKind::Activity,
        "textbox" => EntityKind::Textbox,
        "say" => EntityKind::DialogueLine,
        "text" => EntityKind::Text,
        "item" => EntityKind::Other("item".to_owned()),
        "asset" => EntityKind::Asset,
        "image" => EntityKind::Image,
        "anim" => EntityKind::Animation,
        "capture" => EntityKind::Capture,
        "hook" => EntityKind::Hook,
        "signal" => EntityKind::Signal,
        "metric" => EntityKind::Metric,
        "scene" => EntityKind::Scene,
        "source" => EntityKind::Source,
        "test" => EntityKind::Test,
        "bench" => EntityKind::Bench,
        "layer" => EntityKind::Layer,
        "voice" => EntityKind::Voice,
        "se" => EntityKind::Se,
        "bgm" => EntityKind::Bgm,
        "bus" => EntityKind::AudioBus,
        "mix" => EntityKind::MixerSnapshot,
        "duck" => EntityKind::Ducking,
        "motion" => EntityKind::Motion,
        "rig" => EntityKind::Rig,
        "slot" => EntityKind::Slot,
        "target" => EntityKind::Target,
        "scope" => EntityKind::Other("scope".to_owned()),
        "ent" => EntityKind::Other("ent".to_owned()),
        _ => return None,
    })
}

pub(super) fn expr_path_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) => Some(path.clone()),
        Expr::Field { target, field } => Some(format!("{}.{}", expr_path_label(target)?, field)),
        _ => None,
    }
}

pub(super) fn entity_kind_for_decl(kind: EntityDeclKind) -> EntityKind {
    match kind {
        EntityDeclKind::Asset => EntityKind::Asset,
        EntityDeclKind::Image => EntityKind::Image,
        EntityDeclKind::Character => EntityKind::Character,
        EntityDeclKind::Component => EntityKind::Component,
        EntityDeclKind::Activity => EntityKind::Activity,
        EntityDeclKind::Signal => EntityKind::Signal,
        EntityDeclKind::Metric => EntityKind::Metric,
        EntityDeclKind::Layer => EntityKind::Layer,
        EntityDeclKind::Textbox => EntityKind::Textbox,
        EntityDeclKind::Voice => EntityKind::Voice,
        EntityDeclKind::Se => EntityKind::Se,
        EntityDeclKind::Bgm => EntityKind::Bgm,
        EntityDeclKind::AudioBus => EntityKind::AudioBus,
        EntityDeclKind::MixerSnapshot => EntityKind::MixerSnapshot,
        EntityDeclKind::Ducking => EntityKind::Ducking,
        EntityDeclKind::Motion => EntityKind::Motion,
        EntityDeclKind::Rig => EntityKind::Rig,
    }
}

pub(super) fn literal_type(literal: &Literal) -> Option<TypeKind> {
    match literal {
        Literal::String(_) => Some(TypeKind::String),
        Literal::Char { .. } => Some(TypeKind::Char),
        Literal::Int { suffix, .. } => numeric_literal_suffix_type(suffix.as_deref()),
        Literal::Float { suffix, .. } => {
            numeric_literal_suffix_type(suffix.as_ref().map(|suffix| suffix.as_str()))
        }
        Literal::UnitNumber { suffix, .. } => numeric_literal_suffix_type(Some(suffix.as_str())),
        Literal::Bool(_) => Some(TypeKind::Bool),
        Literal::Duration { .. } => Some(TypeKind::Duration),
    }
}

pub(super) fn numeric_suffix_type(suffix: Option<&str>) -> Option<TypeKind> {
    suffix.and_then(TypeKind::primitive_name)
}

pub(super) fn numeric_literal_suffix_type(suffix: Option<&str>) -> Option<TypeKind> {
    numeric_suffix_type(suffix).or_else(|| {
        let suffix = suffix?;
        Some(match suffix {
            "px" | "pt" | "em" | "rem" | "vw" | "vh" | "%" => TypeKind::Named("Length".to_owned()),
            "deg" | "rad" | "turn" => TypeKind::Named("Angle".to_owned()),
            "db" | "lufs" => TypeKind::Named("AudioLevel".to_owned()),
            "bpm" => TypeKind::Named("Tempo".to_owned()),
            _ => return None,
        })
    })
}

pub(super) fn is_dialogue_callee_type(ty: Option<&TypeKind>) -> bool {
    ty.is_some_and(|ty| ty.is_entity_ref_kind(&EntityKind::Character))
        || matches!(ty, Some(TypeKind::Speaker(_)))
        || matches!(ty, Some(TypeKind::SpeakerPreset(_)))
        || matches!(ty, Some(TypeKind::Named(name)) if name == "SpeakerPreset")
}

pub(super) fn is_character_entity_literal(source: &str) -> bool {
    let trimmed = source.trim();
    trimmed
        .strip_prefix("@<")
        .and_then(|inner| inner.strip_suffix('>'))
        .map_or_else(
            || trimmed.strip_prefix("@character.").is_some(),
            |inner| inner.starts_with("character."),
        )
}

pub(super) fn typed_pattern_binding(pattern: &Pattern) -> Option<(&str, &TypeRef)> {
    match pattern {
        Pattern::Typed { name, ty } => Some((name, ty)),
        _ => None,
    }
}

pub(super) fn ident_pattern_name(pattern: &Pattern) -> Option<&str> {
    match pattern {
        Pattern::Ident(name) => Some(name),
        _ => None,
    }
}

pub(super) fn iter_item_type(source_type: Option<&TypeKind>) -> TypeKind {
    match source_type {
        Some(
            TypeKind::Vec(item)
            | TypeKind::Array { item, .. }
            | TypeKind::Seq(item)
            | TypeKind::Slice(item)
            | TypeKind::Stream { item, .. }
            | TypeKind::Source { item, .. },
        ) => item.as_ref().clone(),
        Some(TypeKind::Named(name)) => named_iter_item_type(name).map_or_else(
            || TypeKind::Named("ChoiceOptionSource".to_owned()),
            TypeKind::Named,
        ),
        _ => TypeKind::Named("ChoiceOptionSource".to_owned()),
    }
}

pub(crate) fn named_iter_item_type(name: &str) -> Option<String> {
    if let Some(inner) = generic_named_type_arg(name, "Vec")
        .or_else(|| generic_named_type_arg(name, "Seq"))
        .or_else(|| generic_named_type_arg(name, "Slice"))
    {
        return Some(inner.to_owned());
    }
    let inner = generic_named_type_arg(name, "Array")?;
    Some(
        inner
            .split_once(',')
            .map_or(inner, |(item, _)| item)
            .trim()
            .to_owned(),
    )
}

pub(super) fn generic_named_type_arg<'a>(name: &'a str, base: &str) -> Option<&'a str> {
    name.strip_prefix(base)?
        .strip_prefix('<')?
        .strip_suffix('>')
        .map(str::trim)
}

pub(super) fn well_known_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "choice_id" | "id" => TypeKind::entity_ref(EntityKind::ChoiceOption),
        "route_override" => TypeKind::Option(Box::new(TypeKind::entity_ref(EntityKind::Flow))),
        "target" => TypeKind::entity_ref(EntityKind::Flow),
        "enabled" | "visible" | "ready" => TypeKind::Bool,
        "order" | "count" | "index" => TypeKind::I64,
        "ratio" => TypeKind::F64,
        "stage" => TypeKind::Named("StageApi".to_owned()),
        "label" | "disabled_reason" | "badge" | "hotkey" | "text" => TypeKind::String,
        _ => return None,
    })
}

pub(super) fn let_else_bindings(
    pattern: &Pattern,
    expr_type: Option<&TypeKind>,
) -> Vec<(String, TypeKind)> {
    match pattern {
        Pattern::Ident(name) => expr_type
            .cloned()
            .map(|ty| vec![(name.to_owned(), ty)])
            .unwrap_or_default(),
        Pattern::MutIdent(name) => expr_type
            .cloned()
            .map(|ty| vec![(name.to_owned(), ty)])
            .unwrap_or_default(),
        Pattern::Variant { name, payload, .. } => payload
            .iter()
            .flat_map(variant_payload_bindings)
            .filter_map(|binding| {
                variant_payload_type_for_name(name, expr_type).map(|ty| (binding, ty))
            })
            .collect(),
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(|item| let_else_bindings(item, None))
            .collect(),
        Pattern::BracketSeq { items, rest } => {
            let mut bindings = items
                .iter()
                .flat_map(|item| let_else_bindings(item, None))
                .collect::<Vec<_>>();
            if let Some(rest) = rest.as_ref().filter(|name| is_local_ident(name)) {
                bindings.push((rest.to_owned(), TypeKind::Unit));
            }
            bindings
        }
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| let_else_bindings(field.pattern(), None))
            .collect(),
        Pattern::Whole { name, pattern } => {
            let mut bindings = expr_type
                .cloned()
                .map(|ty| vec![(name.to_owned(), ty)])
                .unwrap_or_default();
            bindings.extend(let_else_bindings(pattern, expr_type));
            bindings
        }
        Pattern::Typed { name, ty } => vec![(name.to_owned(), type_ref_kind(ty))],
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => Vec::new(),
    }
}

pub(super) fn pattern_bindings_with_fallback(
    pattern: &Pattern,
    fallback: &TypeKind,
) -> Vec<(String, TypeKind)> {
    let mut bindings = let_else_bindings(pattern, Some(fallback));
    for name in collect_pattern_binding_names(pattern) {
        if !bindings.iter().any(|(bound, _)| bound == &name) {
            bindings.push((name, TypeKind::Unit));
        }
    }
    bindings
}

pub(super) fn collect_pattern_binding_names(pattern: &Pattern) -> Vec<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) if is_local_ident(name) => {
            vec![name.to_owned()]
        }
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(collect_pattern_binding_names)
            .collect(),
        Pattern::BracketSeq { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(collect_pattern_binding_names)
                .collect::<Vec<_>>();
            if let Some(rest) = rest.as_ref().filter(|name| is_local_ident(name)) {
                names.push(rest.to_owned());
            }
            names
        }
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| collect_pattern_binding_names(field.pattern()))
            .collect(),
        Pattern::Variant { payload, .. } => payload
            .iter()
            .flat_map(|payload| match payload {
                VariantPatternPayload::Tuple(items) => items
                    .iter()
                    .flat_map(collect_pattern_binding_names)
                    .collect::<Vec<_>>(),
                VariantPatternPayload::Record { fields, .. } => fields
                    .iter()
                    .flat_map(|field| collect_pattern_binding_names(field.pattern()))
                    .collect(),
            })
            .collect(),
        Pattern::Whole { name, pattern } => {
            let mut names = is_local_ident(name)
                .then(|| name.to_owned())
                .into_iter()
                .collect::<Vec<_>>();
            names.extend(collect_pattern_binding_names(pattern));
            names
        }
        Pattern::Typed { name, .. } if is_local_ident(name) => vec![name.to_owned()],
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Discard
        | Pattern::Raw(_)
        | Pattern::Typed { .. }
        | Pattern::Ident(_)
        | Pattern::MutIdent(_) => Vec::new(),
    }
}

pub(super) fn variant_payload_bindings(payload: &VariantPatternPayload) -> Vec<String> {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .filter_map(|pattern| match pattern {
                Pattern::Ident(name) if is_local_ident(name) => Some(name.to_owned()),
                _ => None,
            })
            .collect(),
        VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| {
                let names = let_else_bindings(field.pattern(), None);
                if names.is_empty() {
                    vec![(field.name().to_owned(), TypeKind::Unit)]
                } else {
                    names
                }
            })
            .map(|(name, _)| name)
            .collect(),
    }
}

pub(super) fn option_payload_type(expr_type: Option<&TypeKind>) -> Option<TypeKind> {
    match expr_type {
        Some(TypeKind::Option(inner)) => Some(inner.as_ref().clone()),
        Some(TypeKind::Named(name)) if name == "Option<Ref<Flow>>" => {
            Some(TypeKind::entity_ref(EntityKind::Flow))
        }
        Some(TypeKind::Named(name)) if name == "Option<Bool>" => Some(TypeKind::Bool),
        Some(TypeKind::Named(name)) if name == "Option<i64>" => Some(TypeKind::I64),
        Some(TypeKind::Named(name)) if name == "Option<String>" => Some(TypeKind::String),
        _ => None,
    }
}

pub(super) fn variant_payload_type(expr_type: Option<&TypeKind>) -> Option<TypeKind> {
    option_payload_type(expr_type).or_else(|| expr_type.cloned())
}

pub(super) fn variant_payload_type_for_name(
    variant: &str,
    expr_type: Option<&TypeKind>,
) -> Option<TypeKind> {
    match (variant, expr_type) {
        ("Ok", Some(TypeKind::Result { ok, .. })) => Some(ok.as_ref().clone()),
        ("Err", Some(TypeKind::Result { error, .. })) => Some(error.as_ref().clone()),
        ("Some", _) => option_payload_type(expr_type),
        _ => variant_payload_type(expr_type),
    }
}

pub(super) fn is_drop_callee(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(name) if is_drop_name(name))
        || matches!(expr, Expr::Call { callee, .. } if is_drop_callee(callee))
}

pub(super) fn is_drop_name(name: &str) -> bool {
    matches!(name, "drop" | "drop_optional" | "on_drop")
}

pub(super) fn result_ok_type(name: &str) -> Option<TypeKind> {
    let inner = name
        .strip_prefix("Result<")
        .and_then(|value| value.strip_suffix('>'))?;
    let ok = inner.split_once(',').map_or(inner, |(ok, _)| ok).trim();
    Some(named_type_label(ok))
}

pub(super) fn well_known_runtime_method_type(name: &str) -> Option<TypeKind> {
    if let Some(ty) = well_known_static_capacity_method_type(name) {
        return Some(ty);
    }
    if matches!(name, "panic" | "fail" | "bail") {
        return Some(TypeKind::Never);
    }
    if matches!(name, "ensure" | "assert" | "debug_assert") {
        return Some(TypeKind::Unit);
    }
    if name == "load_bg" {
        return Some(TypeKind::Need {
            ready: Box::new(TypeKind::Named("ImageHandle".to_owned())),
            error: Box::new(TypeKind::Named("ArcError".to_owned())),
        });
    }
    if name == "asset.image" {
        return Some(TypeKind::Need {
            ready: Box::new(TypeKind::Named("ImageHandle".to_owned())),
            error: Box::new(TypeKind::Named("AssetError".to_owned())),
        });
    }
    if name == "voice.load" {
        return Some(TypeKind::Need {
            ready: Box::new(TypeKind::Named("VoiceHandle".to_owned())),
            error: Box::new(TypeKind::Named("VoiceError".to_owned())),
        });
    }
    if name == "len" {
        return Some(TypeKind::I64);
    }
    (name.starts_with("log.")
        || matches!(
            name,
            "drop"
                | "drop_optional"
                | "on_drop"
                | "signal.set"
                | "metric.set"
                | "event.emit"
                | "adapter.events"
                | "scene.show"
                | "scene.clear"
                | "progress.set"
                | "meter.show"
                | "text.show"
                | "text.flush"
                | "voice.stop"
                | "cues.stop"
        ))
    .then_some(TypeKind::Unit)
}

pub(super) fn well_known_static_capacity_method_type(name: &str) -> Option<TypeKind> {
    if let Some(item) = name
        .strip_prefix("Vec<")
        .and_then(|tail| tail.strip_suffix(">::with_capacity"))
    {
        return Some(TypeKind::Vec(Box::new(named_type_label(item.trim()))));
    }
    match name {
        "Vec.with_capacity" => Some(TypeKind::Vec(Box::new(TypeKind::Named("_".to_owned())))),
        "String.with_capacity" => Some(TypeKind::String),
        "Bytes.with_capacity" => Some(TypeKind::Named("Bytes".to_owned())),
        _ => None,
    }
}

pub(super) fn well_known_capacity_method_type(
    receiver: &TypeKind,
    method: &str,
    arg_count: usize,
) -> Option<TypeKind> {
    if matches!(receiver, TypeKind::String)
        && let ("trim" | "to_string", 0) = (method, arg_count)
    {
        return Some(TypeKind::String);
    }
    if matches!(receiver, TypeKind::Named(name) if name == "LineContext")
        && matches!((method, arg_count), ("voice_handle", 0))
    {
        return Some(TypeKind::Named("VoiceHandle".to_owned()));
    }
    if matches!(receiver, TypeKind::Named(name) if name == "StageApi")
        && matches!((method, arg_count), ("acquire", 1))
    {
        return Some(TypeKind::Named("StageActorHandle".to_owned()));
    }
    if matches!(receiver, TypeKind::Named(name) if name == "StageActorHandle")
        && matches!((method, arg_count), ("look", 1 | 2))
    {
        return Some(TypeKind::Named("CueHandle".to_owned()));
    }
    if let TypeKind::Vec(item) = receiver
        && matches!((method, arg_count), ("pop" | "pop_front", 0))
    {
        return Some(TypeKind::Option(item.clone()));
    }
    if let TypeKind::Vec(item) = receiver {
        match method {
            "collect" if arg_count == 0 => return Some(TypeKind::Vec(item.clone())),
            _ => {}
        }
    }
    if !is_reservable_type(receiver) {
        return None;
    }
    match (method, arg_count) {
        ("push" | "reserve" | "shrink_to", 1) | ("shrink", 0) => Some(TypeKind::Unit),
        _ => None,
    }
}

pub(super) fn is_reservable_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Vec(_) | TypeKind::String)
        || matches!(ty, TypeKind::Named(name) if name == "Bytes")
}

pub(super) fn collection_index_type(target_type: &TypeKind) -> Option<TypeKind> {
    match target_type {
        TypeKind::Vec(item) | TypeKind::Array { item, .. } | TypeKind::Slice(item) => {
            Some(item.as_ref().clone())
        }
        TypeKind::Map { value, .. } => Some(value.as_ref().clone()),
        TypeKind::String => Some(TypeKind::TextCluster),
        _ => None,
    }
}

pub(super) fn first_arg_type(types: &[Option<TypeKind>]) -> TypeKind {
    types
        .first()
        .and_then(Clone::clone)
        .unwrap_or(TypeKind::Unit)
}

pub(super) fn merge_line_output(
    current: TypeKind,
    next: &TypeKind,
    errors: &mut Vec<TypeCheckError>,
) -> TypeKind {
    if &current == next {
        return current;
    }
    if let Some(merged) = merge_result_types(&current, next) {
        return merged;
    }
    errors.push(TypeCheckError::new(format!(
        "line-plan out expressions must have the same type, found {current:?} and {next:?}"
    )));
    current
}

pub(super) fn merge_result_types(left: &TypeKind, right: &TypeKind) -> Option<TypeKind> {
    let (
        TypeKind::Result {
            ok: left_ok,
            error: left_error,
        },
        TypeKind::Result {
            ok: right_ok,
            error: right_error,
        },
    ) = (left, right)
    else {
        return None;
    };

    let ok = merge_placeholder_type(left_ok, right_ok)?;
    let error = merge_placeholder_type(left_error, right_error)?;
    Some(TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(error),
    })
}

pub(super) fn merge_placeholder_type(left: &TypeKind, right: &TypeKind) -> Option<TypeKind> {
    if left == right {
        return Some(left.clone());
    }
    if is_placeholder_type(left) {
        return Some(right.clone());
    }
    if is_placeholder_type(right) {
        return Some(left.clone());
    }
    None
}

pub(super) fn is_placeholder_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if name == "_")
}

pub(super) fn named_type_label(name: &str) -> TypeKind {
    TypeKind::primitive_name(name).unwrap_or_else(|| TypeKind::Named(name.to_owned()))
}

pub(super) fn unify_loop_break_types(types: &[TypeKind]) -> Option<TypeKind> {
    let first = types.first()?.clone();
    if types.iter().all(|ty| ty == &first) {
        Some(first)
    } else {
        None
    }
}

pub(super) fn stmts_diverge(stmts: &[Stmt]) -> bool {
    stmts.last().is_some_and(stmt_diverges)
}

pub(super) fn stmt_diverges(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Goto(_) | Stmt::Break { .. } | Stmt::Continue { .. } => true,
        Stmt::Expr(expr) => expr_diverges(expr),
        Stmt::Raw(raw) => raw.source().starts_with("break"),
        _ => false,
    }
}

pub(super) fn expr_diverges(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call { callee, .. }
            if matches!(expr_path_label(callee).as_deref(), Some("panic" | "fail" | "bail"))
    )
}

pub(super) fn is_local_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(super) fn choice_output_type(choice: &arcweft_lang_hir::model::HirChoice) -> Option<TypeKind> {
    let mut inferred = None;
    for option in choice.options() {
        let ty = match option.action() {
            ChoiceAction::Out(expr) => simple_expr_type(expr)?,
            ChoiceAction::SelectBlock(statements) => {
                let [Stmt::Out { expr, .. }] = statements.as_slice() else {
                    return None;
                };
                simple_expr_type(expr)?
            }
            ChoiceAction::Goto(_) | ChoiceAction::None => return None,
        };
        match &inferred {
            Some(existing) if existing != &ty => return None,
            Some(_) => {}
            None => inferred = Some(ty),
        }
    }
    inferred
}

pub(super) fn simple_expr_type(expr: &Expr) -> Option<TypeKind> {
    match expr {
        Expr::EntityRef(entity) => entity
            .as_absolute()
            .and_then(entity_kind)
            .map(TypeKind::entity_ref),
        Expr::Literal(literal) => literal_type(literal),
        Expr::Tuple(items) => items
            .iter()
            .map(simple_expr_type)
            .collect::<Option<Vec<_>>>()
            .map(TypeKind::Tuple),
        Expr::BracketSeq(items) => {
            let item = items
                .first()
                .and_then(simple_expr_type)
                .unwrap_or(TypeKind::Unit);
            Some(TypeKind::Vec(Box::new(item)))
        }
        Expr::ArrayRepeat { value, len } => {
            let item = simple_expr_type(value)?;
            Some(TypeKind::Array {
                item: Box::new(item),
                len: array_repeat_len_label(len)?,
            })
        }
        Expr::RecordLiteral(_) => Some(TypeKind::Named("Record".to_owned())),
        _ => None,
    }
}

pub(super) fn array_repeat_len_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) if *value >= 0 => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn array_len_matches(label: &str, actual: usize) -> bool {
    label
        .parse::<usize>()
        .ok()
        .or_else(|| label.strip_prefix('N')?.parse::<usize>().ok())
        .is_none_or(|expected| expected == actual)
}

pub(super) fn default_presentation_slot_family(expr: &Expr) -> Option<&'static str> {
    let Expr::Call { callee, args } = expr else {
        return None;
    };
    if args
        .iter()
        .any(|arg| matches!(arg, CallArg::Named { name, .. } if name == "slot"))
    {
        return None;
    }
    match callee.as_ref() {
        Expr::Path(name) if name == "bg" => Some("background"),
        Expr::Path(name) if name == "show" => Some("character"),
        _ => None,
    }
}

pub(super) fn await_branch_pattern_type(
    kind: AwaitBranchKind,
    ready: &TypeKind,
    error: &TypeKind,
) -> TypeKind {
    match kind {
        AwaitBranchKind::Pending => TypeKind::Named("Progress".to_owned()),
        AwaitBranchKind::Ready => ready.clone(),
        AwaitBranchKind::Error => error.clone(),
        AwaitBranchKind::Denied => TypeKind::Named("AwaitDenied".to_owned()),
    }
}

pub(super) fn is_map_type_name(name: &str) -> bool {
    matches!(name, "OrderedMap" | "SortedMap" | "BTreeMap")
}

pub(super) fn map_kind_for_type_name(name: &str) -> MapKind {
    match name {
        "OrderedMap" => MapKind::Ordered,
        "SortedMap" => MapKind::Sorted,
        "BTreeMap" => MapKind::BTree,
        _ => unreachable!("map type names are filtered before kind selection"),
    }
}

pub(crate) fn type_ref_kind(ty: &TypeRef) -> TypeKind {
    match ty {
        TypeRef::Never => TypeKind::Never,
        TypeRef::ConstInt(value) => TypeKind::Named(value.to_string()),
        TypeRef::Path(path) => named_type_label(path),
        TypeRef::Choice(alternatives) => {
            normalize_choice_type(alternatives.iter().map(type_ref_kind).collect::<Vec<_>>())
        }
        TypeRef::Generic { base, args } if base == "Vec" && args.len() == 1 => {
            TypeKind::Vec(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Array" && args.len() == 2 => TypeKind::Array {
            item: Box::new(type_ref_kind(&args[0])),
            len: type_ref_label(&args[1]),
        },
        TypeRef::Generic { base, args } if base == "Seq" && args.len() == 1 => {
            TypeKind::Seq(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if is_map_type_name(base) && args.len() == 2 => {
            TypeKind::Map {
                kind: map_kind_for_type_name(base),
                key: Box::new(type_ref_kind(&args[0])),
                value: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Generic { base, args } if base == "Result" && args.len() == 2 => {
            TypeKind::Result {
                ok: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Generic { base, args } if base == "ArcResult" && args.len() == 1 => {
            TypeKind::Result {
                ok: Box::new(type_ref_kind(&args[0])),
                error: Box::new(TypeKind::Named("ArcError".to_owned())),
            }
        }
        TypeRef::Generic { base, args } if base == "Option" && args.len() == 1 => {
            TypeKind::Option(Box::new(type_ref_kind(&args[0])))
        }
        TypeRef::Generic { base, args } if base == "Need" && args.len() == 2 => TypeKind::Need {
            ready: Box::new(type_ref_kind(&args[0])),
            error: Box::new(type_ref_kind(&args[1])),
        },
        TypeRef::Generic { base, args } if base == "Stream" && args.len() == 2 => {
            TypeKind::Stream {
                item: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Generic { base, args } if base == "Source" && args.len() == 2 => {
            TypeKind::Source {
                item: Box::new(type_ref_kind(&args[0])),
                error: Box::new(type_ref_kind(&args[1])),
            }
        }
        TypeRef::Ref { lifetime, inner } => TypeKind::BorrowRef {
            lifetime: lifetime
                .as_ref()
                .map(|lifetime| LifetimeScopeKind::parse(lifetime.name())),
            inner: Box::new(type_ref_kind(inner)),
        },
        TypeRef::Slice(inner) => array_type_from_slice_inner(inner)
            .unwrap_or_else(|| TypeKind::Slice(Box::new(type_ref_kind(inner)))),
        TypeRef::Generic { .. } => TypeKind::Named(type_ref_label(ty)),
    }
}

fn array_type_from_slice_inner(inner: &TypeRef) -> Option<TypeKind> {
    let TypeRef::Path(path) = inner else {
        return None;
    };
    let (item, len) = path.split_once(';')?;
    Some(TypeKind::Array {
        item: Box::new(named_type_label(item.trim())),
        len: len.trim().to_owned(),
    })
}

pub(super) fn stream_return_types(ty: &TypeRef) -> Option<(TypeKind, TypeKind)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Stream" && args.len() == 2 => {
            Some((type_ref_kind(&args[0]), type_ref_kind(&args[1])))
        }
        TypeRef::Generic { base, .. } if base == "Source" => None,
        _ => None,
    }
}

pub(super) fn source_return_types(ty: &TypeRef) -> Option<(TypeKind, TypeKind)> {
    match ty {
        TypeRef::Generic { base, args } if base == "Source" && args.len() == 2 => {
            Some((type_ref_kind(&args[0]), type_ref_kind(&args[1])))
        }
        _ => None,
    }
}

pub(super) fn type_ref_label(ty: &TypeRef) -> String {
    match ty {
        TypeRef::Never => "Never".to_owned(),
        TypeRef::ConstInt(value) => value.to_string(),
        TypeRef::Path(path) => path.clone(),
        TypeRef::Choice(alternatives) => alternatives
            .iter()
            .map(type_ref_label)
            .collect::<Vec<_>>()
            .join(" | "),
        TypeRef::Generic { base, args } => format!(
            "{base}<{}>",
            args.iter()
                .map(type_ref_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeRef::Ref { lifetime, inner } => {
            let lifetime = lifetime
                .as_ref()
                .map(|lifetime| format!("'{} ", lifetime.name()))
                .unwrap_or_default();
            format!("&{lifetime}{}", type_ref_label(inner))
        }
        TypeRef::Slice(inner) => format!("[{}]", type_ref_label(inner)),
    }
}

pub(super) fn normalize_choice_type(alternatives: Vec<TypeKind>) -> TypeKind {
    let mut flattened = alternatives
        .into_iter()
        .flat_map(|ty| match ty {
            TypeKind::Choice(alternatives) => alternatives,
            ty => vec![ty],
        })
        .collect::<Vec<_>>();
    flattened.sort_by_key(type_kind_label);
    flattened.dedup();
    match flattened.as_slice() {
        [single] => single.clone(),
        _ => TypeKind::Choice(flattened),
    }
}

fn atomic_type_kind_label(ty: &TypeKind) -> Option<&'static str> {
    match ty {
        TypeKind::Bool => Some("Bool"),
        TypeKind::I8 => Some("i8"),
        TypeKind::I16 => Some("i16"),
        TypeKind::I32 => Some("i32"),
        TypeKind::I64 => Some("i64"),
        TypeKind::I128 => Some("i128"),
        TypeKind::ISize => Some("isize"),
        TypeKind::U8 => Some("u8"),
        TypeKind::U16 => Some("u16"),
        TypeKind::U32 => Some("u32"),
        TypeKind::U64 => Some("u64"),
        TypeKind::U128 => Some("u128"),
        TypeKind::USize => Some("usize"),
        TypeKind::F32 => Some("f32"),
        TypeKind::F64 => Some("f64"),
        TypeKind::String => Some("String"),
        TypeKind::Char => Some("Char"),
        TypeKind::TextCluster => Some("TextCluster"),
        TypeKind::Duration => Some("Duration"),
        TypeKind::Range => Some("Range"),
        TypeKind::DisplayText => Some("DisplayText"),
        TypeKind::Predicate => Some("Predicate"),
        TypeKind::Observation => Some("Observation"),
        TypeKind::ActionName => Some("ActionName"),
        TypeKind::ActionResult => Some("ActionResult"),
        TypeKind::AgentValue => Some("AgentValue"),
        TypeKind::CaptureTarget => Some("CaptureTarget"),
        TypeKind::CaptureRef => Some("CaptureRef"),
        TypeKind::AgentResource => Some("AgentResource"),
        TypeKind::AgentResourceBody => Some("AgentResourceBody"),
        TypeKind::RagContextPack => Some("RagContextPack"),
        TypeKind::FocusPatch => Some("FocusPatch"),
        TypeKind::Unit => Some("()"),
        TypeKind::Never => Some("Never"),
        _ => None,
    }
}

pub(super) fn type_kind_label(ty: &TypeKind) -> String {
    if let Some(label) = atomic_type_kind_label(ty) {
        return label.to_owned();
    }

    match ty {
        TypeKind::Ref(entity) => entity_type_label(entity),
        TypeKind::Probe(inner) => format!("Probe<{}>", type_kind_label(inner)),
        TypeKind::Vec(inner) => format!("Vec<{}>", type_kind_label(inner)),
        TypeKind::Array { item, len } => format!("Array<{}, {len}>", type_kind_label(item)),
        TypeKind::Slice(inner) => format!("[{}]", type_kind_label(inner)),
        TypeKind::Seq(inner) => format!("Seq<{}>", type_kind_label(inner)),
        TypeKind::Map { kind, key, value } => format!(
            "{kind:?}<{}, {}>",
            type_kind_label(key),
            type_kind_label(value)
        ),
        TypeKind::BorrowRef { lifetime, inner } => {
            format!("&{lifetime:?} {}", type_kind_label(inner))
        }
        TypeKind::Need { ready, error } => {
            format!(
                "Need<{}, {}>",
                type_kind_label(ready),
                type_kind_label(error)
            )
        }
        TypeKind::Stream { item, error } => {
            format!(
                "Stream<{}, {}>",
                type_kind_label(item),
                type_kind_label(error)
            )
        }
        TypeKind::Source { item, error } => {
            format!(
                "Source<{}, {}>",
                type_kind_label(item),
                type_kind_label(error)
            )
        }
        TypeKind::Result { ok, error } => {
            format!(
                "Result<{}, {}>",
                type_kind_label(ok),
                type_kind_label(error)
            )
        }
        TypeKind::Option(inner) => format!("Option<{}>", type_kind_label(inner)),
        TypeKind::Handle {
            name,
            lifetime,
            state,
            must_drop,
        } => format!("Handle<{name}, {lifetime:?}, {state:?}, {must_drop}>"),
        TypeKind::ThreadHandle(inner) => format!("ThreadHandle<{}>", type_kind_label(inner)),
        TypeKind::Shared(inner) => format!("Shared<{}>", type_kind_label(inner)),
        TypeKind::Function { return_type } => format!("fn -> {}", type_kind_label(return_type)),
        TypeKind::Speaker(kind) => format!("Speaker<{kind:?}>"),
        TypeKind::SpeakerPreset(kind) => format!("SpeakerPreset<{kind:?}>"),
        TypeKind::CharacterPatch(kind) => format!("CharacterPatch<{kind:?}>"),
        TypeKind::Named(name) => name.clone(),
        TypeKind::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(type_kind_label)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeKind::Choice(alternatives) => alternatives
            .iter()
            .map(type_kind_label)
            .collect::<Vec<_>>()
            .join(" | "),
        _ => unreachable!("atomic type labels are handled before structured labels"),
    }
}

fn entity_type_label(entity: &crate::types::EntityType) -> String {
    if let Some(value) = entity.value() {
        format!("Ref<{:?}, {}>", entity.kind(), type_kind_label(value))
    } else {
        format!("Ref<{:?}>", entity.kind())
    }
}
