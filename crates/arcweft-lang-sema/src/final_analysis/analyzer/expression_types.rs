//! Expression type projection and builtin iteration helpers.

use super::{
    CheckedIteratorFamily, CheckedTypeSelection, CheckedValueResolution, HirFloatLiteral,
    HirFloatWidth, HirIntegerLiteral, HirLiteral, IteratorStateKind, RegisteredSemanticWorld,
    TypeKind,
};
use crate::final_analysis::type_rules::{integer_suffix_type, is_integer};
use crate::types::{CompileTimeScalarKind, TypeEffectDomain};
use arcweft_lang_hir::leaf::{HirUnitNumberLiteral, HirUnitNumberUnit};

/// Borrowed source catalog plus a loan of the existing preparation work.
/// Cloning this context does not copy types, effect graphs, or a budget.
#[derive(Clone)]
pub(super) struct SourceTypePreparation<'a> {
    types: &'a super::BTreeMap<super::TypeId, TypeKind>,
    callables: &'a crate::callable::CheckedCallableCatalogBuilder,
    topology: &'a std::sync::Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology>,
    work: super::preparation_work::AnalyzerPreparationWork<'a>,
}

impl<'a> SourceTypePreparation<'a> {
    pub(super) fn new(
        types: &'a super::BTreeMap<super::TypeId, TypeKind>,
        callables: &'a crate::callable::CheckedCallableCatalogBuilder,
        topology: &'a std::sync::Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology>,
        work: super::preparation_work::AnalyzerPreparationWork<'a>,
    ) -> Self {
        Self {
            types,
            callables,
            topology,
            work,
        }
    }
    fn owner(
        &self,
        owner: arcweft_lang_hir::project::HirSemanticPathOwnerId,
        work: &mut super::preparation_work::AnalyzerPreparationWork<'_>,
    ) -> Result<crate::callable::CheckedConstraintSourceOwner, super::FinalSemanticAnalysisError>
    {
        use crate::final_analysis::{FinalSemanticAnalysisError, SourceTypeRetentionFailure};
        use crate::types::TypeProjectionControl;
        // Authenticate only after the original control admits copying the
        // retained HIR root and source-owner metadata.
        work.check()?;
        work.visit_binding()?;
        self.callables
            .constraint_source_owner(self.topology, owner)
            .map_err(|error| FinalSemanticAnalysisError::SourceTypeRetention {
                owner,
                error: SourceTypeRetentionFailure::Owner(error),
            })
    }

    pub(super) fn declared_type(
        &self,
        annotation: super::TypeId,
    ) -> Result<&TypeKind, super::FinalSemanticAnalysisError> {
        self.types
            .get(&annotation)
            .ok_or(super::FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation })
    }

    /// Retention preserves authored Unknown rows and does not perform admission
    /// into the original constraint relation or issue a completed identity.
    pub(super) fn retain(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        ty: &TypeKind,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        let owner = owner.into();
        let mut work = self.work.clone();
        self.owner(owner, &mut work)?
            .retain_value_type(ty, &mut work)
            .map_err(|error| {
                super::FinalSemanticAnalysisError::source_type_projection(owner, error)
            })
    }

    pub(super) fn annotation(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        annotation: super::TypeId,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        self.retain(owner, self.declared_type(annotation)?)
    }

    /// One actual nominal application owns the argument index and one graph
    /// edit for every declaration payload selected by the containing operation.
    pub(super) fn with_nominal_type_arguments<T>(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        declaration: &arcweft_lang_hir::symbol::nominal::ProjectNominalDeclaration,
        nominal: &crate::types::ProjectNominalType<crate::types::PreparedEffects>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        operation: impl FnOnce(
            crate::types::ProjectNominalType<crate::types::PreparedEffects>,
            &mut dyn FnMut(
                super::TypeId,
            ) -> Result<
                TypeKind<crate::types::PreparedEffects>,
                super::FinalSemanticAnalysisError,
            >,
        ) -> Result<T, super::FinalSemanticAnalysisError>,
    ) -> Result<T, super::FinalSemanticAnalysisError> {
        use crate::final_analysis::FinalSemanticAnalysisError;
        use crate::types::{
            PreparedTypeArguments, PreparedTypeRetentionFailure, TypeProjectionControl,
            TypeProjectionError,
        };
        let owner = owner.into();
        let mut work = self.work.clone();
        let source = self.owner(owner, &mut work)?;
        let caller_formals = source
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        let formals = nominal
            .declaration_formals_with_control(declaration, &mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        let scope = caller_formals.template_scope();
        let arguments = PreparedTypeArguments::new(&formals, nominal.arguments(), scope, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
            })?;
        // Retain the owner's copy without remapping its operational term IDs.
        work.visit_binding()?;
        let mut retained = Vec::new();
        for argument in nominal.arguments() {
            work.visit_binding()?;
            retained.push(
                argument
                    .retain_effects(scope, graph.read(), &mut work)
                    .map_err(|error| match error {
                        TypeProjectionError::Instantiation(error) => {
                            FinalSemanticAnalysisError::source_type_projection(
                                owner,
                                TypeProjectionError::Instantiation(error),
                            )
                        }
                        TypeProjectionError::Control(PreparedTypeRetentionFailure::Control(
                            error,
                        )) => error,
                        TypeProjectionError::Control(PreparedTypeRetentionFailure::Effect(
                            error,
                        )) => FinalSemanticAnalysisError::from(
                            crate::effect_row::prepared::PreparedEffectFailure::<
                                FinalSemanticAnalysisError,
                            >::Invariant(error),
                        ),
                    })?,
            );
        }
        let retained = crate::types::ProjectNominalType::new(declaration.id().clone(), retained);
        let mut edit = graph.edit_terms();
        let result = operation(retained, &mut |annotation| {
            let template = self
                .declared_type(annotation)
                .inspect_err(|_| edit.invalidate())?;
            arguments
                .project_in(template, &mut edit, &mut work)
                .map_err(|error| {
                    FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
                })
        })?;
        edit.commit().map_err(|error| {
            FinalSemanticAnalysisError::from(crate::effect_row::prepared::PreparedEffectFailure::<
                FinalSemanticAnalysisError,
            >::Invariant(error))
        })?;
        Ok(result)
    }

    pub(super) fn environment_variant(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
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
        let owner = owner.into();
        let mut work = self.work.clone();
        let source = self.owner(owner, &mut work)?;
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

impl super::Analyzer<'_, '_, '_> {
    pub(super) fn source_type_preparation<'a>(
        &'a self,
        work: super::preparation_work::AnalyzerPreparationWork<'a>,
    ) -> Result<SourceTypePreparation<'a>, super::FinalSemanticAnalysisError> {
        Ok(SourceTypePreparation::new(
            &self.types,
            &self
                .staged_callables
                .as_ref()
                .ok_or(super::FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .builder,
            &self.topology,
            work,
        ))
    }

    pub(super) fn retain_source_value_type(
        &self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        ty: &TypeKind,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        self.source_type_preparation(context.work())?
            .retain(owner, ty)
    }

    pub(super) fn retain_source_annotation_type(
        &self,
        context: &super::expression_error::AnalyzerExpressionContext<'_>,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        annotation: super::TypeId,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        self.source_type_preparation(context.work())?
            .annotation(owner, annotation)
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
        self.source_type_preparation(context.work())?
            .environment_variant(owner, schema, ty)
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
