//! Typed source vocabulary for the candidate-wide constraint transaction.
//!
//! This module is intentionally independent of callable and analyzer types.
//! A caller supplies the domain identities and the callback only returns
//! evidence; the lower owner derives every checked projection and expected
//! type from the prepared source constraint.

use super::ConstraintSourceId;

use std::sync::Arc;

use super::super::{ArrayLength, MapKind, TypeKind};
use super::{PreparedSourceConstraintInvariant, TypeConstraintError, TypeConstraintInvariant};

/// Minimal domain used only by the private lower tests.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NoConstraintClient;

/// Domain identities owned by the caller of the lower transaction.
///
/// Semantic branch and sealed-value identities deliberately require only
/// equality.  The lower transaction stores them behind private `Arc` trace
/// cells, so a callback does not have to make semantic values `Copy`, `Clone`,
/// or `Ord` merely to let a frontier fork.
pub(crate) trait ConstraintDomain: 'static {
    type Application: Copy + Ord;
    type Source: Copy + Ord;
    type AlternativeIndex: Copy + Eq + Ord;
    type EvidenceRule: Eq;
    type ObservedEvidence: Eq;
    type CheckedEvidence: Eq;
    type ProbeSemanticBranch: Eq;
    type SealedBranchValue: Eq;
    type Projection: Eq + Ord;
    type SourceErrorCause;
    type ClientInvariant;

    /// Select a prepared rule from source observation, before stable evidence
    /// is issued at completion.
    fn evidence_accepts(rule: &Self::EvidenceRule, observed: &Self::ObservedEvidence) -> bool;

    /// Project observed evidence alongside the lower-owned terminal actual
    /// type. Domains may retain semantic coordinates that change identity
    /// when generic/effect substitutions close; lower never interprets or
    /// reconstructs those coordinates itself.
    fn project_checked_evidence(
        observed: &Self::ObservedEvidence,
        actual: &TypeKind,
    ) -> Option<Self::CheckedEvidence>;

    /// Return the exact schema-relative ordinal of one alternative key.
    /// Prepared rows are accepted only when this is equal to their physical
    /// position, so opaque gaps cannot bypass schema ordering validation.
    fn alternative_ordinal(index: &Self::AlternativeIndex) -> u32;

    /// Return the source coordinate owned by a client invariant.  The lower
    /// layer uses this only for driver-side protocol validation and never
    /// interprets the invariant payload.
    fn client_invariant_source(invariant: &Self::ClientInvariant) -> Self::Source;

    fn empty_sealed_branch() -> Self::SealedBranchValue;
}

#[cfg(test)]
impl ConstraintDomain for NoConstraintClient {
    type Application = ();
    type Source = ();
    type AlternativeIndex = ();
    type EvidenceRule = ();
    type ObservedEvidence = ();
    type CheckedEvidence = ();
    type ProbeSemanticBranch = ();
    type SealedBranchValue = ();
    type Projection = ();
    type SourceErrorCause = ();
    type ClientInvariant = ();

    fn evidence_accepts(_: &Self::EvidenceRule, _: &Self::ObservedEvidence) -> bool {
        false
    }

    fn project_checked_evidence(_: &Self::ObservedEvidence, _: &TypeKind) -> Option<()> {
        Some(())
    }

    fn alternative_ordinal(_: &Self::AlternativeIndex) -> u32 {
        0
    }

    fn client_invariant_source(_: &Self::ClientInvariant) -> Self::Source {}

    fn empty_sealed_branch() -> Self::SealedBranchValue {}
}

/// Source shape selected by argument mapping before a candidate solution is
/// known.  A spread container is intentionally not a claimed item type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ConstraintSourceContainerPolicy {
    Positional,
    Named,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum PreparedConstraintSourceProjection {
    Scalar,
    InferSpreadContainer {
        policy: ConstraintSourceContainerPolicy,
    },
}

impl PreparedConstraintSourceProjection {
    pub(crate) const fn is_scalar(self) -> bool {
        matches!(self, Self::Scalar)
    }
}

