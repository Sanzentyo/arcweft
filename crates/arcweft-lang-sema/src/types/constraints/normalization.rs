//! Canonical type projection, path normalization, and equality.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use super::super::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind};
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::{
    CheckedConstraintSourceProjection, ClosedConstraintProbe, ConstraintAcceptance,
    ConstraintDomain, ConstraintPath, SourceError, TypeConstraintAbort, TypeConstraintError,
    TypeConstraintInvariant, TypeConstraintRejection, TypeConstraintShape, TypeConstraintSolution,
};

#[cfg(test)]
mod tests;

/// Closure phase used by the one typed projected-type visitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConstraintClosurePolicy {
    Hint,
    ProjectionClosed,
    ProjectionFuture,
    /// Seal one constraint-group solution relative to its exact scope.
    /// Future-eligible atoms remain open for the next group; bindable atoms
    /// must be present in the completed solution.
    SolutionCompletion,
}

/// A kind-separated generic parameter remaining after one projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RemainingConstraintParameter(super::ConstraintGenericParameterId);

impl RemainingConstraintParameter {
    pub(crate) const fn parameter(&self) -> &super::ConstraintGenericParameterId {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedConstraintType {
    pub(crate) value: TypeKind,
    pub(crate) remaining: Box<[RemainingConstraintParameter]>,
}

/// Borrow-only substitution authority accepted by the single projection
/// visitor. This trait is confined to the private constraint owner, so the
/// active path map and the opaque completed solution are its only producers.
pub(super) trait ConstraintBindingLookup {
    fn binding(&self, parameter: &GenericTypeReference) -> Option<&TypeKind>;
}

pub(super) trait ConstraintConstBindingLookup {
    fn const_binding(&self, parameter: &GenericConstReference) -> Option<&ArrayLength>;
}

impl ConstraintBindingLookup for BTreeMap<GenericTypeReference, TypeKind> {
    fn binding(&self, parameter: &GenericTypeReference) -> Option<&TypeKind> {
        self.get(parameter)
    }
}

impl ConstraintConstBindingLookup for BTreeMap<GenericConstReference, ArrayLength> {
    fn const_binding(&self, parameter: &GenericConstReference) -> Option<&ArrayLength> {
        self.get(parameter)
    }
}

mod projection;
use projection::{project_array_length, project_type_inner};
pub(super) use projection::{project_const_argument, project_type};

pub(crate) fn const_occurs_in<A, D>(
    value: &ArrayLength,
    parameter: &GenericConstReference,
    bindings: &BTreeMap<GenericConstReference, ArrayLength>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    let ArrayLength::Generic(candidate) = value else {
        return match value {
            ArrayLength::Const(_) => Ok(false),
            ArrayLength::Error(_) | ArrayLength::Inferred => {
                Err(TypeConstraintRejection::UnresolvedType.into())
            }
            ArrayLength::Generic(_) => unreachable!("generic handled above"),
        };
    };
    if candidate == parameter {
        return Ok(true);
    }
    let Some(bound) = bindings.get(candidate) else {
        return Ok(false);
    };
    const_occurs_in(bound, parameter, bindings, context)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KeyedConstraintProjection<P> {
    key: P,
    value: super::super::ScopedType,
}

impl<P> KeyedConstraintProjection<P> {
    pub(super) fn new(key: P, value: TypeKind, scope: super::super::GenericScope) -> Self {
        Self {
            key,
            value: super::super::ScopedType::new(value, scope),
        }
    }

    pub(crate) const fn key(&self) -> &P {
        &self.key
    }

    pub(crate) const fn value(&self) -> super::super::ScopedTypeView<'_> {
        self.value.view()
    }
}

#[derive(Eq, PartialEq)]
pub(crate) struct SolvedCandidate<D: ConstraintDomain> {
    pub(crate) solution: Arc<TypeConstraintSolution>,
    pub(crate) sealed_branch: D::SealedBranchValue,
    pub(crate) projections: Box<[KeyedConstraintProjection<D::Projection>]>,
    pub(crate) closed_sources: Box<[ClosedConstraintProbe<D>]>,
}

impl<D: ConstraintDomain> fmt::Debug for SolvedCandidate<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SolvedCandidate")
            .field("solution", &self.solution)
            .field("projection_count", &self.projections.len())
            .field("closed_source_count", &self.closed_sources.len())
            .finish()
    }
}

