//! Plan-owned control/effect contracts for static task semantic sealing.
//!
//! Declaration order is semantic. This is not the sorted effect-authorization
//! set on an executable body: it also owns the endpoint ABI and lifecycle.

use std::{num::NonZeroU32, sync::Arc};

use thiserror::Error;

use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};

use super::{RuntimePlanTypeTable, RuntimeTaskPlanSealLimits};

pub(crate) mod semantic;
pub(super) mod source_codec;

const CONTROL_EFFECT_DOMAIN: &[u8] = b"arcweft.task.control-effect-contract.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTaskControlMode {
    StraightLine,
    MaySuspend,
    MustSuspend,
    RuntimeAggregate,
    TimeoutRace,
    LineTimeline,
}

impl RuntimeTaskControlMode {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::StraightLine,
            1 => Self::MaySuspend,
            2 => Self::MustSuspend,
            3 => Self::RuntimeAggregate,
            4 => Self::TimeoutRace,
            5 => Self::LineTimeline,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::StraightLine => 0,
            Self::MaySuspend => 1,
            Self::MustSuspend => 2,
            Self::RuntimeAggregate => 3,
            Self::TimeoutRace => 4,
            Self::LineTimeline => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeControlEffectKind {
    HostOperation,
    RuntimeOperation,
    ViewSubscription,
    AwaitObservation,
    TimeoutClock,
    LineEmission,
    CancellationObservation,
}

impl RuntimeControlEffectKind {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::HostOperation,
            1 => Self::RuntimeOperation,
            2 => Self::ViewSubscription,
            3 => Self::AwaitObservation,
            4 => Self::TimeoutClock,
            5 => Self::LineEmission,
            6 => Self::CancellationObservation,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::HostOperation => 0,
            Self::RuntimeOperation => 1,
            Self::ViewSubscription => 2,
            Self::AwaitObservation => 3,
            Self::TimeoutClock => 4,
            Self::LineEmission => 5,
            Self::CancellationObservation => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeControlEffectCardinality {
    ExactlyOnce,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

impl RuntimeControlEffectCardinality {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::ExactlyOnce,
            1 => Self::ZeroOrOne,
            2 => Self::ZeroOrMore,
            3 => Self::OneOrMore,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::ExactlyOnce => 0,
            Self::ZeroOrOne => 1,
            Self::ZeroOrMore => 2,
            Self::OneOrMore => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeControlEffectOrdering {
    SourceOrder,
    CompletionOrder,
    SingleTerminal,
}

impl RuntimeControlEffectOrdering {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::SourceOrder,
            1 => Self::CompletionOrder,
            2 => Self::SingleTerminal,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::SourceOrder => 0,
            Self::CompletionOrder => 1,
            Self::SingleTerminal => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeControlEffectCancellation {
    NotObserved,
    ObservedNoPayload,
    PropagatesToChildren,
}

impl RuntimeControlEffectCancellation {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::NotObserved,
            1 => Self::ObservedNoPayload,
            2 => Self::PropagatesToChildren,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::NotObserved => 0,
            Self::ObservedNoPayload => 1,
            Self::PropagatesToChildren => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeControlEffectTerminalBehavior {
    Value,
    ResultValue,
    OptionValue,
    NonreturningCancellation,
    InfrastructureFailureControl,
}

impl RuntimeControlEffectTerminalBehavior {
    pub(super) const fn from_semantic_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            0 => Self::Value,
            1 => Self::ResultValue,
            2 => Self::OptionValue,
            3 => Self::NonreturningCancellation,
            4 => Self::InfrastructureFailureControl,
            _ => return None,
        })
    }
    #[must_use]
    pub const fn semantic_tag(self) -> u8 {
        match self {
            Self::Value => 0,
            Self::ResultValue => 1,
            Self::OptionValue => 2,
            Self::NonreturningCancellation => 3,
            Self::InfrastructureFailureControl => 4,
        }
    }
}

/// An accepted endpoint/effect identity supplied by its semantic owner.
/// This is metadata input, never a completed control-contract digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeControlEffectIdentity([u8; 32]);

impl RuntimeControlEffectIdentity {
    #[must_use]
    pub const fn from_checked_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Shared grammar for transient semantic-type seeds and admitted type rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeControlEffectRow<Type> {
    pub kind: RuntimeControlEffectKind,
    pub identity: Option<RuntimeControlEffectIdentity>,
    pub inputs: Box<[Type]>,
    pub output: Option<Type>,
    pub cardinality: RuntimeControlEffectCardinality,
    pub ordering: RuntimeControlEffectOrdering,
    pub cancellation: RuntimeControlEffectCancellation,
    pub terminal: RuntimeControlEffectTerminalBehavior,
}

/// Source-ordered definition. The builder alone resolves its type/child roles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeControlEffectContractDefinition<Type, Child> {
    pub mode: RuntimeTaskControlMode,
    pub effects: Box<[RuntimeControlEffectRow<Type>]>,
    pub children: Box<[Child]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeControlEffectContractId(NonZeroU32);

impl RuntimeControlEffectContractId {
    pub(super) fn for_index(index: usize) -> Option<Self> {
        index
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .and_then(NonZeroU32::new)
            .map(Self)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// Issued only by the complete typed contract transcript below; no raw-byte
/// constructor or deserializer can issue this proof.
///
/// ```compile_fail
/// use arcweft_core::plan::ControlEffectContractDigest;
/// let _ = ControlEffectContractDigest::from_bytes([0; 32]);
/// ```
/// ```compile_fail
/// fn encode<T: serde::Serialize>() {}
/// encode::<arcweft_core::plan::ControlEffectContractDigest>();
/// ```
/// ```compile_fail
/// fn decode<T: serde::de::DeserializeOwned>() {}
/// decode::<arcweft_core::plan::ControlEffectContractDigest>();
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlEffectContractDigest([u8; 32]);

impl ControlEffectContractDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn from_hasher_output(output: blake3::Hash) -> Self {
        Self(*output.as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeControlEffectContract {
    definition:
        RuntimeControlEffectContractDefinition<RuntimePlanTypeId, RuntimeControlEffectContractId>,
}

impl RuntimeControlEffectContract {
    pub(super) const fn new(
        definition: RuntimeControlEffectContractDefinition<
            RuntimePlanTypeId,
            RuntimeControlEffectContractId,
        >,
    ) -> Self {
        Self { definition }
    }

    #[must_use]
    pub const fn mode(&self) -> RuntimeTaskControlMode {
        self.definition.mode
    }

    #[must_use]
    pub fn effects(&self) -> &[RuntimeControlEffectRow<RuntimePlanTypeId>] {
        &self.definition.effects
    }

    #[must_use]
    pub fn children(&self) -> &[RuntimeControlEffectContractId] {
        &self.definition.children
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeControlEffectContractTable {
    // Finished rows are immutable and shared by cloned plan leases. Candidate
    // reservations remain exclusively owned by the aggregate builder.
    rows: Arc<[RuntimeControlEffectContract]>,
    proofs: Option<Arc<[ControlEffectContractDigest]>>,
}

impl RuntimeControlEffectContractTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[must_use]
    pub fn get(&self, id: RuntimeControlEffectContractId) -> Option<&RuntimeControlEffectContract> {
        self.rows.get(id.index())
    }

    #[must_use]
    pub fn digest(
        &self,
        id: RuntimeControlEffectContractId,
    ) -> Option<ControlEffectContractDigest> {
        self.proofs.as_ref()?.get(id.index()).copied()
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&RuntimeControlEffectContract, ControlEffectContractDigest)>
    {
        self.rows
            .iter()
            .zip(self.proofs.as_deref().unwrap_or(&[]).iter().copied())
    }

    pub(super) fn from_definitions(rows: Vec<RuntimeControlEffectContract>) -> Self {
        Self {
            rows: rows.into(),
            proofs: None,
        }
    }

    pub(super) fn complete(
        self,
        proofs: Box<[ControlEffectContractDigest]>,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        if self.rows.len() != proofs.len() {
            return Err(RuntimeControlEffectContractError::OwnerRejected);
        }
        Ok(Self {
            rows: self.rows,
            proofs: Some(proofs.into()),
        })
    }

    /// Iterative child-first sealing preserves declaration rows and each
    /// parent's child order. Shared children are computed once; every edge is
    /// still charged and written. Reserved forward edges may not form cycles.
    #[cfg(test)]
    pub(super) fn seal(
        rows: Vec<RuntimeControlEffectContract>,
        types: &RuntimePlanTypeTable,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        let digests = {
            let mut pass =
                semantic::RuntimeControlEffectSemanticPass::from_rows(&rows, types, limits, meter)?;
            let mut digests = Vec::with_capacity(rows.len());
            for index in 0..rows.len() {
                let id = RuntimeControlEffectContractId::for_index(index)
                    .ok_or(RuntimeControlEffectContractError::IdentityExhausted)?;
                digests.push(pass.complete(id, meter)?);
            }
            digests
        };
        Self::from_definitions(rows).complete(digests.into_boxed_slice())
    }
}

impl RuntimeControlEffectContract {
    fn semantic_digest(
        &self,
        types: &RuntimePlanTypeTable,
        mut child_digest: impl FnMut(
            RuntimeControlEffectContractId,
        ) -> Option<ControlEffectContractDigest>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<ControlEffectContractDigest, RuntimeControlEffectContractError> {
        let mut encoder = TaskSemanticEncoder::new(CONTROL_EFFECT_DOMAIN, meter);
        encoder.tag(self.mode().semantic_tag());
        encoder.count(self.effects().len());
        for (ordinal, row) in self.effects().iter().enumerate() {
            encoder.enter_element();
            encoder.enter_role(); // control/effect row
            encoder.count(ordinal);
            encoder.tag(row.kind.semantic_tag());
            encoder.tag(u8::from(row.identity.is_some()));
            if let Some(identity) = row.identity {
                encoder.digest(identity.as_bytes());
            }
            encoder.count(row.inputs.len());
            for ty in &row.inputs {
                encoder.enter_element();
                encoder.enter_role(); // input type
                Self::write_type(&mut encoder, types, *ty)?;
            }
            encoder.tag(u8::from(row.output.is_some()));
            if let Some(ty) = row.output {
                Self::write_type(&mut encoder, types, ty)?;
            }
            encoder.tag(row.cardinality.semantic_tag());
            encoder.tag(row.ordering.semantic_tag());
            encoder.tag(row.cancellation.semantic_tag());
            encoder.tag(row.terminal.semantic_tag());
        }
        encoder.count(self.children().len());
        for (ordinal, child) in self.children().iter().enumerate() {
            encoder.enter_role(); // child contract reference; edge was entered above
            encoder.count(ordinal);
            encoder.status()?;
            let digest =
                child_digest(*child).ok_or(RuntimeControlEffectContractError::UnsealedChild)?;
            encoder.digest(digest.as_bytes());
        }
        encoder
            .finish()
            .map(ControlEffectContractDigest::from_hasher_output)
            .map_err(RuntimeControlEffectContractError::from)
    }

    fn write_type(
        encoder: &mut TaskSemanticEncoder,
        types: &RuntimePlanTypeTable,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimeControlEffectContractError> {
        encoder.status()?;
        let row = types
            .get(ty)
            .ok_or(RuntimeControlEffectContractError::UnknownType { ty })?;
        encoder.digest(row.semantic_identity().as_bytes());
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeControlEffectContractError {
    #[error("control/effect contract identity space is exhausted")]
    IdentityExhausted,
    #[error("control/effect contract handle belongs to another plan builder")]
    ForeignSeed,
    #[error("control/effect contract {index} is already defined")]
    AlreadyDefined { index: usize },
    #[error("control/effect contract {index} is not defined")]
    Incomplete { index: usize },
    #[error("control/effect contract references missing child {index}")]
    UnknownChild { index: usize },
    #[error("control/effect contract graph has a cycle at row {index}")]
    Cycle { index: usize },
    #[error("control/effect contract references unknown type {ty}")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("control/effect contract semantic work limit exceeded")]
    WorkLimit,
    #[error("control/effect transcript byte limit exceeded")]
    TranscriptByteLimit,
    #[error("control/effect transcript arithmetic overflow")]
    ArithmeticOverflow,
    #[error("control/effect contract {index} has {actual} children; maximum is {maximum}")]
    ChildrenLimit {
        index: usize,
        actual: u32,
        maximum: u32,
    },
    #[error("control/effect table has {actual} effect rows; maximum is {maximum}")]
    EffectRowsLimit { actual: u32, maximum: u32 },
    #[error("control/effect contract source-order count exceeds the version-one grammar")]
    CountOverflow,
    #[error("control/effect contract string length exceeds the version-one grammar")]
    StringLengthOverflow,
    #[error("control/effect contract child has not been sealed")]
    UnsealedChild,
    #[error("control/effect transcript owner rejected semantic input")]
    OwnerRejected,
}

impl From<TaskSemanticEncodingError> for RuntimeControlEffectContractError {
    fn from(error: TaskSemanticEncodingError) -> Self {
        match error {
            TaskSemanticEncodingError::OwnerRejected => Self::OwnerRejected,
            TaskSemanticEncodingError::CountOverflow => Self::CountOverflow,
            TaskSemanticEncodingError::StringLengthOverflow => Self::StringLengthOverflow,
            TaskSemanticEncodingError::ArithmeticOverflow => Self::ArithmeticOverflow,
            TaskSemanticEncodingError::SemanticWork => Self::WorkLimit,
            TaskSemanticEncodingError::TranscriptBytes => Self::TranscriptByteLimit,
        }
    }
}

#[cfg(test)]
mod tests;
