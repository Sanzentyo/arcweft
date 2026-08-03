//! Candidate-only typed `TypeRef` lowering for E34 interpretations.

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::AttachedCandidateNode;
use arcweft_lang_syntax::types::TypeRefNodeStep;

use crate::expr::HirPoisonState;
use crate::identity::{ScopeId, SyntheticKey, SyntheticOwner, TypeId};
use crate::lower::{HirInvariantFailure, HirLowerFailure};
use crate::source_index::HirSourceSite;
use crate::type_ref::HirType;

use super::CandidateCursor;
use crate::final_lowering::StagedHirModuleTransaction;

impl StagedHirModuleTransaction<'_> {
    /// Lowers one parser-retained candidate type without assigning a syntax ID.
    ///
    /// Type ordinals are an independent preorder for the interpretation's
    /// `(SyntheticKey, HirIdKind::Type)` entries. The source-backed E34 owner
    /// and interpretation role stay identical to the candidate expressions.
    pub(crate) fn lower_candidate_type(
        &mut self,
        node: AttachedCandidateNode<'_>,
        scope: ScopeId,
        cursor: &mut CandidateCursor,
    ) -> Result<TypeId, HirLowerFailure> {
        let projection = node
            .type_projection()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let ordinal = cursor.take_type_ordinal()?;
        let key = SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner), cursor.role, ordinal)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let source = HirSourceSite::from_attached_span(
            self.request.source().document(),
            &node.source_span(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation =
            self.arenas
                .types()
                .reserve_synthetic(&mut self.slots, key, source.clone())?;
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            let retained = self
                .arenas
                .types()
                .resolve_staged(&self.slots, owner)
                .map_err(HirLowerFailure::from)?;
            if retained.scope() != scope {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            return Ok(owner);
        }

        let mut children = BTreeMap::<TypeRefNodeStep, TypeId>::new();
        for child in node.direct_semantic_type_children() {
            let id = self.lower_candidate_type(child.node(), scope, cursor)?;
            if children.insert(child.step(), id).is_some() {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
        }

        let (kind, state) = self.project_type(owner, projection.value(), &children)?;
        let poisoned = matches!(state, HirPoisonState::Poisoned(_));
        let payload = HirType::try_new(owner, kind, scope, state, self)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if poisoned {
            self.stage_candidate_recovery_diagnostic(SyntheticOwner::Type(owner), source);
        }
        self.arenas
            .types()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }
}
