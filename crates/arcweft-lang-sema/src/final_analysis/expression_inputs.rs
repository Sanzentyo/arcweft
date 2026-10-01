//! Root-bound input ABI for evaluating a checked expression value.
//!
//! The existing eager execution fold owns membership. The shared free-local
//! collector authenticates external bindings, and the local-use catalog owns
//! value transfer and place access. This projection stores no second access index and never
//! treats a callable value's latent body as value-creation execution.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    identity::{ExprId, StmtId},
    scope::CaptureAccess,
};

use crate::{
    effects::EffectSet,
    semantic_coordinate::{
        CheckedLocalInputCoordinate, CheckedSemanticPath, SemanticCoordinateIndex,
    },
    types::TypeKind,
};

use super::{
    CheckedExecutableCapture, CheckedExecutableControlRole, CheckedLocalAccess,
    CheckedLocalCopyEvidence, CheckedLocalCopyRequirement, CheckedLocalUseSite,
    CheckedSuspensionRole, FinalSemanticAnalysisError,
    execution_regions::CheckedExpressionExecutionRegion,
    free_capture::{CheckedCaptureExpression, CheckedFreeLocalCollector},
};

/// One external binding and its selected value-transfer/place occurrences.
/// `CaptureAccess` retains a latent body's requirement; a creation transfer
/// does not itself perform that latent mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionInput {
    binding: CheckedExecutableCapture,
    uses: Box<[CheckedExpressionInputUse]>,
    copy_requirement: Option<CheckedLocalCopyRequirement>,
    copy_evidence: Option<CheckedLocalCopyEvidence>,
}

impl CheckedExpressionInput {
    pub const fn binding(&self) -> &CheckedExecutableCapture {
        &self.binding
    }

    pub const fn uses(&self) -> &[CheckedExpressionInputUse] {
        &self.uses
    }

    /// Exact ingress obligation retained by the local-use owner. Copy mode
    /// does not discharge this requirement for callable/opaque carriers.
    pub const fn copy_requirement(&self) -> Option<&CheckedLocalCopyRequirement> {
        self.copy_requirement.as_ref()
    }

    pub const fn copy_evidence(&self) -> Option<CheckedLocalCopyEvidence> {
        self.copy_evidence
    }
}

/// A stable input occurrence and its exact generation-bound lowering evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionInputUse {
    site: CheckedLocalUseSite,
    coordinate: CheckedLocalInputCoordinate,
    access: CheckedLocalAccess,
    latent_requirement: Option<CaptureAccess>,
}

impl CheckedExpressionInputUse {
    pub const fn site(&self) -> CheckedLocalUseSite {
        self.site
    }

    pub const fn coordinate(&self) -> &CheckedLocalInputCoordinate {
        &self.coordinate
    }

    pub const fn access(&self) -> &CheckedLocalAccess {
        &self.access
    }

    pub const fn latent_requirement(&self) -> Option<CaptureAccess> {
        self.latent_requirement
    }
}

/// Final-analysis-issued ABI for producing one expression value. Inputs and
/// occurrences are ordered by stable accepted coordinates, never `LocalId` or
/// expression arena position. The execution inventory expands the report's
/// shared eager DAG. The context closes input/result types and issues an owned
/// snapshot of its selected local-use certificates.
/// Callers cannot manufacture an accepted region.
pub struct CheckedExpressionInputAbi {
    authority: crate::callable::CheckedCallableAuthorityLease,
    instance: Option<super::CheckedLocalUseInstanceIdentity>,
    source: ExprId,
    coordinate: CheckedSemanticPath,
    execution: CheckedExpressionExecutionRegion,
    result: TypeKind,
    effects: EffectSet,
    inputs: Box<[CheckedExpressionInput]>,
}

