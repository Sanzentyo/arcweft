//! Typed project-function call plans.
//!
//! Project calls are control transfers, not synchronous expression helpers.
//! This module owns the HIR-free ABI product consumed by both the native flow
//! runtime and AWBC.  Physical operands retain source evaluation order while
//! logical parameter rows describe the checked ABI materialization.  No row
//! contains a source/HIR ID or a value-derived type guess.

use std::collections::BTreeSet;

use arcweft_id::runtime_program::RuntimeProjectContinuationLineageId;

use super::RuntimeFunctionSiteSeedId;
use crate::pattern::RuntimePattern;
use crate::runtime_id::{RuntimeFunctionSiteId, RuntimePlanTypeId, RuntimeProjectCallSiteId};
use crate::value::{RuntimeCallArgumentMode, RuntimeExpr, RuntimeProjectContinuationAbi};
use thiserror::Error;

/// Source-order physical operand of one checked project call.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallOperand {
    value: RuntimeExpr,
    mode: RuntimeCallArgumentMode,
}

impl RuntimeProjectCallOperand {
    pub(crate) const fn from_admitted_parts(
        value: RuntimeExpr,
        mode: RuntimeCallArgumentMode,
    ) -> Self {
        Self { value, mode }
    }

    #[must_use]
    pub const fn value(&self) -> &RuntimeExpr {
        &self.value
    }

    #[must_use]
    pub const fn mode(&self) -> RuntimeCallArgumentMode {
        self.mode
    }
}

/// Construction-only source-order physical operand.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallOperandSeed {
    pub value: super::RuntimeExprSeed,
    pub mode: RuntimeCallArgumentMode,
    pub abi_position: u32,
}

/// Source of one capture when an omitted attached default constructs its
/// terminal FunctionSite.  Captures are selected from already materialized
/// logical values and are never reevaluated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallDefaultCaptureSource {
    ContinuationPrefix { position: u32 },
    CurrentLogical { position: u32 },
}

/// Exact terminal FunctionSite and once-only logical captures for an omitted
/// attached default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallDefaultFunction {
    site: RuntimeFunctionSiteId,
    captures: Box<[RuntimeProjectCallDefaultCaptureSource]>,
}

impl RuntimeProjectCallDefaultFunction {
    pub(crate) fn from_admitted_parts(
        site: RuntimeFunctionSiteId,
        captures: Box<[RuntimeProjectCallDefaultCaptureSource]>,
    ) -> Self {
        Self { site, captures }
    }

    #[must_use]
    pub const fn site(&self) -> RuntimeFunctionSiteId {
        self.site
    }

    #[must_use]
    pub const fn captures(&self) -> &[RuntimeProjectCallDefaultCaptureSource] {
        &self.captures
    }
}

/// Construction-only terminal default FunctionSite description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallDefaultFunctionSeed {
    pub site: RuntimeFunctionSiteSeedId,
    pub captures: Box<[RuntimeProjectCallDefaultCaptureSource]>,
}

/// Five-way checked presence classification for the optional attached Content
/// ABI member. Ordinary parameters do not use this state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallAttachedPresence {
    RequiredPresent,
    OptionalPresent,
    OptionalOmitted,
    DefaultedPresent,
    DefaultedOmitted(RuntimeProjectCallDefaultFunction),
}

/// Construction-only presence classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallAttachedPresenceSeed {
    RequiredPresent,
    OptionalPresent,
    OptionalOmitted,
    DefaultedPresent,
    DefaultedOmitted(RuntimeProjectCallDefaultFunctionSeed),
}

/// One ordinary fixed parameter and its single physical source row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallFixedMaterialization {
    parameter: u32,
    abi_ty: RuntimePlanTypeId,
    binding_ty: RuntimePlanTypeId,
    source_index: u32,
}

impl RuntimeProjectCallFixedMaterialization {
    pub(crate) fn from_admitted_parts(
        parameter: u32,
        abi_ty: RuntimePlanTypeId,
        binding_ty: RuntimePlanTypeId,
        source_index: u32,
    ) -> Self {
        Self {
            parameter,
            abi_ty,
            binding_ty,
            source_index,
        }
    }

    #[must_use]
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    #[must_use]
    pub const fn abi_ty(&self) -> RuntimePlanTypeId {
        self.abi_ty
    }

    #[must_use]
    pub const fn binding_ty(&self) -> RuntimePlanTypeId {
        self.binding_ty
    }

    #[must_use]
    pub const fn source_index(&self) -> u32 {
        self.source_index
    }
}

