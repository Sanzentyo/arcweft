//! Explicit recursive program-bound save projection for live runtime values.
//!
//! `RuntimeValue` deliberately rejects generic serde for function values.  A
//! quiescent save or rollback uses the typed DTOs in this module rather than
//! serializing or deserializing the live value graph.

use super::{
    AgentPredicateOperands, DenseSeq, RecordSeq, RuntimeAgentActionTarget,
    RuntimeAgentCaptureTarget, RuntimeAgentCompareOp, RuntimeAgentConstructionError,
    RuntimeAgentPath, RuntimeAgentPredicate, RuntimeAgentProbe, RuntimeAgentValue,
    RuntimeCallableValue, RuntimeCommand, RuntimeEntityReference, RuntimeIterator,
    RuntimeNominalRecordValue, RuntimeOpaqueValue, RuntimePayload, RuntimeReductionValue,
    RuntimeSeq, RuntimeValue, TupleSeq,
};
use crate::entry::{RuntimeCommandConstructorId, RuntimeCommandTargetId};
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
use crate::runtime_id::RuntimeCallableStateId;
use crate::task::RuntimeProgramOwner;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::VecDeque;
use thiserror::Error;

#[cfg(test)]
pub(super) mod tests;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AwbcRuntimeValueSnapshotError {
    #[error("{message}")]
    Message { message: String },
    #[error("invalid Agent predicate: {0}")]
    AgentPredicate(#[from] RuntimeAgentConstructionError),
    #[error("unrestricted snapshot contains an affine value")]
    AffineValue,
    #[error("retained type {expected:?} requires its original program owner")]
    ProgramOwnerRequired { expected: RuntimeSemanticTypeId },
    #[error(transparent)]
    Nesting(#[from] super::RuntimeValueNestingError),
    #[error(transparent)]
    Resource(crate::entry::RuntimeSchemaError),
    #[error(transparent)]
    ProgramType(Box<crate::program_types::RuntimeProgramTypeError>),
    #[error(transparent)]
    TaskOutcome(#[from] crate::task::TaskOutcomeBindingError),
}

impl From<crate::entry::RuntimeSchemaError> for AwbcRuntimeValueSnapshotError {
    fn from(error: crate::entry::RuntimeSchemaError) -> Self {
        match error {
            crate::entry::RuntimeSchemaError::ValueNesting(error) => Self::Nesting(error),
            error => Self::Resource(error),
        }
    }
}

impl From<crate::program_types::RuntimeProgramTypeError> for AwbcRuntimeValueSnapshotError {
    fn from(error: crate::program_types::RuntimeProgramTypeError) -> Self {
        Self::ProgramType(Box::new(error))
    }
}

impl From<crate::task::TaskOutcomeValueError> for AwbcRuntimeValueSnapshotError {
    fn from(error: crate::task::TaskOutcomeValueError) -> Self {
        match error {
            crate::task::TaskOutcomeValueError::Resource(error) => (*error).into(),
            crate::task::TaskOutcomeValueError::Program(error) => error.into(),
            error => Self::Message {
                message: error.to_string(),
            },
        }
    }
}

impl From<super::RuntimeCallableValueError> for AwbcRuntimeValueSnapshotError {
    fn from(error: super::RuntimeCallableValueError) -> Self {
        match error {
            super::RuntimeCallableValueError::PendingResource { source, .. } => Self::from(*source),
            super::RuntimeCallableValueError::RetainedType {
                state,
                position,
                error,
            } => Self::ProgramType(Box::new(
                error.within_value_path(&format!("callable[{state}].retained[{position}]")),
            )),
            super::RuntimeCallableValueError::ArgumentType {
                state,
                position,
                error,
            } => Self::ProgramType(Box::new(
                error.within_value_path(&format!("callable[{state}].arguments[{position}]")),
            )),
            error => Self::Message {
                message: error.to_string(),
            },
        }
    }
}

impl AwbcRuntimeValueSnapshotError {
    fn new(message: impl Into<String>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

/// Typed recursive AWBC session-save representation of one live value.
///
/// Callable values snapshot only their program-local state identity and
/// retained values; restore supplies the exact immutable program lease.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeValueSnapshot {
    Unit,
    Bool(bool),
    Int(super::RuntimeInt),
    UInt(super::RuntimeUInt),
    F32(u32),
    F64(u64),
    DenseNumeric(crate::math::RuntimeDenseNumericPayload),
    String(String),
    Color(super::RuntimeColor),
    NeedHandle(crate::task::RuntimeNeedHandleSaveSnapshot),
    Char(char),
    Duration(crate::time::LogicalDuration),
    Progress {
        ratio: f32,
        label: Option<String>,
    },
    Range(super::RuntimeRange),
    Iterator(AwbcRuntimeIteratorSnapshot),
    EntityRef(RuntimeEntityReference),
    Tuple(Vec<Self>),
    Seq(AwbcRuntimeSeqSnapshot),
    Record(Vec<AwbcRuntimeFieldSnapshot>),
    NominalRecord(AwbcRuntimeNominalRecordSnapshot),
    Opaque(AwbcRuntimeOpaqueSnapshot),
    Reduction(AwbcRuntimeReductionSnapshot),
    Agent(AwbcRuntimeAgentSnapshot),
    Callable(AwbcRuntimeCallableSnapshot),
    Variant {
        owner: super::RuntimeVariantIdentity,
        ordinal: u32,
        name: String,
        type_instantiation: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
        payload: Option<Box<Self>>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeIteratorSnapshot {
    Values {
        items: Vec<AwbcRuntimeValueSnapshot>,
    },
    Range(super::RuntimeRangeIterator),
    Witness {
        state: Box<AwbcRuntimeValueSnapshot>,
        next: crate::plan::RuntimeTraitMethodId,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeSeqSnapshot {
    Values(Vec<AwbcRuntimeValueSnapshot>),
    Dense(DenseSeq),
    TupleColumns {
        len: u64,
        columns: Vec<AwbcRuntimeSeqSnapshot>,
    },
    RecordColumns {
        len: u64,
        fields: Vec<AwbcRuntimeRecordSeqFieldSnapshot>,
    },
}

impl PartialEq for AwbcRuntimeSeqSnapshot {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Values(left), Self::Values(right)) => left == right,
            (Self::Dense(DenseSeq::F32(left)), Self::Dense(DenseSeq::F32(right))) => left
                .as_slice()
                .iter()
                .map(|value| value.to_bits())
                .eq(right.as_slice().iter().map(|value| value.to_bits())),
            (Self::Dense(DenseSeq::F64(left)), Self::Dense(DenseSeq::F64(right))) => left
                .as_slice()
                .iter()
                .map(|value| value.to_bits())
                .eq(right.as_slice().iter().map(|value| value.to_bits())),
            (Self::Dense(left), Self::Dense(right)) => left == right,
            (
                Self::TupleColumns {
                    len: left_len,
                    columns: left,
                },
                Self::TupleColumns {
                    len: right_len,
                    columns: right,
                },
            ) => left_len == right_len && left == right,
            (
                Self::RecordColumns {
                    len: left_len,
                    fields: left,
                },
                Self::RecordColumns {
                    len: right_len,
                    fields: right,
                },
            ) => left_len == right_len && left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeRecordSeqFieldSnapshot {
    pub field: super::RuntimeRecordFieldId,
    pub name: String,
    pub values: AwbcRuntimeSeqSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeFieldSnapshot {
    pub field: super::RuntimeRecordFieldId,
    pub name: String,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeNominalRecordSnapshot {
    pub type_id: crate::entry::RuntimeNominalTypeId,
    pub semantic_identity: RuntimeSemanticTypeId,
    pub layout: crate::entry::TypeLayoutHash,
    pub type_instantiation: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    pub fields: Vec<AwbcRuntimeNominalFieldSnapshot>,
}

/// One defining-order nominal field in the inert snapshot grammar.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeNominalFieldSnapshot {
    pub field: super::RuntimeRecordFieldId,
    pub value: AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeOpaqueSnapshot {
    pub producer: crate::pattern::RuntimeOpaqueTypeProducerId,
    pub semantic_identity: crate::pattern::RuntimeSemanticTypeId,
    pub value_class: super::RuntimeOpaqueValueClass,
    pub persistence: super::RuntimeOpaquePersistence,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeReductionSnapshot {
    pub owner: RuntimeOpaqueTypeOwner,
    pub state: Box<AwbcRuntimeValueSnapshot>,
    pub commands: Vec<AwbcRuntimeCommandSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeCommandSnapshot {
    pub constructor: RuntimeCommandConstructorId,
    pub target: RuntimeCommandTargetId,
    pub payload: Box<AwbcRuntimeValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeAgentSnapshot {
    ActionTarget(RuntimeAgentActionTarget),
    CaptureTarget(RuntimeAgentCaptureTarget),
    DebugStatePath(RuntimeAgentPath),
    ObservationFieldPath(RuntimeAgentPath),
    Probe(RuntimeAgentProbe),
    Diagnostics,
    Predicate(AwbcRuntimeAgentPredicateSnapshot),
    ViewportPoint {
        x: u32,
        y: u32,
    },
    BinaryData(String),
    DataShape {
        shape_type: RuntimeSemanticTypeId,
        value_type: RuntimeSemanticTypeId,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AwbcRuntimeAgentPredicateSnapshot {
    Compare {
        probe: RuntimeAgentProbe,
        op: RuntimeAgentCompareOp,
        value: Box<AwbcRuntimeValueSnapshot>,
    },
    Exists {
        probe: RuntimeAgentProbe,
    },
    ActionEnabled {
        target: RuntimeCommandTargetId,
    },
    DiagnosticsHasError,
    All {
        predicates: AgentPredicateOperands<AwbcRuntimeAgentPredicateSnapshot>,
    },
    Any {
        predicates: AgentPredicateOperands<AwbcRuntimeAgentPredicateSnapshot>,
    },
    Not {
        predicate: Box<AwbcRuntimeAgentPredicateSnapshot>,
    },
}

/// AWBC session-save projection of a program-owned callable state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeCallableSnapshot {
    pub state: RuntimeCallableStateId,
    pub retained: Vec<AwbcRuntimeValueSnapshot>,
}

mod admission;
mod projection;
mod retained_admission;
pub use retained_admission::RuntimeRetainedValueAdmission;

impl RuntimeValue {
    /// Checks borrowed retained data before an enclosing publication copies
    /// any of its roots. Uses the same admission as direct retained copying.
    pub fn validate_unrestricted_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(Some(owner), None, limits)
    }

    /// Admits a borrowed retained value against its original expected type
    /// without constructing another live value or snapshot.
    pub fn validate_unrestricted_typed_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        expected: RuntimeSemanticTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(Some(owner), Some(expected), limits)
    }

    /// Admits borrowed context-free retained data without guessing an owner
    /// for program-bound descendants.
    pub fn validate_unrestricted_standalone_with_limits(
        &self,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(None, None, limits)
    }
    /// Copies retained data after bounded deep Copy and original executable
    /// admission, without constructing an intermediate snapshot.
    pub fn copy_unrestricted_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        self.copy_unrestricted_value_for_program(owner, None, limits)
    }

    /// Checks the original expected type before copying any live descendant.
    pub fn copy_unrestricted_typed_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        expected: RuntimeSemanticTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        self.copy_unrestricted_value_for_program(owner, Some(expected), limits)
    }

    fn copy_unrestricted_value_for_program(
        &self,
        owner: &RuntimeProgramOwner,
        expected: Option<RuntimeSemanticTypeId>,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(Some(owner), expected, limits)?;
        Ok(self.clone())
    }

    fn validate_unrestricted_value_for_program(
        &self,
        owner: Option<&RuntimeProgramOwner>,
        expected: Option<RuntimeSemanticTypeId>,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let mut admission = match owner {
            Some(owner) => RuntimeRetainedValueAdmission::for_program(owner.clone(), limits),
            None => RuntimeRetainedValueAdmission::standalone(limits),
        };
        match expected {
            Some(expected) => admission.validate_typed(self, expected),
            None => admission.validate(self),
        }
    }
}

impl AwbcRuntimeValueSnapshot {
    pub(crate) fn validate_admitted_storage_view(
        value: super::RuntimeValueView<'_>,
        owner: Option<&RuntimeProgramOwner>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        admission::SnapshotStorageAdmission::new(owner, resources).validate(value)
    }
    /// Captures a borrowed callable through its exact executable lease without
    /// constructing a second live `RuntimeValue::Callable` carrier.
    pub(crate) fn from_callable_for_program(
        value: &RuntimeCallableValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let view = super::RuntimeValueView::Callable(super::RuntimeCallableView::Live(value));
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        resources.validate_view(view, 0, None)?;
        Self::validate_admitted_storage_view(view, Some(owner), &resources)?;
        Self::from_admitted_callable(value, owner)
    }

    pub(crate) fn from_admitted_callable(
        value: &RuntimeCallableValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::callable_from_live(value, Some(owner)).map(Self::Callable)
    }

    /// Derives the complete line-handle token inventory from the typed save
    /// representation without activating a second live `RuntimeValue` graph.
    pub(crate) fn affine_line_handle_tokens(
        &self,
    ) -> Result<Vec<crate::runtime_id::RuntimeLineHandleToken>, AwbcRuntimeValueSnapshotError> {
        crate::entry::schema::value_encoding::preflight_snapshot_view(
            self.view(),
            crate::entry::RuntimeSchemaLimits::engine_default(),
            None,
        )?;
        self.view()
            .affine_line_handles()
            .map(|handles| {
                handles
                    .into_iter()
                    .map(|handle| handle.token().clone())
                    .collect()
            })
            .map_err(|error| match error {
                super::ownership::RuntimeAffineLineHandleError::Schema(error) => (*error).into(),
                error => crate::entry::RuntimeSchemaError::Encoding {
                    message: error.to_string(),
                }
                .into(),
            })
    }

    /// Rejects affine descendants and excessive nesting while this graph is
    /// still inert, before any Need or opaque handle can acquire a live owner.
    pub fn into_unrestricted_runtime_value_for_program(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_unrestricted_runtime_value_for_program_with_limits(
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Restores a copyable value after applying the selected entry's graph
    /// limits and rejecting affine descendants while the complete graph is inert.
    pub fn into_unrestricted_runtime_value_for_program_with_limits(
        self,
        owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_unrestricted_value_for_program(owner, None, limits)
    }

    /// Admits the selected original expected type before constructing any live
    /// descendant, while rejecting affine values in this copyable storage role.
    pub fn into_unrestricted_typed_runtime_value_for_program_with_limits(
        self,
        owner: &RuntimeProgramOwner,
        expected: RuntimeSemanticTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_unrestricted_value_for_program(owner, Some(expected), limits)
    }

    fn into_unrestricted_value_for_program(
        self,
        owner: &RuntimeProgramOwner,
        expected: Option<RuntimeSemanticTypeId>,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(owner, expected, limits)?;
        self.into_live_with_owner(Some(owner))
    }

    /// Copies an inert retained value only after admitting its complete graph
    /// against the original program. The borrowed pass precedes DTO copying
    /// and live construction and uses the same rules as owned restoration.
    pub fn copy_unrestricted_runtime_value_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(owner, None, limits)?;
        self.clone().into_live_with_owner(Some(owner))
    }

    /// Checks the original expected type while this retained snapshot is
    /// borrowed, before copying its DTO or constructing any live descendants.
    pub fn copy_unrestricted_typed_runtime_value_for_program_with_limits(
        &self,
        owner: &RuntimeProgramOwner,
        expected: RuntimeSemanticTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.validate_unrestricted_value_for_program(owner, Some(expected), limits)?;
        self.clone().into_live_with_owner(Some(owner))
    }

    fn validate_unrestricted_value_for_program(
        &self,
        owner: &RuntimeProgramOwner,
        expected: Option<RuntimeSemanticTypeId>,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let mut admission = RuntimeRetainedValueAdmission::for_program(owner.clone(), limits);
        match expected {
            Some(expected) => admission.validate_typed_snapshot(self, expected),
            None => admission.validate_snapshot(self),
        }
    }

    /// Captures copyable standalone data. Program-owned descendants require
    /// the explicit executable entry point and cannot acquire a guessed owner.
    pub fn from_unrestricted_standalone_value(
        value: &RuntimeValue,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        RuntimeRetainedValueAdmission::standalone(
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .validate(value)?;
        Self::from_admitted_storage(value, None)
    }

    /// Restores context-free copyable data through the same exhaustive graph
    /// decoder, rejecting affine and program-owned descendants before construction.
    pub fn into_unrestricted_standalone_value(
        self,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        RuntimeRetainedValueAdmission::standalone(
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
        .validate_snapshot(&self)?;
        self.into_live_with_owner(None)
    }

    pub(crate) fn from_opaque_for_program(
        value: &RuntimeOpaqueValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let view = super::RuntimeValueView::Opaque(super::RuntimeOpaqueView::Live(value));
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        resources.validate_view(view, 0, None)?;
        Self::validate_admitted_storage_view(view, Some(owner), &resources)?;
        Self::opaque_from_live(value, Some(owner)).map(Self::Opaque)
    }

    pub(crate) fn from_admitted_opaque(
        value: &RuntimeOpaqueValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::opaque_from_live(value, Some(owner)).map(Self::Opaque)
    }

    /// Internal standalone task/data capture. Program-owned descendants are
    /// rejected; executable snapshots use the selected program entry point.
    pub(crate) fn from_standalone_value(
        value: &RuntimeValue,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        resources.validate_view(value.view(), 0, None)?;
        Self::validate_admitted_storage_view(value.view(), None, &resources)?;
        Self::from_admitted_storage(value, None)
    }

    /// Captures an inert value under its exact immutable executable lease.
    /// Callable values anywhere in the graph must belong to this owner. Every
    /// nominal value is validated through its type graph before DTO exposure.
    pub fn from_runtime_value_for_program(
        value: &RuntimeValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::from_runtime_value_for_program_with_limits(
            value,
            owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Checks the borrowed snapshot value grammar against the selected resource
    /// policy before cloning any retained payload into the inert projection.
    pub fn from_runtime_value_for_program_with_limits(
        value: &RuntimeValue,
        owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            limits,
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        resources.validate_view(value.view(), 0, None)?;
        Self::validate_admitted_storage_view(value.view(), Some(owner), &resources)?;
        Self::from_admitted_storage(value, Some(owner))
    }

    /// Captures a retained copyable value without duplicating an affine owner.
    pub fn from_unrestricted_runtime_value_for_program(
        value: &RuntimeValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        value.view().admit_unrestricted(&mut resources)?;
        Self::validate_admitted_storage_view(value.view(), Some(owner), &resources)?;
        Self::from_admitted_storage(value, Some(owner))
    }

    pub(crate) fn from_runtime_iterator_for_program(
        value: &RuntimeIterator,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        let view = super::RuntimeValueView::Iterator(super::RuntimeIteratorView::Live(value));
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::Live,
        );
        resources.validate_view(view, 0, None)?;
        Self::validate_admitted_storage_view(view, Some(owner), &resources)?;
        Self::iterator_from_live(value, Some(owner)).map(Self::Iterator)
    }

    pub(crate) fn into_runtime_iterator_for_program(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeIterator, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            crate::entry::RuntimeSchemaLimits::engine_default(),
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
        );
        resources.validate_view(self.view(), 0, None)?;
        Self::validate_admitted_storage_view(self.view(), Some(owner), &resources)?;
        let Self::Iterator(iterator) = self else {
            return Err(AwbcRuntimeValueSnapshotError::new(
                "rollback image does not contain an iterator",
            ));
        };
        Self::iterator_into_live(iterator, Some(owner))
    }

    /// Projects a borrowed value only after its enclosing retained owner has
    /// admitted the complete input graph, original types and cumulative limits.
    pub(crate) fn from_admitted_runtime_value(
        value: &RuntimeValue,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::from_admitted_storage(value, Some(owner))
    }

    /// Constructs one previously admitted inert value under its original owner.
    /// The enclosing packet must finish admission before mapping any of its rows.
    pub(crate) fn into_admitted_runtime_value(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_live_with_owner(Some(owner))
    }

    /// Projects only after the enclosing owner admits the complete graph,
    /// original executable storage and cumulative resource allowance.
    pub(crate) fn from_admitted_storage(
        value: &RuntimeValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<Self, AwbcRuntimeValueSnapshotError> {
        Self::from_admitted_view(value.view(), owner)
    }

    /// Restores values, including `DataShape` witnesses, after the enclosing
    /// snapshot has been correlated with this exact executable artifact.
    pub fn into_runtime_value_for_program(
        self,
        program_owner: &RuntimeProgramOwner,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.into_runtime_value_for_program_with_limits(
            program_owner,
            crate::entry::RuntimeSchemaLimits::engine_default(),
        )
    }

    /// Applies the selected graph policy before entering any live constructor.
    pub fn into_runtime_value_for_program_with_limits(
        self,
        program_owner: &RuntimeProgramOwner,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let mut resources = crate::entry::schema::value_encoding::ValueResourceAdmission::snapshot(
            limits,
            crate::entry::schema::value_encoding::ValueAdmissionOrigin::InertSnapshot,
        );
        resources.validate_view(self.view(), 0, None)?;
        Self::validate_admitted_storage_view(self.view(), Some(program_owner), &resources)?;
        self.into_live_with_owner(Some(program_owner))
    }

    /// Restores an owned candidate only after its complete borrowed graph has
    /// been admitted against the selected original executable type.
    pub fn into_typed_runtime_value_for_program_with_limits(
        self,
        owner: &RuntimeProgramOwner,
        expected: RuntimeSemanticTypeId,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        owner
            .types()
            .validate_inert_snapshot_value(expected, &self, limits)
            .map_err(|error| AwbcRuntimeValueSnapshotError::ProgramType(Box::new(error)))?;
        self.into_live_with_owner(Some(owner))
    }

    fn into_live_with_owner(
        self,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        Ok(match self {
            Self::Unit => RuntimeValue::Unit,
            Self::Bool(value) => RuntimeValue::Bool(value),
            Self::Int(value) => RuntimeValue::Int(value),
            Self::UInt(value) => RuntimeValue::UInt(value),
            Self::F32(value) => RuntimeValue::F32(f32::from_bits(value)),
            Self::F64(value) => RuntimeValue::F64(f64::from_bits(value)),
            Self::DenseNumeric(value) => RuntimeValue::try_from(&value)
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            Self::String(value) => RuntimeValue::String(value),
            Self::Color(value) => RuntimeValue::Color(value),
            Self::NeedHandle(value) => RuntimeValue::NeedHandle(value.into_live_admitted(
                program_owner.ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "Need values require a program-bound snapshot",
                    )
                })?,
            )?),
            Self::Char(value) => RuntimeValue::Char(value),
            Self::Duration(value) => RuntimeValue::Duration(value),
            Self::Progress { ratio, label } => {
                let progress = crate::value::Progress::new(ratio)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                RuntimeValue::Progress(match label {
                    Some(label) => progress.with_label(label),
                    None => progress,
                })
            }
            Self::Range(value) => RuntimeValue::Range(value),
            Self::Iterator(value) => {
                RuntimeValue::Iterator(Self::iterator_into_live(value, program_owner)?)
            }
            Self::EntityRef(value) => RuntimeValue::EntityRef(value),
            Self::Tuple(values) => RuntimeValue::Tuple(
                values
                    .into_iter()
                    .map(|value| value.into_live_with_owner(program_owner))
                    .collect::<Result<_, _>>()?,
            ),
            Self::Seq(value) => RuntimeValue::Seq(Self::sequence_into_live(value, program_owner)?),
            Self::Record(fields) => {
                let fields = fields
                    .into_iter()
                    .map(|field| {
                        Ok(super::RuntimeFieldValue::new_accepted(
                            field.field,
                            field.name,
                            field.value.into_live_with_owner(program_owner)?,
                        ))
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?;
                RuntimeValue::Record(
                    super::RuntimeRecordValue::try_from_fields(fields)
                        .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
                )
            }
            Self::NominalRecord(value) => {
                RuntimeValue::NominalRecord(Self::nominal_into_live(value, program_owner)?)
            }
            Self::Opaque(value) => Self::opaque_into_live(value, program_owner)?,
            Self::Reduction(value) => {
                RuntimeValue::Reduction(Self::reduction_into_live(value, program_owner)?)
            }
            Self::Agent(value) => RuntimeValue::Agent(Self::agent_into_live(value, program_owner)?),
            Self::Callable(value) => {
                RuntimeValue::Callable(Self::callable_into_live(value, program_owner)?)
            }
            value @ Self::Variant { .. } => value.variant_into_live(program_owner)?,
        })
    }

    fn variant_into_live(
        self,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let Self::Variant {
            owner,
            ordinal,
            name,
            payload,
            type_instantiation,
        } = self
        else {
            unreachable!("variant restore is selected by the closed value dispatch");
        };
        if let super::RuntimeVariantIdentity::Nominal {
            semantic_identity, ..
        } = &owner
        {
            let authority = program_owner
                .ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "nominal values require a program-bound snapshot",
                    )
                })?
                .types();
            authority
                .require_variant_snapshot_header(
                    *semantic_identity,
                    &owner,
                    ordinal,
                    &name,
                    payload.is_some(),
                    type_instantiation.as_ref(),
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
            return authority
                .try_snapshot_variant_value(
                    *semantic_identity,
                    ordinal,
                    payload
                        .map(|value| value.into_live_with_owner(program_owner))
                        .transpose()?,
                    type_instantiation.map(std::sync::Arc::new),
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()));
        }
        let super::RuntimeVariantIdentity::Builtin(owner) = owner else {
            unreachable!()
        };
        let case = owner.case_at(ordinal).ok_or_else(|| {
            AwbcRuntimeValueSnapshotError::new("unknown builtin variant snapshot case")
        })?;
        if name != case.name()
            || type_instantiation.is_some()
            || match (case.payload_arity(), payload.as_deref()) {
                (None, None) => false,
                (Some(arity), Some(Self::Tuple(fields))) if fields.len() == arity => false,
                _ => true,
            }
        {
            return Err(AwbcRuntimeValueSnapshotError::new(
                "invalid builtin variant snapshot",
            ));
        }
        let payload = match payload {
            Some(payload) => {
                let Self::Tuple(mut fields) = *payload else {
                    unreachable!()
                };
                Some(fields.remove(0).into_live_with_owner(program_owner)?)
            }
            None => None,
        };
        RuntimeValue::try_builtin_variant(case.identity(), payload)
            .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    pub(super) fn iterator_from_live(
        value: &RuntimeIterator,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeIteratorSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeIterator::Values { items } => AwbcRuntimeIteratorSnapshot::Values {
                items: items
                    .iter()
                    .map(|value| Self::from_admitted_storage(value, owner))
                    .collect::<Result<_, _>>()?,
            },
            RuntimeIterator::Range(value) => {
                value.validated_end().ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "range iterator has an invalid retained cursor",
                    )
                })?;
                AwbcRuntimeIteratorSnapshot::Range(value.clone())
            }
            RuntimeIterator::Witness(witness) => {
                let owner = owner.ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "iterator witnesses require their original executable program",
                    )
                })?;
                witness
                    .validate_for_owner(owner)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                AwbcRuntimeIteratorSnapshot::Witness {
                    state: Box::new(Self::from_admitted_storage(witness.state(), Some(owner))?),
                    next: witness.next_method(),
                }
            }
        })
    }

    fn iterator_into_live(
        value: AwbcRuntimeIteratorSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeIterator, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeIteratorSnapshot::Values { items } => RuntimeIterator::Values {
                items: items
                    .into_iter()
                    .map(|value| value.into_live_with_owner(program_owner))
                    .collect::<Result<VecDeque<_>, _>>()?,
            },
            AwbcRuntimeIteratorSnapshot::Range(value) => {
                value.validated_end().ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "range iterator has an invalid retained cursor",
                    )
                })?;
                RuntimeIterator::Range(value)
            }
            AwbcRuntimeIteratorSnapshot::Witness { state, next } => {
                let owner = program_owner.ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "iterator witnesses require their original executable program",
                    )
                })?;
                owner.types().iterator_witness_types(next).ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "iterator witness has no matching original next signature",
                    )
                })?;
                let state = state.into_live_with_owner(Some(owner))?;
                RuntimeIterator::try_witness(owner.clone(), state, next)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?
            }
        })
    }

    fn sequence_from_live(
        value: &RuntimeSeq,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeSeqSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeSeq::Values(values) => AwbcRuntimeSeqSnapshot::Values(
                values
                    .iter()
                    .map(|value| Self::from_admitted_storage(value, owner))
                    .collect::<Result<_, _>>()?,
            ),
            RuntimeSeq::Dense(value) => AwbcRuntimeSeqSnapshot::Dense(value.clone()),
            RuntimeSeq::TupleColumns(value) => AwbcRuntimeSeqSnapshot::TupleColumns {
                len: u64::try_from(value.len()).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "tuple sequence length does not fit the AWBC save field",
                    )
                })?,
                columns: value
                    .columns()
                    .iter()
                    .map(|value| Self::sequence_from_live(value, owner))
                    .collect::<Result<_, _>>()?,
            },
            RuntimeSeq::RecordColumns(value) => AwbcRuntimeSeqSnapshot::RecordColumns {
                len: u64::try_from(value.len()).map_err(|_| {
                    AwbcRuntimeValueSnapshotError::new(
                        "record sequence length does not fit the AWBC save field",
                    )
                })?,
                fields: value
                    .fields()
                    .iter()
                    .map(|field| {
                        Ok(AwbcRuntimeRecordSeqFieldSnapshot {
                            field: field.field(),
                            name: field.name().to_owned(),
                            values: Self::sequence_from_live(field.values(), owner)?,
                        })
                    })
                    .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
            },
        })
    }

    fn sequence_into_live(
        value: AwbcRuntimeSeqSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeSeq, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeSeqSnapshot::Values(values) => RuntimeSeq::Values(
                values
                    .into_iter()
                    .map(|value| value.into_live_with_owner(program_owner))
                    .collect::<Result<_, _>>()?,
            ),
            AwbcRuntimeSeqSnapshot::Dense(value) => RuntimeSeq::Dense(value),
            AwbcRuntimeSeqSnapshot::TupleColumns { len, columns } => RuntimeSeq::TupleColumns(
                TupleSeq::new(
                    usize::try_from(len).map_err(|_| {
                        AwbcRuntimeValueSnapshotError::new(
                            "AWBC tuple sequence length does not fit this platform",
                        )
                    })?,
                    columns
                        .into_iter()
                        .map(|value| Self::sequence_into_live(value, program_owner))
                        .collect::<Result<_, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            ),
            AwbcRuntimeSeqSnapshot::RecordColumns { len, fields } => RuntimeSeq::RecordColumns(
                RecordSeq::try_from_fields(
                    usize::try_from(len).map_err(|_| {
                        AwbcRuntimeValueSnapshotError::new(
                            "AWBC record sequence length does not fit this platform",
                        )
                    })?,
                    fields
                        .into_iter()
                        .map(|field| {
                            Ok(super::RecordSeqField::new_accepted(
                                field.field,
                                field.name,
                                Self::sequence_into_live(field.values, program_owner)?,
                            ))
                        })
                        .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            ),
        })
    }

    fn nominal_from_live(
        value: &RuntimeNominalRecordValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeNominalRecordSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeNominalRecordSnapshot {
            type_id: value.type_id().clone(),
            semantic_identity: value.semantic_identity(),
            layout: value.layout(),
            type_instantiation: value.type_instantiation().cloned(),
            fields: value
                .fields()
                .iter()
                .enumerate()
                .map(|(ordinal, field_value)| {
                    Ok::<_, AwbcRuntimeValueSnapshotError>(AwbcRuntimeNominalFieldSnapshot {
                        field: value.field_id(ordinal).ok_or_else(|| {
                            AwbcRuntimeValueSnapshotError::new("nominal field identity overflow")
                        })?,
                        value: Self::from_admitted_storage(field_value, owner)?,
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }

    fn nominal_into_live(
        value: AwbcRuntimeNominalRecordSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeNominalRecordValue, AwbcRuntimeValueSnapshotError> {
        let authority = program_owner
            .ok_or_else(|| {
                AwbcRuntimeValueSnapshotError::new(
                    "nominal values require a program-bound snapshot",
                )
            })?
            .types();
        authority
            .require_nominal_record_snapshot_header(
                value.semantic_identity,
                &value.type_id,
                value.layout,
                value.fields.len(),
                value.type_instantiation.as_ref(),
            )
            .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
        for (ordinal, field) in value.fields.iter().enumerate() {
            let expected = super::RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
            if field.field != expected {
                return Err(AwbcRuntimeValueSnapshotError::new(format!(
                    "nominal snapshot field {ordinal} has identity {:?}, expected {expected:?}",
                    field.field
                )));
            }
        }
        authority
            .try_snapshot_record_value(
                value.semantic_identity,
                value
                    .fields
                    .into_iter()
                    .map(|field| field.value.into_live_with_owner(program_owner))
                    .collect::<Result<_, _>>()?,
                value.type_instantiation.map(std::sync::Arc::new),
                crate::entry::RuntimeSchemaLimits::engine_default(),
            )
            .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    fn opaque_from_live(
        value: &RuntimeOpaqueValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeOpaqueSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeOpaqueSnapshot {
            producer: value.producer().clone(),
            semantic_identity: value.semantic_identity(),
            value_class: value.value_class(),
            persistence: value.persistence(),
            payload: Box::new(Self::from_admitted_storage(value.payload(), owner)?),
        })
    }

    fn opaque_into_live(
        value: AwbcRuntimeOpaqueSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            value.producer,
            value.semantic_identity,
            value.value_class,
            value.persistence,
        );
        owner
            .try_wrap((*value.payload).into_live_with_owner(program_owner)?)
            .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    fn reduction_from_live(
        value: &RuntimeReductionValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeReductionSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(AwbcRuntimeReductionSnapshot {
            owner: value.owner().clone(),
            state: Box::new(Self::from_admitted_storage(value.state(), owner)?),
            commands: value
                .commands()
                .iter()
                .map(|command| {
                    Ok(AwbcRuntimeCommandSnapshot {
                        constructor: command.constructor().clone(),
                        target: command.target().clone(),
                        payload: Box::new(Self::from_admitted_storage(
                            command.payload().value(),
                            owner,
                        )?),
                    })
                })
                .collect::<Result<_, AwbcRuntimeValueSnapshotError>>()?,
        })
    }

    fn reduction_into_live(
        value: AwbcRuntimeReductionSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeReductionValue, AwbcRuntimeValueSnapshotError> {
        let commands = value
            .commands
            .into_iter()
            .map(|command| {
                Ok(RuntimeCommand::new_accepted(
                    command.constructor,
                    command.target,
                    RuntimePayload::new((*command.payload).into_live_with_owner(program_owner)?),
                ))
            })
            .collect::<Result<Vec<_>, AwbcRuntimeValueSnapshotError>>()?;
        RuntimeReductionValue::try_from_admitted_parts(
            value.owner,
            (*value.state).into_live_with_owner(program_owner)?,
            commands,
        )
        .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }

    fn agent_from_live(
        value: &RuntimeAgentValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeAgentSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeAgentValue::ActionTarget(value) => {
                AwbcRuntimeAgentSnapshot::ActionTarget(value.clone())
            }
            RuntimeAgentValue::CaptureTarget(value) => {
                AwbcRuntimeAgentSnapshot::CaptureTarget(value.clone())
            }
            RuntimeAgentValue::DebugStatePath(value) => {
                AwbcRuntimeAgentSnapshot::DebugStatePath(value.clone())
            }
            RuntimeAgentValue::ObservationFieldPath(value) => {
                AwbcRuntimeAgentSnapshot::ObservationFieldPath(value.clone())
            }
            RuntimeAgentValue::Probe(value) => AwbcRuntimeAgentSnapshot::Probe(value.clone()),
            RuntimeAgentValue::Diagnostics => AwbcRuntimeAgentSnapshot::Diagnostics,
            RuntimeAgentValue::Predicate(value) => {
                AwbcRuntimeAgentSnapshot::Predicate(Self::predicate_from_live(value, owner)?)
            }
            RuntimeAgentValue::ViewportPoint { x, y } => {
                AwbcRuntimeAgentSnapshot::ViewportPoint { x: *x, y: *y }
            }
            RuntimeAgentValue::BinaryData(data) => {
                AwbcRuntimeAgentSnapshot::BinaryData(data.clone())
            }
            RuntimeAgentValue::DataShape(shape) => {
                let owner = owner.ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "DataShape witnesses require a program-bound snapshot",
                    )
                })?;
                shape
                    .validate_for(owner, shape.shape_type(), shape.value_type())
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                AwbcRuntimeAgentSnapshot::DataShape {
                    shape_type: shape.shape_type(),
                    value_type: shape.value_type(),
                }
            }
        })
    }

    fn agent_into_live(
        value: AwbcRuntimeAgentSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeAgentValue, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeAgentSnapshot::ActionTarget(value) => RuntimeAgentValue::ActionTarget(value),
            AwbcRuntimeAgentSnapshot::CaptureTarget(value) => {
                RuntimeAgentValue::CaptureTarget(value)
            }
            AwbcRuntimeAgentSnapshot::DebugStatePath(value) => {
                RuntimeAgentValue::DebugStatePath(value)
            }
            AwbcRuntimeAgentSnapshot::ObservationFieldPath(value) => {
                RuntimeAgentValue::ObservationFieldPath(value)
            }
            AwbcRuntimeAgentSnapshot::Probe(value) => RuntimeAgentValue::Probe(value),
            AwbcRuntimeAgentSnapshot::Diagnostics => RuntimeAgentValue::Diagnostics,
            AwbcRuntimeAgentSnapshot::Predicate(value) => {
                RuntimeAgentValue::Predicate(Self::predicate_into_live(value, program_owner)?)
            }
            AwbcRuntimeAgentSnapshot::ViewportPoint { x, y } => {
                RuntimeAgentValue::ViewportPoint { x, y }
            }
            AwbcRuntimeAgentSnapshot::BinaryData(data) => RuntimeAgentValue::BinaryData(data),
            AwbcRuntimeAgentSnapshot::DataShape {
                shape_type,
                value_type,
            } => {
                let program_owner = program_owner.ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "DataShape values require a program-bound snapshot",
                    )
                })?;
                let shape = super::RuntimeDataShape::bind(program_owner.clone(), shape_type)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                shape
                    .validate_for(program_owner, shape_type, value_type)
                    .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?;
                RuntimeAgentValue::DataShape(shape)
            }
        })
    }

    fn predicate_from_live(
        value: &RuntimeAgentPredicate,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeAgentPredicateSnapshot, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            RuntimeAgentPredicate::Compare { probe, op, value } => {
                AwbcRuntimeAgentPredicateSnapshot::Compare {
                    probe: probe.clone(),
                    op: *op,
                    value: Box::new(Self::from_admitted_storage(value, owner)?),
                }
            }
            RuntimeAgentPredicate::Exists { probe } => AwbcRuntimeAgentPredicateSnapshot::Exists {
                probe: probe.clone(),
            },
            RuntimeAgentPredicate::ActionEnabled { target } => {
                AwbcRuntimeAgentPredicateSnapshot::ActionEnabled {
                    target: target.clone(),
                }
            }
            RuntimeAgentPredicate::DiagnosticsHasError => {
                AwbcRuntimeAgentPredicateSnapshot::DiagnosticsHasError
            }
            RuntimeAgentPredicate::All { predicates } => AwbcRuntimeAgentPredicateSnapshot::All {
                predicates: AgentPredicateOperands::try_from(
                    predicates
                        .iter()
                        .map(|value| Self::predicate_from_live(value, owner))
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            },
            RuntimeAgentPredicate::Any { predicates } => AwbcRuntimeAgentPredicateSnapshot::Any {
                predicates: AgentPredicateOperands::try_from(
                    predicates
                        .iter()
                        .map(|value| Self::predicate_from_live(value, owner))
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))?,
            },
            RuntimeAgentPredicate::Not { predicate } => AwbcRuntimeAgentPredicateSnapshot::Not {
                predicate: Box::new(Self::predicate_from_live(predicate, owner)?),
            },
        })
    }

    fn predicate_into_live(
        value: AwbcRuntimeAgentPredicateSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeAgentPredicate, AwbcRuntimeValueSnapshotError> {
        Ok(match value {
            AwbcRuntimeAgentPredicateSnapshot::Compare { probe, op, value } => {
                RuntimeAgentPredicate::Compare {
                    probe,
                    op,
                    value: Box::new((*value).into_live_with_owner(program_owner)?),
                }
            }
            AwbcRuntimeAgentPredicateSnapshot::Exists { probe } => {
                RuntimeAgentPredicate::Exists { probe }
            }
            AwbcRuntimeAgentPredicateSnapshot::ActionEnabled { target } => {
                RuntimeAgentPredicate::ActionEnabled { target }
            }
            AwbcRuntimeAgentPredicateSnapshot::DiagnosticsHasError => {
                RuntimeAgentPredicate::DiagnosticsHasError
            }
            AwbcRuntimeAgentPredicateSnapshot::All { predicates } => {
                RuntimeAgentPredicate::try_all(
                    predicates
                        .into_iter()
                        .map(|value| Self::predicate_into_live(value, program_owner))
                        .collect::<Result<_, _>>()?,
                )?
            }
            AwbcRuntimeAgentPredicateSnapshot::Any { predicates } => {
                RuntimeAgentPredicate::try_any(
                    predicates
                        .into_iter()
                        .map(|value| Self::predicate_into_live(value, program_owner))
                        .collect::<Result<_, _>>()?,
                )?
            }
            AwbcRuntimeAgentPredicateSnapshot::Not { predicate } => RuntimeAgentPredicate::Not {
                predicate: Box::new(Self::predicate_into_live(*predicate, program_owner)?),
            },
        })
    }

    fn callable_from_live(
        value: &RuntimeCallableValue,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<AwbcRuntimeCallableSnapshot, AwbcRuntimeValueSnapshotError> {
        match owner {
            None => {
                return Err(AwbcRuntimeValueSnapshotError::new(
                    "callable values require a program-bound snapshot",
                ));
            }
            Some(owner) if !value.owner().same_program(owner) => {
                return Err(AwbcRuntimeValueSnapshotError::new(
                    "callable rollback image belongs to another program",
                ));
            }
            Some(_) => {}
        }
        Ok(AwbcRuntimeCallableSnapshot {
            state: value.state(),
            retained: value
                .retained()
                .iter()
                .map(|value| Self::from_admitted_storage(value, owner))
                .collect::<Result<_, _>>()?,
        })
    }

    fn callable_into_live(
        value: AwbcRuntimeCallableSnapshot,
        program_owner: Option<&RuntimeProgramOwner>,
    ) -> Result<RuntimeCallableValue, AwbcRuntimeValueSnapshotError> {
        let retained = value
            .retained
            .into_iter()
            .map(|value| value.into_live_with_owner(program_owner))
            .collect::<Result<Vec<_>, _>>()?;
        RuntimeCallableValue::try_new(
            program_owner
                .ok_or_else(|| {
                    AwbcRuntimeValueSnapshotError::new(
                        "callable values require a program-bound snapshot",
                    )
                })?
                .clone(),
            value.state,
            retained,
        )
        .map_err(|error| AwbcRuntimeValueSnapshotError::new(error.to_string()))
    }
}

impl AwbcRuntimeValueSnapshot {
    /// Checks the complete borrowed logical value, storage metadata, and encoded
    /// bytes using the same resource authorities as live snapshot admission.
    pub fn validate_graph_limits(
        &self,
        limits: crate::entry::RuntimeSchemaLimits,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        crate::entry::schema::value_encoding::preflight_snapshot_view(self.view(), limits, None)?;
        Ok(())
    }
}

// Keep the public serde boundary deliberately local to the DTO.  These impls
// make accidental use in non-JSON codecs behave exactly like a regular typed
// serializable value while never invoking `RuntimeValue` serde.
impl Serialize for AwbcRuntimeValueSnapshotError {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(self.to_string()))
    }
}

impl<'de> Deserialize<'de> for AwbcRuntimeValueSnapshotError {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(serde::de::Error::custom(
            "AWBC runtime-value snapshot errors are not wire values",
        ))
    }
}
