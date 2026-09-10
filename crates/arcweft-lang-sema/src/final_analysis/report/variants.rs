//! Constructor execution projected from the accepted application and variant facts.

use arcweft_lang_hir::expr::{HirCallCallee, HirExprKind};

use crate::{
    callable::{
        CallableCandidateId, CheckedCallApplication, OptionConstructorKind,
        ResolvedCallableBaseInstantiation, ResultConstructorKind,
    },
    final_analysis::{CheckedExpressionResolution, CheckedVariantOwner, CheckedVariantResolution},
    types::TypeKind,
};

use super::{
    FinalAnalysisExecutionProjection, FinalAnalysisExecutionProjectionError, HirAnalysisProjectView,
};

impl FinalAnalysisExecutionProjection<'_> {
    /// Resolves all language constructor families through the completed call's
    /// result type. A declaration-template identity never identifies an
    /// instantiated owner; its application solution connects the two.
    pub fn variant_constructor(
        &self,
        project: HirAnalysisProjectView<'_>,
        application: &CheckedCallApplication,
    ) -> Result<Option<CheckedVariantResolution>, FinalAnalysisExecutionProjectionError> {
        let owner = application.core().site().expression();
        let invalid = || FinalAnalysisExecutionProjectionError::InvalidVariantConstructor { owner };
        let selected = application.core().candidates().selected();
        if !matches!(
            selected.instantiation(),
            ResolvedCallableBaseInstantiation::EnumConstructor
                | ResolvedCallableBaseInstantiation::Option
                | ResolvedCallableBaseInstantiation::Result { .. }
        ) {
            return Ok(None);
        }
        let accepted = self
            .analysis
            .call(owner)
            .ok_or(FinalAnalysisExecutionProjectionError::MissingCallFacts { owner })?
            .selected_application()
            .ok_or(FinalAnalysisExecutionProjectionError::UnselectedCall { owner })?;
        if accepted.digest() != application.digest() {
            return Err(invalid());
        }
        let result = application.result().value_type().ok_or_else(invalid)?;
        let variant = match selected.instantiation() {
            ResolvedCallableBaseInstantiation::EnumConstructor => {
                let CallableCandidateId::EnumVariant(candidate) = selected.id() else {
                    return Err(invalid());
                };
                let expected = selected.schema().value_type().ok_or_else(invalid)?;
                if expected.semantic_identity_digest().map_err(|_| invalid())? != candidate.owner()
                    || application
                        .core()
                        .solution()
                        .instantiate_template(expected)
                        .map_err(|_| invalid())?
                        != *result
                {
                    return Err(invalid());
                }
                let module = project
                    .modules()
                    .find_map(|(_, module)| {
                        (module.module_id() == owner.module()).then_some(module.as_ref())
                    })
                    .ok_or_else(invalid)?;
                let expression = module.resolve_expr(owner).map_err(|_| invalid())?;
                let HirExprKind::Call(call) = expression.kind() else {
                    return Err(invalid());
                };
                let HirCallCallee::Value { value } = call.callee() else {
                    return Err(invalid());
                };
                let CheckedExpressionResolution::Variant(variant) = self
                    .analysis
                    .expression(*value)
                    .ok_or(FinalAnalysisExecutionProjectionError::MissingExpression {
                        owner: *value,
                    })?
                    .resolution()
                else {
                    return Err(invalid());
                };
                if variant.ordinal() != candidate.case() {
                    return Err(invalid());
                }
                variant.clone()
            }
            ResolvedCallableBaseInstantiation::Option => {
                let TypeKind::Option(item) = result else {
                    return Err(invalid());
                };
                if !matches!(
                    selected.id(),
                    CallableCandidateId::Option(OptionConstructorKind::Some)
                ) {
                    return Err(invalid());
                }
                CheckedVariantResolution::try_new(
                    CheckedVariantOwner::try_option(item.as_ref().clone())
                        .map_err(|_| invalid())?,
                    0,
                )
                .ok_or_else(invalid)?
            }
            ResolvedCallableBaseInstantiation::Result { kind } => {
                let TypeKind::Result { ok, error } = result else {
                    return Err(invalid());
                };
                if selected.id() != &CallableCandidateId::Result(*kind) {
                    return Err(invalid());
                }
                let ordinal = match kind {
                    ResultConstructorKind::Ok => 0,
                    ResultConstructorKind::Err => 1,
                };
                CheckedVariantResolution::try_new(
                    CheckedVariantOwner::try_result(ok.as_ref().clone(), error.as_ref().clone())
                        .map_err(|_| invalid())?,
                    ordinal,
                )
                .ok_or_else(invalid)?
            }
            ResolvedCallableBaseInstantiation::None
            | ResolvedCallableBaseInstantiation::Character { .. }
            | ResolvedCallableBaseInstantiation::Receiver { .. }
            | ResolvedCallableBaseInstantiation::TypeReceiver { .. }
            | ResolvedCallableBaseInstantiation::Extension { .. } => return Ok(None),
        };
        if variant.owner().ty() != *result {
            return Err(invalid());
        }
        Ok(Some(variant))
    }
}
