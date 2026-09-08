//! Source trace transitions from probing to normalized materialization evidence.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    effect_row::EffectSubstitution,
    types::{ArrayLength, GenericConstReference, GenericTypeReference, TypeKind},
};

use super::super::context::{TypeConstraintAccounting, TypeConstraintContext};

#[cfg(test)]
mod tests;
use super::{
    CheckedConstraintSourceProjection, ConstraintClosurePolicy, ConstraintDomain,
    PreparedConstraintSourceProjection, StoredSourceSelection, TypeConstraintError,
    TypeConstraintSourceProtocolInvariant, project_type, protocol_error,
};

pub(in crate::types::constraints) struct ActiveConstraintProbe<D: ConstraintDomain> {
    pub(super) source: D::Source,
    pub(super) source_ordinal: u32,
    pub(super) branch: Arc<D::ProbeSemanticBranch>,
    pub(super) selection: StoredSourceSelection<D>,
    pub(super) prepared_source_projection: PreparedConstraintSourceProjection,
    pub(super) value_expected: Option<TypeKind>,
    pub(super) actual: TypeKind,
}

/// The selected source's type and constant references are closed against the
/// candidate application. Declaration-Free references and function-local
/// binders retain their own owners. Residual callee quantifiers belong to the
/// continuation result, not to an already evaluated operand.
pub(crate) struct ClosedConstraintProbe<D: ConstraintDomain> {
    source: D::Source,
    source_ordinal: u32,
    branch: Arc<D::ProbeSemanticBranch>,
    selection: ClosedSourceSelection<D>,
    prepared_source_projection: PreparedConstraintSourceProjection,
    actual: TypeKind,
    source_projection: CheckedConstraintSourceProjection,
}

pub(crate) enum ClosedSourceSelection<D: ConstraintDomain> {
    Unchecked,
    Checked {
        alternative: D::AlternativeIndex,
        evidence: Arc<D::CheckedEvidence>,
        expected: TypeKind,
    },
}

pub(in crate::types::constraints) enum ConstraintProbe<D: ConstraintDomain> {
    Active(ActiveConstraintProbe<D>),
    Closed(ClosedConstraintProbe<D>),
}

impl<D: ConstraintDomain> ActiveConstraintProbe<D> {
    fn close<A: TypeConstraintAccounting>(
        self,
        bindings: &BTreeMap<GenericTypeReference, TypeKind>,
        const_bindings: &BTreeMap<GenericConstReference, ArrayLength>,
        effects: &EffectSubstitution,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ClosedConstraintProbe<D>, TypeConstraintError> {
        let mut project = |ty: &TypeKind| {
            project_type(
                ty,
                bindings,
                const_bindings,
                ConstraintClosurePolicy::ProjectionClosed,
                context,
            )?
            .value
            .substitute_effect_rows(effects)
            .map_err(|_| {
                super::super::effect_invariant(
                    super::super::TypeConstraintEffectInvariantKind::NonCanonicalInherited,
                    None,
                )
            })
        };
        let actual = project(&self.actual)?;
        let source_projection =
            CheckedConstraintSourceProjection::derive(self.prepared_source_projection, &actual)
                .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::Outcome))?;
        let selection = match (self.selection, self.value_expected) {
            (StoredSourceSelection::Unchecked, None) => ClosedSourceSelection::Unchecked,
            (
                StoredSourceSelection::Checked {
                    alternative,
                    evidence,
                },
                Some(expected),
            ) => {
                let value_expected = project(&expected)?;
                let evidence =
                    D::project_checked_evidence(evidence.as_ref(), &actual).ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::InvalidEvidence)
                    })?;
                ClosedSourceSelection::Checked {
                    alternative,
                    evidence: Arc::new(evidence),
                    expected: source_projection.compose_expected(&value_expected),
                }
            }
            _ => {
                return Err(protocol_error(
                    TypeConstraintSourceProtocolInvariant::Outcome,
                ));
            }
        };
        Ok(ClosedConstraintProbe {
            source: self.source,
            source_ordinal: self.source_ordinal,
            branch: self.branch,
            selection,
            prepared_source_projection: self.prepared_source_projection,
            actual,
            source_projection,
        })
    }
}

impl<D: ConstraintDomain> ConstraintProbe<D> {
    pub(super) fn close<A: TypeConstraintAccounting>(
        self,
        bindings: &BTreeMap<GenericTypeReference, TypeKind>,
        const_bindings: &BTreeMap<GenericConstReference, ArrayLength>,
        effects: &EffectSubstitution,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<ClosedConstraintProbe<D>, TypeConstraintError> {
        match self {
            Self::Active(probe) => probe.close(bindings, const_bindings, effects, context),
            Self::Closed(_) => Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            )),
        }
    }

    pub(super) fn closed(&self) -> Result<&ClosedConstraintProbe<D>, TypeConstraintError> {
        match self {
            Self::Closed(probe) => Ok(probe),
            Self::Active(_) => Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            )),
        }
    }

    pub(super) fn into_closed(self) -> Result<ClosedConstraintProbe<D>, TypeConstraintError> {
        match self {
            Self::Closed(probe) => Ok(probe),
            Self::Active(_) => Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            )),
        }
    }

    pub(super) const fn source(&self) -> D::Source {
        match self {
            Self::Active(probe) => probe.source,
            Self::Closed(probe) => probe.source,
        }
    }

    pub(super) const fn ordinal(&self) -> u32 {
        match self {
            Self::Active(probe) => probe.source_ordinal,
            Self::Closed(probe) => probe.source_ordinal,
        }
    }
}

