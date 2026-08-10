//! Expression type projection and builtin iteration helpers.

use super::calls::callable_schema_type;
use super::{
    CheckedIteratorFamily, CheckedTypeSelection, CheckedValueResolution, HirFloatLiteral,
    HirFloatWidth, HirIntegerLiteral, HirLiteral, IteratorStateKind, RegisteredSemanticWorld,
    TypeKind,
};
use crate::final_analysis::type_rules::{integer_suffix_type, is_integer};

pub(super) fn literal_type(
    literal: &HirLiteral,
    expected: Option<&TypeKind>,
) -> Option<(TypeKind, CheckedTypeSelection)> {
    let inferred = match literal {
        HirLiteral::String(_) => (TypeKind::String, CheckedTypeSelection::Inferred),
        HirLiteral::Character(_) => (TypeKind::Char, CheckedTypeSelection::Inferred),
        HirLiteral::Boolean(_) => (TypeKind::Bool, CheckedTypeSelection::Inferred),
        HirLiteral::Duration(_) => (TypeKind::Duration, CheckedTypeSelection::Inferred),
        HirLiteral::Integer(HirIntegerLiteral::Value { suffix, .. }) => match suffix {
            Some(suffix) => (
                integer_suffix_type(Some(*suffix))?,
                CheckedTypeSelection::Explicit,
            ),
            None if expected.is_some_and(is_integer) => {
                (expected?.clone(), CheckedTypeSelection::Expected)
            }
            None => (TypeKind::I32, CheckedTypeSelection::DefaultNumericFallback),
        },
        HirLiteral::Float(HirFloatLiteral::Value { explicit_width, .. }) => match explicit_width {
            Some(HirFloatWidth::F32) => (TypeKind::F32, CheckedTypeSelection::Explicit),
            Some(HirFloatWidth::F64) => (TypeKind::F64, CheckedTypeSelection::Explicit),
            None if matches!(expected, Some(TypeKind::F32 | TypeKind::F64)) => {
                (expected?.clone(), CheckedTypeSelection::Expected)
            }
            None => (TypeKind::F64, CheckedTypeSelection::DefaultNumericFallback),
        },
        HirLiteral::UnitNumber(_) => expected.cloned().map_or(
            (TypeKind::F64, CheckedTypeSelection::DefaultNumericFallback),
            |ty| (ty, CheckedTypeSelection::Expected),
        ),
        HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
        | HirLiteral::Float(HirFloatLiteral::Invalid(_)) => return None,
    };
    Some(inferred)
}

pub(super) fn expected_item(expected: Option<&TypeKind>) -> Option<&TypeKind> {
    match expected {
        Some(
            TypeKind::Vec(item)
            | TypeKind::Slice(item)
            | TypeKind::Seq(item)
            | TypeKind::Range(item)
            | TypeKind::Array { item, .. }
            | TypeKind::Stream { item, .. },
        ) => Some(item),
        _ => None,
    }
}

pub(super) fn common_type<'a>(
    values: impl IntoIterator<Item = &'a TypeKind>,
    expected: Option<&TypeKind>,
) -> Option<TypeKind> {
    let mut values = values.into_iter();
    let first = values.next().cloned().or_else(|| expected.cloned())?;
    values.try_fold(first, |joined, value| match (&joined, value) {
        (TypeKind::CharacterDialogue(left), TypeKind::CharacterDialogue(right)) => {
            Some(TypeKind::CharacterDialogue(
                crate::types::CharacterDialogueType::join(left.clone(), right),
            ))
        }
        _ if value == &joined => Some(joined),
        _ => None,
    })
}

pub(super) fn indexed_item(ty: &TypeKind) -> Option<TypeKind> {
    match ty {
        TypeKind::Vec(item)
        | TypeKind::Array { item, .. }
        | TypeKind::Slice(item)
        | TypeKind::Seq(item)
        | TypeKind::Range(item) => Some((**item).clone()),
        TypeKind::Map { value, .. } => Some((**value).clone()),
        TypeKind::String => Some(TypeKind::Char),
        _ => None,
    }
}

pub(super) fn builtin_iteration(
    ty: &TypeKind,
) -> Option<(CheckedIteratorFamily, IteratorStateKind, TypeKind)> {
    let (family, state, item) = match ty {
        TypeKind::Range(item) => (
            CheckedIteratorFamily::Range,
            IteratorStateKind::Range,
            item.as_ref(),
        ),
        TypeKind::Seq(item) => (
            CheckedIteratorFamily::Seq,
            IteratorStateKind::Seq,
            item.as_ref(),
        ),
        TypeKind::Stream { item, .. } => (
            CheckedIteratorFamily::Stream,
            IteratorStateKind::Stream,
            item.as_ref(),
        ),
        TypeKind::Vec(item) => (
            CheckedIteratorFamily::Vec,
            IteratorStateKind::Vec,
            item.as_ref(),
        ),
        TypeKind::Array { item, .. } => (
            CheckedIteratorFamily::Array,
            IteratorStateKind::Array,
            item.as_ref(),
        ),
        TypeKind::Slice(item) => (
            CheckedIteratorFamily::Slice,
            IteratorStateKind::Slice,
            item.as_ref(),
        ),
        _ => return None,
    };
    Some((family, state, item.clone()))
}

pub(super) fn value_resolution_type(
    world: &RegisteredSemanticWorld,
    resolution: &CheckedValueResolution,
) -> Option<TypeKind> {
    match resolution {
        CheckedValueResolution::Local(_) => None,
        CheckedValueResolution::Registered(registered) => registered
            .environment_binding()
            .and_then(|binding| world.environment().environment_binding(binding))
            .cloned(),
        CheckedValueResolution::ProjectCallable(callable) => world
            .environment()
            .callable_catalog()
            .project_record(callable.declaration())
            .and_then(|record| callable_schema_type(record.schema())),
        CheckedValueResolution::ProjectItem(item) => Some(item.ty()),
        CheckedValueResolution::Entry(entry) => Some(entry.ty()),
        CheckedValueResolution::Constant(literal) => literal_type(literal, None).map(|(ty, _)| ty),
    }
}