/// One ordinary rest parameter and its source-ordered physical rows. An empty
/// source list is the canonical empty rest pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallRestMaterialization {
    parameter: u32,
    abi_ty: RuntimePlanTypeId,
    binding_ty: RuntimePlanTypeId,
    source_indices: Box<[u32]>,
}

impl RuntimeProjectCallRestMaterialization {
    pub(crate) fn from_admitted_parts(
        parameter: u32,
        abi_ty: RuntimePlanTypeId,
        binding_ty: RuntimePlanTypeId,
        source_indices: Box<[u32]>,
    ) -> Self {
        Self {
            parameter,
            abi_ty,
            binding_ty,
            source_indices,
        }
    }

    #[must_use]
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    #[must_use]
    pub const fn abi_ty(&self) -> RuntimePlanTypeId {
        self.abi_ty
    }

    #[must_use]
    pub const fn binding_ty(&self) -> RuntimePlanTypeId {
        self.binding_ty
    }

    #[must_use]
    pub const fn source_indices(&self) -> &[u32] {
        &self.source_indices
    }
}

/// Ordinary logical materialization rows. Fixed and rest have different
/// source cardinality, so they remain distinct rather than sharing a
/// presence-bearing catch-all row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallOrdinaryMaterialization {
    Fixed(RuntimeProjectCallFixedMaterialization),
    Rest(RuntimeProjectCallRestMaterialization),
}

impl RuntimeProjectCallOrdinaryMaterialization {
    #[must_use]
    pub const fn parameter(&self) -> u32 {
        match self {
            Self::Fixed(row) => row.parameter(),
            Self::Rest(row) => row.parameter(),
        }
    }

    #[must_use]
    pub const fn binding_ty(&self) -> RuntimePlanTypeId {
        match self {
            Self::Fixed(row) => row.binding_ty(),
            Self::Rest(row) => row.binding_ty(),
        }
    }

    #[must_use]
    pub fn source_indices(&self) -> Box<[u32]> {
        match self {
            Self::Fixed(row) => Box::new([row.source_index()]),
            Self::Rest(row) => row.source_indices().to_vec().into_boxed_slice(),
        }
    }
}

/// Construction-only ordinary fixed parameter row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallFixedMaterializationSeed {
    pub parameter: u32,
    pub abi_ty: crate::pattern::RuntimeSemanticTypeId,
    pub binding_ty: crate::pattern::RuntimeSemanticTypeId,
    pub source_index: u32,
}

/// Construction-only ordinary rest parameter row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallRestMaterializationSeed {
    pub parameter: u32,
    pub abi_ty: crate::pattern::RuntimeSemanticTypeId,
    pub binding_ty: crate::pattern::RuntimeSemanticTypeId,
    pub source_indices: Box<[u32]>,
}

/// Construction-only ordinary logical materialization row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallOrdinaryMaterializationSeed {
    Fixed(RuntimeProjectCallFixedMaterializationSeed),
    Rest(RuntimeProjectCallRestMaterializationSeed),
}

/// Attached Content materialization is the only row with the five-way
/// presence classification. Present forms reference one physical source;
/// omitted forms reference none and may construct the exact default site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallAttachedMaterialization {
    abi_ty: RuntimePlanTypeId,
    binding_ty: RuntimePlanTypeId,
    source_index: Option<u32>,
    presence: RuntimeProjectCallAttachedPresence,
}

impl RuntimeProjectCallAttachedMaterialization {
    pub(crate) fn from_admitted_parts(
        abi_ty: RuntimePlanTypeId,
        binding_ty: RuntimePlanTypeId,
        source_index: Option<u32>,
        presence: RuntimeProjectCallAttachedPresence,
    ) -> Self {
        Self {
            abi_ty,
            binding_ty,
            source_index,
            presence,
        }
    }

    #[must_use]
    pub const fn abi_ty(&self) -> RuntimePlanTypeId {
        self.abi_ty
    }

    #[must_use]
    pub const fn binding_ty(&self) -> RuntimePlanTypeId {
        self.binding_ty
    }

    #[must_use]
    pub const fn source_index(&self) -> Option<u32> {
        self.source_index
    }

    #[must_use]
    pub const fn presence(&self) -> &RuntimeProjectCallAttachedPresence {
        &self.presence
    }
}

/// Construction-only attached Content materialization row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallAttachedMaterializationSeed {
    pub abi_ty: crate::pattern::RuntimeSemanticTypeId,
    pub binding_ty: crate::pattern::RuntimeSemanticTypeId,
    pub source_index: Option<u32>,
    pub presence: RuntimeProjectCallAttachedPresenceSeed,
}

