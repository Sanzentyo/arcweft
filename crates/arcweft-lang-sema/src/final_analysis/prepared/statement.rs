//! Affine statement evidence shared by early reachability and the late seal.

use arcweft_lang_hir::{
    dialogue_application::HirDialogueMarkId,
    identity::{ExprId, ItemId, LocalId, StmtId},
    symbol::{CallableDeclarationKey, FlowDeclarationId},
};

use crate::{
    final_analysis::{
        CheckedAssertionDisposition, CheckedEvaluatedEffectReference, CheckedIteration,
        CheckedSuspensionStatement,
    },
    types::{CanonicalEffects, PreparedEffects, SemanticTypeDigest, TypeEffectDomain, TypeKind},
};

use super::PreparedEvaluatedEffect;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedRegistryWrite {
    pub(crate) address: std::sync::Arc<crate::final_analysis::CheckedRegistryAddress>,
    pub(crate) value: ExprId,
}

/// One typed assignment awaiting the final target place seal.
///
/// This evidence is affine: the all-statement seal consumes it while joining
/// the already checked local, target, value, and nominal field projection.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedAssignmentStatement {
    target: ExprId,
    value: ExprId,
    local: LocalId,
}

impl PreparedAssignmentStatement {
    pub(crate) const fn new(target: ExprId, value: ExprId, local: LocalId) -> Self {
        Self {
            target,
            value,
            local,
        }
    }

    pub(super) const fn target(&self) -> ExprId {
        self.target
    }

    pub(crate) fn into_parts(self) -> (ExprId, ExprId, LocalId) {
        (self.target, self.value, self.local)
    }
}

/// Affine analyzer output consumed exactly once by the exhaustive all-31
/// statement seal.
///
/// No variant embeds a final [`super::super::CheckedStatementPayload`].
/// `HirOwned` merely proves that no earlier specialized pass owns the payload;
/// only the late exhaustive HIR match may select its final meaning.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedStatementPayload<D: TypeEffectDomain = PreparedEffects> {
    HirOwned,
    /// Issued by the late statement producer before the effect fold enters
    /// latent bodies, while the complete callable fact inventory is available.
    Return(crate::final_analysis::CheckedCallableBoundary),
    Assignment(PreparedAssignmentStatement),
    RegistryWrite(PreparedRegistryWrite),
    Assertion(CheckedAssertionDisposition),
    Iteration(Box<CheckedIteration<D>>),
    Suspension(Box<CheckedSuspensionStatement>),
    Yield,
    EvaluatedEffect(PreparedEvaluatedEffect),
    SealedEvaluatedEffectReference(CheckedEvaluatedEffectReference),
}

impl<D: TypeEffectDomain> PreparedStatementPayload<D> {
    /// A place target or a storage operand remains a runtime type root even
    /// when it does not own an ordinary expression evaluation.
    pub(in crate::final_analysis) fn storage_type_source(&self) -> Option<ExprId> {
        match self {
            Self::Assignment(assignment) => Some(assignment.target()),
            Self::RegistryWrite(write) => Some(write.value),
            _ => None,
        }
    }

    pub(crate) fn visit_types<'a, E>(
        &'a self,
        visitor: &mut impl FnMut(&'a TypeKind<D>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Iteration(value) => value.visit_types(visitor),
            _ => Ok(()),
        }
    }
}

impl PreparedStatementPayload {
    pub(in crate::final_analysis) fn seal_types<E>(
        self,
        map: impl FnMut(TypeKind<PreparedEffects>) -> Result<TypeKind, E>,
    ) -> Result<PreparedStatementPayload<CanonicalEffects>, E> {
        Ok(match self {
            Self::HirOwned => PreparedStatementPayload::HirOwned,
            Self::Return(value) => PreparedStatementPayload::Return(value),
            Self::Assignment(value) => PreparedStatementPayload::Assignment(value),
            Self::RegistryWrite(value) => PreparedStatementPayload::RegistryWrite(value),
            Self::Assertion(value) => PreparedStatementPayload::Assertion(value),
            Self::Iteration(value) => {
                PreparedStatementPayload::Iteration(Box::new(value.try_map_types(map)?))
            }
            Self::Suspension(value) => PreparedStatementPayload::Suspension(value),
            Self::Yield => PreparedStatementPayload::Yield,
            Self::EvaluatedEffect(value) => PreparedStatementPayload::EvaluatedEffect(value),
            Self::SealedEvaluatedEffectReference(value) => {
                PreparedStatementPayload::SealedEvaluatedEffectReference(value)
            }
        })
    }
}