/// Exact lower-owned source relation that eliminated one candidate frontier.
///
/// The callback supplies only the actual and selected semantic alternative.
/// Lower derives the closed source projection and projected expectation, then
/// retains both here when their directional relation rejects. Domain owners
/// may map this proof to a terminal authored diagnostic without rechecking the
/// expression or duplicating type compatibility.
pub(crate) struct RejectedConstraintSourceProjection<D: ConstraintDomain> {
    source: D::Source,
    alternative: Option<D::AlternativeIndex>,
    source_projection: CheckedConstraintSourceProjection,
    acceptance: ConstraintAcceptance,
    expected: TypeKind,
    actual: TypeKind,
}

impl<D: ConstraintDomain> RejectedConstraintSourceProjection<D> {
    pub(super) fn new(
        source: D::Source,
        alternative: Option<D::AlternativeIndex>,
        source_projection: CheckedConstraintSourceProjection,
        acceptance: ConstraintAcceptance,
        expected: TypeKind,
        actual: TypeKind,
    ) -> Self {
        Self {
            source,
            alternative,
            source_projection,
            acceptance,
            expected,
            actual,
        }
    }

    pub(crate) const fn source(&self) -> D::Source {
        self.source
    }

    pub(crate) const fn alternative(&self) -> Option<D::AlternativeIndex> {
        self.alternative
    }

    pub(crate) const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }

    pub(crate) const fn acceptance(&self) -> ConstraintAcceptance {
        self.acceptance
    }

    pub(crate) const fn expected(&self) -> &TypeKind {
        &self.expected
    }

    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }

    #[cfg(test)]
    pub(crate) fn test_new(
        source: D::Source,
        alternative: Option<D::AlternativeIndex>,
        source_projection: CheckedConstraintSourceProjection,
        acceptance: ConstraintAcceptance,
        expected: TypeKind,
        actual: TypeKind,
    ) -> Self {
        Self::new(
            source,
            alternative,
            source_projection,
            acceptance,
            expected,
            actual,
        )
    }
}

impl<D: ConstraintDomain> fmt::Debug for RejectedConstraintSourceProjection<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RejectedConstraintSourceProjection")
            .field("source", &"<domain source>")
            .field(
                "alternative_ordinal",
                &self.alternative.map(|index| D::alternative_ordinal(&index)),
            )
            .field("source_projection", &self.source_projection)
            .field("acceptance", &self.acceptance)
            .field("expected", &self.expected)
            .field("actual", &self.actual)
            .finish()
    }
}

pub(crate) enum TypeConstraintCandidateFailure<D: ConstraintDomain> {
    Constraint(TypeConstraintRejection),
    Source(Box<SourceError<D::Source, Box<[D::SourceErrorCause]>>>),
    SourceProjection(Box<RejectedConstraintSourceProjection<D>>),
}

impl<D: ConstraintDomain> fmt::Debug for TypeConstraintCandidateFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constraint(error) => formatter.debug_tuple("Constraint").field(error).finish(),
            Self::Source(error) => formatter
                .debug_struct("Source")
                .field("source_phase", &error.phase())
                .finish(),
            Self::SourceProjection(rejection) => formatter
                .debug_tuple("SourceProjection")
                .field(rejection)
                .finish(),
        }
    }
}

pub(crate) enum TypeConstraintFailureInvariant<D: ConstraintDomain> {
    Constraint(TypeConstraintInvariant),
    Client(Box<D::ClientInvariant>),
}

impl<D: ConstraintDomain> fmt::Debug for TypeConstraintFailureInvariant<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Constraint(error) => formatter.debug_tuple("Constraint").field(error).finish(),
            Self::Client(_) => formatter.write_str("Client(..)"),
        }
    }
}

pub(crate) enum TypeConstraintFailure<D: ConstraintDomain> {
    Rejected(TypeConstraintCandidateFailure<D>),
    FatalSource(Box<SourceError<D::Source, D::SourceErrorCause>>),
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintFailureInvariant<D>),
}

