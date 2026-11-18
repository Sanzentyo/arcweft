//! Expression type projection and source type preparation.

use super::{
    CheckedTypeSelection, CheckedValueResolution, HirFloatLiteral, HirFloatWidth,
    HirIntegerLiteral, HirLiteral, RegisteredSemanticWorld, TypeKind,
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
    pub(super) fn nominal_argument_inference(
        &self,
        owner: super::ExprId,
        declaration: &super::ProjectNominalDeclaration,
    ) -> Result<crate::types::PreparedNominalArgumentInference, super::FinalSemanticAnalysisError>
    {
        use super::FinalSemanticAnalysisError;
        use crate::types::PreparedNominalArgumentInference;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        PreparedNominalArgumentInference::new(declaration, formals.template_scope(), &mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))
    }

    pub(super) fn generic_uses(
        &self,
        owner: super::ExprId,
        ty: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<crate::types::TypeGenericReferenceUseInventory, super::FinalSemanticAnalysisError>
    {
        use crate::final_analysis::{FinalSemanticAnalysisError, SourceTypeRetentionFailure};
        use crate::types::{PreparedGenericUseFailure, TypeGenericReferenceUseCollector};
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        TypeGenericReferenceUseCollector::collect_prepared(
            ty,
            formals.template_scope(),
            graph,
            &mut work,
        )
        .map_err(|error| match error {
            PreparedGenericUseFailure::Type(error) => {
                FinalSemanticAnalysisError::SourceTypeRetention {
                    owner,
                    error: SourceTypeRetentionFailure::GenericUses(error),
                }
            }
            PreparedGenericUseFailure::Effect(error) => {
                FinalSemanticAnalysisError::from(error.map_control(|never| match never {}))
            }
            PreparedGenericUseFailure::Control(error) => error,
        })
    }

    pub(super) fn visit_binding(&self) -> Result<(), super::FinalSemanticAnalysisError> {
        use crate::types::TypeProjectionControl;
        self.work.clone().visit_binding()
    }

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

    pub(super) fn retain_prepared(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        ty: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        ty.retain_effects(formals.template_scope(), graph, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_prepared_type_retention(owner, error)
            })
    }

    pub(super) fn observe_canonical(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        ty: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<TypeKind, super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        ty.observe_canonical_effects(formals.template_scope(), graph, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
            })
    }

    pub(super) fn observe_diagnostic_type(
        &self,
        owner: super::ExprId,
        ty: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<Option<TypeKind>, super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        crate::types::PreparedTypeEffectProjectionFailure::optional_observation(
            ty.observe_canonical_effects(formals.template_scope(), graph, &mut work),
        )
        .map_err(|error| FinalSemanticAnalysisError::source_type_effect_projection(owner, error))
    }

    pub(super) fn call_value_operand(
        &self,
        expression: super::ExprId,
        ty: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<crate::callable::PreparedCallValueOperand, super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = expression.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        let scope = formals.template_scope();
        let original = ty
            .retain_effects(scope, graph, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_prepared_type_retention(owner, error)
            })?;
        let observed = original
            .observe_canonical_effects(scope, graph, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
            })?;
        Ok(crate::callable::PreparedCallValueOperand::new(
            expression, observed, original,
        ))
    }

    pub(super) fn observe_compatibility_inputs(
        &self,
        owner: super::ExprId,
        expected: &TypeKind<crate::types::PreparedEffects>,
        actual: &TypeKind<crate::types::PreparedEffects>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
    ) -> Result<(TypeKind, TypeKind), super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        expected
            .observe_compatibility_inputs(actual, formals.template_scope(), graph, &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
            })
    }

    /// A signature that comes entirely from authored annotations needs no
    /// solution from the live relation.
    pub(super) fn project_annotations(
        &self,
        owner: super::ExprId,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<Option<TypeKind>, super::FinalSemanticAnalysisError> {
        use super::FinalSemanticAnalysisError;
        let owner = owner.into();
        let mut work = self.work.clone();
        let formals = self
            .owner(owner, &mut work)?
            .declaration_formals_with_control(&mut work)
            .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))?;
        ty.project_effect_annotations(formals.template_scope(), &mut work)
            .map_err(|error| {
                FinalSemanticAnalysisError::source_type_effect_projection(owner, error)
            })
    }

    /// An early rejection for a consumer that retains the relation inputs
    /// for its seal. A comparison that needs live effect evidence can continue
    /// through preparation without closing the candidate graph here.
    pub(super) fn contextual_type_mismatch(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        expected: &TypeKind<crate::types::PreparedEffects>,
        actual: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<bool, super::FinalSemanticAnalysisError> {
        Ok(self.prepared_compatibility(owner, expected, actual)?
            == crate::types::PreparedTypeCompatibility::Rejected)
    }

    pub(super) fn prepared_compatibility(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        expected: &TypeKind<crate::types::PreparedEffects>,
        actual: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<crate::types::PreparedTypeCompatibility, super::FinalSemanticAnalysisError> {
        let mut work = self.work.clone();
        self.owner(owner.into(), &mut work)?;
        expected.check_prepared_compatibility(actual, &mut work)
    }

    pub(super) fn complete_binding_effects(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        annotation: &TypeKind<crate::types::PreparedEffects>,
        actual: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<Option<TypeKind<crate::types::PreparedEffects>>, super::FinalSemanticAnalysisError>
    {
        let owner = owner.into();
        annotation
            .complete_prepared_binding_effects(actual, &mut self.work.clone())
            .map_err(|error| {
                super::FinalSemanticAnalysisError::source_type_projection(owner, error)
            })
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
            &mut SourceNominalProjection<'_, '_, '_>,
        ) -> Result<T, super::FinalSemanticAnalysisError>,
    ) -> Result<T, super::FinalSemanticAnalysisError> {
        use crate::final_analysis::FinalSemanticAnalysisError;
        use crate::types::{PreparedTypeArguments, TypeProjectionControl};
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
                    .map_err(|error| {
                        FinalSemanticAnalysisError::source_prepared_type_retention(owner, error)
                    })?,
            );
        }
        let retained = crate::types::ProjectNominalType::new(declaration.id().clone(), retained);
        let mut edit = graph.edit_terms();
        let result = operation(
            retained,
            &mut SourceNominalProjection {
                owner,
                types: self.types,
                arguments: &arguments,
                scope,
                edit: &mut edit,
                work: &mut work,
            },
        )?;
        edit.commit().map_err(|error| {
            FinalSemanticAnalysisError::from(crate::effect_row::prepared::PreparedEffectFailure::<
                FinalSemanticAnalysisError,
            >::Invariant(error))
        })?;
        Ok(result)
    }

    pub(super) fn project_variant_owner(
        &self,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        declaration: &arcweft_lang_hir::symbol::nominal::ProjectNominalDeclaration,
        nominal: &crate::types::ProjectNominalType<crate::types::PreparedEffects>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
    ) -> Result<
        super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
        super::FinalSemanticAnalysisError,
    > {
        use super::FinalSemanticAnalysisError;
        use arcweft_lang_hir::symbol::nominal::{ProjectNominalBody, ProjectNominalVariantPayload};
        let ProjectNominalBody::Enum { variants } = declaration.body() else {
            return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
        };
        self.with_nominal_type_arguments(owner, declaration, nominal, graph, |nominal, project| {
            // Each case and owned record field is admitted before allocation.
            for variant in variants {
                project.visit_binding()?;
                if let ProjectNominalVariantPayload::Record(fields) = variant.payload() {
                    for _ in fields {
                        project.visit_binding()?;
                    }
                }
            }
            let mut cases = Vec::with_capacity(variants.len());
            for (ordinal, variant) in variants.iter().enumerate() {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?;
                cases.push(super::PreparedVariantCaseSeed::from_project(
                    ordinal,
                    variant,
                    |annotation| project.annotation(annotation),
                    |_| FinalSemanticAnalysisError::InvalidNominalOwner,
                )?);
            }
            super::PreparedVariantOwnerSeed::try_project(nominal, cases)
                .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)
        })
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
        use crate::final_analysis::FinalSemanticAnalysisError;
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
        let seed =
            Self::prepare_environment_variant_owner(owner, &source, schema, owner_type, &mut work)?;
        Ok((value_type, seed))
    }

    pub(super) fn environment_variant_owner(
        &self,
        owner: super::PatternId,
        schema: &super::EnvironmentEnumSchema,
        ty: TypeKind<crate::types::PreparedEffects>,
    ) -> Result<
        super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
        super::FinalSemanticAnalysisError,
    > {
        let owner = owner.into();
        let mut work = self.work.clone();
        let source = self.owner(owner, &mut work)?;
        Self::prepare_environment_variant_owner(owner, &source, schema, ty, &mut work)
    }

    fn prepare_environment_variant_owner(
        owner: arcweft_lang_hir::project::HirSemanticPathOwnerId,
        source: &crate::callable::CheckedConstraintSourceOwner,
        schema: &super::EnvironmentEnumSchema,
        ty: TypeKind<crate::types::PreparedEffects>,
        work: &mut super::preparation_work::AnalyzerPreparationWork<'_>,
    ) -> Result<
        super::PreparedVariantOwnerSeed<crate::types::PreparedEffects>,
        super::FinalSemanticAnalysisError,
    > {
        use crate::final_analysis::{FinalSemanticAnalysisError, SourceTypeRetentionFailure};
        super::PreparedVariantOwnerSeed::try_environment_with(schema, ty, work, |field, work| {
            source
                .retain_value_type(field, work)
                .map_err(|error| FinalSemanticAnalysisError::source_type_projection(owner, error))
        })
        .map_err(|error| match error {
            FinalSemanticAnalysisError::VariantOwner(error) => {
                FinalSemanticAnalysisError::SourceTypeRetention {
                    owner,
                    error: SourceTypeRetentionFailure::Variant(error),
                }
            }
            error => error,
        })
    }
}

