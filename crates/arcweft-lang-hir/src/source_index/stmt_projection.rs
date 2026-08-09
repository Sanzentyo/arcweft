//! Direct attached-statement projection for final HIR-owned source roles.

use arcweft_lang_syntax::attachment::node::UnsafeLifetimeStatementKind;
use arcweft_lang_syntax::attachment::{AstNode, StatementNode, SyntaxAccessError};
use arcweft_lang_syntax::grammar::SyntaxKind;
use arcweft_lang_syntax::incremental::ParsedSource;

use super::{
    HirInsertionPoint, HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery,
    HirSourceQueryError, HirSourceRequirement, HirSourceSite, HirStmtSourceRole,
    StagedHirSourceIndex,
};
use crate::arena::ArenaSnapshot;
use crate::identity::{StmtId, SyntheticOwner};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::stmt::{HirStmt, HirStmtKind, HirStmtPoisonState};

impl StagedHirSourceIndex {
    /// Projects the sole statement-owned edit component from exact attached
    /// syntax. A recovered statement never fabricates an edit anchor.
    #[allow(
        clippy::result_large_err,
        reason = "statement staging failures retain the complete typed owner, source component, and syntax evidence"
    )]
    pub(crate) fn stage_attached_stmt(
        &mut self,
        parsed: &ParsedSource,
        owner: StmtId,
        attached: &StatementNode,
        statement: &HirStmt,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if attached.snapshot_id() != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: attached.snapshot_id().clone(),
            });
        }

        let semantic_owner = SyntheticOwner::Stmt(owner);
        if matches!(statement.kind(), HirStmtKind::Error) {
            return Ok(());
        }
        if !matches!(statement.kind(), HirStmtKind::UnsafeLifetime { .. }) {
            if attached.kind() == SyntaxKind::UnsafeLifetimeStatement {
                return self.reject(
                    HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                        owner: semantic_owner,
                    },
                );
            }
            return Ok(());
        }
        if attached.kind() != SyntaxKind::UnsafeLifetimeStatement {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: semantic_owner,
                },
            );
        }

        let audit = match attached.cast::<UnsafeLifetimeStatementKind>() {
            Ok(audit) => audit,
            Err(error) => {
                return self.reject(HirSourceCommitInvariantError::AttachedSyntaxAccess {
                    owner: semantic_owner,
                    error: SyntaxAccessError::from(error),
                });
            }
        };
        let query = HirSourceQuery::Stmt {
            owner,
            role: HirStmtSourceRole::UnsafeAuditInsertion,
        };
        self.bind_syntax_owner(semantic_owner, attached.id())?;
        match statement.state() {
            HirStmtPoisonState::Clean => {
                let Some(insertion) = complete_unsafe_audit_insertion(parsed, &audit) else {
                    return self.reject(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: semantic_owner,
                        },
                    );
                };
                self.require(&query, HirSourceRequirement::Required)?;
                self.stage(&query, HirSourceSite::Insertion(insertion))
            }
            HirStmtPoisonState::Poisoned(_) => self.require(&query, HirSourceRequirement::Optional),
        }
    }
}

impl HirSourceIndex {
    /// Re-derives the final statement-owned component from the exact accepted
    /// syntax snapshot and checks it against the immutable semantic arena.
    pub(crate) fn validates_attached_statements(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        statements: &ArenaSnapshot<HirStmt, StmtId>,
    ) -> bool {
        let Ok(entries) = statements.try_iter_prepared(slots) else {
            return false;
        };
        entries.into_iter().all(|(owner, payload)| {
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            match metadata.origin() {
                HirOrigin::Source(source) => {
                    let Ok(attached) = parsed.statement_node(source.syntax()) else {
                        return false;
                    };
                    metadata.source_site() == &HirSourceSite::Span(attached.source_span())
                        && statement_manifest_matches(self, parsed, owner, payload, &attached)
                }
                HirOrigin::Synthetic(_) => !source_index_has_stmt_owner(self, owner),
            }
        })
    }
}