impl<D: ConstraintDomain> TypeConstraintFailure<D> {
    pub(crate) fn rejected(error: TypeConstraintCandidateFailure<D>) -> Self {
        Self::Rejected(error)
    }

    pub(crate) fn fatal_source(error: SourceError<D::Source, D::SourceErrorCause>) -> Self {
        Self::FatalSource(Box::new(error))
    }

    pub(crate) fn client_invariant(invariant: D::ClientInvariant) -> Self {
        Self::Invariant(TypeConstraintFailureInvariant::Client(Box::new(invariant)))
    }
}

impl<D: ConstraintDomain> fmt::Debug for TypeConstraintFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(error) => formatter.debug_tuple("Rejected").field(error).finish(),
            Self::FatalSource(error) => formatter
                .debug_struct("FatalSource")
                .field("source_phase", &error.phase())
                .finish(),
            Self::Abort(error) => formatter.debug_tuple("Abort").field(error).finish(),
            Self::Invariant(error) => match error {
                TypeConstraintFailureInvariant::Constraint(error) => formatter
                    .debug_tuple("Invariant::Constraint")
                    .field(error)
                    .finish(),
                TypeConstraintFailureInvariant::Client(_) => {
                    formatter.write_str("Invariant::Client(..)")
                }
            },
        }
    }
}

impl<D: ConstraintDomain> From<TypeConstraintError> for TypeConstraintFailure<D> {
    fn from(error: TypeConstraintError) -> Self {
        match error {
            TypeConstraintError::Rejected(error) => {
                Self::rejected(TypeConstraintCandidateFailure::Constraint(error))
            }
            TypeConstraintError::Abort(error) => Self::Abort(error),
            TypeConstraintError::Invariant(error) => {
                Self::Invariant(TypeConstraintFailureInvariant::Constraint(error))
            }
        }
    }
}

impl<D: ConstraintDomain> From<super::MaterializationImmediateFailure<D>>
    for TypeConstraintFailure<D>
{
    fn from(error: super::MaterializationImmediateFailure<D>) -> Self {
        match error {
            super::MaterializationImmediateFailure::Abort(error) => Self::Abort(error),
            super::MaterializationImmediateFailure::Invariant(error) => Self::Invariant(error),
        }
    }
}

impl<D: ConstraintDomain> fmt::Debug for super::MaterializationImmediateFailure<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Abort(error) => formatter.debug_tuple("Abort").field(error).finish(),
            Self::Invariant(error) => formatter.debug_tuple("Invariant").field(error).finish(),
        }
    }
}

pub(super) fn validate_selected_call_self<A, D>(
    value: &TypeKind,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let accepted = value
        .accepts_with(
            value,
            super::super::compatibility::TypeCompatibilityPolicy::SelectedCall,
            context,
        )
        .map_err(super::super::compatibility::binding_plan::map_compatibility_error)?;
    accepted.then_some(()).ok_or(TypeConstraintError::Rejected(
        TypeConstraintRejection::Mismatch,
    ))
}

pub(crate) fn validate_type<A, D>(
    ty: &TypeKind,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    project_type(
        ty,
        &BTreeMap::<GenericTypeReference, TypeKind>::new(),
        &BTreeMap::<GenericConstReference, ArrayLength>::new(),
        ConstraintClosurePolicy::Hint,
        context,
    )
    .map(|_| ())
}

