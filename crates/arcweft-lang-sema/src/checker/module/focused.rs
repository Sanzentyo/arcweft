//! Focused-call semantic analysis and retained call-fact reports.

#[cfg(test)]
use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::model::HirModule;

#[cfg(test)]
use crate::callable::{
    CallTargetFact, CallableArgumentIndex, CheckedCallArgumentSlotFact, ResolverWork,
};
use crate::callable::{CallTargetFactError, CallTargetFactMode, CallTargetFacts};
#[cfg(test)]
use crate::checker::TypeCheckEnv;
use crate::checker::call_target_facts::{CallResolverControl, SignatureFocusedAnalysis};
use crate::checker::{TypeCheckReport, TypeChecker, TypeExpressionId};
use crate::registration::RegisteredSemanticWorld;
use crate::style::check_view_styles;
use crate::view_part::check_view_parts;

use super::{FocusedCallTypeCheckReport, finish_type_check, finish_type_check_with_call_facts};

impl TypeCheckReport {
    /// Returns committed facts for one checker expression identity.
    ///
    /// Accepted registered-project analysis records ordinary call surfaces. A
    /// report produced with fact recording disabled returns `Ok(None)`.
    pub fn call_target_facts(
        &self,
        expression: TypeExpressionId,
    ) -> Result<Option<&CallTargetFacts>, CallTargetFactError> {
        if let Some(error) = &self.call_target_fact_report.error {
            return Err(error.clone());
        }
        Ok(self.call_target_fact_report.facts.get(&expression))
    }

    /// Returns the sole committed call fact from focused semantic analysis.
    pub fn focused_call_target_facts(&self) -> Result<&CallTargetFacts, CallTargetFactError> {
        let CallTargetFactMode::Focused { call, .. } = &self.call_target_fact_report.mode else {
            return Err(CallTargetFactError::FocusedModeRequired);
        };
        if let Some(error) = &self.call_target_fact_report.error {
            return Err(error.clone());
        }
        self.call_target_fact_report
            .facts
            .values()
            .next()
            .ok_or_else(|| CallTargetFactError::FocusedTargetMissing { call: call.clone() })
    }

    pub(crate) fn retained_call_target_facts(&self) -> impl Iterator<Item = &CallTargetFacts> {
        self.call_target_fact_report.facts.values()
    }

    #[cfg(test)]
    pub(crate) fn retained_argument_inference_facts(
        &self,
    ) -> impl Iterator<
        Item = (
            TypeExpressionId,
            CallableArgumentIndex,
            &CheckedCallArgumentSlotFact,
        ),
    > {
        self.retained_call_target_facts()
            .filter(|call| {
                matches!(
                    call.target(),
                    CallTargetFact::Selected { .. }
                        | CallTargetFact::Ambiguous { .. }
                        | CallTargetFact::Rejected { .. }
                )
            })
            .flat_map(|call| {
                call.arguments().iter().flat_map(move |argument| {
                    argument
                        .slots()
                        .iter()
                        .map(move |slot| (call.expression(), argument.index(), slot))
                })
            })
    }

    #[cfg(test)]
    pub(crate) fn physical_candidate_argument_evaluations(
        &self,
    ) -> &[super::super::PhysicalCandidateArgumentEvaluation] {
        &self.physical_candidate_argument_evaluations
    }

    #[cfg(test)]
    pub(crate) const fn physical_candidate_argument_evaluations_overflowed(&self) -> bool {
        self.physical_candidate_argument_evaluations_overflowed
    }
}

impl FocusedCallTypeCheckReport {
    #[cfg(test)]
    pub(crate) const fn report(&self) -> &TypeCheckReport {
        &self.report
    }

    pub(crate) fn focused_call_target_facts(
        &self,
    ) -> Result<&CallTargetFacts, CallTargetFactError> {
        let CallTargetFactMode::Focused { call, .. } = &self.call_targets.mode else {
            return Err(CallTargetFactError::FocusedModeRequired);
        };
        if let Some(error) = &self.call_targets.error {
            return Err(error.clone());
        }
        self.call_targets
            .facts
            .values()
            .next()
            .ok_or_else(|| CallTargetFactError::FocusedTargetMissing { call: call.clone() })
    }
}

