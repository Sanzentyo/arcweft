//! Bounded, deterministic type constraints for generic call application.
//!
//! The facade keeps the typed failure and reexport surface cohesive. Candidate
//! accounting/context and affine transaction state live in dedicated children;
//! compatibility-owned binding planning enters through `binding_plan`.

use thiserror::Error;

use crate::{
    effect_row::{EffectConstraintEnvironmentError, EffectVar},
    effects::EffectSet,
};

use super::GenericTypeParameterId;
pub(crate) mod context;
mod hints;
mod normalization;
mod shape;
mod solution;
#[cfg(test)]
mod tests;
pub(crate) mod transaction;

pub(crate) use super::compatibility::binding_plan::ConstraintAcceptance;
pub(super) use super::compatibility::binding_plan::relate_selected_call;
#[cfg(test)]
pub(crate) use context::LocalConstraintAccounting;
pub(crate) use context::{
    TypeConstraintConstEligibility, TypeConstraintEffectScope, TypeConstraintParameterEligibility,
    TypeConstraintParameterScope, TypeConstraintProjectionClosure,
};
#[cfg(test)]
pub(crate) use hints::NoConstraintClient;
pub use hints::{CheckedConstraintContainerConstructor, CheckedConstraintSourceProjection};
pub(crate) use hints::{
    ConstraintDomain, ConstraintSourceContainerPolicy, ExpectedHint, MaterializationOutcome,
    MaterializedSourceRequest, PreparedConstraintSourceProjection, PreparedSourceAlternative,
    PreparedSourceConstraint, ProjectedExpectedHint, SourceAlternativeHint, SourceError,
    SourcePhase, SourceProbeOutcome, SourceProbeResult, SourceProbeSelection,
};
pub(crate) use normalization::{
    ConstraintClosurePolicy, KeyedConstraintProjection, RejectedConstraintSourceProjection,
    SolvedCandidate, TypeConstraintCandidateFailure, TypeConstraintFailure,
    TypeConstraintFailureInvariant,
};
pub(super) use normalization::{
    bindings_equal, occurs_in_shape, seal_path, seal_type, validate_type,
};
pub(crate) use shape::TypeConstraintShape;
pub(crate) use solution::TypeConstraintSolution;
pub(crate) use transaction::ClosedConstraintProbe;
pub(super) use transaction::{ChoiceDerivationStep, ChoiceForkRole, ConstraintPath};

/// Ordinary semantic incompatibilities are candidate rejections. They never
/// describe malformed authority or operational exhaustion.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintRejection {
    #[error("type constraint relation has no compatible path")]
    Mismatch,
    #[error("type constraint relation retained {actual} distinct solutions")]
    AmbiguousSolution { actual: usize },
    #[error("generic parameter {parameter:?} occurs in its own binding")]
    CyclicInstantiation { parameter: GenericTypeParameterId },
    #[error("unresolved type placeholder reached call constraint sealing")]
    UnresolvedType,
    #[error("generic parameter {parameter:?} remains unbound at the terminal boundary")]
    IncompleteInstantiation { parameter: GenericTypeParameterId },
    #[error("effect-row subset is missing effects {missing:?}")]
    EffectSubset { missing: EffectSet },
}