/// Exact source projection derived after the callback's actual type is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedConstraintSourceProjection {
    Scalar,
    SpreadContainer(CheckedConstraintContainerConstructor),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedConstraintContainerConstructor {
    Vec,
    Seq,
    Slice,
    Array { len: ArrayLength },
    MapValue { kind: MapKind, key: Box<TypeKind> },
}

impl CheckedConstraintSourceProjection {
    /// Derive the only admissible checked constructor from the prepared source
    /// shape and the callback's actual type.  Returning `None` is a lower
    /// mismatch; callers cannot manufacture a constructor in a callback.
    pub(crate) fn derive(
        prepared: PreparedConstraintSourceProjection,
        actual: &TypeKind,
    ) -> Option<Self> {
        match prepared {
            PreparedConstraintSourceProjection::Scalar => Some(Self::Scalar),
            PreparedConstraintSourceProjection::InferSpreadContainer { policy } => {
                let constructor = match (policy, actual) {
                    (ConstraintSourceContainerPolicy::Positional, TypeKind::Vec(_)) => {
                        CheckedConstraintContainerConstructor::Vec
                    }
                    (ConstraintSourceContainerPolicy::Positional, TypeKind::Seq(_)) => {
                        CheckedConstraintContainerConstructor::Seq
                    }
                    (ConstraintSourceContainerPolicy::Positional, TypeKind::Slice(_)) => {
                        CheckedConstraintContainerConstructor::Slice
                    }
                    (ConstraintSourceContainerPolicy::Positional, TypeKind::Array { len, .. }) => {
                        CheckedConstraintContainerConstructor::Array { len: len.clone() }
                    }
                    (ConstraintSourceContainerPolicy::Named, TypeKind::Map { kind, key, .. }) => {
                        CheckedConstraintContainerConstructor::MapValue {
                            kind: *kind,
                            key: key.clone(),
                        }
                    }
                    _ => return None,
                };
                Some(Self::SpreadContainer(constructor))
            }
        }
    }

    /// Validate that this lower-derived constructor still names the exact
    /// materialized actual. Scalar projection is the identity for every type;
    /// spread constructors retain all container header evidence.
    pub(crate) fn matches_actual(&self, actual: &TypeKind) -> bool {
        match (self, actual) {
            (Self::Scalar, _) => true,
            (
                Self::SpreadContainer(CheckedConstraintContainerConstructor::Vec),
                TypeKind::Vec(_),
            )
            | (
                Self::SpreadContainer(CheckedConstraintContainerConstructor::Seq),
                TypeKind::Seq(_),
            )
            | (
                Self::SpreadContainer(CheckedConstraintContainerConstructor::Slice),
                TypeKind::Slice(_),
            ) => true,
            (
                Self::SpreadContainer(CheckedConstraintContainerConstructor::Array {
                    len: expected,
                }),
                TypeKind::Array { len: actual, .. },
            ) => expected == actual,
            (
                Self::SpreadContainer(CheckedConstraintContainerConstructor::MapValue {
                    kind: expected_kind,
                    key: expected_key,
                }),
                TypeKind::Map {
                    kind: actual_kind,
                    key: actual_key,
                    ..
                },
            ) => expected_kind == actual_kind && expected_key.as_ref() == actual_key.as_ref(),
            _ => false,
        }
    }

    /// Compose the selected value expectation with this already checked
    /// source constructor.  Array length, map kind, and map key are retained
    /// exactly; no callable-side reconstruction is possible.
    pub(crate) fn compose_expected(&self, value_expected: &TypeKind) -> TypeKind {
        match self {
            Self::Scalar => value_expected.clone(),
            Self::SpreadContainer(CheckedConstraintContainerConstructor::Vec) => {
                TypeKind::Vec(Box::new(value_expected.clone()))
            }
            Self::SpreadContainer(CheckedConstraintContainerConstructor::Seq) => {
                TypeKind::Seq(Box::new(value_expected.clone()))
            }
            Self::SpreadContainer(CheckedConstraintContainerConstructor::Slice) => {
                TypeKind::Slice(Box::new(value_expected.clone()))
            }
            Self::SpreadContainer(CheckedConstraintContainerConstructor::Array { len }) => {
                TypeKind::Array {
                    item: Box::new(value_expected.clone()),
                    len: len.clone(),
                }
            }
            Self::SpreadContainer(CheckedConstraintContainerConstructor::MapValue {
                kind,
                key,
            }) => TypeKind::Map {
                kind: *kind,
                key: key.clone(),
                value: Box::new(value_expected.clone()),
            },
        }
    }
}

