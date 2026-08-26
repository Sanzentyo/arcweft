//! Opaque normalized binding rows and deterministic read-only iteration.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, slice,
    sync::Arc,
};

use crate::effect_row::{
    EffectIssuerRebindError, EffectRow, EffectSubstitution, EffectVar, EffectVarIssuer,
};

use super::super::{GenericTypeParameterId, TypeKind};
use super::context::{TypeConstraintAccounting, TypeConstraintContext};
use super::{
    CheckedConstraintSourceProjection, ClosedConstraintProbe, ConstraintAcceptance,
    ConstraintDomain, ConstraintPath, SourceError, TypeConstraintAbort,
    TypeConstraintConstEligibility, TypeConstraintError, TypeConstraintInvariant,
    TypeConstraintParameterEligibility, TypeConstraintRejection, TypeConstraintShape,
};

/// Closure phase used by the one typed projected-type visitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConstraintClosurePolicy {
    Hint,
    InheritedSeed,
    ProjectionClosed,
    ProjectionFuture,
    Terminal,
}

/// A typed type parameter remaining after one projection.  Constant
/// parameters do not appear here because the lower solver has no const
/// binding path: rigid constants are exact headers and all other const
/// entries are rejected by the visitor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RemainingConstraintParameter(GenericTypeParameterId);

