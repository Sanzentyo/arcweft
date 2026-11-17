//! Expression type projection and builtin iteration helpers.

use super::{
    CheckedIteratorFamily, CheckedTypeSelection, CheckedValueResolution, HirFloatLiteral,
    HirFloatWidth, HirIntegerLiteral, HirLiteral, IteratorStateKind, RegisteredSemanticWorld,
    TypeKind,
};
use crate::final_analysis::type_rules::{integer_suffix_type, is_integer};
use crate::types::{CompileTimeScalarKind, TypeEffectDomain};
use arcweft_lang_hir::leaf::{HirUnitNumberLiteral, HirUnitNumberUnit};

impl super::Analyzer<'_, '_, '_> {
    fn source_type_owner(
        &self,
        owner: super::ExprId,
    ) -> Result<crate::callable::CheckedConstraintSourceOwner, super::FinalSemanticAnalysisError>
    {
        use crate::final_analysis::{FinalSemanticAnalysisError, SourceTypeRetentionFailure};
        self.staged_callables
            .as_ref()
            .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?
            .builder
            .constraint_source_owner(&self.topology, owner)
            .map_err(|error| FinalSemanticAnalysisError::SourceTypeRetention {
                owner,
                error: SourceTypeRetentionFailure::Owner(error),
            })
    }

    /// Canonical annotations enter source preparation through their actual
    /// producer. The existing expression work loan pays for the entire fold.
    pub(super) fn retain_source_value_type(
        &self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        ty: &TypeKind,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        use crate::final_analysis::FinalSemanticAnalysisError;
        let source = self.source_type_owner(owner)?;
        source
            .retain_value_type(ty, &mut context.work())
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))
    }

    pub(super) fn retain_source_annotation_type(
        &self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        annotation: super::TypeId,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        let ty = self
            .types
            .get(&annotation)
            .ok_or(super::FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation })?;
        self.retain_source_value_type(context, owner, ty)
    }

    pub(super) fn retain_environment_variant(
        &self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: super::ExprId,
        schema: &super::EnvironmentEnumSchema,
        ty: &TypeKind,
    ) -> Result<
        (
            TypeKind<crate::types::PreparedEffects>,
            super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
        ),
        super::FinalSemanticAnalysisError,
    > {
        use crate::final_analysis::{FinalSemanticAnalysisError, SourceTypeRetentionFailure};
        let source = self.source_type_owner(owner)?;
        let mut work = context.work();
        // Both owned type copies are charged through the same work loan.
        let value_type = source
            .retain_value_type(ty, &mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        let owner_type = source
            .retain_value_type(ty, &mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        let seed = super::PreparedVariantOwnerSeed::try_environment_with(
            schema,
            owner_type,
            &mut work,
            |field, work| {
                source.retain_value_type(field, work).map_err(|error| {
                    FinalSemanticAnalysisError::source_type_projection(owner, error)
                })
            },
        )
        .map_err(|error| match error {
            FinalSemanticAnalysisError::VariantOwner(error) => {
                FinalSemanticAnalysisError::SourceTypeRetention {
                    owner,
                    error: SourceTypeRetentionFailure::Variant(error),
                }
            }
            error => error,
        })?;
        Ok((value_type, seed))
    }
}

pub(super) fn unit_scalar_kind<E: TypeEffectDomain>(
    unit: HirUnitNumberUnit,
    expected: Option<&TypeKind<E>>,
) -> Option<CompileTimeScalarKind> {
    let length_expected = expected.is_some_and(|ty| match ty {
        TypeKind::CompileTimeScalar(scalar) => scalar.kind() == CompileTimeScalarKind::Length,
        TypeKind::Choice(items) => items.iter().any(|ty| {
            matches!(ty,
            TypeKind::CompileTimeScalar(scalar) if scalar.kind() == CompileTimeScalarKind::Length)
        }),
        _ => false,
    });
    match unit {
        HirUnitNumberUnit::Px
        | HirUnitNumberUnit::Pt
        | HirUnitNumberUnit::Em
        | HirUnitNumberUnit::Rem
        | HirUnitNumberUnit::Vw
        | HirUnitNumberUnit::Vh => Some(CompileTimeScalarKind::Length),
        HirUnitNumberUnit::Percent if length_expected => Some(CompileTimeScalarKind::Length),
        HirUnitNumberUnit::Percent => Some(CompileTimeScalarKind::Ratio),
        HirUnitNumberUnit::Deg | HirUnitNumberUnit::Rad | HirUnitNumberUnit::Turn => {
            Some(CompileTimeScalarKind::Angle)
        }
        HirUnitNumberUnit::Db
        | HirUnitNumberUnit::Lufs
        | HirUnitNumberUnit::Bpm
        | HirUnitNumberUnit::Bars => None,
    }
}

pub(super) fn literal_type<E: TypeEffectDomain>(
    literal: &HirLiteral,
    expected: Option<&TypeKind<E>>,
) -> Option<(TypeKind<E>, CheckedTypeSelection)> {
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
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { unit, .. }) => {
            if let Some(kind) = unit_scalar_kind(*unit, expected) {
                let accepts = |ty: &&TypeKind<E>| matches!(ty, TypeKind::CompileTimeScalar(scalar) if scalar.kind() == kind);
                let ty = match expected? {
                    TypeKind::Choice(items) => items.iter().find(accepts)?,
                    ty if accepts(&ty) => ty,
                    _ => return None,
                };
                (ty.clone(), CheckedTypeSelection::Expected)
            } else {
                (TypeKind::F64, CheckedTypeSelection::DefaultNumericFallback)
            }
        }
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_))
        | HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
        | HirLiteral::Float(HirFloatLiteral::Invalid(_)) => return None,
    };
    Some(inferred)
}

