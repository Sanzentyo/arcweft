//! Source-validated containment of retained postfix interpretations.
//!
//! This proof is issued by the attached-to-HIR candidate traversal. It does not
//! reconstruct membership from synthetic key ordinals or source spans.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_syntax::incremental::ParseStatus;
use thiserror::Error;

use crate::expr::HirExpressionChildRole;
use crate::identity::{ExprId, SyntheticOwner};

/// Semantic interpretation retained by one postfix choice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPostfixInterpretation {
    Index,
    Dialogue,
}

impl HirPostfixInterpretation {
    pub const fn edge_role(self) -> HirExpressionChildRole {
        match self {
            Self::Index => HirExpressionChildRole::PostfixIndexCandidate,
            Self::Dialogue => HirExpressionChildRole::PostfixDialogueCandidate,
        }
    }
}

/// One exact interpretation region in a source-validated module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCandidateRegion {
    selector: ExprId,
    root: ExprId,
    parent: Option<ExprId>,
    interpretation: HirPostfixInterpretation,
    recovery: ParseStatus,
}

impl HirCandidateRegion {
    pub const fn selector(&self) -> ExprId {
        self.selector
    }
    pub const fn root(&self) -> ExprId {
        self.root
    }
    pub const fn parent(&self) -> Option<ExprId> {
        self.parent
    }
    pub const fn interpretation(&self) -> HirPostfixInterpretation {
        self.interpretation
    }
    pub const fn recovery_status(&self) -> ParseStatus {
        self.recovery
    }
}

/// Complete, immutable candidate containment proof for all descendant arenas.
/// Synthetic source identities and their aggregate budget remain unchanged.
#[derive(Debug, Eq, PartialEq)]
pub struct HirCandidateProvenance {
    regions: BTreeMap<ExprId, HirCandidateRegion>,
    owners: BTreeMap<SyntheticOwner, ExprId>,
}

/// A required enclosing interpretation lacks an exact accepted decision.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HirCandidateSelectionError {
    #[error("owner {owner:?} is outside the interpretation region of {boundary:?}")]
    OutsideRegion {
        owner: SyntheticOwner,
        boundary: SyntheticOwner,
    },
    #[error("postfix selector {selector:?} has no accepted interpretation")]
    Missing { selector: ExprId },
    #[error("postfix selector {selector:?} cannot select candidate {candidate:?}")]
    Invalid { selector: ExprId, candidate: ExprId },
}