/// One nominal application loans its index, actual caller scope, edit, and work.
/// Every failed projection permanently invalidates the containing edit.
pub(super) struct SourceNominalProjection<'operation, 'graph, 'control> {
    owner: arcweft_lang_hir::project::HirSemanticPathOwnerId,
    types: &'operation super::BTreeMap<super::TypeId, TypeKind>,
    arguments: &'operation crate::types::PreparedTypeArguments<'operation>,
    scope: &'operation crate::types::GenericScope,
    edit: &'operation mut crate::effect_row::prepared::PreparedEffectTermEdit<'graph>,
    work: &'operation mut super::preparation_work::AnalyzerPreparationWork<'control>,
}

impl SourceNominalProjection<'_, '_, '_> {
    pub(super) fn meta_type(
        &mut self,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        ty.retain_as_meta_type(self.scope, self.edit.read(), self.work)
            .map_err(|error| {
                super::FinalSemanticAnalysisError::source_prepared_type_retention(self.owner, error)
            })
            .inspect_err(|_| self.edit.invalidate())
    }

    pub(super) fn visit_binding(&mut self) -> Result<(), super::FinalSemanticAnalysisError> {
        use crate::types::TypeProjectionControl;
        self.work
            .visit_binding()
            .inspect_err(|_| self.edit.invalidate())
    }

    pub(super) fn annotation(
        &mut self,
        annotation: super::TypeId,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        let template = self
            .types
            .get(&annotation)
            .ok_or(super::FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation })
            .inspect_err(|_| self.edit.invalidate())?;
        self.arguments
            .project_in(template, self.edit, self.work)
            .map_err(|error| {
                super::FinalSemanticAnalysisError::source_type_effect_projection(self.owner, error)
            })
    }

    /// A second owned field copy keeps the same operational term identities.
    pub(super) fn retain(
        &mut self,
        ty: &TypeKind<crate::types::PreparedEffects>,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, super::FinalSemanticAnalysisError> {
        ty.retain_effects(self.scope, self.edit.read(), self.work)
            .map_err(|error| {
                super::FinalSemanticAnalysisError::source_prepared_type_retention(self.owner, error)
            })
            .inspect_err(|_| self.edit.invalidate())
    }
}