impl<D: ConstraintDomain> ClosedConstraintProbe<D> {
    pub(crate) const fn source(&self) -> D::Source {
        self.source
    }
    pub(super) const fn ordinal(&self) -> u32 {
        self.source_ordinal
    }
    pub(super) fn branch(&self) -> &Arc<D::ProbeSemanticBranch> {
        &self.branch
    }
    pub(crate) const fn actual(&self) -> &TypeKind {
        &self.actual
    }
    pub(crate) const fn final_expected(&self) -> Option<&TypeKind> {
        match &self.selection {
            ClosedSourceSelection::Unchecked => None,
            ClosedSourceSelection::Checked { expected, .. } => Some(expected),
        }
    }
    pub(crate) const fn selection(&self) -> &ClosedSourceSelection<D> {
        &self.selection
    }
    pub(crate) const fn prepared_source_projection(&self) -> PreparedConstraintSourceProjection {
        self.prepared_source_projection
    }
    pub(crate) const fn source_projection(&self) -> &CheckedConstraintSourceProjection {
        &self.source_projection
    }
}

impl<D: ConstraintDomain> ClosedSourceSelection<D> {
    pub(crate) const fn is_unchecked(&self) -> bool {
        matches!(self, Self::Unchecked)
    }
    pub(crate) const fn alternative(&self) -> Option<D::AlternativeIndex> {
        match self {
            Self::Unchecked => None,
            Self::Checked { alternative, .. } => Some(*alternative),
        }
    }
    pub(crate) fn evidence(&self) -> Option<&D::CheckedEvidence> {
        match self {
            Self::Unchecked => None,
            Self::Checked { evidence, .. } => Some(evidence.as_ref()),
        }
    }
}

impl<D: ConstraintDomain> Clone for ActiveConstraintProbe<D> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            source_ordinal: self.source_ordinal,
            branch: Arc::clone(&self.branch),
            selection: self.selection.clone(),
            prepared_source_projection: self.prepared_source_projection,
            value_expected: self.value_expected.clone(),
            actual: self.actual.clone(),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ActiveConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_ordinal == other.source_ordinal
            && self.branch == other.branch
            && self.selection == other.selection
            && self.prepared_source_projection == other.prepared_source_projection
            && self.value_expected == other.value_expected
            && self.actual == other.actual
    }
}

impl<D: ConstraintDomain> Eq for ActiveConstraintProbe<D> {}

impl<D: ConstraintDomain> Clone for ClosedSourceSelection<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Unchecked => Self::Unchecked,
            Self::Checked {
                alternative,
                evidence,
                expected,
            } => Self::Checked {
                alternative: *alternative,
                evidence: Arc::clone(evidence),
                expected: expected.clone(),
            },
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ClosedSourceSelection<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unchecked, Self::Unchecked) => true,
            (
                Self::Checked {
                    alternative: left,
                    evidence: left_evidence,
                    expected: left_expected,
                },
                Self::Checked {
                    alternative: right,
                    evidence: right_evidence,
                    expected: right_expected,
                },
            ) => {
                left == right && left_evidence == right_evidence && left_expected == right_expected
            }
            _ => false,
        }
    }
}

impl<D: ConstraintDomain> Eq for ClosedSourceSelection<D> {}

impl<D: ConstraintDomain> Clone for ClosedConstraintProbe<D> {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            source_ordinal: self.source_ordinal,
            branch: Arc::clone(&self.branch),
            selection: self.selection.clone(),
            prepared_source_projection: self.prepared_source_projection,
            actual: self.actual.clone(),
            source_projection: self.source_projection.clone(),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ClosedConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.source_ordinal == other.source_ordinal
            && self.branch == other.branch
            && self.selection == other.selection
            && self.prepared_source_projection == other.prepared_source_projection
            && self.actual == other.actual
            && self.source_projection == other.source_projection
    }
}

impl<D: ConstraintDomain> Eq for ClosedConstraintProbe<D> {}

impl<D: ConstraintDomain> Clone for ConstraintProbe<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Active(probe) => Self::Active(probe.clone()),
            Self::Closed(probe) => Self::Closed(probe.clone()),
        }
    }
}

impl<D: ConstraintDomain> PartialEq for ConstraintProbe<D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Active(left), Self::Active(right)) => left == right,
            (Self::Closed(left), Self::Closed(right)) => left == right,
            _ => false,
        }
    }
}

impl<D: ConstraintDomain> Eq for ConstraintProbe<D> {}