/// Analyzes one exact accepted call span and retains only its checked call facts.
///
/// This bounded public entry uses the production callable-work limit and a
/// non-cancelled checker control. Interactive signature queries retain their
/// separate caller-owned cancellation and accounting path.
pub fn analyze_registered_project_types_for_focused_call(
    module: &HirModule,
    registered: &RegisteredSemanticWorld,
    call: arcweft_source::SourceSpan,
) -> Result<TypeCheckReport, CallTargetFactError> {
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(call.source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: call.source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call,
            active_argument: None,
            byte_offset: None,
        },
        CallResolverControl::ordinary(),
    );
    let report = finish_type_check(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    report.focused_call_target_facts()?;
    Ok(report)
}

#[cfg(test)]
pub(crate) fn analyze_registered_project_types_for_call_facts(
    module: &HirModule,
    registered: &RegisteredSemanticWorld,
    call: arcweft_source::SourceSpan,
    cancellation: &AtomicBool,
    work: &mut ResolverWork,
) -> Result<FocusedCallTypeCheckReport, CallTargetFactError> {
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(call.source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: call.source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call,
            active_argument: None,
            byte_offset: None,
        },
        CallResolverControl::caller_owned(cancellation, work, None, None),
    );
    let (report, call_targets) = finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    Ok(FocusedCallTypeCheckReport {
        report,
        call_targets,
    })
}

#[cfg(test)]
pub(crate) fn analyze_detached_types_for_call_facts(
    module: &HirModule,
    environment: &TypeCheckEnv,
    call: arcweft_source::SourceSpan,
    cancellation: &AtomicBool,
    work: &mut ResolverWork,
) -> Result<FocusedCallTypeCheckReport, CallTargetFactError> {
    if module.source_identity() != Some(call.source()) {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: call.source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        environment,
        module,
        None,
        None,
        CallTargetFactMode::Focused {
            call,
            active_argument: None,
            byte_offset: None,
        },
        CallResolverControl::caller_owned(cancellation, work, None, None),
    );
    let (report, call_targets) = finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    Ok(FocusedCallTypeCheckReport {
        report,
        call_targets,
    })
}

pub(crate) fn analyze_registered_project_types_for_signature_call(
    analysis: SignatureFocusedAnalysis<'_>,
) -> Result<FocusedCallTypeCheckReport, CallTargetFactError> {
    let SignatureFocusedAnalysis {
        module,
        registered,
        site,
        cancellation,
        work,
        signature_work,
        signature_control,
    } = analysis;
    if !registered
        .symbols()
        .modules()
        .any(|module| registered.symbols().source_identity(module) == Some(site.call().source()))
    {
        return Err(CallTargetFactError::FocusedSourceUnavailable {
            document: site.call().source().clone(),
        });
    }
    let (style_catalog, style_diagnostics) = check_view_styles(module);
    let (view_part_catalog, view_part_diagnostics) = check_view_parts(module);
    let mut checker = TypeChecker::new_with_project(
        registered.environment().typecheck_env(),
        module,
        Some(registered.symbols()),
        Some(registered),
        CallTargetFactMode::Focused {
            call: site.call().clone(),
            active_argument: site.active_argument(),
            byte_offset: site.byte_offset(),
        },
        CallResolverControl::caller_owned(
            cancellation,
            work,
            Some(signature_work),
            Some(signature_control),
        ),
    );
    let (report, call_targets) = finish_type_check_with_call_facts(
        module,
        style_catalog,
        style_diagnostics,
        view_part_catalog,
        view_part_diagnostics,
        &mut checker,
    );
    #[cfg(not(test))]
    drop(report);
    Ok(FocusedCallTypeCheckReport {
        #[cfg(test)]
        report,
        call_targets,
    })
}