impl super::Analyzer<'_, '_, '_> {
    pub(super) fn with_declared_project_nominal<T>(
        &mut self,
        work: super::preparation_work::AnalyzerPreparationWork<'_>,
        owner: impl Into<arcweft_lang_hir::project::HirSemanticPathOwnerId>,
        ty: &TypeKind,
        operation: impl FnOnce(
            crate::types::ProjectNominalType<crate::types::PreparedEffects>,
            &mut SourceNominalProjection<'_, '_, '_>,
        ) -> Result<T, super::FinalSemanticAnalysisError>,
    ) -> Result<T, super::FinalSemanticAnalysisError> {
        let owner = owner.into();
        let source = SourceTypePreparation::new(
            &self.types,
            &self
                .staged_callables
                .as_ref()
                .ok_or(super::FinalSemanticAnalysisError::CheckedCallableCatalog)?
                .builder,
            &self.topology,
            work,
        );
        let retained = source.retain(owner, ty)?;
        let TypeKind::ProjectNominal(nominal) = retained else {
            return Err(super::FinalSemanticAnalysisError::InvalidNominalOwner);
        };
        let declaration = self
            .symbols
            .nominal(nominal.declaration())
            .ok_or(super::FinalSemanticAnalysisError::InvalidNominalOwner)?;
        let (graph, _) = self.facts.effect_declarations_mut()?.relation();
        source.with_nominal_type_arguments(owner, declaration, &nominal, graph, operation)
    }

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

pub(super) fn common_type<'a, D: crate::types::TypeEffectDomain + 'a>(
    values: impl IntoIterator<Item = &'a TypeKind<D>>,
    expected: Option<&TypeKind<D>>,
) -> Option<TypeKind<D>> {
    use crate::final_analysis::completion::ResultTypeJoin;
    let mut values = values.into_iter();
    let first = values.next().cloned().or_else(|| expected.cloned())?;
    values.try_fold(first, |joined, value| {
        match ResultTypeJoin::for_values(&joined, value)? {
            ResultTypeJoin::Left => Some(joined),
            ResultTypeJoin::Right => Some(value.clone()),
            ResultTypeJoin::Combined(ty) => Some(ty),
        }
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