/// One schema-keyed value alternative.  Evidence is held behind an `Arc` so
/// prepared constraints can be shared by every frontier row without imposing
/// a `Clone` bound on the semantic evidence itself.
pub(crate) struct PreparedSourceAlternative<D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    evidence: Arc<D::EvidenceRule>,
    value_expected: TypeKind,
}

impl<D: ConstraintDomain> PreparedSourceAlternative<D> {
    pub(crate) fn new(
        alternative: D::AlternativeIndex,
        evidence: D::EvidenceRule,
        value_expected: TypeKind,
    ) -> Self {
        Self {
            alternative,
            evidence: Arc::new(evidence),
            value_expected,
        }
    }

    pub(crate) const fn alternative(&self) -> D::AlternativeIndex {
        self.alternative
    }

    pub(crate) fn evidence(&self) -> &D::EvidenceRule {
        self.evidence.as_ref()
    }

    pub(crate) const fn value_expected(&self) -> &TypeKind {
        &self.value_expected
    }
}

impl<D: ConstraintDomain> Clone for PreparedSourceAlternative<D> {
    fn clone(&self) -> Self {
        Self {
            alternative: self.alternative,
            evidence: Arc::clone(&self.evidence),
            value_expected: self.value_expected.clone(),
        }
    }
}

/// A prepared source is the sole input to lower source probing.
pub(crate) enum PreparedSourceConstraint<D: ConstraintDomain> {
    Unchecked {
        source: D::Source,
        source_projection: PreparedConstraintSourceProjection,
    },
    Checked {
        source: D::Source,
        source_projection: PreparedConstraintSourceProjection,
        guarded: Arc<[PreparedSourceAlternative<D>]>,
        otherwise: PreparedSourceAlternative<D>,
    },
}

impl<D: ConstraintDomain> Clone for PreparedSourceConstraint<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Unchecked {
                source,
                source_projection,
            } => Self::Unchecked {
                source: *source,
                source_projection: *source_projection,
            },
            Self::Checked {
                source,
                source_projection,
                guarded,
                otherwise,
            } => Self::Checked {
                source: *source,
                source_projection: *source_projection,
                guarded: Arc::clone(guarded),
                otherwise: otherwise.clone(),
            },
        }
    }
}

impl<D: ConstraintDomain> PreparedSourceConstraint<D> {
    pub(crate) const fn unchecked(
        source: D::Source,
        source_projection: PreparedConstraintSourceProjection,
    ) -> Self {
        Self::Unchecked {
            source,
            source_projection,
        }
    }

    pub(crate) fn checked(
        source: D::Source,
        source_projection: PreparedConstraintSourceProjection,
        guarded: impl IntoIterator<Item = PreparedSourceAlternative<D>>,
        otherwise: PreparedSourceAlternative<D>,
    ) -> Result<Self, TypeConstraintError> {
        let prepared = Self::Checked {
            source,
            source_projection,
            guarded: Arc::from(guarded.into_iter().collect::<Vec<_>>()),
            otherwise,
        };
        prepared.validate()?;
        Ok(prepared)
    }

