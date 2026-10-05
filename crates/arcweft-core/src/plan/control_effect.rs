//! Plan-owned control/effect contracts for static task semantic sealing.
//!
//! Declaration order is semantic. This is not the sorted effect-authorization
//! set on an executable body: it also owns the endpoint ABI and lifecycle.

use std::{num::NonZeroU32, sync::Arc};

use thiserror::Error;

use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::{TaskSemanticEncoder, TaskSemanticEncodingError};

use super::RuntimePlanTypeTable;

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
        max_semantic_work: u64,
    ) -> Result<Self, RuntimeControlEffectContractError> {
        if u64::try_from(rows.len())
            .ok()
            .is_none_or(|count| count > max_semantic_work)
        {
            return Err(RuntimeControlEffectContractError::WorkLimit);
        }
        let mut state = vec![VisitState::Unvisited; rows.len()];
        let mut digests = vec![None; rows.len()];
        let mut work = SemanticWork {
            remaining: max_semantic_work,
        };
        let mut stack = Vec::new();
        for root in 0..rows.len() {
            stack.push(Visit::Enter(root));
            while let Some(visit) = stack.pop() {
                work.charge()?;
                match visit {
                    Visit::Enter(index) => match state[index] {
                        VisitState::Done => {}
                        VisitState::Visiting => {
                            return Err(RuntimeControlEffectContractError::Cycle { index });
                        }
                        VisitState::Unvisited => {
                            state[index] = VisitState::Visiting;
                            stack.push(Visit::Finish(index));
                            for child in rows[index].children().iter().rev() {
                                work.charge()?;
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
                        digests[index] =
                            Some(rows[index].semantic_digest(types, &digests, &mut work)?);
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
struct SemanticWork {
    remaining: u64,
}

impl SemanticWork {
    fn charge(&mut self) -> Result<(), RuntimeControlEffectContractError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(RuntimeControlEffectContractError::WorkLimit)?;
        Ok(())
    }
}

impl RuntimeControlEffectContract {
    fn semantic_digest(
        &self,
        types: &RuntimePlanTypeTable,
        children: &[Option<ControlEffectContractDigest>],
        work: &mut SemanticWork,
    ) -> Result<ControlEffectContractDigest, RuntimeControlEffectContractError> {
        let mut encoder = TaskSemanticEncoder::new(b"arcweft.task.control-effect-contract.v1\0");
        work.charge()?;
        encoder.tag(self.mode().semantic_tag());
        encoder.count(self.effects().len());
        for (ordinal, row) in self.effects().iter().enumerate() {
            work.charge()?;
            encoder.count(ordinal);
            encoder.tag(row.kind.semantic_tag());
            encoder.tag(u8::from(row.identity.is_some()));
            if let Some(identity) = row.identity {
                work.charge()?;
                encoder.digest(identity.as_bytes());
            }
            encoder.count(row.inputs.len());
            for ty in &row.inputs {
                Self::write_type(&mut encoder, types, *ty, work)?;
            }
            encoder.tag(u8::from(row.output.is_some()));
            if let Some(ty) = row.output {
                Self::write_type(&mut encoder, types, ty, work)?;
            }
            encoder.tag(row.cardinality.semantic_tag());
            encoder.tag(row.ordering.semantic_tag());
            encoder.tag(row.cancellation.semantic_tag());
            encoder.tag(row.terminal.semantic_tag());
        }
        encoder.count(self.children().len());
        for (ordinal, child) in self.children().iter().enumerate() {
            work.charge()?;
            encoder.count(ordinal);
            let digest = children
                .get(child.index())
                .and_then(|value| *value)
                .ok_or(RuntimeControlEffectContractError::UnsealedChild)?;
            encoder.digest(digest.as_bytes());
        }
        encoder
            .finish()
            .map(ControlEffectContractDigest::from_hasher_output)
            .map_err(|error| match error {
                TaskSemanticEncodingError::CountOverflow => {
                    RuntimeControlEffectContractError::CountOverflow
                }
                TaskSemanticEncodingError::StringLengthOverflow => {
                    RuntimeControlEffectContractError::StringLengthOverflow
                }
            })
    }

    fn write_type(
        encoder: &mut TaskSemanticEncoder,
        types: &RuntimePlanTypeTable,
        ty: RuntimePlanTypeId,
        work: &mut SemanticWork,
    ) -> Result<(), RuntimeControlEffectContractError> {
        work.charge()?;
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
    #[error("control/effect contract source-order count exceeds the version-one grammar")]
    CountOverflow,
    #[error("control/effect contract string length exceeds the version-one grammar")]
    StringLengthOverflow,
    #[error("control/effect contract child has not been sealed")]
    UnsealedChild,
}

#[cfg(test)]
mod tests;