pub(crate) fn occurs_in_shape<A, D>(
    shape: TypeConstraintShape<'_>,
    parameter: &GenericTypeReference,
    bindings: &BTreeMap<GenericTypeReference, TypeKind>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.validate_type_header(shape)?;
    match shape {
        TypeConstraintShape::Unresolved => Err(TypeConstraintRejection::UnresolvedType.into()),
        TypeConstraintShape::Generic(candidate) => {
            if candidate == parameter {
                return Ok(true);
            }
            let Some(bound) = bindings.get(candidate) else {
                return Ok(false);
            };
            occurs_in_type(bound, parameter, bindings, context)
        }
        shape => context.with_binder(shape.binder(), |context| {
            for child in shape.children() {
                if occurs_in_type(child, parameter, bindings, context)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }),
    }
}

pub(crate) fn occurs_in_type<A, D>(
    ty: &TypeKind,
    parameter: &GenericTypeReference,
    bindings: &BTreeMap<GenericTypeReference, TypeKind>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    occurs_in_shape(ty.constraint_shape(), parameter, bindings, context)
}

pub(crate) fn seal_path<A, D>(
    path: ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstraintPath<D>, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if path.bindings.is_empty() && path.const_bindings.is_empty() {
        return Ok(path);
    }
    let ConstraintPath {
        bindings: source,
        const_bindings: const_source,
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
    } = path;
    let mut sealed = ConstraintPath {
        bindings: BTreeMap::new(),
        const_bindings: BTreeMap::new(),
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
    };
    for (parameter, value) in &source {
        let mut visiting = BTreeSet::new();
        let value = seal_type(value, &source, &const_source, &mut visiting, context)?;
        context.add_sealed_binding(&mut sealed, parameter.clone(), value)?;
    }
    for (parameter, value) in &const_source {
        let mut visiting = BTreeSet::new();
        let value = seal_const(value, &const_source, &mut visiting, context)?;
        context.add_sealed_const_binding(&mut sealed, parameter.clone(), value)?;
    }
    Ok(sealed)
}

pub(crate) fn seal_type<A, D>(
    ty: &TypeKind,
    bindings: &BTreeMap<GenericTypeReference, TypeKind>,
    const_bindings: &BTreeMap<GenericConstReference, ArrayLength>,
    visiting: &mut BTreeSet<GenericTypeReference>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut remaining = BTreeSet::new();
    project_type_inner(
        ty,
        bindings,
        const_bindings,
        ConstraintClosurePolicy::Hint,
        context,
        visiting,
        &mut remaining,
    )
}

fn seal_const<A, D>(
    value: &ArrayLength,
    bindings: &BTreeMap<GenericConstReference, ArrayLength>,
    visiting: &mut BTreeSet<GenericConstReference>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ArrayLength, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut remaining = BTreeSet::new();
    project_array_length(
        value,
        bindings,
        ConstraintClosurePolicy::Hint,
        context,
        visiting,
        &mut remaining,
    )
}

pub(crate) fn bindings_equal<A, D>(
    left: &ConstraintPath<D>,
    right: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    if left.bindings.len() != right.bindings.len()
        || left.const_bindings != right.const_bindings
        || !left
            .effects
            .bindings_equal(&right.effects)
            .map_err(super::map_effect_environment_error)?
    {
        return Ok(false);
    }
    for ((left_parameter, left_value), (right_parameter, right_value)) in
        left.bindings.iter().zip(&right.bindings)
    {
        if left_parameter != right_parameter || !types_equal(left_value, right_value, context)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn types_equal<A, D>(
    left: &TypeKind,
    right: &TypeKind,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    let left_shape = left.constraint_shape();
    let right_shape = right.constraint_shape();
    types_equal_entered(left_shape, right_shape, context)
}

pub(crate) fn types_equal_entered<A, D>(
    left_shape: TypeConstraintShape<'_>,
    right_shape: TypeConstraintShape<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.validate_type_header(left_shape)?;
    context.validate_type_header(right_shape)?;
    if matches!(left_shape, TypeConstraintShape::Unresolved)
        || matches!(right_shape, TypeConstraintShape::Unresolved)
    {
        return Err(TypeConstraintRejection::UnresolvedType.into());
    }
    if let (
        TypeConstraintShape::Function {
            effects: left_effects,
            ..
        },
        TypeConstraintShape::Function {
            effects: right_effects,
            ..
        },
    ) = (left_shape, right_shape)
        && left_effects != right_effects
    {
        return Ok(false);
    }
    if !left_shape.same_header(right_shape) {
        return Ok(false);
    }
    context.with_binder(left_shape.binder(), |context| {
        let mut left_children = left_shape.children();
        let mut right_children = right_shape.children();
        loop {
            match (left_children.next(), right_children.next()) {
                (Some(left), Some(right)) if types_equal(left, right, context)? => {}
                (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => return Ok(false),
                (None, None) => return Ok(true),
            }
        }
    })
}
