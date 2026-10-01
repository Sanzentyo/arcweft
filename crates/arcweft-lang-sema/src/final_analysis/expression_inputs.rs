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
    CheckedSuspensionRole, FinalSemanticAnalysis, FinalSemanticAnalysisError,
    execution_regions::CheckedExpressionExecutionRegion,
    free_capture::{CheckedCaptureExpression, CheckedFreeLocalCollector},
};

/// One external binding and its selected value-transfer/place occurrences.
/// `CaptureAccess` retains a latent body's requirement; a creation transfer
/// does not itself perform that latent mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionInput<'analysis> {
    binding: CheckedExecutableCapture,
    uses: Box<[CheckedExpressionInputUse<'analysis>]>,
    copy_requirement: Option<&'analysis CheckedLocalCopyRequirement>,
    copy_evidence: Option<CheckedLocalCopyEvidence>,
}

impl CheckedExpressionInput<'_> {
    pub const fn binding(&self) -> &CheckedExecutableCapture {
        &self.binding
    }

    pub const fn uses(&self) -> &[CheckedExpressionInputUse<'_>] {
        &self.uses
    }

    /// Exact ingress obligation retained by the local-use owner. Copy mode
    /// does not discharge this requirement for callable/opaque carriers.
    pub const fn copy_requirement(&self) -> Option<&CheckedLocalCopyRequirement> {
        self.copy_requirement
    }

    pub const fn copy_evidence(&self) -> Option<CheckedLocalCopyEvidence> {
        self.copy_evidence
    }
}

/// A stable input occurrence and its exact generation-bound lowering evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedExpressionInputUse<'analysis> {
    site: CheckedLocalUseSite,
    coordinate: CheckedLocalInputCoordinate,
    access: &'analysis CheckedLocalAccess,
    latent_requirement: Option<CaptureAccess>,
}

impl CheckedExpressionInputUse<'_> {
    pub const fn site(&self) -> CheckedLocalUseSite {
        self.site
    }

    pub const fn coordinate(&self) -> &CheckedLocalInputCoordinate {
        &self.coordinate
    }

    pub const fn access(&self) -> &CheckedLocalAccess {
        self.access
    }

    pub const fn latent_requirement(&self) -> Option<CaptureAccess> {
        self.latent_requirement
    }
}

/// Final-analysis-issued ABI for producing one expression value. Inputs and
/// occurrences are ordered by stable accepted coordinates, never `LocalId` or
/// expression arena position. The execution inventory expands the report's
/// shared eager DAG; result/effects borrow their final owning expression.
/// Callers cannot manufacture an accepted region.
pub struct CheckedExpressionInputAbi<'analysis> {
    source: ExprId,
    coordinate: CheckedSemanticPath,
    execution: CheckedExpressionExecutionRegion,
    result: &'analysis TypeKind,
    effects: &'analysis EffectSet,
    inputs: Box<[CheckedExpressionInput<'analysis>]>,
}

impl CheckedExpressionInputAbi<'_> {
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
        self.result
    }

    pub const fn effects(&self) -> &EffectSet {
        self.effects
    }

    pub const fn suspension(&self) -> CheckedSuspensionRole {
        self.execution.suspension()
    }

    pub const fn control(&self) -> CheckedExecutableControlRole {
        self.execution.control()
    }

    pub const fn inputs(&self) -> &[CheckedExpressionInput<'_>] {
        &self.inputs
    }
}

impl FinalSemanticAnalysis {
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
    ) -> Result<CheckedExpressionInputAbi<'_>, FinalSemanticAnalysisError> {
        let expression = self
            .expression(source)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        let result = expression
            .value_type()
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: source })?;
        let execution = self
            .expression_execution_region(source)
            .ok_or(FinalSemanticAnalysisError::ExpressionExecutionUnavailable { owner: source })?;
        let coordinates = SemanticCoordinateIndex::new(self.accepted_root_catalog(), self);
        let coordinate = coordinates
            .expression(source)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let mut collector = CheckedFreeLocalCollector::new(source, &coordinates, |local| {
            self.local(local).map(|binding| binding.ty().clone())
        })?;
        let mut sources = Vec::new();
        for &owner in execution.expressions() {
            collector.include_with_free_sources(self.checked_capture_inputs(owner)?, |source| {
                sources.push(source);
            })?;
        }
        for &owner in execution.statements() {
            let statement = self
                .statement(owner)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            collector.include_with_free_sources(
                CheckedCaptureExpression::from_statement(owner, statement)?,
                |source| sources.push(source),
            )?;
        }
        let mut uses = BTreeMap::new();
        for &owner in execution.places() {
            let site = CheckedLocalUseSite::Place(owner);
            let access = self
                .checked_local_uses()
                .access_at(site)
                .and_then(CheckedLocalAccess::place_access)
                .ok_or(FinalSemanticAnalysisError::ExpressionInputAccessUnavailable { site })?;
            if self
                .expression(owner)
                .and_then(super::CheckedExpression::mutable_place)
                .as_ref()
                != Some(access.place())
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let ty = self
                .local(access.place().local_id())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                .ty();
            collector.include_with_free_sources(
                CheckedCaptureExpression::from_place(owner, access.place(), ty)?,
                |source| sources.push(source),
            )?;
        }
        for source in sources {
            let local_use = self.checked_local_uses().access_at(source.site()).ok_or(
                FinalSemanticAnalysisError::ExpressionInputAccessUnavailable {
                    site: source.site(),
                },
            )?;
            if local_use.local() != source.local() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            let coordinate = coordinates
                .local_input(source.site(), source.local())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            uses.entry(source.local())
                .or_insert_with(Vec::new)
                .push(CheckedExpressionInputUse {
                    site: source.site(),
                    coordinate,
                    access: local_use,
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
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            inputs.push(CheckedExpressionInput {
                copy_requirement: self.checked_local_uses().copy_requirement(binding.local()),
                copy_evidence: self.checked_local_uses().copy_evidence(binding.local()),
                binding,
                uses: occurrences.into_boxed_slice(),
            });
        }
        if !uses.is_empty() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        inputs.sort_by(|left, right| left.binding.origin().cmp(right.binding.origin()));
        Ok(CheckedExpressionInputAbi {
            source,
            coordinate,
            execution,
            result,
            effects: expression.effects(),
            inputs: inputs.into_boxed_slice(),
        })
    }
}