    /// Revalidate the closed shape at the lower boundary as well as at the
    /// public constructor.  Crate-private enum fields are intentionally not
    /// trusted as an alternate publication path.
    pub(crate) fn validate(&self) -> Result<(), TypeConstraintError> {
        let Self::Checked {
            source_projection,
            guarded,
            otherwise,
            ..
        } = self
        else {
            return Ok(());
        };
        let mut previous = None;
        for (position, alternative) in guarded.iter().chain(std::iter::once(otherwise)).enumerate()
        {
            if let Some(previous) = previous
                && previous >= alternative.alternative
            {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::PreparedSource(
                        if previous == alternative.alternative {
                            PreparedSourceConstraintInvariant::DuplicateCoordinate
                        } else {
                            PreparedSourceConstraintInvariant::Unordered
                        },
                    ),
                ));
            }
            previous = Some(alternative.alternative);

            let ordinal = D::alternative_ordinal(&alternative.alternative);
            let expected = u32::try_from(position).map_err(|_| {
                TypeConstraintError::Invariant(TypeConstraintInvariant::PreparedSource(
                    PreparedSourceConstraintInvariant::Unordered,
                ))
            })?;
            if ordinal != expected {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::PreparedSource(
                        PreparedSourceConstraintInvariant::Unordered,
                    ),
                ));
            }
        }
        if matches!(
            source_projection,
            PreparedConstraintSourceProjection::InferSpreadContainer { .. }
        ) && !guarded.is_empty()
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::PreparedSource(
                    PreparedSourceConstraintInvariant::SpreadPlan,
                ),
            ));
        }
        Ok(())
    }

    pub(crate) const fn source(&self) -> D::Source {
        match self {
            Self::Unchecked { source, .. } | Self::Checked { source, .. } => *source,
        }
    }

    pub(crate) const fn source_projection(&self) -> PreparedConstraintSourceProjection {
        match self {
            Self::Unchecked {
                source_projection, ..
            }
            | Self::Checked {
                source_projection, ..
            } => *source_projection,
        }
    }

    pub(crate) fn alternatives(
        &self,
    ) -> impl Clone + DoubleEndedIterator<Item = &PreparedSourceAlternative<D>> {
        let (guarded, otherwise) = match self {
            Self::Unchecked { .. } => (&[][..], None),
            Self::Checked {
                guarded, otherwise, ..
            } => (guarded.as_ref(), Some(otherwise)),
        };
        guarded.iter().chain(otherwise)
    }

    pub(crate) fn alternative(
        &self,
        index: D::AlternativeIndex,
    ) -> Option<&PreparedSourceAlternative<D>> {
        self.alternatives()
            .find(|alternative| alternative.alternative() == index)
    }

    pub(crate) const fn otherwise(&self) -> Option<&PreparedSourceAlternative<D>> {
        match self {
            Self::Unchecked { .. } => None,
            Self::Checked { otherwise, .. } => Some(otherwise),
        }
    }

    pub(crate) const fn is_unchecked(&self) -> bool {
        matches!(self, Self::Unchecked { .. })
    }
}

/// Expected information for one value alternative.  The source constructor is
/// retained separately so the callback cannot mistake it for an expected
/// container type.
pub(crate) struct SourceAlternativeHint<'h, D: ConstraintDomain> {
    alternative: D::AlternativeIndex,
    evidence: &'h D::EvidenceRule,
    value_expected: ProjectedExpectedHint<'h>,
    source_projection: PreparedConstraintSourceProjection,
}

impl<'h, D: ConstraintDomain> SourceAlternativeHint<'h, D> {
    pub(crate) const fn new(
        alternative: D::AlternativeIndex,
        evidence: &'h D::EvidenceRule,
        value_expected: ProjectedExpectedHint<'h>,
        source_projection: PreparedConstraintSourceProjection,
    ) -> Self {
        Self {
            alternative,
            evidence,
            value_expected,
            source_projection,
        }
    }

    pub(crate) const fn alternative(&self) -> D::AlternativeIndex {
        self.alternative
    }

