//! Type contracts for the deterministic Agent constructor ABI.

use super::RuntimeAgentConstructor;
use crate::entry::RuntimeCommandTargetId;
use crate::plan::{RuntimeAgentOperationalType, RuntimeAgentTypeProjection};

/// The type-table observations needed by the constructor contract. Type IDs
/// remain in their original table; in particular, a Probe result is not
/// reconstructed through the narrower checked-value image.
pub(crate) trait RuntimeAgentTypeContext {
    type Type: Copy + Eq;

    fn is_string(&self, ty: Self::Type) -> bool;
    fn is_entity_reference(&self, ty: Self::Type) -> bool;
    fn is_u32(&self, ty: Self::Type) -> bool;
    fn agent_type(&self, ty: Self::Type) -> Option<RuntimeAgentTypeProjection<Self::Type>>;
    fn sequence_type(&self, ty: Self::Type) -> Option<(Self::Type, Option<u64>)>;
    fn tuple_type(&self, ty: Self::Type) -> Option<&[Self::Type]>;
}

/// Native expressions retain an accepted choice identity directly. Execution
/// and AWBC materialize that same operand as a target value.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RuntimeAgentTypeOperand<'a, T> {
    Typed(T),
    Choice(&'a RuntimeCommandTargetId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAgentSignatureError {
    OperandCount { actual: usize },
    OperandType { operand: usize },
    ResultType,
}

#[derive(Clone, Copy, Debug)]
enum OperandType {
    String,
    Target,
    U32,
    Agent(RuntimeAgentOperationalType),
    ProbeResult { probe: usize },
}

#[derive(Clone, Copy, Debug)]
enum Operands {
    Fixed(&'static [OperandType]),
    NonEmptyFlatCollection(RuntimeAgentOperationalType),
}

#[derive(Clone, Copy, Debug)]
struct Signature {
    result: RuntimeAgentOperationalType,
    operands: Operands,
}

impl RuntimeAgentConstructor {
    const fn signature(self) -> Signature {
        use OperandType::{Agent, ProbeResult, String, Target, U32};
        use Operands::{Fixed, NonEmptyFlatCollection};
        use RuntimeAgentOperationalType as AgentType;

        let (result, operands) = match self {
            Self::ChoiceAction => (AgentType::ActionTarget, Fixed(&[Target])),
            Self::CaptureViewport => (AgentType::CaptureTarget, Fixed(&[])),
            Self::CaptureLayer | Self::CaptureObject => {
                (AgentType::CaptureTarget, Fixed(&[Target]))
            }
            Self::StatePath => (AgentType::DebugStatePath, Fixed(&[String])),
            Self::ObservationPath => (AgentType::ObservationFieldPath, Fixed(&[String])),
            Self::ProbeSignal | Self::ProbeMetric => (AgentType::Probe, Fixed(&[Target])),
            Self::ProbeState => (AgentType::Probe, Fixed(&[Agent(AgentType::DebugStatePath)])),
            Self::ProbeObservation => (
                AgentType::Probe,
                Fixed(&[Agent(AgentType::ObservationFieldPath)]),
            ),
            Self::Diagnostics => (AgentType::Diagnostics, Fixed(&[])),
            Self::PredicateExists => (AgentType::Predicate, Fixed(&[Agent(AgentType::Probe)])),
            Self::PredicateActionEnabled => (
                AgentType::Predicate,
                Fixed(&[Agent(AgentType::ActionTarget)]),
            ),
            Self::PredicateDiagnosticsHasError => (
                AgentType::Predicate,
                Fixed(&[Agent(AgentType::Diagnostics)]),
            ),
            Self::PredicateNot => (AgentType::Predicate, Fixed(&[Agent(AgentType::Predicate)])),
            Self::PredicateAll | Self::PredicateAny => (
                AgentType::Predicate,
                NonEmptyFlatCollection(AgentType::Predicate),
            ),
            Self::PredicateEq
            | Self::PredicateNotEq
            | Self::PredicateGreater
            | Self::PredicateGreaterOrEqual
            | Self::PredicateLess
            | Self::PredicateLessOrEqual => (
                AgentType::Predicate,
                Fixed(&[Agent(AgentType::Probe), ProbeResult { probe: 0 }]),
            ),
            Self::ViewportPoint => (AgentType::ViewportPoint, Fixed(&[U32, U32])),
        };
        Signature { result, operands }
    }