pub(super) fn expected_item(
    expected: Option<&TypeKind<crate::types::PreparedEffects>>,
) -> Option<&TypeKind<crate::types::PreparedEffects>> {
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
    values: impl IntoIterator<Item = &'a TypeKind<crate::types::PreparedEffects>>,
    expected: Option<&TypeKind<crate::types::PreparedEffects>>,
) -> Option<TypeKind<crate::types::PreparedEffects>> {
    let mut values = values.into_iter();
    let first = values.next().cloned().or_else(|| expected.cloned())?;
    values.try_fold(first, |joined, value| match (&joined, value) {
        (TypeKind::Never, _) => Some(value.clone()),
        (_, TypeKind::Never) => Some(joined),
        (TypeKind::CharacterDialogue(left), TypeKind::CharacterDialogue(right)) => {
            Some(TypeKind::CharacterDialogue(
                crate::types::CharacterDialogueType::join(left.clone(), right),
            ))
        }
        _ if value == &joined => Some(joined),
        _ => None,
    })
}

pub(super) fn indexed_item(
    ty: &TypeKind<crate::types::PreparedEffects>,
) -> Option<TypeKind<crate::types::PreparedEffects>> {
    match ty {
        TypeKind::Vec(item)
        | TypeKind::Array { item, .. }
        | TypeKind::Slice(item)
        | TypeKind::Seq(item) => Some((**item).clone()),
        TypeKind::String => Some(TypeKind::Char),
        _ => None,
    }
}

pub(super) fn builtin_iteration(
    ty: &TypeKind<crate::types::PreparedEffects>,
) -> Option<(
    CheckedIteratorFamily,
    IteratorStateKind,
    TypeKind<crate::types::PreparedEffects>,
)> {
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
        CheckedValueResolution::Local(_) | CheckedValueResolution::RegistryRead(_) => None,
        CheckedValueResolution::LineContext => Some(TypeKind::LineContext),
        CheckedValueResolution::CharacterField {
            character, field, ..
        } => Some(field.ty(character.clone())),
        CheckedValueResolution::Registered(registered) => registered
            .environment_binding()
            .and_then(|binding| world.environment().environment_binding(binding))
            .cloned(),
        // Named callable values require the staged body-effect authority.
        CheckedValueResolution::ProjectCallable(_)
        | CheckedValueResolution::EnvironmentCallable(_) => None,
        CheckedValueResolution::ProjectItem(item) => Some(item.ty()),
        CheckedValueResolution::ImportedProjectEntity(entity) => Some(entity.ty()),
        CheckedValueResolution::PresentationAddress(identity) => {
            Some(TypeKind::presentation_address_reference(identity.kind()))
        }
        CheckedValueResolution::CatalogAsset(_) => {
            Some(TypeKind::entity_ref(crate::types::EntityKind::Asset))
        }
        CheckedValueResolution::Entry(entry) => Some(entry.ty()),
        CheckedValueResolution::Constant(literal) => literal_type(literal, None).map(|(ty, _)| ty),
    }
}