impl RemainingConstraintParameter {
    pub(crate) const fn parameter(&self) -> &GenericTypeParameterId {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedConstraintType {
    pub(crate) value: TypeKind,
    pub(crate) remaining: Box<[RemainingConstraintParameter]>,
}

/// Borrow-only substitution authority accepted by the single projection
/// visitor.  This trait is confined to the private normalization module, so
/// the solver path map and the opaque sorted solution are its only owners.
pub(super) trait ConstraintBindingLookup {
    fn binding(&self, parameter: &GenericTypeParameterId) -> Option<&TypeKind>;
}

impl ConstraintBindingLookup for BTreeMap<GenericTypeParameterId, TypeKind> {
    fn binding(&self, parameter: &GenericTypeParameterId) -> Option<&TypeKind> {
        self.get(parameter)
    }
}

/// Project one semantic type through substitutions and closure policy.
///
/// Every node enters the same cancellation and node meter, validates its
/// namespace against the candidate scope, substitutes through the extended
/// binding map, and then projects children.  Array-length headers are visited
/// as their own typed nodes as well.
pub(super) fn project_type<A, D, B>(
    ty: &TypeKind,
    bindings: &B,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<ProjectedConstraintType, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    B: ConstraintBindingLookup + ?Sized,
{
    let mut visiting = BTreeSet::new();
    let mut remaining = BTreeSet::new();
    let value = project_type_inner(ty, bindings, policy, context, &mut visiting, &mut remaining)?;
    Ok(ProjectedConstraintType {
        value,
        remaining: remaining.into_iter().collect(),
    })
}

fn project_type_inner<A, D, B>(
    ty: &TypeKind,
    bindings: &B,
    policy: ConstraintClosurePolicy,
    context: &mut TypeConstraintContext<'_, A, D>,
    visiting: &mut BTreeSet<GenericTypeParameterId>,
    remaining: &mut BTreeSet<RemainingConstraintParameter>,
) -> Result<TypeKind, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
    B: ConstraintBindingLookup + ?Sized,
{
    context.check_cancelled()?;
    context.enter_node()?;
    let shape = ty.constraint_shape();
    if matches!(shape, TypeConstraintShape::Unresolved) {
        return Err(TypeConstraintRejection::UnresolvedType.into());
    }
    if let TypeConstraintShape::Generic(parameter) = shape {
        let eligibility = context.parameter_eligibility(parameter).ok_or_else(|| {
            TypeConstraintError::Invariant(TypeConstraintInvariant::ParameterScope(
                super::TypeConstraintParameterScopeInvariant::TypeParameterOutOfScope {
                    parameter: parameter.clone(),
                },
            ))
        })?;
        if let Some(bound) = bindings.binding(parameter) {
            if !visiting.insert(parameter.clone()) {
                return match policy {
                    ConstraintClosurePolicy::Hint => {
                        remaining.insert(RemainingConstraintParameter(parameter.clone()));
                        Ok(TypeKind::GenericParam(parameter.clone()))
                    }
                    ConstraintClosurePolicy::InheritedSeed
                    | ConstraintClosurePolicy::ProjectionClosed
                    | ConstraintClosurePolicy::ProjectionFuture
                    | ConstraintClosurePolicy::Terminal => {
                        Err(TypeConstraintRejection::CyclicInstantiation {
                            parameter: parameter.clone(),
                        }
                        .into())
                    }
                };
            }
            let projected =
                project_type_inner(bound, bindings, policy, context, visiting, remaining);
            visiting.remove(parameter);
            return projected;
        }
        if allows_unbound_type(policy, eligibility) {
            if !matches!(eligibility, TypeConstraintParameterEligibility::Rigid) {
                remaining.insert(RemainingConstraintParameter(parameter.clone()));
            }
            return Ok(TypeKind::GenericParam(parameter.clone()));
        }
        return Err(TypeConstraintRejection::IncompleteInstantiation {
            parameter: parameter.clone(),
        }
        .into());
    }

    if let TypeConstraintShape::Array { len, .. } = shape {
        visit_array_length(len, context)?;
    }
    if let TypeConstraintShape::Function { effects, .. } = shape {
        context.validate_effect_row(effects)?;
    }
    let mut children = Vec::new();
    for child in shape.children() {
        children.push(project_type_inner(
            child, bindings, policy, context, visiting, remaining,
        )?);
    }
    shape.rebuild(children)
}

fn allows_unbound_type(
    policy: ConstraintClosurePolicy,
    eligibility: TypeConstraintParameterEligibility,
) -> bool {
    match policy {
        ConstraintClosurePolicy::Hint | ConstraintClosurePolicy::InheritedSeed => true,
        ConstraintClosurePolicy::ProjectionClosed => {
            matches!(eligibility, TypeConstraintParameterEligibility::Rigid)
        }
        ConstraintClosurePolicy::ProjectionFuture => matches!(
            eligibility,
            TypeConstraintParameterEligibility::Rigid
                | TypeConstraintParameterEligibility::FutureEligible
        ),
        // A completed candidate solution may retain a future-eligible atom
        // only because the exact scope classifies it for a later callable
        // group.  A truly terminal call is closed by its higher owner by
        // issuing no FutureEligible entries at all.
        ConstraintClosurePolicy::Terminal => matches!(
            eligibility,
            TypeConstraintParameterEligibility::Rigid
                | TypeConstraintParameterEligibility::FutureEligible
        ),
    }
}

fn visit_array_length<A, D>(
    length: &super::super::ArrayLength,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<(), TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
    context.check_cancelled()?;
    context.enter_node()?;
    match length {
        super::super::ArrayLength::Const(_) => Ok(()),
        super::super::ArrayLength::Generic(parameter) => {
            match context.const_parameter_eligibility(parameter) {
                Some(TypeConstraintConstEligibility::Rigid) => Ok(()),
                None => Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::ParameterScope(
                        super::TypeConstraintParameterScopeInvariant::ConstParameterOutOfScope {
                            parameter: parameter.clone(),
                        },
                    ),
                )),
            }
        }
        super::super::ArrayLength::Error(_) | super::super::ArrayLength::Inferred => {
            Err(TypeConstraintRejection::UnresolvedType.into())
        }
    }
}

/// One declaration-owned generic binding in a sealed constraint path.
#[derive(Debug, Eq, Hash, PartialEq)]
struct CheckedTypeArgumentBinding {
    parameter: GenericTypeParameterId,
    value: TypeKind,
}

impl CheckedTypeArgumentBinding {
    fn new(parameter: GenericTypeParameterId, value: TypeKind) -> Self {
        Self { parameter, value }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct CheckedEffectArgumentBinding {
    variable: EffectVar,
    value: EffectRow,
}

impl CheckedEffectArgumentBinding {
    fn new(variable: EffectVar, value: EffectRow) -> Self {
        Self { variable, value }
    }
}

/// Sorted, opaque binding solution. It intentionally does not implement
/// `Clone`; sharing is represented by `Arc<TypeConstraintSolution>` only.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TypeConstraintSolution {
    bindings: Box<[CheckedTypeArgumentBinding]>,
    effect_bindings: Box<[CheckedEffectArgumentBinding]>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KeyedConstraintProjection<P> {
    key: P,
    value: TypeKind,
}

impl<P> KeyedConstraintProjection<P> {
    pub(crate) fn new(key: P, value: TypeKind) -> Self {
        Self { key, value }
    }

    pub(crate) const fn key(&self) -> &P {
        &self.key
    }

    pub(crate) const fn value(&self) -> &TypeKind {
        &self.value
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
    Source(SourceError<D::Source, Box<[D::SourceErrorCause]>>),
    SourceProjection(RejectedConstraintSourceProjection<D>),
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
    Client(D::ClientInvariant),
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
    FatalSource(SourceError<D::Source, D::SourceErrorCause>),
    Abort(TypeConstraintAbort),
    Invariant(TypeConstraintFailureInvariant<D>),
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
                Self::Rejected(TypeConstraintCandidateFailure::Constraint(error))
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

impl TypeConstraintSolution {
    pub(crate) fn bindings(&self) -> TypeConstraintBindingIter<'_> {
        TypeConstraintBindingIter(self.bindings.iter())
    }

    pub(crate) fn effect_bindings(&self) -> TypeConstraintEffectBindingIter<'_> {
        TypeConstraintEffectBindingIter(self.effect_bindings.iter())
    }

    pub(super) fn from_maps(
        bindings: BTreeMap<GenericTypeParameterId, TypeKind>,
        effect_bindings: BTreeMap<EffectVar, EffectRow>,
    ) -> Self {
        Self {
            bindings: bindings
                .into_iter()
                .map(|(parameter, value)| CheckedTypeArgumentBinding::new(parameter, value))
                .collect(),
            effect_bindings: effect_bindings
                .into_iter()
                .map(|(variable, value)| CheckedEffectArgumentBinding::new(variable, value))
                .collect(),
        }
    }

    /// Apply the sealed lower solution without exposing a caller-owned
    /// substitution table. Callers only receive the normalized type result.
    pub(crate) fn apply(&self, ty: &TypeKind) -> TypeKind {
        let bindings = self
            .bindings()
            .map(|(parameter, value)| (parameter.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();
        let effects = EffectSubstitution::from_rows(
            self.effect_bindings()
                .map(|(variable, value)| (*variable, value.clone())),
        );
        ty.substitute_type_parameters(&bindings)
            .substitute_effect_rows(&effects)
            .expect("sealed constraint solutions contain only canonical effect rows")
    }

    pub(crate) fn checked_rebind_effect_issuer(
        &self,
        prepared: EffectVarIssuer,
        checked: EffectVarIssuer,
        authorized_ordinals: &BTreeSet<u32>,
    ) -> Result<Self, EffectIssuerRebindError> {
        let bindings = self
            .bindings()
            .map(|(parameter, value)| {
                Ok((
                    parameter.clone(),
                    value.checked_rebind_effect_rows(prepared, checked, authorized_ordinals)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let effect_bindings = self
            .effect_bindings()
            .map(|(variable, value)| {
                if variable.issuer() != prepared {
                    return Err(EffectIssuerRebindError::ForeignVariable {
                        variable: *variable,
                    });
                }
                if !authorized_ordinals.contains(&variable.index()) {
                    return Err(EffectIssuerRebindError::UnauthorizedVariable {
                        variable: *variable,
                    });
                }
                Ok((
                    variable.rebind_issuer(prepared, checked),
                    value.checked_rebind_issuer(prepared, checked, authorized_ordinals)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self::from_maps(bindings, effect_bindings))
    }
}

impl ConstraintBindingLookup for TypeConstraintSolution {
    fn binding(&self, parameter: &GenericTypeParameterId) -> Option<&TypeKind> {
        self.bindings
            .binary_search_by(|binding| binding.parameter.cmp(parameter))
            .ok()
            .map(|index| &self.bindings[index].value)
    }
}

pub(crate) struct TypeConstraintBindingIter<'a>(slice::Iter<'a, CheckedTypeArgumentBinding>);

pub(crate) struct TypeConstraintEffectBindingIter<'a>(
    slice::Iter<'a, CheckedEffectArgumentBinding>,
);

impl<'a> Iterator for TypeConstraintBindingIter<'a> {
    type Item = (&'a GenericTypeParameterId, &'a TypeKind);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|binding| (&binding.parameter, &binding.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|binding| (&binding.parameter, &binding.value))
    }
}

impl ExactSizeIterator for TypeConstraintBindingIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> Iterator for TypeConstraintEffectBindingIter<'a> {
    type Item = (&'a EffectVar, &'a EffectRow);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|binding| (&binding.variable, &binding.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for TypeConstraintEffectBindingIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|binding| (&binding.variable, &binding.value))
    }
}

impl ExactSizeIterator for TypeConstraintEffectBindingIter<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
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
        &BTreeMap::<GenericTypeParameterId, TypeKind>::new(),
        ConstraintClosurePolicy::Hint,
        context,
    )
    .map(|_| ())
}

pub(crate) fn occurs_in_shape<A, D>(
    shape: TypeConstraintShape<'_>,
    parameter: &GenericTypeParameterId,
    bindings: &BTreeMap<GenericTypeParameterId, TypeKind>,
    context: &mut TypeConstraintContext<'_, A, D>,
) -> Result<bool, TypeConstraintError>
where
    A: TypeConstraintAccounting,
    D: ConstraintDomain,
{
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
        shape => {
            for child in shape.children() {
                if occurs_in_type(child, parameter, bindings, context)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

pub(crate) fn occurs_in_type<A, D>(
    ty: &TypeKind,
    parameter: &GenericTypeParameterId,
    bindings: &BTreeMap<GenericTypeParameterId, TypeKind>,
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
    if path.bindings.is_empty() {
        return Ok(path);
    }
    let ConstraintPath {
        bindings: source,
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
    } = path;
    let mut sealed = ConstraintPath {
        bindings: BTreeMap::new(),
        effects,
        equations,
        choice_key,
        deferred_cycles,
        probe_trace,
    };
    for (parameter, value) in &source {
        let mut visiting = BTreeSet::new();
        let value = seal_type(value, &source, &mut visiting, context)?;
        context.add_sealed_binding(&mut sealed, parameter.clone(), value)?;
    }
    Ok(sealed)
}

pub(crate) fn seal_type<A, D>(
    ty: &TypeKind,
    bindings: &BTreeMap<GenericTypeParameterId, TypeKind>,
    visiting: &mut BTreeSet<GenericTypeParameterId>,
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
    let mut left_children = left_shape.children();
    let mut right_children = right_shape.children();
    loop {
        match (left_children.next(), right_children.next()) {
            (Some(left), Some(right)) if types_equal(left, right, context)? => {}
            (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => return Ok(false),
            (None, None) => return Ok(true),
        }
    }
}

#[cfg(test)]
mod malformed_inherited_tests {
    use super::*;
    use crate::types::constraints::context::{
        LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits,
    };
    use crate::types::constraints::transaction::TypeConstraintTransaction;
    use crate::types::constraints::{
        InheritedSolutionInvariant, InheritedSolutionInvariantKind, NoConstraintClient,
        TypeConstraintInitializationFailure, TypeConstraintInvariant,
        TypeConstraintParameterEligibility, TypeConstraintParameterScope,
    };
    use crate::types::{DetachedGenericOwnerId, GenericParameterOwnerId, GenericTypeParameterId};
    use std::{
        collections::BTreeSet,
        sync::{Arc, atomic::AtomicBool},
    };

    fn parameter(ordinal: u16) -> GenericTypeParameterId {
        GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(190)),
            ordinal,
        )
    }

    fn initialize(
        rows: Vec<(GenericTypeParameterId, TypeKind)>,
    ) -> Result<(), TypeConstraintInitializationFailure> {
        let parameters = rows
            .iter()
            .map(|(parameter, _)| parameter.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|parameter| (parameter, TypeConstraintParameterEligibility::Bindable))
            .collect::<Vec<_>>();
        let scope = TypeConstraintParameterScope::new(parameters).expect("unique test scope");
        let solution = TypeConstraintSolution {
            bindings: rows
                .into_iter()
                .map(|(parameter, value)| CheckedTypeArgumentBinding::new(parameter, value))
                .collect(),
            effect_bindings: Box::new([]),
        };
        let cancellation = AtomicBool::new(false);
        let mut context =
            TypeConstraintContext::<LocalConstraintAccounting<'_>, NoConstraintClient>::with_scope(
                TypeConstraintLimits::new(256, 128, 32, 16),
                &cancellation,
                scope,
            );
        let mut transaction = TypeConstraintTransaction::<NoConstraintClient>::new();
        transaction.initialize(&mut context, Some(Arc::new(solution)))
    }

    #[test]
    fn malformed_inherited_rows_are_typed_duplicate_or_unordered_invariants() {
        let first = parameter(0);
        let second = parameter(1);
        for (rows, expected) in [
            (
                vec![(first.clone(), TypeKind::I32), (first, TypeKind::String)],
                InheritedSolutionInvariantKind::DuplicateOrUnordered,
            ),
            (
                vec![(second, TypeKind::I32), (parameter(0), TypeKind::String)],
                InheritedSolutionInvariantKind::DuplicateOrUnordered,
            ),
        ] {
            assert!(matches!(
                initialize(rows),
                Err(TypeConstraintInitializationFailure::Invariant(
                    TypeConstraintInvariant::InheritedSolution(InheritedSolutionInvariant {
                        kind,
                        ..
                    }),
                )) if kind == expected
            ));
        }
    }
}
