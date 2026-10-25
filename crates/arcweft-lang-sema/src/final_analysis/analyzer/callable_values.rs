//! Full declaration values projected through the shared callable resolver.

use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    Analyzer, CheckedExpression, CheckedExpressionResolution, CheckedTypeSelection,
    CheckedValueResolution, EffectSet, ExprId, FinalSemanticAnalysisError, HirModule,
    PreparedExpressionFact, TypeKind, expression_error::AnalyzerExpressionError,
};
use crate::{
    callable::{
        CallResolverAuthority, CallResolverContext, CallResolverRequest, CallableProjection,
        CheckedCallableDeclaration, ResolveCallError, ResolveCallOutcome, ResolvedCallTarget,
        ResolverWork, resolve_callable_reference,
    },
    types::{TypeCompatibilityControl, TypeCompatibilityPolicy},
};

struct ReferenceTypeControl<'a> {
    work: &'a mut ResolverWork,
    cancellation: &'a AtomicBool,
}

impl TypeCompatibilityControl for ReferenceTypeControl<'_> {
    type Error = ResolveCallError;

    fn enter(&mut self, _: &TypeKind, _: &TypeKind) -> Result<(), Self::Error> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(ResolveCallError::Cancelled);
        }
        self.work.charge(1).map_err(ResolveCallError::Work)
    }
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn prepare_declared_callable_value(
        &mut self,
        module: &HirModule,
        owner: ExprId,
        expected: Option<&TypeKind>,
    ) -> Result<Option<PreparedExpressionFact>, AnalyzerExpressionError> {
        let authority = CallResolverAuthority::accepted(
            self.project,
            module,
            self.symbols,
            self.catalogs.world,
        );
        let Some(reference) = authority
            .prepare_callable_reference(owner, &self.catalogs.callable_limits)
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ValueResolutionFailed {
                    owner,
                })
            })?
        else {
            return Ok(None);
        };
        let expression = module.resolve_expr(owner).map_err(|_| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner)
        })?;
        let source = super::statements::expression_span(module, owner)
            .map_err(AnalyzerExpressionError::fatal)?;
        // A lexical receiver shadows a project namespace before the complete
        // HIR reference is queried. Namespace prefixes themselves are not values.
        let mut root = reference.path().clone();
        while let Some((prefix, _)) = root.split_terminal_segment() {
            root = prefix;
        }
        match module
            .lookup_path_local(expression.scope(), &root, &source)
            .map_err(|_| AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::InvalidOwner))?
        {
            arcweft_lang_hir::scope::LocalLookup::Found(_) => return Ok(None),
            arcweft_lang_hir::scope::LocalLookup::AmbiguousPoisoned(_) => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                ));
            }
            arcweft_lang_hir::scope::LocalLookup::NotFound => {}
        }
        if let arcweft_lang_hir::symbol::ProjectValueLookup::Present(callable) = self
            .symbols
            .resolve_hir_value_target(module.key().path(), reference.path(), source)
            .map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::ValueResolutionFailed {
                    owner,
                })
            })?
        {
            let callable = crate::final_analysis::CheckedProjectCallable::new(
                callable.declaration().clone(),
                callable.source_item(),
            );
            let ty = self.prepare_named_callable_value_type(owner, &callable)?;
            return Ok(Some(
                CheckedExpression::value(
                    ty,
                    CheckedTypeSelection::Inferred,
                    EffectSet::new(),
                    CheckedExpressionResolution::Value(CheckedValueResolution::ProjectCallable(
                        callable,
                    )),
                )
                .into(),
            ));
        }
        if self
            .catalogs
            .world
            .environment()
            .callable_catalog()
            .free(reference.callable_path())
            .is_none()
        {
            return Ok(None);
        }
        // Real lexical/project/environment values retain precedence over a
        // free declaration at every prefix. No receiver is evaluated here.
        let mut prefix = reference.path().clone();
        loop {
            if self
                .resolve_path_value(module, owner, expression.scope(), &prefix)
                .map_err(AnalyzerExpressionError::fatal)?
                .is_some()
            {
                return Ok(None);
            }
            let Some((shorter, _)) = prefix.split_terminal_segment() else {
                break;
            };
            prefix = shorter;
        }
        let staged = self.staged_callables.as_ref().ok_or_else(|| {
            AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
        })?;
        let failed = |error| {
            AnalyzerExpressionError::fatal(
                FinalSemanticAnalysisError::CallableReferenceResolution {
                    owner,
                    error: Box::new(error),
                },
            )
        };
        let mut work = ResolverWork::new(self.catalogs.callable_limits.max_query_work());
        let request = CallResolverRequest::try_new_reference(
            &reference,
            &CallResolverContext {
                authority,
                checked: (&staged.builder).into(),
                presentation_character_owner: None,
                expression: owner,
                cancellation: self.control.cancellation(),
                prepared_continuations: self
                    .facts
                    .prepared_calls()
                    .map_err(AnalyzerExpressionError::fact)?,
                limits: &self.catalogs.callable_limits,
                implicit_extension_receiver: None,
            },
            &mut work,
        )
        .map_err(failed)?;
        let candidates = match resolve_callable_reference(request) {
            ResolveCallOutcome::Resolved(ResolvedCallTarget::Candidates(candidates)) => {
                candidates.into_shared().map_err(failed)?
            }
            ResolveCallOutcome::Rejected(error) => return Err(failed(error)),
            ResolveCallOutcome::Invariant(_) => {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CheckedCallableCatalog,
                ));
            }
            _ => return Ok(None),
        };
        let mut selected = None;
        for candidate in &candidates {
            let Some(checked) = candidate.checked().filter(|checked| {
                matches!(
                    checked.declaration(),
                    CheckedCallableDeclaration::Environment(_)
                )
            }) else {
                continue;
            };
            let terminal = self
                .source_callable_terminal_effects(candidate)
                .map_err(AnalyzerExpressionError::fatal)?;
            let ty = candidate.callable_value_type(terminal).map_err(|_| {
                AnalyzerExpressionError::fatal(FinalSemanticAnalysisError::CheckedCallableCatalog)
            })?;
            let CallableProjection::Ready(ty) = ty else {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::CheckedCallableCatalog,
                ));
            };
            if candidates.len() > 1 {
                let Some(expected) = expected else {
                    return Err(AnalyzerExpressionError::fatal(
                        FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                    ));
                };
                let mut control = ReferenceTypeControl {
                    work: &mut work,
                    cancellation: self.control.cancellation(),
                };
                match expected.accepts_with(
                    &ty,
                    TypeCompatibilityPolicy::SelectedCall,
                    &mut control,
                ) {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(crate::types::TypeCompatibilityFailure::Control(error)) => {
                        return Err(failed(error));
                    }
                    Err(crate::types::TypeCompatibilityFailure::Forbidden { .. }) => {
                        return Err(AnalyzerExpressionError::fatal(
                            FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                        ));
                    }
                }
            }
            if selected.replace((checked.clone(), ty)).is_some() {
                return Err(AnalyzerExpressionError::fatal(
                    FinalSemanticAnalysisError::ValueResolutionFailed { owner },
                ));
            }
        }
        Ok(selected.map(|(checked, ty)| {
            CheckedExpression::value(
                ty,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                CheckedExpressionResolution::Value(CheckedValueResolution::EnvironmentCallable(
                    checked,
                )),
            )
            .into()
        }))
    }
}