/// Exact Include edge resolved once against the accepted symbol generation.
///
/// The executable-ingress worklist borrows the source/target edge. The late
/// statement seal consumes the same proof to construct the final Include
/// payload; there is no second resolver or copied digest side table.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedIncludeFlowProof {
    statement: StmtId,
    source: CallableDeclarationKey,
    target: FlowDeclarationId,
}

impl PreparedIncludeFlowProof {
    pub(crate) const fn new(
        statement: StmtId,
        source: CallableDeclarationKey,
        target: FlowDeclarationId,
    ) -> Self {
        Self {
            statement,
            source,
            target,
        }
    }

    pub(crate) const fn statement(&self) -> StmtId {
        self.statement
    }

    pub(crate) const fn source(&self) -> &CallableDeclarationKey {
        &self.source
    }

    pub(crate) const fn target(&self) -> &FlowDeclarationId {
        &self.target
    }

    pub(crate) fn into_target(self) -> FlowDeclarationId {
        self.target
    }
}

/// Statement-scoped proof that one exact Event type is contributed by every
/// accepted stateful Entry root reaching its declaration.
///
/// Contributors are retained only until the final Entry comparison. The
/// proof cannot be cloned or published in `FinalSemanticAnalysis`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedEventScrutineeProof {
    statement: StmtId,
    event_digest: SemanticTypeDigest,
    contributors: Box<[ItemId]>,
}

impl PreparedEventScrutineeProof {
    pub(crate) fn new(
        statement: StmtId,
        event_digest: SemanticTypeDigest,
        contributors: Box<[ItemId]>,
    ) -> Self {
        Self {
            statement,
            event_digest,
            contributors,
        }
    }

    pub(crate) const fn statement(&self) -> StmtId {
        self.statement
    }

    pub(crate) const fn event_digest(&self) -> SemanticTypeDigest {
        self.event_digest
    }

    pub(crate) const fn contributors(&self) -> &[ItemId] {
        &self.contributors
    }

    #[cfg(test)]
    pub(crate) fn into_parts(self) -> (StmtId, SemanticTypeDigest, Box<[ItemId]>) {
        (self.statement, self.event_digest, self.contributors)
    }
}

/// Completed non-child Trigger meaning after every contextual child check.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedTriggerScrutineeProof {
    Input,
    InputAction(arcweft_interaction_model::input::InputActionId),
    Event,
    Signal,
    Timeout,
    Mark(HirDialogueMarkId),
    Select,
    Task,
    Scope,
    Expression,
}

/// Completed source-ordered Select head family.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedSelectBranchHeadProof {
    Bind,
    Frame,
    Event,
}

/// Completed non-child Select meaning after every contextual child check.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedSelectScrutineeProof {
    Operand,
    Branches(Box<[PreparedSelectBranchHeadProof]>),
}

/// Affine statement-specific result of the contextual scrutinee transaction.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedStatementScrutineeProof {
    Trigger(PreparedTriggerScrutineeProof),
    Select(PreparedSelectScrutineeProof),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_branch_proof_retains_source_order_without_an_ordinal_side_field() {
        let proof = PreparedSelectScrutineeProof::Branches(
            vec![
                PreparedSelectBranchHeadProof::Event,
                PreparedSelectBranchHeadProof::Bind,
                PreparedSelectBranchHeadProof::Frame,
            ]
            .into_boxed_slice(),
        );

        let PreparedSelectScrutineeProof::Branches(heads) = proof else {
            panic!("branch proof retains the branch family")
        };
        assert!(matches!(heads[0], PreparedSelectBranchHeadProof::Event));
        assert!(matches!(heads[1], PreparedSelectBranchHeadProof::Bind));
        assert!(matches!(heads[2], PreparedSelectBranchHeadProof::Frame));
    }
}