/// Direct or continuation input of one typed project call.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectCallInput {
    Direct,
    Continuation {
        callee: RuntimeExpr,
        expected_abi: RuntimeProjectContinuationAbi,
    },
}

impl RuntimeProjectCallInput {
    #[must_use]
    pub const fn expected_abi(&self) -> Option<&RuntimeProjectContinuationAbi> {
        match self {
            Self::Direct => None,
            Self::Continuation { expected_abi, .. } => Some(expected_abi),
        }
    }
}

/// Construction-only direct/continuation input.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectCallInputSeed {
    Direct,
    Continuation {
        callee: super::RuntimeExprSeed,
        expected_abi: RuntimeProjectCallAbiSeed,
    },
}

/// Construction-time ABI identity. Semantic identities are rewritten to
/// plan-local IDs by the aggregate builder before a final call plan exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectCallAbiSeed {
    pub lineage: RuntimeProjectContinuationLineageId,
    pub function_type: crate::pattern::RuntimeSemanticTypeId,
    pub prefix_types: Box<[crate::pattern::RuntimeSemanticTypeId]>,
}

/// Checked result of one project-call group.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectCallOutcome {
    Continue {
        result_abi: RuntimeProjectContinuationAbi,
        next_group: u32,
    },
    Invoke {
        function_site: RuntimeFunctionSiteId,
    },
}

/// Construction-only project-call outcome.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeProjectCallOutcomeSeed {
    Continue {
        result_abi: RuntimeProjectCallAbiSeed,
        next_group: u32,
    },
    Invoke {
        function_site: RuntimeFunctionSiteSeedId,
    },
}

/// Fully typed HIR-free project-call plan.  The result pattern intentionally
/// lives on [`super::FlowOp`], because it is a caller-owned destination rather
/// than part of the reusable callable ABI descriptor.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallPlan {
    input: RuntimeProjectCallInput,
    completed_group: u32,
    operands: Box<[RuntimeProjectCallOperand]>,
    ordinary: Box<[RuntimeProjectCallOrdinaryMaterialization]>,
    attached: Option<RuntimeProjectCallAttachedMaterialization>,
    outcome: RuntimeProjectCallOutcome,
}

/// Construction-only project-call plan.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallPlanSeed {
    pub input: RuntimeProjectCallInputSeed,
    pub completed_group: u32,
    pub operands: Box<[RuntimeProjectCallOperandSeed]>,
    pub ordinary: Box<[RuntimeProjectCallOrdinaryMaterializationSeed]>,
    pub attached: Option<RuntimeProjectCallAttachedMaterializationSeed>,
    pub outcome: RuntimeProjectCallOutcomeSeed,
}

