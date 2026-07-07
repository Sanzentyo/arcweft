use super::super::helpers::{named_type_label, type_ref_kind};
use crate::diagnostics::TypeCheckError;
use crate::env::{FunctionParam, FunctionSignature};
use crate::types::{EntityKind, MapKind, TypeKind};
use arcweft_lang_syntax::ast::pattern::Pattern;
use arcweft_lang_syntax::expr::{BinaryOp, Expr};
use arcweft_lang_syntax::types::FnParam;
pub(super) enum ChoicePatternCoverage {
    All,
    Type(TypeKind),
}

#[derive(Clone, Copy)]
pub(super) struct AgentInvokeArgs<'a> {
    pub(super) target: Option<&'a Expr>,
    pub(super) action: Option<&'a Expr>,
    pub(super) action_args: Option<&'a Expr>,
}

pub(super) fn choice_pattern_coverage(pattern: &Pattern) -> ChoicePatternCoverage {
    match pattern {
        Pattern::Typed { ty, .. } => ChoicePatternCoverage::Type(type_ref_kind(ty)),
        Pattern::Whole { pattern, .. } => choice_pattern_coverage(pattern),
        Pattern::Ident(_) | Pattern::MutIdent(_) | Pattern::Discard => ChoicePatternCoverage::All,
        Pattern::Literal(_)
        | Pattern::Entity(_)
        | Pattern::Variant { .. }
        | Pattern::Tuple(_)
        | Pattern::Record { .. }
        | Pattern::BracketSeq { .. }
        | Pattern::Raw(_) => ChoicePatternCoverage::Type(TypeKind::Never),
    }
}

pub(super) fn unique_numeric_choice_alternative(
    expected: &TypeKind,
    predicate: impl Fn(&TypeKind) -> bool,
) -> Option<TypeKind> {
    let TypeKind::Choice(alternatives) = expected else {
        return None;
    };
    let mut compatible_alternatives = alternatives
        .iter()
        .filter(|alternative| predicate(alternative));
    let selected = compatible_alternatives.next()?;
    compatible_alternatives
        .next()
        .is_none()
        .then(|| selected.clone())
}

pub(super) fn has_multiple_numeric_choice_alternatives(
    expected: &TypeKind,
    predicate: impl Fn(&TypeKind) -> bool,
) -> bool {
    let TypeKind::Choice(alternatives) = expected else {
        return false;
    };
    alternatives
        .iter()
        .filter(|alternative| predicate(alternative))
        .count()
        > 1
}

pub(super) enum TraitMethodCallOutcome {
    Missing,
    Typed(TypeKind),
    Rejected,
}

pub(super) enum BuiltinCollectionMethodCallOutcome {
    Missing,
    Checked(Option<TypeKind>),
}

pub(super) fn trait_method_call_signature(
    signature: &arcweft_lang_syntax::types::FnSignature,
    return_type: TypeKind,
) -> FunctionSignature {
    let return_type = curried_trait_method_return_type(signature, return_type);
    let params = signature
        .param_groups()
        .first()
        .into_iter()
        .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
        .filter(|param| !is_trait_receiver_param(param))
        .map(|param| {
            let name = match param.pattern() {
                Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
                    name.as_str()
                }
                _ => "_",
            };
            if param.is_rest() {
                FunctionParam::rest(name, type_ref_kind(param.ty()))
            } else if param.default().is_some() {
                FunctionParam::defaulted(name, type_ref_kind(param.ty()))
            } else {
                FunctionParam::required(name, type_ref_kind(param.ty()))
            }
        })
        .collect::<Vec<_>>();
    FunctionSignature::new(return_type, params)
        .with_remaining_call_groups(signature.param_groups().len().saturating_sub(1))
}

fn curried_trait_method_return_type(
    signature: &arcweft_lang_syntax::types::FnSignature,
    return_type: TypeKind,
) -> TypeKind {
    signature
        .param_groups()
        .iter()
        .skip(1)
        .rev()
        .fold(return_type, |return_type, group| TypeKind::Function {
            params: group
                .params()
                .iter()
                .filter(|param| !is_trait_receiver_param(param))
                .map(|param| type_ref_kind(param.ty()))
                .collect(),
            return_type: Box::new(return_type),
        })
}

fn is_trait_receiver_param(param: &FnParam) -> bool {
    param.receiver_kind().is_some()
        || matches!(
            param.ty(),
            arcweft_lang_syntax::types::TypeRef::Path(path) if path == "Self"
        )
}