impl HirStmtKind {
    /// Validates statement-role applicability before the source identity or
    /// component manifest is consulted.
    pub(crate) const fn validate_source_role(
        &self,
        owner: StmtId,
        role: HirStmtSourceRole,
    ) -> Result<(), HirSourceQueryError> {
        match role {
            HirStmtSourceRole::Whole => Ok(()),
            HirStmtSourceRole::UnsafeAuditInsertion
                if matches!(self, Self::UnsafeLifetime { .. }) =>
            {
                Ok(())
            }
            HirStmtSourceRole::UnsafeAuditInsertion => {
                Err(HirSourceQueryError::StmtRoleNotApplicable { owner, role })
            }
        }
    }
}

fn statement_manifest_matches(
    index: &HirSourceIndex,
    parsed: &ParsedSource,
    owner: StmtId,
    statement: &HirStmt,
    attached: &StatementNode,
) -> bool {
    match statement.kind() {
        HirStmtKind::Error => !source_index_has_stmt_owner(index, owner),
        HirStmtKind::UnsafeLifetime { .. } => {
            if attached.kind() != SyntaxKind::UnsafeLifetimeStatement {
                return false;
            }
            let Ok(audit) = attached.cast::<UnsafeLifetimeStatementKind>() else {
                return false;
            };
            match statement.state() {
                HirStmtPoisonState::Clean => complete_unsafe_audit_insertion(parsed, &audit)
                    .is_some_and(|insertion| {
                        exact_unsafe_audit_manifest(
                            index,
                            owner,
                            attached.id(),
                            HirSourceRequirement::Required,
                            Some(&HirSourceSite::Insertion(insertion)),
                        )
                    }),
                HirStmtPoisonState::Poisoned(_) => exact_unsafe_audit_manifest(
                    index,
                    owner,
                    attached.id(),
                    HirSourceRequirement::Optional,
                    None,
                ),
            }
        }
        _ => {
            attached.kind() != SyntaxKind::UnsafeLifetimeStatement
                && !source_index_has_stmt_owner(index, owner)
        }
    }
}

fn complete_unsafe_audit_insertion(
    parsed: &ParsedSource,
    audit: &AstNode<UnsafeLifetimeStatementKind>,
) -> Option<HirInsertionPoint> {
    let body = audit.body().ok()?;
    let open = audit.audit_insertion_anchor().ok()?;
    let close = body.close_delimiter().ok()?;
    if open.range().is_empty() || close.range().is_empty() {
        return None;
    }
    HirInsertionPoint::try_new(parsed.document(), open.range().end()).ok()
}

fn exact_unsafe_audit_manifest(
    index: &HirSourceIndex,
    owner: StmtId,
    syntax: arcweft_lang_syntax::attachment::SyntaxNodeId,
    requirement: HirSourceRequirement,
    expected: Option<&HirSourceSite>,
) -> bool {
    let semantic_owner = SyntheticOwner::Stmt(owner);
    let query = HirSourceQuery::Stmt {
        owner,
        role: HirStmtSourceRole::UnsafeAuditInsertion,
    };
    index.syntax_owners.get(&semantic_owner) == Some(&syntax)
        && index.requirements.get(&query) == Some(&requirement)
        && match expected {
            Some(expected) => index.components.get(&query) == Some(expected),
            None => !index.components.contains_key(&query),
        }
        && index
            .requirements
            .keys()
            .filter(|candidate| candidate.owner() == semantic_owner)
            .count()
            == 1
        && index
            .components
            .keys()
            .filter(|candidate| candidate.owner() == semantic_owner)
            .count()
            == usize::from(expected.is_some())
}

fn source_index_has_stmt_owner(index: &HirSourceIndex, owner: StmtId) -> bool {
    let semantic_owner = SyntheticOwner::Stmt(owner);
    index.syntax_owners.contains_key(&semantic_owner)
        || index
            .requirements
            .keys()
            .any(|query| query.owner() == semantic_owner)
        || index
            .components
            .keys()
            .any(|query| query.owner() == semantic_owner)
}
