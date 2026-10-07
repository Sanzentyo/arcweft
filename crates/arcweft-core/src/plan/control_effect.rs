//! Plan-owned control/effect contracts for static task semantic sealing.
//!
//! Declaration order is semantic. This is not the sorted effect-authorization
//! set on an executable body: it also owns the endpoint ABI and lifecycle.

use std::{num::NonZeroU32, sync::Arc};

use thiserror::Error;

use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError, TaskSemanticMeter};

use super::{RuntimePlanTypeTable, RuntimeTaskPlanSealLimits};

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
struct SealedContract {
    contract: RuntimeControlEffectContract,
    digest: ControlEffectContractDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeControlEffectContractTable {
    // Finished rows are immutable and shared by cloned plan leases. Candidate
    // reservations remain exclusively owned by the aggregate builder.
    rows: Arc<[SealedContract]>,
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
        self.rows.get(id.index()).map(|row| &row.contract)
    }

    #[must_use]
    pub fn digest(
        &self,
        id: RuntimeControlEffectContractId,
    ) -> Option<ControlEffectContractDigest> {
        self.rows.get(id.index()).map(|row| row.digest)
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&RuntimeControlEffectContract, ControlEffectContractDigest)>
    {
        self.rows.iter().map(|row| (&row.contract, row.digest))
    }

    /// Iterative child-first sealing preserves declaration rows and each
    /// parent's child order. Shared children are computed once; every edge is
    /// still charged and written. Reserved forward edges may not form cycles.
    pub(super) fn seal(
        rows: Vec<RuntimeControlEffectContract>,
        types: &RuntimePlanTypeTable,
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        Self::preflight(&rows, limits, meter)?;
        let mut state = vec![VisitState::Unvisited; rows.len()];
        let mut digests = vec![None; rows.len()];
        let mut stack = Vec::new();
        for root in 0..rows.len() {
            stack.push(Visit::Enter(root));
            while let Some(visit) = stack.pop() {
                match visit {
                    Visit::Enter(index) => match state[index] {
                        VisitState::Done => {}
                        VisitState::Visiting => {
                            return Err(RuntimeControlEffectContractError::Cycle { index });
                        }
                        VisitState::Unvisited => {
                            meter.charge_work(1)?; // contract row visit, once
                            state[index] = VisitState::Visiting;
                            stack.push(Visit::Finish(index));
                            for child in rows[index].children().iter().rev() {
                                meter.charge_work(1)?; // source-order child edge
                                if child.index() >= rows.len() {
                                    return Err(RuntimeControlEffectContractError::UnknownChild {
                                        index: child.index(),
                                    });
                                }
                                stack.push(Visit::Enter(child.index()));
                            }
                        }
                    },
                    Visit::Finish(index) => {
                        digests[index] = Some(rows[index].semantic_digest(types, &digests, meter)?);
                        state[index] = VisitState::Done;
                    }
                }
            }
        }
        let rows = rows
            .into_iter()
            .zip(digests)
            .map(|(contract, digest)| {
                let digest = digest.ok_or(RuntimeControlEffectContractError::UnsealedChild)?;
                Ok(SealedContract { contract, digest })
            })
            .collect::<Result<Box<[_]>, RuntimeControlEffectContractError>>()?;
        Ok(Self { rows: rows.into() })
    }

    /// C-owned fields follow the common preflight order: child counts, total
    /// control/effect rows, then the known byte bound. Arithmetic precedes each
    /// corresponding limit. Whole-image fields stay on the common seal owner.
    fn preflight(
        rows: &[RuntimeControlEffectContract],
        limits: RuntimeTaskPlanSealLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), RuntimeControlEffectContractError> {
        meter.status()?;
        for (index, row) in rows.iter().enumerate() {
            let actual = u32::try_from(row.children().len())
                .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
            if actual > limits.max_children_per_row {
                return Err(RuntimeControlEffectContractError::ChildrenLimit {
                    index,
                    actual,
                    maximum: limits.max_children_per_row,
                });
            }
        }
        let mut effect_rows = 0_u32;
        for row in rows {
            let count = u32::try_from(row.effects().len())
                .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
            effect_rows = effect_rows
                .checked_add(count)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
        }
        if effect_rows > limits.max_control_effect_rows {
            return Err(RuntimeControlEffectContractError::EffectRowsLimit {
                actual: effect_rows,
                maximum: limits.max_control_effect_rows,
            });
        }
        let mut bytes = 0_u64;
        for row in rows {
            // Domain + mode + effect count + child count; all scalar fields
            // use the same framing as the actual inherent row visitor.
            let fixed = u64::try_from(CONTROL_EFFECT_DOMAIN.len())
                .ok()
                .and_then(|count| count.checked_add(9))
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            bytes = bytes
                .checked_add(fixed)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            for effect in row.effects() {
                let inputs = u32::try_from(effect.inputs.len())
                    .map_err(|_| RuntimeControlEffectContractError::ArithmeticOverflow)?;
                let payloads = u64::from(inputs)
                    .checked_add(u64::from(effect.identity.is_some()))
                    .and_then(|count| count.checked_add(u64::from(effect.output.is_some())))
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
                let row_bytes = payloads
                    .checked_mul(32)
                    .and_then(|count| count.checked_add(15))
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
                bytes = bytes
                    .checked_add(row_bytes)
                    .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            }
            let children = u64::try_from(row.children().len())
                .ok()
                .and_then(|count| count.checked_mul(36))
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
            bytes = bytes
                .checked_add(children)
                .ok_or(RuntimeControlEffectContractError::ArithmeticOverflow)?;
        }
        meter.preflight_bytes(bytes)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum VisitState {
    Unvisited,
    Visiting,
    Done,
}
enum Visit {
    Enter(usize),
    Finish(usize),
}
impl RuntimeControlEffectContract {
    fn semantic_digest(
        &self,
        types: &RuntimePlanTypeTable,
        children: &[Option<ControlEffectContractDigest>],
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
            let digest = children
                .get(child.index())
                .and_then(|value| *value)
                .ok_or(RuntimeControlEffectContractError::UnsealedChild)?;
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