pub(super) fn spread_item_type(ty: &TypeKind) -> Option<&TypeKind> {
    match ty {
        TypeKind::Vec(item)
        | TypeKind::Seq(item)
        | TypeKind::Slice(item)
        | TypeKind::Array { item, .. } => Some(item),
        _ => None,
    }
}

pub(super) fn join_branch_types(left: TypeKind, right: TypeKind) -> TypeKind {
    TypeKind::join_branch(left, right)
}

pub(super) fn rhs_expected_type_for_binary(
    op: BinaryOp,
    lhs_type: Option<&TypeKind>,
) -> Option<&TypeKind> {
    let lhs_type = lhs_type?;
    match op {
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Gte
        | BinaryOp::Lte
        | BinaryOp::Gt
        | BinaryOp::Lt
            if lhs_type.is_integer() || lhs_type.is_float() || lhs_type == &TypeKind::Duration =>
        {
            Some(lhs_type)
        }
        _ => None,
    }
}

pub(super) fn expr_kind_name(expr: &Expr) -> &'static str {
    match expr {
        Expr::Literal(_) => "literal",
        Expr::EntityRef(_) => "entity_ref",
        Expr::LifetimePath { .. } => "lifetime_path",
        Expr::Path(_) => "path",
        Expr::ShortVariant(_) => "short_variant",
        Expr::Placeholder(_) => "placeholder",
        Expr::Tuple(_) => "tuple",
        Expr::BracketSeq(_) => "bracket_seq",
        Expr::NumericBracketSeq(_) => "numeric_bracket_seq",
        Expr::ArrayRepeat { .. } => "array_repeat",
        Expr::Call { .. } => "call",
        Expr::MethodCall { .. } => "method_call",
        Expr::Field { .. } => "field",
        Expr::DialogueCall { .. } => "dialogue_call",
        Expr::Index { .. } => "index",
        Expr::Pipe { .. } => "pipe",
        Expr::Try { .. } => "try",
        Expr::Await { .. } => "await",
        Expr::Thread { .. } => "thread",
        Expr::Range { .. } => "range",
        Expr::Record { .. } => "record",
        Expr::RecordLiteral(_) => "record_literal",
        Expr::Binary { .. } => "binary",
        Expr::Closure { .. } => "closure",
        Expr::Unary { .. } => "unary",
        Expr::Block { .. } => "block",
        Expr::ComputationBlock { .. } => "computation_block",
        Expr::NamedBlock { .. } => "named_block",
        Expr::MemoBlock { .. } => "memo_block",
        Expr::If { .. } => "if",
        Expr::IfLet { .. } => "if_let",
        Expr::Match { .. } => "match",
        Expr::Raw(_) => "raw",
    }
}

pub(super) fn collection_index_key_type(target_type: &TypeKind) -> Option<TypeKind> {
    match target_type {
        TypeKind::Vec(_) | TypeKind::Array { .. } | TypeKind::Slice(_) | TypeKind::String => {
            Some(TypeKind::I64)
        }
        TypeKind::Map { key, .. } => Some(key.as_ref().clone()),
        TypeKind::Named(name) => map_key_type_from_name(name),
        _ => None,
    }
}

pub(super) fn agent_observation_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "tick" => TypeKind::U64,
        "frame_id" | "state_hash" | "render_hash" => TypeKind::String,
        "actions" => TypeKind::Vec(Box::new(TypeKind::ActionTarget)),
        "objects" => TypeKind::Vec(Box::new(TypeKind::ObservedObject)),
        "signals" => TypeKind::Map {
            kind: MapKind::BTree,
            key: Box::new(TypeKind::AgentValue),
            value: Box::new(TypeKind::AgentValue),
        },
        _ => return None,
    })
}

pub(super) fn agent_observed_object_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "id" => TypeKind::Named("ObservedObjectId".to_owned()),
        "parent_id" | "entity" | "layer" | "role" | "text" => TypeKind::String,
        "visible" | "enabled" => TypeKind::Bool,
        "bbox" => TypeKind::AgentBBox,
        _ => return None,
    })
}

pub(super) fn agent_bbox_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "space" => TypeKind::String,
        "x" | "y" | "width" | "height" => TypeKind::U32,
        _ => return None,
    })
}

pub(super) fn agent_action_result_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "accepted" => TypeKind::Bool,
        "before_tick" | "after_tick" => TypeKind::U64,
        "before_state_hash" | "after_state_hash" => TypeKind::String,
        _ => return None,
    })
}

