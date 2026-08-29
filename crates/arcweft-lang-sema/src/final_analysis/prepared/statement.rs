//! Affine statement evidence shared by early reachability and the late seal.

use arcweft_lang_hir::{
    dialogue_application::HirDialogueMarkId,
    identity::{ExprId, ItemId, LocalId, StmtId},
    symbol::{CallableDeclarationKey, FlowDeclarationId},
};

use crate::{
    final_analysis::{
        CheckedAssertionDisposition, CheckedEvaluatedEffect, CheckedIteration,
        CheckedProjectNominal, CheckedSuspensionStatement,
    },
    types::{SemanticTypeDigest, TypeKind},
};

use super::PreparedEvaluatedEffect;

/// One direct-local project-field assignment awaiting the project-wide field
/// coordinate seal.
///
/// This evidence is affine: the all-statement seal consumes it while joining
/// the already checked local, target, value, and nominal field projection.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedAssignmentStatement {
    local: LocalId,
    nominal: CheckedProjectNominal,
    target: ExprId,
    value: ExprId,
    field_type: TypeKind,
}

impl PreparedAssignmentStatement {
    pub(crate) const fn new(
        local: LocalId,
        nominal: CheckedProjectNominal,
        target: ExprId,
        value: ExprId,
        field_type: TypeKind,
    ) -> Self {
        Self {
            local,
            nominal,
            target,
            value,
            field_type,
        }
    }

    pub(crate) fn into_parts(self) -> (LocalId, CheckedProjectNominal, ExprId, ExprId, TypeKind) {
        (
            self.local,
            self.nominal,
            self.target,
            self.value,
            self.field_type,
        )
    }

    fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        self.nominal.visit_types(visitor)?;
        visitor(&self.field_type)
    }
}

/// Affine analyzer output consumed exactly once by the exhaustive all-31
/// statement seal.
///
/// No variant embeds a final [`super::super::CheckedStatementPayload`].
/// `HirOwned` merely proves that no earlier specialized pass owns the payload;
/// only the late exhaustive HIR match may select its final meaning.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PreparedStatementPayload {
    HirOwned,
    Assignment(PreparedAssignmentStatement),
    Assertion(CheckedAssertionDisposition),
    Iteration(Box<CheckedIteration>),
    Suspension(Box<CheckedSuspensionStatement>),
    Yield,
    EvaluatedEffect(PreparedEvaluatedEffect),
    SealedEvaluatedEffect(Box<CheckedEvaluatedEffect>),
}

impl PreparedStatementPayload {
    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Assignment(value) => value.visit_types(visitor),
            Self::Iteration(value) => value.visit_types(visitor),
            Self::HirOwned
            | Self::Assertion(_)
            | Self::Suspension(_)
            | Self::Yield
            | Self::EvaluatedEffect(_)
            | Self::SealedEvaluatedEffect(_) => Ok(()),
        }
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