impl RuntimeProjectCallPlanSeed {
    pub(super) fn collect_free_locals(
        &self,
        bound: &[super::RuntimeLocalSeedId],
        locals: &mut Vec<super::RuntimeLocalSeedId>,
    ) {
        if let RuntimeProjectCallInputSeed::Continuation { callee, .. } = &self.input {
            callee.collect_free_locals_for_flow(bound, locals);
        }
        for operand in &self.operands {
            operand.value.collect_free_locals_for_flow(bound, locals);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProjectCallPlanError {
    #[error("project-call logical parameter rows are not dense at {parameter}")]
    NonCanonicalParameter { parameter: u32 },
    #[error("project-call physical operand position {position} is repeated")]
    DuplicateAbiPosition { position: u32 },
    #[error("project-call physical operand positions are not contiguous at {position}")]
    NonContiguousAbiPosition { position: u32 },
    #[error("project-call logical materialization references source operand {index} out of range")]
    SourceOutOfRange { index: u32 },
    #[error("project-call source operand {index} is referenced more than once")]
    DuplicateSourceReference { index: u32 },
    #[error("project-call source operand {index} is not referenced by a logical row")]
    UnreferencedSource { index: u32 },
    #[error("project-call source indices for parameter {parameter} are not source ordered")]
    NonCanonicalSourceOrder { parameter: u32 },
    #[error("fixed project-call parameter {parameter} must reference exactly one value operand")]
    FixedSourceShape { parameter: u32 },
    #[error("attached project-call Content source is missing for a present row")]
    AttachedSourceMissing,
    #[error("attached project-call Content source is present for an omitted row")]
    AttachedSourceUnexpected,
    #[error("attached default capture references logical parameter {position} out of range")]
    DefaultCaptureOutOfRange { position: u32 },
    #[error("project-call continuation result ABI does not retain its input prefix")]
    ResultAbiPrefixMismatch,
    #[error("project-call continuation result ABI has the wrong logical prefix length")]
    ResultAbiLengthMismatch,
    #[error("project-call continuation advances to group {actual}; expected {expected}")]
    NextGroupMismatch { expected: u32, actual: u32 },
    #[error("direct project-call input is only valid for group 0, found {actual}")]
    DirectGroupMismatch { actual: u32 },
    #[error("continuation project-call input is invalid for group 0")]
    ContinuationGroupMismatch,
    #[error("project-call continuation cannot carry an attached materialization")]
    ContinueAttachedMismatch,
    #[error("an omitted attached default is only valid for an invoking project-call")]
    DefaultedOmittedOutcomeMismatch,
}

impl RuntimeProjectCallPlan {
    pub(crate) fn try_from_admitted_parts(
        input: RuntimeProjectCallInput,
        completed_group: u32,
        operands: Box<[RuntimeProjectCallOperand]>,
        ordinary: Box<[RuntimeProjectCallOrdinaryMaterialization]>,
        attached: Option<RuntimeProjectCallAttachedMaterialization>,
        outcome: RuntimeProjectCallOutcome,
    ) -> Result<Self, RuntimeProjectCallPlanError> {
        match &input {
            RuntimeProjectCallInput::Direct if completed_group != 0 => {
                return Err(RuntimeProjectCallPlanError::DirectGroupMismatch {
                    actual: completed_group,
                });
            }
            RuntimeProjectCallInput::Continuation { .. } if completed_group == 0 => {
                return Err(RuntimeProjectCallPlanError::ContinuationGroupMismatch);
            }
            RuntimeProjectCallInput::Direct | RuntimeProjectCallInput::Continuation { .. } => {}
        }
        if matches!(outcome, RuntimeProjectCallOutcome::Continue { .. }) && attached.is_some() {
            return Err(RuntimeProjectCallPlanError::ContinueAttachedMismatch);
        }
        if attached.as_ref().is_some_and(|row| {
            matches!(
                row.presence(),
                RuntimeProjectCallAttachedPresence::DefaultedOmitted(_)
            )
        }) && !matches!(outcome, RuntimeProjectCallOutcome::Invoke { .. })
        {
            return Err(RuntimeProjectCallPlanError::DefaultedOmittedOutcomeMismatch);
        }
        let mut sources = BTreeSet::new();
        for (expected, row) in ordinary.iter().enumerate() {
            if u32::try_from(expected).ok() != Some(row.parameter()) {
                return Err(RuntimeProjectCallPlanError::NonCanonicalParameter {
                    parameter: row.parameter(),
                });
            }
            let source_indices = row.source_indices();
            if source_indices.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RuntimeProjectCallPlanError::NonCanonicalSourceOrder {
                    parameter: row.parameter(),
                });
            }
            if matches!(row, RuntimeProjectCallOrdinaryMaterialization::Fixed(_))
                && (source_indices.len() != 1
                    || !usize::try_from(source_indices[0])
                        .ok()
                        .and_then(|index| operands.get(index))
                        .is_some_and(|operand| operand.mode() == RuntimeCallArgumentMode::Value))
            {
                return Err(RuntimeProjectCallPlanError::FixedSourceShape {
                    parameter: row.parameter(),
                });
            }
            for index in source_indices {
                let Some(_) = usize::try_from(index)
                    .ok()
                    .and_then(|index| operands.get(index))
                else {
                    return Err(RuntimeProjectCallPlanError::SourceOutOfRange { index });
                };
                if !sources.insert(index) {
                    return Err(RuntimeProjectCallPlanError::DuplicateSourceReference { index });
                }
            }
        }
        if let Some(attached) = &attached {
            if let Some(index) = attached.source_index() {
                let Some(_) = usize::try_from(index)
                    .ok()
                    .and_then(|index| operands.get(index))
                else {
                    return Err(RuntimeProjectCallPlanError::SourceOutOfRange { index });
                };
                if !sources.insert(index) {
                    return Err(RuntimeProjectCallPlanError::DuplicateSourceReference { index });
                }
            }
            let present = matches!(
                attached.presence(),
                RuntimeProjectCallAttachedPresence::RequiredPresent
                    | RuntimeProjectCallAttachedPresence::OptionalPresent
                    | RuntimeProjectCallAttachedPresence::DefaultedPresent
            );
            if present != attached.source_index().is_some() {
                return Err(if present {
                    RuntimeProjectCallPlanError::AttachedSourceMissing
                } else {
                    RuntimeProjectCallPlanError::AttachedSourceUnexpected
                });
            }
            if let RuntimeProjectCallAttachedPresence::DefaultedOmitted(default) =
                attached.presence()
            {
                for capture in default.captures() {
                    if let RuntimeProjectCallDefaultCaptureSource::CurrentLogical { position } =
                        capture
                    {
                        if usize::try_from(*position)
                            .ok()
                            .is_none_or(|position| position >= ordinary.len())
                        {
                            return Err(RuntimeProjectCallPlanError::DefaultCaptureOutOfRange {
                                position: *position,
                            });
                        }
                    }
                }
            }
        }
        for index in 0..operands.len() {
            let index = u32::try_from(index)
                .map_err(|_| RuntimeProjectCallPlanError::SourceOutOfRange { index: u32::MAX })?;
            if !sources.contains(&index) {
                return Err(RuntimeProjectCallPlanError::UnreferencedSource { index });
            }
        }
        if let RuntimeProjectCallOutcome::Continue {
            result_abi,
            next_group,
        } = &outcome
        {
            let expected_next_group = completed_group.checked_add(1).ok_or(
                RuntimeProjectCallPlanError::NextGroupMismatch {
                    expected: u32::MAX,
                    actual: *next_group,
                },
            )?;
            if *next_group != expected_next_group {
                return Err(RuntimeProjectCallPlanError::NextGroupMismatch {
                    expected: expected_next_group,
                    actual: *next_group,
                });
            }
            let input_prefix = input
                .expected_abi()
                .map_or(&[][..], RuntimeProjectContinuationAbi::prefix_types);
            if !result_abi.prefix_types().starts_with(input_prefix) {
                return Err(RuntimeProjectCallPlanError::ResultAbiPrefixMismatch);
            }
            let expected_len = input_prefix
                .len()
                .checked_add(ordinary.len())
                .and_then(|length| length.checked_add(usize::from(attached.is_some())))
                .ok_or(RuntimeProjectCallPlanError::ResultAbiLengthMismatch)?;
            if result_abi.prefix_types().len() != expected_len {
                return Err(RuntimeProjectCallPlanError::ResultAbiLengthMismatch);
            }
            // The continuation ABI deliberately stores stable semantic type
            // identities while materialization rows are plan-local IDs.  The
            // enclosing plan lowerer performs this cross-domain comparison;
            // this context-free constructor only enforces prefix shape and
            // cardinality.
        }
        Ok(Self {
            input,
            completed_group,
            operands,
            ordinary,
            attached,
            outcome,
        })
    }

    #[must_use]
    pub const fn input(&self) -> &RuntimeProjectCallInput {
        &self.input
    }

    #[must_use]
    pub const fn completed_group(&self) -> u32 {
        self.completed_group
    }

    #[must_use]
    pub const fn operands(&self) -> &[RuntimeProjectCallOperand] {
        &self.operands
    }

    #[must_use]
    pub const fn ordinary(&self) -> &[RuntimeProjectCallOrdinaryMaterialization] {
        &self.ordinary
    }

    #[must_use]
    pub const fn attached(&self) -> Option<&RuntimeProjectCallAttachedMaterialization> {
        self.attached.as_ref()
    }

    #[must_use]
    pub const fn outcome(&self) -> &RuntimeProjectCallOutcome {
        &self.outcome
    }
}

/// Immutable plan-owned catalog of every lowered ProjectCall descriptor.
/// Flow operations carry only [`RuntimeProjectCallSiteId`]; all ABI and
/// materialization authority is recovered from this table.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallSiteTable {
    rows: Box<[RuntimeProjectCallSite]>,
}

impl RuntimeProjectCallSiteTable {
    pub(crate) fn from_admitted_rows(rows: Box<[RuntimeProjectCallSite]>) -> Self {
        Self { rows }
    }

    #[must_use]
    pub fn get(&self, site: RuntimeProjectCallSiteId) -> Option<&RuntimeProjectCallSite> {
        self.rows.get(site.index())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RuntimeProjectCallSite> {
        self.rows.iter()
    }
}

/// One caller-owned ProjectCall site. The operation carries only its site ID;
/// the result pattern and reusable call descriptor are sealed together here.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallSite {
    plan: RuntimeProjectCallPlan,
    result: RuntimePattern,
}

impl RuntimeProjectCallSite {
    pub(crate) fn from_admitted_parts(
        plan: RuntimeProjectCallPlan,
        result: RuntimePattern,
    ) -> Self {
        Self { plan, result }
    }

    #[must_use]
    pub const fn plan(&self) -> &RuntimeProjectCallPlan {
        &self.plan
    }

    #[must_use]
    pub const fn result(&self) -> &RuntimePattern {
        &self.result
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProjectCallSiteTableError {
    #[error("project-call site identity space is exhausted")]
    IdentityExhausted,
}