/// Operational exhaustion and cancellation are closed aborts.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintAbort {
    #[error("type constraint relation was cancelled")]
    Cancelled,
    #[error("type constraint work arithmetic overflow")]
    ArithmeticOverflow,
    #[error("type constraint work {requested} exceeds remaining budget {consumed}/{limit}")]
    WorkLimit {
        requested: u64,
        consumed: u64,
        limit: u64,
    },
    #[error("type constraint nodes {actual} exceed limit {limit}")]
    NodeLimit { actual: u64, limit: u64 },
    #[error("type constraint branches {actual} exceed limit {limit}")]
    BranchLimit { actual: u64, limit: u64 },
    #[error("type constraint bindings {actual} exceed limit {limit}")]
    BindingLimit { actual: u64, limit: u64 },
    #[error("type constraint source probes {actual} exceed limit {limit}")]
    SourceProbeLimit { actual: u64, limit: u64 },
    #[error("type constraint materializations {actual} exceed limit {limit}")]
    MaterializationLimit { actual: u64, limit: u64 },
    #[error("nested call depth {actual} exceeds limit {limit}")]
    CallDepth { actual: u64, limit: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InheritedSolutionInvariant {
    pub(crate) kind: InheritedSolutionInvariantKind,
    pub(crate) parameter: Option<GenericTypeParameterId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InheritedSolutionInvariantKind {
    OutOfScope,
    RigidBinding,
    DuplicateOrUnordered,
    UnexpectedKey,
    SelfBinding,
    Forbidden,
    OccursOrCycle,
    Unclosed,
    NonCanonical,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintParameterScopeInvariant {
    #[error("type parameter is outside the candidate parameter scope")]
    TypeParameterOutOfScope { parameter: GenericTypeParameterId },
    #[error("constant parameter is outside the candidate parameter scope")]
    ConstParameterOutOfScope {
        parameter: super::GenericConstParameterId,
    },
    #[error("constant parameter is unsupported by the type-constraint solver")]
    UnsupportedConstParameter {
        parameter: super::GenericConstParameterId,
    },
    #[error("candidate parameter scope contains a duplicate row")]
    DuplicateParameter,
    #[error("candidate parameter scope rows are not in exact order")]
    ParameterUnordered,
    #[error("candidate constant scope rows are not in exact order")]
    ConstParameterUnordered,
    #[error("required inherited binding key is outside the type scope")]
    RequiredInheritedKeyOutOfScope { parameter: GenericTypeParameterId },
    #[error("required inherited binding key is not bindable")]
    RequiredInheritedKeyNotBindable { parameter: GenericTypeParameterId },
    #[error("inherited binding targets a rigid parameter")]
    RigidBinding { parameter: GenericTypeParameterId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeConstraintEffectInvariantKind {
    UnknownRow,
    ForeignVariable,
    DuplicateOrUnorderedScope,
    RequiredInheritedOutOfScope,
    RequiredInheritedNotBindable,
    DuplicateOrUnorderedInherited,
    UnexpectedInherited,
    MissingInherited,
    NonCanonicalInherited,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("effect constraint authority is invalid: {kind:?}")]
pub(crate) struct TypeConstraintEffectInvariant {
    pub(crate) kind: TypeConstraintEffectInvariantKind,
    pub(crate) variable: Option<EffectVar>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PreparedSourceConstraintInvariant {
    #[error("prepared source alternatives are not in exact schema order")]
    Unordered,
    #[error("prepared source alternatives contain a duplicate coordinate")]
    DuplicateCoordinate,
    #[error("spread source constraint has an invalid alternative plan")]
    SpreadPlan,
}

/// Protocol categories intentionally carry no source values: source identity
/// is owned by the domain and is validated before this closed category is
/// emitted.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintSourceProtocolInvariant {
    #[error("source callback returned a value for the wrong source")]
    WrongSource,
    #[error("source callback returned a value for the wrong phase")]
    WrongPhase,
    #[error("source callback selected an unknown alternative")]
    UnknownAlternative,
    #[error("source callback returned invalid checked evidence")]
    InvalidEvidence,
    #[error("source callback ticket is stale, foreign, or already consumed")]
    Ticket,
    #[error("source callback checkpoint is stale, foreign, or already closed")]
    Checkpoint,
    #[error("source callback returned an invalid outcome shape")]
    Outcome,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintProjectionInvariant {
    #[error("required final keyed projection is missing")]
    MissingKey,
    #[error("final keyed projection key is duplicated")]
    DuplicateKey,
    #[error("final keyed projection does not satisfy its selected closure")]
    Mismatch,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintInvariant {
    #[error("inherited solution is invalid: {0:?}")]
    InheritedSolution(InheritedSolutionInvariant),
    #[error("parameter scope is invalid: {0}")]
    ParameterScope(TypeConstraintParameterScopeInvariant),
    #[error("effect scope or binding is invalid: {0}")]
    Effect(TypeConstraintEffectInvariant),
    #[error("prepared source is invalid: {0}")]
    PreparedSource(PreparedSourceConstraintInvariant),
    #[error("source callback protocol is invalid: {0}")]
    SourceProtocol(TypeConstraintSourceProtocolInvariant),
    #[error("projection is invalid: {0}")]
    Projection(TypeConstraintProjectionInvariant),
}

pub(super) fn map_effect_environment_error(
    error: EffectConstraintEnvironmentError,
) -> TypeConstraintError {
    match error {
        EffectConstraintEnvironmentError::MissingEffects { missing } => {
            TypeConstraintError::Rejected(TypeConstraintRejection::EffectSubset { missing })
        }
        EffectConstraintEnvironmentError::UnknownRow => {
            effect_invariant(TypeConstraintEffectInvariantKind::UnknownRow, None)
        }
        EffectConstraintEnvironmentError::ForeignVariable { variable } => effect_invariant(
            TypeConstraintEffectInvariantKind::ForeignVariable,
            Some(variable),
        ),
        EffectConstraintEnvironmentError::NonCanonicalScope => effect_invariant(
            TypeConstraintEffectInvariantKind::DuplicateOrUnorderedScope,
            None,
        ),
    }
}

pub(super) fn effect_invariant(
    kind: TypeConstraintEffectInvariantKind,
    variable: Option<EffectVar>,
) -> TypeConstraintError {
    TypeConstraintError::Invariant(TypeConstraintInvariant::Effect(
        TypeConstraintEffectInvariant { kind, variable },
    ))
}

/// Failure of one bounded type relation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintError {
    #[error("candidate relation rejected: {0}")]
    Rejected(TypeConstraintRejection),
    #[error("candidate relation aborted: {0}")]
    Abort(TypeConstraintAbort),
    #[error("candidate relation invariant failed: {0}")]
    Invariant(TypeConstraintInvariant),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum TypeConstraintInitializationFailure {
    #[error("constraint initialization aborted: {0}")]
    Abort(TypeConstraintAbort),
    #[error("constraint initialization invariant failed: {0}")]
    Invariant(TypeConstraintInvariant),
}

/// Immediate failures while opening, closing, or submitting one affine
/// materialization correlation. Ordinary source rejection and fatal payloads
/// remain closed submissions so lower can preserve global precedence.
pub(crate) enum MaterializationImmediateFailure<D: ConstraintDomain> {
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintFailureInvariant<D>),
}

pub(crate) enum ClosedMaterializationSubmission<D: ConstraintDomain> {
    Sealed(D::SealedBranchValue),
    Rejected {
        source: D::Source,
        cause: D::SourceErrorCause,
    },
    Fatal(SourceError<D::Source, D::SourceErrorCause>),
}

impl From<TypeConstraintAbort> for TypeConstraintError {
    fn from(error: TypeConstraintAbort) -> Self {
        Self::Abort(error)
    }
}

impl From<TypeConstraintRejection> for TypeConstraintError {
    fn from(error: TypeConstraintRejection) -> Self {
        Self::Rejected(error)
    }
}

impl From<TypeConstraintInvariant> for TypeConstraintError {
    fn from(error: TypeConstraintInvariant) -> Self {
        Self::Invariant(error)
    }
}
