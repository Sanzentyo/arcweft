//! Canonical type projection, path normalization, and equality.

use super::ConstraintSourceId;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use super::super::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind};
use super::application::ConstraintApplicationScopes;
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::{
    CheckedConstraintSourceProjection, ConstraintAcceptance, ConstraintDomain, ConstraintPath,
    SourceError, TypeConstraintAbort, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintRejection, TypeConstraintShape,
};

#[cfg(test)]
mod tests;

/// Closure phase used by the one typed projected-type visitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConstraintClosurePolicy {
    /// Admit source type structure and scope without following active bindings.
    Validation,
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

/// Read-only state of one path's scoped type/constant projection. Only the
/// path and its consuming normalization operation construct this view.
pub(crate) struct ConstraintProjectionView<'a, D: ConstraintDomain> {
    applications: &'a ConstraintApplicationScopes<D>,
    bindings: &'a BTreeMap<GenericTypeReference, TypeKind>,
    const_bindings: &'a BTreeMap<GenericConstReference, ArrayLength>,
}

impl<D: ConstraintDomain> Copy for ConstraintProjectionView<'_, D> {}

impl<D: ConstraintDomain> Clone for ConstraintProjectionView<'_, D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, D: ConstraintDomain> ConstraintProjectionView<'a, D> {
    pub(super) const fn applications(self) -> &'a ConstraintApplicationScopes<D> {
        self.applications
    }

    pub(super) fn binding(self, parameter: &GenericTypeReference) -> Option<&'a TypeKind> {
        self.bindings.get(parameter)
    }

    pub(super) fn const_binding(
        self,
        parameter: &GenericConstReference,
    ) -> Option<&'a ArrayLength> {
        self.const_bindings.get(parameter)
    }
}

impl<D: ConstraintDomain> ConstraintPath<D> {
    pub(crate) fn projection_view(&self) -> ConstraintProjectionView<'_, D> {
        ConstraintProjectionView {
            applications: &self.applications,
            bindings: &self.bindings,
            const_bindings: &self.const_bindings,
        }
    }
}

/// The path-local representative of a generic reference after following only
/// direct generic aliases. Structural bindings remain typed values so the
/// ordinary relation visitor can handle their children under the right
/// binders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TypeAliasResolution {
    Parameter(GenericTypeReference),
    Structure(TypeKind),
    Cycle(Box<[GenericTypeReference]>),
}

/// The path-local representative of a generic constant after following only
/// direct generic aliases. Non-alias constant values remain typed array-length
/// values so the relation visitor owns unresolved/error handling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConstAliasResolution {
    Parameter(GenericConstReference),
    Value(ArrayLength),
    Cycle(Box<[GenericConstReference]>),
}

/// Resolve direct alias chains with the same scope checks and node budget as
/// other path projections. A deterministic representative is retained for a
/// genuine alias cycle so callers can defer it without recursing forever.
pub(crate) fn resolve_type_alias<A, D>(
    parameter: &GenericTypeReference,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<TypeAliasResolution, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let view = path.projection_view();
    let mut current = parameter.clone();
    let mut positions = BTreeMap::<GenericTypeReference, usize>::new();
    let mut chain = Vec::new();
    loop {
        context.check_cancelled()?;
        context.enter_node()?;
        if context.parameter_eligibility(&current, view).is_none() {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::ParameterScope(
                    super::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                        parameter: current,
                    },
                ),
            ));
        }
        if let Some(start) = positions.get(&current).copied() {
            let mut cycle = chain[start..].to_vec();
            cycle.sort();
            cycle.dedup();
            return Ok(TypeAliasResolution::Cycle(cycle.into_boxed_slice()));
        }
        positions.insert(current.clone(), chain.len());
        chain.push(current.clone());
        let Some(bound) = view.binding(&current) else {
            return Ok(TypeAliasResolution::Parameter(current));
        };
        match bound.constraint_shape() {
            TypeConstraintShape::Generic(next) => current = next.clone(),
            _ => return Ok(TypeAliasResolution::Structure(bound.clone())),
        }
    }
}