    pub(crate) const fn evidence(&self) -> &'h D::EvidenceRule {
        self.evidence
    }

    pub(crate) const fn value_expected(&self) -> &ProjectedExpectedHint<'h> {
        &self.value_expected
    }

    pub(crate) const fn source_projection(&self) -> PreparedConstraintSourceProjection {
        self.source_projection
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectedExpectedHint<'h> {
    Complete(&'h TypeKind),
    Parametric {
        expected: &'h TypeKind,
        unbound: &'h [super::ConstraintGenericParameterId],
        scope_lease: &'h super::ImportedGenericParameterScopeLease,
    },
}

/// Callback hint.  For checked sources every alternative is visible at once;
/// the lower owner retains the schema key and callback selection is validated
/// against it after the callback returns.
pub(crate) enum ExpectedHint<'h, D: ConstraintDomain> {
    Unchecked,
    Alternatives(&'h [SourceAlternativeHint<'h, D>]),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SourcePhase {
    Probe,
    Materialize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceError<S, C> {
    source: S,
    phase: SourcePhase,
    cause: C,
}

impl<S, C> SourceError<S, C> {
    pub(crate) fn new(source: S, phase: SourcePhase, cause: C) -> Self {
        Self {
            source,
            phase,
            cause,
        }
    }

    pub(crate) const fn source(&self) -> &S {
        &self.source
    }

    pub(crate) const fn phase(&self) -> SourcePhase {
        self.phase
    }

    pub(crate) const fn cause(&self) -> &C {
        &self.cause
    }

    pub(crate) fn into_parts(self) -> (S, SourcePhase, C) {
        (self.source, self.phase, self.cause)
    }
}

/// Which prepared schema row a checked callback selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceProbeSelection<A, E> {
    Unchecked,
    Checked { alternative: A, evidence: E },
}

/// Probe result carrying only actual type, semantic branch, and typed evidence.
/// Expected types and source constructors never cross the callback boundary.
pub(crate) struct SourceProbeResult<D: ConstraintDomain> {
    actual: SourceProbeTerm<D>,
    canonical_branch: Arc<D::ProbeSemanticBranch>,
    selection: SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
}

pub(crate) enum SourceProbeTerm<D: ConstraintDomain> {
    Type(TypeKind),
    Result(super::transaction::ConstraintResultProjection<D>),
}

impl<D: ConstraintDomain> SourceProbeResult<D> {
    pub(crate) fn unchecked(actual: TypeKind, canonical_branch: D::ProbeSemanticBranch) -> Self {
        Self {
            actual: SourceProbeTerm::Type(actual),
            canonical_branch: Arc::new(canonical_branch),
            selection: SourceProbeSelection::Unchecked,
        }
    }

    pub(crate) fn checked(
        actual: TypeKind,
        canonical_branch: D::ProbeSemanticBranch,
        alternative: D::AlternativeIndex,
        evidence: D::ObservedEvidence,
    ) -> Self {
        Self {
            actual: SourceProbeTerm::Type(actual),
            canonical_branch: Arc::new(canonical_branch),
            selection: SourceProbeSelection::Checked {
                alternative,
                evidence: Arc::new(evidence),
            },
        }
    }

    pub(crate) fn projection(
        result: super::transaction::ConstraintResultProjection<D>,
        canonical_branch: Arc<D::ProbeSemanticBranch>,
        selection: SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
    ) -> Self {
        Self {
            actual: SourceProbeTerm::Result(result),
            canonical_branch,
            selection,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        SourceProbeTerm<D>,
        Arc<D::ProbeSemanticBranch>,
        SourceProbeSelection<D::AlternativeIndex, Arc<D::ObservedEvidence>>,
    ) {
        (self.actual, self.canonical_branch, self.selection)
    }
}

pub(crate) enum SourceProbeOutcome<D: ConstraintDomain> {
    Accepted(super::transaction::SourceProbeContribution<D>),
    Rejected(D::SourceErrorCause),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MaterializationOutcome<S, V, C> {
    Sealed(V),
    Rejected { source: S, cause: C },
}

/// Exact closed lower row supplied to final materialization.  Checked rows
/// carry every semantic choice and lower-derived type; unchecked rows retain
/// the physical projection and actual type but no alternative or expected
/// type.
pub(crate) struct MaterializedSourceRequest<'h, D: ConstraintDomain> {
    component: &'h super::CompletedConstraintComponent<D>,
    source: &'h super::ClosedConstraintProbe<D>,
}

/// The exact child application result that supplied a parent source's actual
/// type. Both references come from the same completed component; callers do
/// not reconstruct a child from its source expression or result schema.
pub(crate) struct CompletedResultProjectionView<'h, D: ConstraintDomain> {
    component: &'h super::CompletedConstraintComponent<D>,
    application_id: D::Application,
    application: &'h super::CompletedConstraintApplication<D::Application, D::Projection>,
    projection: &'h super::KeyedConstraintProjection<D::Application, D::Projection>,
}

