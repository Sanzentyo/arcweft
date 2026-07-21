use super::{
    CallArg, EffectId, EffectSet, EnumVariantPayload, Expr, FnParam, FnSignature, FunctionParam,
    FunctionParamHigherOrderBinding, FunctionParamSelector, FunctionParamSelectorSegment,
    FunctionSignature, HashMap, NominalTypeContext, Pattern, RecordPatternField, TypeCheckEnv,
    TypeKind, VariantPatternPayload, expr_path_label, is_local_ident, type_ref_kind,
    variant_payload_type_for_name,
};

pub(super) fn available_effect_set(env: &TypeCheckEnv) -> Option<EffectSet> {
    env.available_effects().map(|available| {
        available
            .iter()
            .filter_map(|capability| EffectId::parse(capability.as_str()).ok())
            .collect::<EffectSet>()
    })
}

pub(crate) fn function_signature_type(signature: &FnSignature) -> FunctionSignature {
    function_signature_type_with_nominal_types(signature, NominalTypeContext::empty())
}

pub(super) fn function_signature_type_with_nominal_types(
    signature: &FnSignature,
    nominal_types: NominalTypeContext<'_>,
) -> FunctionSignature {
    let return_type = curried_signature_return_type(signature);
    let params = signature
        .param_groups()
        .first()
        .into_iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .map(|param| function_param_type(param, nominal_types))
        .collect::<Vec<_>>();
    let remaining_param_groups = signature
        .param_groups()
        .iter()
        .skip(1)
        .map(|group| {
            group
                .params()
                .iter()
                .map(|param| function_param_type(param, nominal_types))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    FunctionSignature::new(return_type, params).with_remaining_param_groups(remaining_param_groups)
}

fn curried_signature_return_type(signature: &FnSignature) -> TypeKind {
    let return_type = signature
        .return_type()
        .map_or(TypeKind::Unit, |ty| type_ref_kind(ty.value()));
    signature
        .param_groups()
        .iter()
        .skip(1)
        .rev()
        .fold(return_type, |return_type, group| {
            TypeKind::function(
                group.params().iter().map(|param| {
                    param
                        .ty()
                        .map_or(TypeKind::Unit, |ty| type_ref_kind(ty.value()))
                }),
                return_type,
            )
        })
}

fn function_param_type(param: &FnParam, nominal_types: NominalTypeContext<'_>) -> FunctionParam {
    let ty = param
        .ty()
        .map_or(TypeKind::Unit, |ty| type_ref_kind(ty.value()));
    FunctionParam::new(
        pattern_param_name(param.pattern()),
        ty.clone(),
        param.kind(),
        param.default().is_some(),
        function_param_higher_order_bindings(param.pattern(), &ty, nominal_types),
    )
}

pub(super) fn function_param_higher_order_bindings(
    pattern: &Pattern,
    ty: &TypeKind,
    nominal_types: NominalTypeContext<'_>,
) -> Vec<FunctionParamHigherOrderBinding> {
    let mut bindings = Vec::new();
    collect_function_param_higher_order_bindings(
        pattern,
        ty,
        FunctionParamSelector::Root,
        nominal_types,
        &mut bindings,
    );
    bindings
}

fn collect_function_param_higher_order_bindings(
    pattern: &Pattern,
    ty: &TypeKind,
    selector: FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. }
            if is_local_ident(name) && matches!(ty, TypeKind::Function { .. }) =>
        {
            bindings.push(FunctionParamHigherOrderBinding::new(
                name.clone(),
                ty.clone(),
                selector,
            ));
        }
        Pattern::Tuple(items) => {
            let TypeKind::Tuple(item_types) = ty else {
                return;
            };
            collect_tuple_function_param_higher_order_bindings(
                items,
                item_types,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Whole { name, pattern } => {
            if is_local_ident(name) && matches!(ty, TypeKind::Function { .. }) {
                bindings.push(FunctionParamHigherOrderBinding::new(
                    name.clone(),
                    ty.clone(),
                    selector.clone(),
                ));
            }
            collect_function_param_higher_order_bindings(
                pattern,
                ty,
                selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Record { fields, .. } => {
            collect_record_function_param_higher_order_bindings(
                pattern,
                fields,
                ty,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::Variant {
            payload: Some(VariantPatternPayload::Record { fields, rest: _ }),
            name,
            ..
        } => {
            collect_variant_record_function_param_higher_order_bindings(
                name,
                fields,
                pattern,
                ty,
                &selector,
                nominal_types,
                bindings,
            );
        }
        Pattern::BracketSeq { items, .. }
        | Pattern::Variant {
            payload: Some(VariantPatternPayload::Tuple(items)),
            name: _,
            ..
        } => {
            if let Pattern::Variant { name, .. } = pattern {
                collect_variant_tuple_function_param_higher_order_bindings(
                    name,
                    items,
                    ty,
                    &selector,
                    nominal_types,
                    bindings,
                );
            } else {
                collect_bracket_seq_function_param_higher_order_bindings(
                    items,
                    &selector,
                    nominal_types,
                    bindings,
                );
            }
        }
        Pattern::Ident(_)
        | Pattern::MutIdent(_)
        | Pattern::Typed { .. }
        | Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Discard
        | Pattern::Raw(_)
        | Pattern::Variant { payload: None, .. } => {}
    }
}

fn collect_tuple_function_param_higher_order_bindings(
    items: &[Pattern],
    item_types: &[TypeKind],
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for (index, (item, item_ty)) in items.iter().zip(item_types).enumerate() {
        collect_function_param_higher_order_bindings(
            item,
            item_ty,
            selector_with_tuple_index(selector, index),
            nominal_types,
            bindings,
        );
    }
}

fn collect_record_function_param_higher_order_bindings(
    pattern: &Pattern,
    fields: &[RecordPatternField],
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for field in fields {
        let Some(field_ty) = pattern_type_hint(field.pattern())
            .or_else(|| record_pattern_field_type(pattern, ty, field.name(), nominal_types.fields))
        else {
            continue;
        };
        collect_function_param_higher_order_bindings(
            field.pattern(),
            &field_ty,
            selector_with_record_field(selector, field.name()),
            nominal_types,
            bindings,
        );
    }
}

fn collect_variant_record_function_param_higher_order_bindings(
    variant: &str,
    fields: &[RecordPatternField],
    pattern: &Pattern,
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    let payload_ty = variant_payload_type_for_name(variant, Some(ty));
    let nominal_payload = enum_variant_payload_type_for_name(
        variant,
        ty,
        nominal_types.variant_payloads,
        nominal_types.env,
    );
    let payload_selector = selector_with_variant_payload(selector, variant);
    for field in fields {
        let Some(field_ty) = pattern_type_hint(field.pattern()).or_else(|| {
            nominal_payload
                .as_ref()
                .and_then(|payload| payload.record_field_type(field.name()))
                .or_else(|| {
                    payload_ty.as_ref().and_then(|payload_ty| {
                        record_pattern_field_type(
                            pattern,
                            payload_ty,
                            field.name(),
                            nominal_types.fields,
                        )
                    })
                })
        }) else {
            continue;
        };
        collect_function_param_higher_order_bindings(
            field.pattern(),
            &field_ty,
            selector_with_record_field(&payload_selector, field.name()),
            nominal_types,
            bindings,
        );
    }
}

fn collect_variant_tuple_function_param_higher_order_bindings(
    variant: &str,
    items: &[Pattern],
    ty: &TypeKind,
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    let nominal_payload = enum_variant_payload_type_for_name(
        variant,
        ty,
        nominal_types.variant_payloads,
        nominal_types.env,
    );
    let Some(payload_ty) = nominal_payload
        .as_ref()
        .and_then(EnumVariantPayload::single_type)
        .or_else(|| {
            nominal_payload
                .is_none()
                .then(|| variant_payload_type_for_name(variant, Some(ty)))
                .flatten()
        })
    else {
        return;
    };
    let payload_selector = selector_with_variant_payload(selector, variant);
    if items.len() == 1 {
        collect_function_param_higher_order_bindings(
            &items[0],
            &payload_ty,
            payload_selector,
            nominal_types,
            bindings,
        );
        return;
    }
    let item_types = match payload_ty {
        TypeKind::Tuple(item_types) => item_types,
        _ => nominal_payload
            .as_ref()
            .and_then(EnumVariantPayload::tuple_items)
            .unwrap_or_default(),
    };
    if item_types.is_empty() {
        return;
    }
    collect_tuple_function_param_higher_order_bindings(
        items,
        &item_types,
        &payload_selector,
        nominal_types,
        bindings,
    );
}

fn collect_bracket_seq_function_param_higher_order_bindings(
    items: &[Pattern],
    selector: &FunctionParamSelector,
    nominal_types: NominalTypeContext<'_>,
    bindings: &mut Vec<FunctionParamHigherOrderBinding>,
) {
    for item in items {
        collect_function_param_higher_order_bindings(
            item,
            &TypeKind::Unit,
            selector.clone(),
            nominal_types,
            bindings,
        );
    }
}

fn selector_with_tuple_index(
    selector: &FunctionParamSelector,
    index: usize,
) -> FunctionParamSelector {
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::TupleIndex(vec![index]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path.clone();
            path.push(index);
            FunctionParamSelector::TupleIndex(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(FunctionParamSelectorSegment::TupleIndex(index));
            FunctionParamSelector::Path(path)
        }
    }
}

fn selector_with_record_field(
    selector: &FunctionParamSelector,
    field: &str,
) -> FunctionParamSelector {
    let segment = FunctionParamSelectorSegment::RecordField(field.to_owned());
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::Path(vec![segment]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path
                .iter()
                .copied()
                .map(FunctionParamSelectorSegment::TupleIndex)
                .collect::<Vec<_>>();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
    }
}

fn selector_with_variant_payload(
    selector: &FunctionParamSelector,
    variant: &str,
) -> FunctionParamSelector {
    let segment = FunctionParamSelectorSegment::VariantPayload(normalize_variant_name(variant));
    match selector {
        FunctionParamSelector::Root => FunctionParamSelector::Path(vec![segment]),
        FunctionParamSelector::TupleIndex(path) => {
            let mut path = path
                .iter()
                .copied()
                .map(FunctionParamSelectorSegment::TupleIndex)
                .collect::<Vec<_>>();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
        FunctionParamSelector::Path(path) => {
            let mut path = path.clone();
            path.push(segment);
            FunctionParamSelector::Path(path)
        }
    }
}

pub(super) fn selected_higher_order_argument<'a>(
    selector: &FunctionParamSelector,
    value: &'a Expr,
    actual: &'a TypeKind,
    fallback_ty: &'a TypeKind,
) -> Option<(&'a Expr, &'a TypeKind)> {
    match selector {
        FunctionParamSelector::Root => Some((value, actual)),
        FunctionParamSelector::TupleIndex(path) => {
            let mut value = value;
            let mut actual = actual;
            for index in path {
                let (Expr::Tuple(values), TypeKind::Tuple(types)) = (value, actual) else {
                    return None;
                };
                value = values.get(*index)?;
                actual = types.get(*index)?;
            }
            Some((value, actual))
        }
        FunctionParamSelector::Path(path) => {
            let mut value = value;
            let mut actual = Some(actual);
            for segment in path {
                match segment {
                    FunctionParamSelectorSegment::TupleIndex(index) => {
                        let Expr::Tuple(values) = value else {
                            return None;
                        };
                        value = values.get(*index)?;
                        actual = match actual {
                            Some(TypeKind::Tuple(types)) => types.get(*index),
                            _ => None,
                        };
                    }
                    FunctionParamSelectorSegment::RecordField(field) => {
                        let (Expr::Record { fields, .. } | Expr::RecordLiteral(fields)) = value
                        else {
                            return None;
                        };
                        value = fields
                            .iter()
                            .find_map(|(name, value)| (name == field).then_some(value))?;
                        actual = None;
                    }
                    FunctionParamSelectorSegment::VariantPayload(variant) => match value {
                        Expr::Call(call) => {
                            let callee = expr_path_label(call.callee())?;
                            if !variant_constructor_matches(&callee, variant) {
                                return None;
                            }
                            let [CallArg::Positional(payload)] = call.args() else {
                                return None;
                            };
                            value = payload;
                            actual = None;
                        }
                        Expr::Record { path, .. } if variant_constructor_matches(path, variant) => {
                            actual = None;
                        }
                        _ => return None,
                    },
                }
            }
            Some((value, actual.unwrap_or(fallback_ty)))
        }
    }
}

fn normalize_variant_name(name: &str) -> String {
    name.strip_prefix('.').unwrap_or(name).to_owned()
}

fn variant_constructor_matches(path: &str, variant: &str) -> bool {
    let path = normalize_variant_name(path);
    path == variant
        || path
            .rsplit_once('.')
            .is_some_and(|(_, name)| name == variant)
}

fn pattern_type_hint(pattern: &Pattern) -> Option<TypeKind> {
    match pattern {
        Pattern::Typed { ty, .. } => Some(type_ref_kind(ty.value())),
        Pattern::Tuple(items) => items
            .iter()
            .map(pattern_type_hint)
            .collect::<Option<Vec<_>>>()
            .map(TypeKind::Tuple),
        Pattern::Whole { pattern, .. } => pattern_type_hint(pattern),
        _ => None,
    }
}

fn record_pattern_field_type(
    pattern: &Pattern,
    ty: &TypeKind,
    field: &str,
    nominal_fields: Option<&HashMap<String, HashMap<String, TypeKind>>>,
) -> Option<TypeKind> {
    let Pattern::Record { path, .. } = pattern else {
        return None;
    };
    let record_name = path.as_deref().or_else(|| match ty {
        TypeKind::Named(name) => Some(name.as_str()),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            nominal_record_type_name(inner)
        }
        _ => None,
    })?;
    nominal_fields?
        .get(record_name)
        .and_then(|fields| fields.get(field))
        .cloned()
}

pub(super) fn enum_variant_payload_type_for_name(
    variant: &str,
    ty: &TypeKind,
    nominal_variant_payloads: Option<&HashMap<String, HashMap<String, EnumVariantPayload>>>,
    env: Option<&TypeCheckEnv>,
) -> Option<EnumVariantPayload> {
    let variant = normalize_variant_name(variant);
    let variant = variant
        .rsplit_once('.')
        .map_or(variant.as_str(), |(_, name)| name);
    nominal_record_type_name(ty)
        .and_then(|enum_name| {
            nominal_variant_payloads?
                .get(enum_name)?
                .get(variant)
                .cloned()
        })
        .or_else(|| env_variant_payload_type_for_name(ty, variant, env))
}

fn env_variant_payload_type_for_name(
    ty: &TypeKind,
    variant: &str,
    env: Option<&TypeCheckEnv>,
) -> Option<EnumVariantPayload> {
    match ty {
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            env_variant_payload_type_for_name(inner, variant, env)
        }
        ty => env?.enum_variant_payload(ty, variant).cloned(),
    }
}

fn nominal_record_type_name(ty: &TypeKind) -> Option<&str> {
    match ty {
        TypeKind::Named(name) => Some(name),
        TypeKind::BorrowRef { inner, .. } | TypeKind::Shared(inner) => {
            nominal_record_type_name(inner)
        }
        _ => None,
    }
}

fn pattern_param_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}