/// Resolve direct constant alias chains under the exact application scope and
/// node budget. A deterministic cycle witness is deferred to path closure.
pub(crate) fn resolve_const_alias<A, D>(
    parameter: &GenericConstReference,
    path: &ConstraintPath<D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ConstAliasResolution, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let view = path.projection_view();
    let mut current = parameter.clone();
    let mut positions = BTreeMap::<GenericConstReference, usize>::new();
    let mut chain = Vec::new();
    loop {
        context.check_cancelled()?;
        context.enter_node()?;
        if context
            .const_parameter_eligibility(&current, view)
            .is_none()
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::ParameterScope(
                    super::TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                        parameter: current,
                    },
                ),
            ));
        }
        if let Some(start) = positions.get(&current).copied() {
            let mut cycle = chain[start..].to_vec();
            cycle.sort();
            cycle.dedup();
            return Ok(ConstAliasResolution::Cycle(cycle.into_boxed_slice()));
        }
        positions.insert(current.clone(), chain.len());
        chain.push(current.clone());
        let Some(bound) = view.const_binding(&current) else {
            return Ok(ConstAliasResolution::Parameter(current));
        };
        match bound {
            ArrayLength::Generic(next) => current = next.clone(),
            _ => return Ok(ConstAliasResolution::Value(bound.clone())),
        }
    }
}

mod projection;
use projection::{project_array_length, project_type_inner};
pub(super) use projection::{project_const_argument, project_type};