    #[must_use]
    pub const fn result_type(self) -> RuntimeAgentOperationalType {
        self.signature().result
    }

    /// Counts materialized ABI operands, including a retained choice identity.
    /// Flat collections also validate their expanded cardinality at execution.
    #[must_use]
    pub const fn accepts_operand_count(self, count: usize) -> bool {
        match self.signature().operands {
            Operands::Fixed(operands) => count == operands.len(),
            Operands::NonEmptyFlatCollection(_) => count >= 1,
        }
    }

    pub(crate) fn validate_types<C: RuntimeAgentTypeContext>(
        self,
        context: &C,
        result: C::Type,
        operands: &[RuntimeAgentTypeOperand<'_, C::Type>],
    ) -> Result<(), RuntimeAgentSignatureError> {
        if !self.accepts_operand_count(operands.len()) {
            return Err(RuntimeAgentSignatureError::OperandCount {
                actual: operands.len(),
            });
        }
        let signature = self.signature();
        match signature.operands {
            Operands::Fixed(expected) => {
                for (ordinal, expected) in expected.iter().enumerate() {
                    if !expected.accepts(context, operands, ordinal) {
                        return Err(RuntimeAgentSignatureError::OperandType { operand: ordinal });
                    }
                }
            }
            Operands::NonEmptyFlatCollection(expected) => {
                let mut can_be_nonempty = false;
                for (ordinal, operand) in operands.iter().enumerate() {
                    let RuntimeAgentTypeOperand::Typed(ty) = operand else {
                        return Err(RuntimeAgentSignatureError::OperandType { operand: ordinal });
                    };
                    let Some(nonempty) = Signature::collection_operand(context, expected, *ty)
                    else {
                        return Err(RuntimeAgentSignatureError::OperandType { operand: ordinal });
                    };
                    can_be_nonempty |= nonempty;
                }
                if !can_be_nonempty {
                    return Err(RuntimeAgentSignatureError::OperandCount { actual: 0 });
                }
            }
        }
        if context
            .agent_type(result)
            .is_none_or(|actual| actual.operational_type() != signature.result)
        {
            return Err(RuntimeAgentSignatureError::ResultType);
        }
        Ok(())
    }
}

impl OperandType {
    fn accepts<C: RuntimeAgentTypeContext>(
        self,
        context: &C,
        operands: &[RuntimeAgentTypeOperand<'_, C::Type>],
        ordinal: usize,
    ) -> bool {
        let ty = match operands[ordinal] {
            RuntimeAgentTypeOperand::Typed(ty) => ty,
            RuntimeAgentTypeOperand::Choice(_identity) => {
                // Carry the admitted identity itself, never a synthetic type ID.
                return matches!(self, Self::Target);
            }
        };
        match self {
            Self::String => context.is_string(ty),
            Self::Target => context.is_string(ty) || context.is_entity_reference(ty),
            Self::U32 => context.is_u32(ty),
            Self::Agent(expected) => context
                .agent_type(ty)
                .is_some_and(|actual| actual.operational_type() == expected),
            Self::ProbeResult { probe } => {
                let RuntimeAgentTypeOperand::Typed(probe) = operands[probe] else {
                    return false;
                };
                matches!(context.agent_type(probe), Some(RuntimeAgentTypeProjection::Probe(item)) if item == ty)
            }
        }
    }
}

impl Signature {
    /// A collection expands exactly one level. Known empty shapes contribute
    /// no elements; an unsized sequence checks its cardinality at execution.
    fn collection_operand<C: RuntimeAgentTypeContext>(
        context: &C,
        expected: RuntimeAgentOperationalType,
        ty: C::Type,
    ) -> Option<bool> {
        let accepts = |ty| {
            context
                .agent_type(ty)
                .is_some_and(|actual| actual.operational_type() == expected)
        };
        if accepts(ty) {
            return Some(true);
        }
        if let Some((item, length)) = context.sequence_type(ty) {
            return accepts(item).then_some(length != Some(0));
        }
        context
            .tuple_type(ty)
            .filter(|items| items.iter().copied().all(accepts))
            .map(|items| !items.is_empty())
    }
}