impl HirCandidateProvenance {
    /// Completes source freeze with owners admitted by an exact desugaring
    /// validator. The caller supplies validated producer relations, not keys
    /// inferred from arbitrary synthetic slots.
    pub(in crate::source_index) fn with_derived_owners(
        mut self,
        owners: impl IntoIterator<Item = (SyntheticOwner, SyntheticOwner)>,
    ) -> Option<Self> {
        for (owner, producer) in owners {
            if owner.module() != producer.module() || self.owners.contains_key(&owner) {
                return None;
            }
            if let Some(region) = self.owners.get(&producer).copied() {
                self.owners.insert(owner, region);
            }
        }
        Some(self)
    }
    /// Evaluates the interpretation context of an already resolved arena owner.
    /// Enclosing choices are checked first, so an unselected outer region never
    /// demands decisions from an unvisited inner interpretation. This query
    /// does not establish arena liveness or a final executable program.
    pub fn selects_region(
        &self,
        owner: SyntheticOwner,
        selected: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<bool, HirCandidateSelectionError> {
        self.selects_region_below(owner, None, selected)
    }

    /// Selects descendants assuming the boundary's own interpretation context.
    /// This is local probe evidence; accepting the boundary and its enclosing
    /// choices remains the final selected program's responsibility.
    pub fn selects_region_within(
        &self,
        owner: SyntheticOwner,
        boundary: SyntheticOwner,
        selected: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<bool, HirCandidateSelectionError> {
        self.selects_region_below(owner, Some(boundary), selected)
    }

    fn selects_region_below(
        &self,
        owner: SyntheticOwner,
        boundary: Option<SyntheticOwner>,
        mut selected: impl FnMut(ExprId) -> Option<ExprId>,
    ) -> Result<bool, HirCandidateSelectionError> {
        let stop = boundary
            .and_then(|owner| self.owner_region(owner))
            .map(HirCandidateRegion::root);
        let mut regions = Vec::new();
        let mut next = self.owner_region(owner);
        while next.map(HirCandidateRegion::root) != stop {
            let Some(region) = next else {
                return Err(HirCandidateSelectionError::OutsideRegion {
                    owner,
                    boundary: boundary.expect("a region boundary is present"),
                });
            };
            regions.push(region);
            next = region.parent.and_then(|root| self.region(root));
        }
        for region in regions.into_iter().rev() {
            let candidate =
                selected(region.selector).ok_or(HirCandidateSelectionError::Missing {
                    selector: region.selector,
                })?;
            if self
                .region(candidate)
                .is_none_or(|chosen| chosen.selector != region.selector)
            {
                return Err(HirCandidateSelectionError::Invalid {
                    selector: region.selector,
                    candidate,
                });
            }
            if candidate != region.root {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Immediate interpretation owning this arena value; shared targets belong
    /// to their enclosing region and can be unconditional.
    pub fn owner_region(&self, owner: SyntheticOwner) -> Option<&HirCandidateRegion> {
        self.owners
            .get(&owner)
            .and_then(|root| self.regions.get(root))
    }

    pub fn region(&self, root: ExprId) -> Option<&HirCandidateRegion> {
        self.regions.get(&root)
    }

    pub fn regions(&self) -> impl ExactSizeIterator<Item = &HirCandidateRegion> {
        self.regions.values()
    }

    pub fn owners(&self) -> impl ExactSizeIterator<Item = (SyntheticOwner, ExprId)> + '_ {
        self.owners.iter().map(|(owner, root)| (*owner, *root))
    }

    pub fn contains(&self, owner: SyntheticOwner) -> bool {
        self.owners.contains_key(&owner)
    }
}

/// The candidate source validator's admission ledger becomes the published
/// proof. There is no independent family membership catalog after sealing.
pub(in crate::source_index) struct CandidateProvenanceBuilder {
    proof: HirCandidateProvenance,
}

impl Default for CandidateProvenanceBuilder {
    fn default() -> Self {
        Self {
            proof: HirCandidateProvenance {
                regions: BTreeMap::new(),
                owners: BTreeMap::new(),
            },
        }
    }
}

impl CandidateProvenanceBuilder {
    pub(in crate::source_index) fn register(
        &mut self,
        selector: ExprId,
        root: ExprId,
        parent: Option<ExprId>,
        interpretation: HirPostfixInterpretation,
        recovery: ParseStatus,
    ) -> Option<()> {
        if selector == root
            || selector.module() != root.module()
            || parent.is_some_and(|parent| !self.proof.regions.contains_key(&parent))
            || self
                .proof
                .owners
                .get(&SyntheticOwner::Expr(selector))
                .copied()
                != parent
        {
            return None;
        }
        self.proof
            .regions
            .insert(
                root,
                HirCandidateRegion {
                    selector,
                    root,
                    parent,
                    interpretation,
                    recovery,
                },
            )
            .is_none()
            .then_some(())
    }

    pub(in crate::source_index) fn admit(&mut self, owner: SyntheticOwner, region: ExprId) -> bool {
        self.proof.regions.contains_key(&region)
            && owner.module() == region.module()
            && self.proof.owners.insert(owner, region).is_none()
    }

    pub(in crate::source_index) fn owners(&self) -> impl Iterator<Item = SyntheticOwner> + '_ {
        self.proof.owners.keys().copied()
    }

    pub(in crate::source_index) fn seal(self) -> Option<HirCandidateProvenance> {
        let mut choices = BTreeMap::<ExprId, BTreeSet<HirPostfixInterpretation>>::new();
        for region in self.proof.regions.values() {
            if self.proof.owners.get(&SyntheticOwner::Expr(region.root)) != Some(&region.root)
                || self
                    .proof
                    .owners
                    .get(&SyntheticOwner::Expr(region.selector))
                    .copied()
                    != region.parent
                || !choices
                    .entry(region.selector)
                    .or_default()
                    .insert(region.interpretation)
            {
                return None;
            }
        }
        choices
            .values()
            .all(|interpretations| interpretations.len() == 2)
            .then_some(self.proof)
    }
}