pub(crate) fn const_occurs_in<A, D>(
    value: &ArrayLength,
    parameter: &GenericConstReference,
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    let mut current = value;
    let mut visited = BTreeSet::new();
    loop {
        context.enter_node()?;
        let ArrayLength::Generic(candidate) = current else {
            return match current {
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
        if !visited.insert(candidate.clone()) {
            return Ok(false);
        }
        let Some(bound) = view.const_binding(candidate) else {
            return Ok(false);
        };
        current = bound;
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KeyedConstraintProjection<P> {
    key: Arc<P>,
    value: super::super::ScopedType,
}

impl<P> KeyedConstraintProjection<P> {
    pub(super) fn new(key: Arc<P>, value: TypeKind, scope: super::super::GenericScope) -> Self {
        Self {
            key,
            value: super::super::ScopedType::new(value, scope),
        }
    }

    pub(crate) fn key(&self) -> &P {
        &self.key
    }

    pub(crate) const fn value(&self) -> super::super::ScopedTypeView<'_> {
        self.value.view()
    }
}

#[derive(Eq, PartialEq)]
pub(crate) struct SolvedCandidate<D: ConstraintDomain> {
    pub(crate) component: super::CompletedConstraintComponent<D>,
    pub(crate) sealed_branch: D::SealedBranchValue,
}

impl<D: ConstraintDomain> fmt::Debug for SolvedCandidate<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SolvedCandidate")
            .field("solution", self.component.selected().solution())
            .field("application_count", &self.component.applications().len())
            .field(
                "projection_count",
                &self.component.selected().projections().len(),
            )
            .field("closed_source_count", &self.component.sources().all().len())
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
    source: ConstraintSourceId<D::Source>,
    alternative: Option<D::AlternativeIndex>,
    source_projection: CheckedConstraintSourceProjection,
    acceptance: ConstraintAcceptance,
    expected: TypeKind,
    actual: TypeKind,
}

impl<D: ConstraintDomain> RejectedConstraintSourceProjection<D> {
    pub(super) fn new(
        source: ConstraintSourceId<D::Source>,
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

    pub(crate) const fn source(&self) -> ConstraintSourceId<D::Source> {
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
        source: ConstraintSourceId<D::Source>,
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
    Source(Box<SourceError<ConstraintSourceId<D::Source>, Box<[D::SourceErrorCause]>>>),
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
    FatalSource(Box<SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>>),
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintFailureInvariant<D>),
}

impl<D: ConstraintDomain> TypeConstraintFailure<D> {
    pub(crate) fn rejected(error: TypeConstraintCandidateFailure<D>) -> Self {
        Self::Rejected(error)
    }

    pub(crate) fn fatal_source(
        error: SourceError<ConstraintSourceId<D::Source>, D::SourceErrorCause>,
    ) -> Self {
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
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    project_type(ty, view, ConstraintClosurePolicy::Validation, context).map(|_| ())
}

pub(crate) fn occurs_in_shape<A, D>(
    shape: TypeConstraintShape<'_>,
    parameter: &GenericTypeReference,
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    occurs_in_shape_inner(shape, parameter, view, context, &mut BTreeSet::new())
}

fn occurs_in_shape_inner<A, D>(
    shape: TypeConstraintShape<'_>,
    parameter: &GenericTypeReference,
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    aliases: &mut BTreeSet<GenericTypeReference>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.validate_type_header(shape, view)?;
    match shape {
        TypeConstraintShape::Unresolved => Err(TypeConstraintRejection::UnresolvedType.into()),
        TypeConstraintShape::Generic(candidate) => {
            if candidate == parameter {
                return Ok(true);
            }
            if !aliases.insert(candidate.clone()) {
                return Ok(false);
            }
            let Some(bound) = view.binding(candidate) else {
                aliases.remove(candidate);
                return Ok(false);
            };
            let result = occurs_in_type_inner(bound, parameter, view, context, aliases);
            aliases.remove(candidate);
            result
        }
        shape => context.with_binder(shape.binder(), |context| {
            for child in shape.children() {
                if occurs_in_type_inner(child, parameter, view, context, aliases)? {
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
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    occurs_in_type_inner(ty, parameter, view, context, &mut BTreeSet::new())
}

fn occurs_in_type_inner<A, D>(
    ty: &TypeKind,
    parameter: &GenericTypeReference,
    view: ConstraintProjectionView<'_, D>,
    context: &mut TypeConstraintContext<'_, A, D>,
    aliases: &mut BTreeSet<GenericTypeReference>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.enter_node()?;
    occurs_in_shape_inner(ty.constraint_shape(), parameter, view, context, aliases)
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
    let source_applications = Arc::clone(&path.applications);
    let ConstraintPath {
        applications,
        bindings: source,
        const_bindings: const_source,
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
        projections,
    } = path;
    let source_view = ConstraintProjectionView {
        applications: &source_applications,
        bindings: &source,
        const_bindings: &const_source,
    };
    let mut sealed = ConstraintPath {
        applications,
        bindings: BTreeMap::new(),
        const_bindings: BTreeMap::new(),
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
        projections,
    };
    for (parameter, value) in &source {
        let mut visiting = BTreeSet::new();
        let value = seal_type(value, source_view, &mut visiting, context)?;
        context.add_sealed_binding(&mut sealed, parameter.clone(), value)?;
    }
    for (parameter, value) in &const_source {
        let mut visiting = BTreeSet::new();
        let value = seal_const(value, source_view, &mut visiting, context)?;
        context.add_sealed_const_binding(&mut sealed, parameter.clone(), value)?;
    }
    Ok(sealed)
}

pub(crate) fn seal_type<A, D>(
    ty: &TypeKind,
    view: ConstraintProjectionView<'_, D>,
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
        view,
        ConstraintClosurePolicy::Hint,
        context,
        visiting,
        &mut remaining,
    )
}

fn seal_const<A, D>(
    value: &ArrayLength,
    view: ConstraintProjectionView<'_, D>,
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
        view,
        ConstraintClosurePolicy::Hint,
        context,
        visiting,
        &mut remaining,
    )
}

/// Completed terms have already passed their own lexical-scope seal. Equality
/// still visits every compared type and effect node using the live work owner.
pub(super) fn completed_types_equal<A: TypeConstraintAccounting, D: ConstraintDomain>(
    left: &TypeKind,
    right: &TypeKind,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError> {
    context.enter_node()?;
    type_shapes_equal(left.constraint_shape(), right.constraint_shape(), context)
}

fn type_shapes_equal<A: TypeConstraintAccounting, D: ConstraintDomain>(
    left_shape: TypeConstraintShape<'_>,
    right_shape: TypeConstraintShape<'_>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError> {
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
        && !left_effects.equal_with(right_effects, context)?
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
                (Some(left), Some(right)) => {
                    context.enter_node()?;
                    if !type_shapes_equal(
                        left.constraint_shape(),
                        right.constraint_shape(),
                        context,
                    )? {
                        return Ok(false);
                    }
                }
                (Some(_), None) | (None, Some(_)) => return Ok(false),
                (None, None) => return Ok(true),
            }
        }
    })
}
