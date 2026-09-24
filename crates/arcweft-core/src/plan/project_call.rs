//! Typed project-function call plans.
//!
//! Project calls are control transfers, not synchronous expression helpers.
//! This module owns the HIR-free ABI product consumed by both the native flow
//! runtime and AWBC.  Physical operands retain source evaluation order while
//! logical parameter rows describe the checked ABI materialization.  No row
//! contains a source/HIR ID or a value-derived type guess.

use std::collections::BTreeSet;

use super::RuntimeCallableStateSeedId;
use crate::pattern::RuntimePattern;
use crate::runtime_id::{RuntimeCallableStateId, RuntimePlanTypeId, RuntimeProjectCallSiteId};
use crate::value::{RuntimeCallArgumentMode, RuntimeExpr};
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

/// Five-way checked presence classification for the optional attached Content
/// ABI member. Ordinary parameters do not use this state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallAttachedPresence {
    RequiredPresent,
    OptionalPresent,
    OptionalOmitted,
    DefaultedPresent,
    DefaultedOmitted,
}

/// Construction-only presence classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeProjectCallAttachedPresenceSeed {
    RequiredPresent,
    OptionalPresent,
    OptionalOmitted,
    DefaultedPresent,
    DefaultedOmitted,
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

    pub(crate) const fn abi_ty(&self) -> RuntimePlanTypeId {
        match self {
            Self::Fixed(row) => row.abi_ty(),
            Self::Rest(row) => row.abi_ty(),
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

/// Caller-owned evaluation and logical argument materialization. Reusable
/// transition/default code belongs exclusively to the referenced callable state.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallPlan {
    callee: RuntimeExpr,
    state: RuntimeCallableStateId,
    completed_group: u32,
    operands: Box<[RuntimeProjectCallOperand]>,
    ordinary: Box<[RuntimeProjectCallOrdinaryMaterialization]>,
    attached: Option<RuntimeProjectCallAttachedMaterialization>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProjectCallPlanSeed {
    pub callee: super::RuntimeExprSeed,
    pub state: RuntimeCallableStateSeedId,
    pub completed_group: u32,
    pub operands: Box<[RuntimeProjectCallOperandSeed]>,
    pub ordinary: Box<[RuntimeProjectCallOrdinaryMaterializationSeed]>,
    pub attached: Option<RuntimeProjectCallAttachedMaterializationSeed>,
}

impl RuntimeProjectCallPlanSeed {
    pub(super) fn collect_free_locals(
        &self,
        bound: &[super::RuntimeLocalSeedId],
        locals: &mut Vec<super::RuntimeLocalSeedId>,
    ) {
        self.callee.collect_free_locals_for_flow(bound, locals);
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
}

impl RuntimeProjectCallPlan {
    pub(crate) fn try_from_admitted_parts(
        callee: RuntimeExpr,
        state: RuntimeCallableStateId,
        completed_group: u32,
        operands: Box<[RuntimeProjectCallOperand]>,
        ordinary: Box<[RuntimeProjectCallOrdinaryMaterialization]>,
        attached: Option<RuntimeProjectCallAttachedMaterialization>,
    ) -> Result<Self, RuntimeProjectCallPlanError> {
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
        }
        for index in 0..operands.len() {
            let index = u32::try_from(index)
                .map_err(|_| RuntimeProjectCallPlanError::SourceOutOfRange { index: u32::MAX })?;
            if !sources.contains(&index) {
                return Err(RuntimeProjectCallPlanError::UnreferencedSource { index });
            }
        }
        Ok(Self {
            callee,
            state,
            completed_group,
            operands,
            ordinary,
            attached,
        })
    }

    #[must_use]
    pub const fn callee(&self) -> &RuntimeExpr {
        &self.callee
    }
    #[must_use]
    pub const fn state(&self) -> RuntimeCallableStateId {
        self.state
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