impl CheckedExpressionInputAbi {
    /// Rejects pairing the snapshot with another generation or substitution.
    pub fn validate_for(
        &self,
        context: &super::CheckedClosedExecutionContext<'_>,
    ) -> Result<(), super::CheckedExecutionContextError> {
        if !self
            .authority
            .admits(context.analysis().checked_callables())
        {
            return Err(super::CheckedExecutionContextError::ForeignAuthority);
        }
        if self.instance.as_ref() != context.instance_identity() {
            return Err(super::CheckedExecutionContextError::InstanceMismatch);
        }
        context.admit_source(self.source)
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn coordinate(&self) -> &CheckedSemanticPath {
        &self.coordinate
    }

    pub fn expressions(&self) -> &[ExprId] {
        self.execution.expressions()
    }

    pub fn statements(&self) -> &[StmtId] {
        self.execution.statements()
    }

    pub fn places(&self) -> &[ExprId] {
        self.execution.places()
    }

    pub fn operations(&self) -> &[super::CheckedExecutionOperation] {
        self.execution.operations()
    }

    pub const fn result(&self) -> &TypeKind {
        &self.result
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }

    pub const fn suspension(&self) -> CheckedSuspensionRole {
        self.execution.suspension()
    }

    pub const fn control(&self) -> CheckedExecutableControlRole {
        self.execution.control()
    }

    pub const fn inputs(&self) -> &[CheckedExpressionInput] {
        &self.inputs
    }
}

impl super::CheckedClosedExecutionContext<'_> {
    /// Issues the complete input ABI for evaluating a selected expression.
    /// A closure/implicit callable root produces its callable value; body
    /// invocation remains a distinct executable boundary.
    /// Issuance requires exact source local-use certificates for every input.
    /// This input proof does not admit an extracted runtime program: its
    /// effect/control summary does not establish boundary-relative control
    /// safety or discharge the retained Copy ingress obligations.
    pub fn checked_expression_input_abi(
        &self,
        source: ExprId,
    ) -> Result<CheckedExpressionInputAbi, super::CheckedExecutionContextError> {
        self.admit_source(source)?;
        let analysis = self.analysis();
        let expression = analysis
            .expression(source)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        let result = expression
            .value_type()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        let execution = analysis
            .expression_execution_region(source)
            .ok_or(FinalSemanticAnalysisError::ExpressionExecutionUnavailable { owner: source })?;
        let coordinates = SemanticCoordinateIndex::new(analysis.accepted_root_catalog(), analysis);
        let coordinate = coordinates
            .expression(source)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let mut projected = Vec::new();
        let mut types = BTreeMap::new();
        for &owner in execution.expressions() {
            projected.push(
                analysis
                    .checked_capture_inputs(owner)?
                    .close_input_types(self, &mut types)?,
            );
        }
        for &owner in execution.statements() {
            let statement = analysis
                .statement(owner)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            projected.push(
                CheckedCaptureExpression::from_statement(owner, statement)?
                    .close_input_types(self, &mut types)?,
            );
        }
        let mut uses = BTreeMap::new();
        for &owner in execution.places() {
            let site = CheckedLocalUseSite::Place(owner);
            let access = self
                .local_uses()
                .access_at(site)
                .and_then(CheckedLocalAccess::place_access)
                .ok_or(FinalSemanticAnalysisError::ExpressionInputAccessUnavailable { site })?;
            if analysis
                .expression(owner)
                .and_then(super::CheckedExpression::mutable_place)
                .as_ref()
                != Some(access.place())
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            let ty = analysis
                .local(access.place().local_id())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                .ty();
            projected.push(
                CheckedCaptureExpression::from_place(owner, access.place(), ty)?
                    .close_input_types(self, &mut types)?,
            );
        }
        let mut collector = CheckedFreeLocalCollector::new(source, &coordinates, |local| {
            types.get(&local).cloned()
        })?;
        let mut sources = Vec::new();
        for projection in projected {
            collector.include_with_free_sources(projection, |source| sources.push(source))?;
        }
        for source in sources {
            let local_use = self.local_uses().access_at(source.site()).ok_or(
                FinalSemanticAnalysisError::ExpressionInputAccessUnavailable {
                    site: source.site(),
                },
            )?;
            if local_use.local() != source.local() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            let coordinate = coordinates
                .local_input(source.site(), source.local())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            uses.entry(source.local())
                .or_insert_with(Vec::new)
                .push(CheckedExpressionInputUse {
                    site: source.site(),
                    coordinate,
                    access: local_use.clone(),
                    latent_requirement: matches!(
                        source.site(),
                        CheckedLocalUseSite::Capture { .. }
                            | CheckedLocalUseSite::StatementCapture { .. }
                    )
                    .then_some(source.access()),
                });
        }
        let mut inputs = Vec::new();
        for binding in collector.finish() {
            let mut occurrences = uses
                .remove(&binding.local())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            occurrences.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));
            if occurrences
                .windows(2)
                .any(|pair| pair[0].coordinate == pair[1].coordinate)
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            inputs.push(CheckedExpressionInput {
                copy_requirement: self.local_uses().copy_requirement(binding.local()).cloned(),
                copy_evidence: self.local_uses().copy_evidence(binding.local()),
                binding,
                uses: occurrences.into_boxed_slice(),
            });
        }
        if !uses.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }
        inputs.sort_by(|left, right| left.binding.origin().cmp(right.binding.origin()));
        Ok(CheckedExpressionInputAbi {
            authority: analysis.checked_callables().authority_lease(),
            instance: self.instance_identity().cloned(),
            source,
            coordinate,
            execution,
            result: self.instantiate_type(result)?,
            effects: expression.effects().clone(),
            inputs: inputs.into_boxed_slice(),
        })
    }
}