pub(super) fn agent_action_target_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "id" | "target" | "kind" => TypeKind::String,
        "action" => TypeKind::ActionName,
        "enabled" => TypeKind::Bool,
        _ => return None,
    })
}

pub(super) fn agent_entity_ref_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "id" | "family" | "name" => TypeKind::String,
        _ => return None,
    })
}

pub(super) fn agent_capture_ref_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "uri" | "content_hash" | "media_type" => TypeKind::String,
        "byte_len" => TypeKind::U64,
        _ => return None,
    })
}

pub(super) fn agent_resource_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "uri" | "kind" | "mime_type" | "hash" => TypeKind::String,
        "body" => TypeKind::AgentResourceBody,
        _ => return None,
    })
}

pub(super) fn agent_resource_body_field_type(field: &str) -> Option<TypeKind> {
    Some(match field {
        "kind" | "json" | "text" | "base64" | "encoding" => TypeKind::String,
        "value" => TypeKind::AgentValue,
        _ => return None,
    })
}

pub(super) fn agent_attach_resource_type() -> TypeKind {
    TypeKind::Choice(vec![TypeKind::CaptureRef, TypeKind::AgentResource])
}

pub(super) fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(TypeKind::Named("AgentError".to_owned())),
    }
}

pub(super) fn set_agent_arg_slot<'a>(
    slot: &mut Option<&'a Expr>,
    value: &'a Expr,
    function_name: &str,
    arg_name: &str,
    errors: &mut Vec<TypeCheckError>,
) {
    if slot.replace(value).is_some() {
        errors.push(TypeCheckError::new(format!(
            "{function_name} argument `{arg_name}` was provided more than once"
        )));
    }
}

pub(super) fn signature_param_label(param: &FunctionParam, index: usize) -> String {
    param
        .name
        .as_deref()
        .map_or_else(|| format!("#{index}"), ToOwned::to_owned)
}

pub(super) fn map_key_type_from_name(name: &str) -> Option<TypeKind> {
    let (_, args) = name.split_once('<')?;
    let args = args.strip_suffix('>')?;
    let (key, _) = args.split_once(',')?;
    Some(match key.trim() {
        "Character" | "Ref<Character>" => TypeKind::entity_ref(EntityKind::Character),
        other => named_type_label(other),
    })
}

pub(super) fn is_character_speaker_type(ty: &TypeKind) -> bool {
    ty.is_entity_ref_kind(&EntityKind::Character)
        || matches!(
            ty,
            TypeKind::Speaker(EntityKind::Character)
                | TypeKind::SpeakerPreset(EntityKind::Character)
        )
}

pub(super) fn is_unit_number_type(ty: &TypeKind) -> bool {
    matches!(ty, TypeKind::Named(name) if matches!(
        name.as_str(),
        "Length" | "Angle" | "AudioLevel" | "Tempo"
    ))
}

pub(super) fn std_float_constant_type(path: &str) -> Option<TypeKind> {
    StdFloatConstant::resolve(path).map(StdFloatConstant::type_kind)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StdFloatConstant {
    F32,
    F64,
}

impl StdFloatConstant {
    fn resolve(path: &str) -> Option<Self> {
        let segments = path.split('.').collect::<Vec<_>>();
        let ["std", width, name] = segments.as_slice() else {
            return None;
        };
        if !matches!(
            *name,
            "nan" | "infinity" | "neg_infinity" | "epsilon" | "min" | "max" | "pi" | "tau"
        ) {
            return None;
        }
        match *width {
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            _ => None,
        }
    }

    const fn type_kind(self) -> TypeKind {
        match self {
            Self::F32 => TypeKind::F32,
            Self::F64 => TypeKind::F64,
        }
    }
}

pub(super) fn inline_failure_builtin_variant_type(path: &str) -> Option<TypeKind> {
    Some(match path {
        "InlineFailure.fail" | "InlineFailure.line_error" | "InlineFailure.discard" => {
            TypeKind::Named("InlineFailure".to_owned())
        }
        "InlineFallback.expr_source"
        | "InlineFallback.call_source"
        | "InlineFallback.value_plain" => TypeKind::Named("InlineFallback".to_owned()),
        "FallbackStyle.plain" | "FallbackStyle.inherit" => {
            TypeKind::Named("FallbackStyle".to_owned())
        }
        _ => return None,
    })
}

pub(super) fn looks_like_os_absolute_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}