impl<'h, D: ConstraintDomain> CompletedResultProjectionView<'h, D> {
    pub(crate) const fn application_id(&self) -> D::Application {
        self.application_id
    }

    pub(crate) const fn application(
        &self,
    ) -> &'h super::CompletedConstraintApplication<D::Application, D::Projection> {
        self.application
    }

    pub(crate) const fn projection(
        &self,
    ) -> &'h super::KeyedConstraintProjection<D::Application, D::Projection> {
        self.projection
    }

    /// A value-use conversion retains the result it consumed. Both ports are
    /// resolved within this same closed component, including when Call and
    /// Specialize share an expression owner.
    pub(crate) fn source(&self) -> Option<Self> {
        let source = self.projection.source()?;
        self.component.projection(source.application, &source.key)
    }
}

impl<D: ConstraintDomain> super::CompletedConstraintComponent<D> {
    pub(crate) fn projection(
        &self,
        application_id: D::Application,
        key: &D::Projection,
    ) -> Option<CompletedResultProjectionView<'_, D>> {
        let application = self.application(application_id)?;
        let projection = application
            .projections()
            .iter()
            .find(|value| value.key() == key)?;
        Some(CompletedResultProjectionView {
            component: self,
            application_id,
            application,
            projection,
        })
    }
}

impl<'h, D: ConstraintDomain> MaterializedSourceRequest<'h, D> {
    pub(super) fn from_component(
        component: &'h super::CompletedConstraintComponent<D>,
        ordinal: usize,
    ) -> Option<Self> {
        Some(Self {
            component,
            source: component.sources().all().get(ordinal)?,
        })
    }

    pub(crate) const fn component(&self) -> &'h super::CompletedConstraintComponent<D> {
        self.component
    }

    /// The owner of this source can be a child of the component's selected
    /// application. It is obtained from the sealed trace, never from a caller.
    pub(crate) fn application_id(&self) -> D::Application {
        self.component
            .sources()
            .domain_application(self.source.source().application())
    }

    pub(crate) fn application(
        &self,
    ) -> &'h super::CompletedConstraintApplication<D::Application, D::Projection> {
        self.component
            .application(self.application_id())
            .expect("completed source retains its admitted application")
    }

    pub(crate) const fn source(&self) -> &ConstraintSourceId<D::Source> {
        self.source.source_ref()
    }

    pub(crate) const fn expected(&self) -> Option<&'h TypeKind> {
        self.source.final_expected()
    }

    pub(crate) const fn alternative(&self) -> Option<D::AlternativeIndex> {
        self.source.selection().alternative()
    }

    pub(crate) fn evidence(&self) -> Option<&'h D::CheckedEvidence> {
        self.source.selection().evidence()
    }

    pub(crate) const fn actual(&self) -> &'h TypeKind {
        self.source.actual()
    }

    pub(crate) const fn source_projection(&self) -> &'h CheckedConstraintSourceProjection {
        self.source.source_projection()
    }

    pub(crate) fn canonical_branch(&self) -> &'h D::ProbeSemanticBranch {
        self.source.branch()
    }

    /// Returns the exact completed child result which produced this source,
    /// when the source was observed from a nested call result.
    pub(crate) fn result_projection(&self) -> Option<CompletedResultProjectionView<'h, D>> {
        let origin = self.source.result_origin()?;
        let application_id = self
            .component
            .sources()
            .domain_application(origin.application());
        self.component.projection(application_id, origin.key())
    }
}
