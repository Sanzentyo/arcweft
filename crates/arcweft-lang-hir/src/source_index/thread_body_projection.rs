//! Exact source projection for the shared statement-only Flow/Thread body.

use std::collections::BTreeSet;

use arcweft_lang_syntax::attachment::node::{BlockKind, FlowItemKind, MissingBodyKind};
use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AttachedFlowStatementBody, AttachedNestedThreadFlowBody, AttachedRequiredFlowBody,
    AttachedRequiredNestedThreadFlowBody, AttachedRequiredThreadExpressionBody,
    AttachedThreadExpressionBody, AttachedThreadFlowItem, AttachedThreadFlowItemFamily,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_source::SourceSpan;

use super::block_projection::{BlockValidationArenas, root_thread_body_graph_matches};
use super::{
    HirSourceCommitInvariantError, HirSourceIndex, HirSourceQuery, HirSourceRequirement,
    HirSourceSite, HirThreadBodySourceRole, HirThreadFlowItemSourcePart, StagedHirSourceIndex,
};
use crate::arena::ArenaSnapshot;
use crate::expr::{
    HirExpr, HirExprKind, HirPoisonState, HirRecoveryIssue, HirThreadBody, HirThreadBodyOwner,
    HirThreadFlowItem, HirThreadIssue,
};
use crate::identity::{ExprId, ItemId, LocalId, PatternId, ScopeId, StmtId};
use crate::item::{HirItem, HirItemKind};
use crate::pattern::HirPattern;
use crate::scope::{HirLocal, HirScope, HirScopeOwner};
use crate::slot::SlotSnapshot;
use crate::stmt::{HirStmt, HirStmtKind};

#[allow(
    clippy::result_large_err,
    reason = "source staging preserves the exact typed owner, query, and source-identity failure"
)]
impl StagedHirSourceIndex {
    /// Stages one ordinary Flow body's exact delimiter and item components.
    pub(crate) fn stage_attached_flow_thread_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredFlowBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        if !matches!(owner, HirThreadBodyOwner::Flow(_)) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: HirSourceQuery::ThreadBody {
                        owner,
                        role: HirThreadBodySourceRole::Whole,
                    }
                    .owner(),
                },
            );
        }
        match attached {
            AttachedRequiredFlowBody::Present(attached) => {
                self.stage_present_thread_body(parsed, owner, attached, body)
            }
            AttachedRequiredFlowBody::Missing {
                syntax,
                missing,
                insertion,
            } => {
                if syntax.snapshot_id() != parsed.snapshot_id() {
                    return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                        expected: parsed.snapshot_id().clone(),
                        actual: syntax.snapshot_id().clone(),
                    });
                }
                if missing.snapshot_id() != parsed.snapshot_id() {
                    return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                        expected: parsed.snapshot_id().clone(),
                        actual: missing.snapshot_id().clone(),
                    });
                }
                if !missing.range().is_empty() || missing.source_span() != *insertion {
                    return self.reject(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: HirSourceQuery::ThreadBody {
                                owner,
                                role: HirThreadBodySourceRole::Whole,
                            }
                            .owner(),
                        },
                    );
                }
                self.stage_missing_thread_body(parsed, owner, insertion, body)
            }
        }
    }

    /// Stages one Thread-expression body's exact delimiter and item components.
    pub(crate) fn stage_attached_thread_expression_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredThreadExpressionBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        if !matches!(owner, HirThreadBodyOwner::ThreadExpression(_)) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: HirSourceQuery::ThreadBody {
                        owner,
                        role: HirThreadBodySourceRole::Whole,
                    }
                    .owner(),
                },
            );
        }
        match attached {
            AttachedRequiredThreadExpressionBody::Present(attached) => {
                self.stage_present_thread_expression_body(parsed, owner, attached, body)
            }
            AttachedRequiredThreadExpressionBody::Missing {
                owner: syntax,
                missing,
            } => {
                if syntax.snapshot_id() != parsed.snapshot_id() {
                    return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                        expected: parsed.snapshot_id().clone(),
                        actual: syntax.snapshot_id().clone(),
                    });
                }
                if missing.snapshot_id() != parsed.snapshot_id() {
                    return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                        expected: parsed.snapshot_id().clone(),
                        actual: missing.snapshot_id().clone(),
                    });
                }
                if !missing.range().is_empty() {
                    return self.reject(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: HirSourceQuery::ThreadBody {
                                owner,
                                role: HirThreadBodySourceRole::Whole,
                            }
                            .owner(),
                        },
                    );
                }
                self.stage_missing_thread_body(parsed, owner, &missing.source_span(), body)
            }
        }
    }

    /// Stages one nested statement-only body's exact delimiter and item components.
    pub(crate) fn stage_attached_nested_thread_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredNestedThreadFlowBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        if !matches!(owner, HirThreadBodyOwner::NestedScope(scope) if scope == body.scope()) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: HirSourceQuery::ThreadBody {
                        owner,
                        role: HirThreadBodySourceRole::Whole,
                    }
                    .owner(),
                },
            );
        }
        match attached {
            AttachedRequiredNestedThreadFlowBody::Present(attached) => {
                self.stage_present_nested_thread_body(parsed, owner, attached, body)
            }
            AttachedRequiredNestedThreadFlowBody::Missing(missing) => {
                if missing.snapshot_id() != parsed.snapshot_id() {
                    return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                        expected: parsed.snapshot_id().clone(),
                        actual: missing.snapshot_id().clone(),
                    });
                }
                if !missing.range().is_empty() {
                    return self.reject(
                        HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                            owner: HirSourceQuery::ThreadBody {
                                owner,
                                role: HirThreadBodySourceRole::Whole,
                            }
                            .owner(),
                        },
                    );
                }
                self.stage_missing_thread_body(parsed, owner, &missing.source_span(), body)
            }
        }
    }

    fn stage_present_thread_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedFlowStatementBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.stage_present_thread_body_parts(
            parsed,
            owner,
            attached.syntax().snapshot_id(),
            &attached.open().source_span(),
            attached.items(),
            &attached.close().source_span(),
            body,
        )
    }

    fn stage_present_thread_expression_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedThreadExpressionBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.stage_present_thread_body_parts(
            parsed,
            owner,
            attached.syntax().snapshot_id(),
            &attached.open().source_span(),
            attached.items(),
            &attached.close().source_span(),
            body,
        )
    }

    fn stage_present_nested_thread_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        attached: &AttachedNestedThreadFlowBody,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.stage_present_thread_body_parts(
            parsed,
            owner,
            attached.syntax().snapshot_id(),
            &attached.open().source_span(),
            attached.items(),
            &attached.close().source_span(),
            body,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the three attached body owners share one exact delimiter/item manifest transaction"
    )]
    fn stage_present_thread_body_parts(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        snapshot: &arcweft_lang_syntax::attachment::SyntaxSnapshotId,
        open: &SourceSpan,
        items: &[AttachedThreadFlowItem],
        close: &SourceSpan,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if snapshot != parsed.snapshot_id() {
            return self.reject(HirSourceCommitInvariantError::WrongSyntaxSnapshot {
                expected: parsed.snapshot_id().clone(),
                actual: snapshot.clone(),
            });
        }
        if !thread_body_families_match(owner, body, items) {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadFamilyMismatch {
                    owner: HirSourceQuery::ThreadBody {
                        owner,
                        role: HirThreadBodySourceRole::Whole,
                    }
                    .owner(),
                },
            );
        }
        self.stage_required_thread_body_component(
            parsed,
            owner,
            HirThreadBodySourceRole::OpenDelimiter,
            open,
        )?;
        self.stage_required_thread_body_component(
            parsed,
            owner,
            HirThreadBodySourceRole::CloseDelimiter,
            close,
        )?;
        for (ordinal, attached) in items.iter().enumerate() {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return self.reject(
                    HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                        owner: HirSourceQuery::ThreadBody {
                            owner,
                            role: HirThreadBodySourceRole::Whole,
                        }
                        .owner(),
                    },
                );
            };
            self.stage_required_thread_body_component(
                parsed,
                owner,
                HirThreadBodySourceRole::Item {
                    ordinal,
                    part: HirThreadFlowItemSourcePart::Whole,
                },
                &attached.syntax().source_span(),
            )?;
        }
        Ok(())
    }

    fn stage_missing_thread_body(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        insertion: &SourceSpan,
        body: &HirThreadBody,
    ) -> Result<(), HirSourceCommitInvariantError> {
        self.ensure_open()?;
        if !body.items().is_empty()
            || body.validate_module(owner.module()).is_err()
            || matches!(owner, HirThreadBodyOwner::NestedScope(scope) if scope != body.scope())
        {
            return self.reject(
                HirSourceCommitInvariantError::AttachedPayloadStateMismatch {
                    owner: HirSourceQuery::ThreadBody {
                        owner,
                        role: HirThreadBodySourceRole::Whole,
                    }
                    .owner(),
                },
            );
        }
        self.stage_required_thread_body_component(
            parsed,
            owner,
            HirThreadBodySourceRole::OpenDelimiter,
            insertion,
        )?;
        self.stage_required_thread_body_component(
            parsed,
            owner,
            HirThreadBodySourceRole::CloseDelimiter,
            insertion,
        )
    }

    fn stage_required_thread_body_component(
        &mut self,
        parsed: &ParsedSource,
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
        span: &SourceSpan,
    ) -> Result<(), HirSourceCommitInvariantError> {
        let site = match HirSourceSite::from_attached_span(parsed.document(), span) {
            Ok(site) => site,
            Err(error) => return self.reject(error.into()),
        };
        let query = HirSourceQuery::ThreadBody { owner, role };
        self.require(&query, HirSourceRequirement::Required)?;
        self.stage(&query, site)
    }
}

impl HirSourceIndex {
    /// Re-derives every shared Flow/Thread body from attached syntax and
    /// rejects both omitted bodies and source-manifest rows without a semantic
    /// owner.
    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "publication exhaustively freezes the three body owners and rejects unowned manifest rows"
    )]
    pub(crate) fn validates_attached_thread_bodies(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        items: &ArenaSnapshot<HirItem, ItemId>,
        expressions: &ArenaSnapshot<HirExpr, ExprId>,
        statements: &ArenaSnapshot<HirStmt, StmtId>,
        scopes: &ArenaSnapshot<HirScope, ScopeId>,
        locals: &ArenaSnapshot<HirLocal, LocalId>,
        patterns: &ArenaSnapshot<HirPattern, PatternId>,
    ) -> bool {
        let validation_arenas = BlockValidationArenas {
            expressions,
            statements,
            scopes,
            locals,
            patterns,
        };
        let mut expected = BTreeSet::new();
        let Ok(item_entries) = items.try_iter_prepared(slots) else {
            return false;
        };
        for (owner, item) in item_entries {
            let HirItemKind::Flow(flow) = item.kind() else {
                continue;
            };
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            let crate::slot::HirOrigin::Source(source) = metadata.origin() else {
                return false;
            };
            let Ok(syntax) = parsed.typed_node::<FlowItemKind>(source.syntax()) else {
                return false;
            };
            let Ok(attached) = syntax.semantics() else {
                return false;
            };
            let body_owner = HirThreadBodyOwner::Flow(owner);
            if !expected.insert(body_owner)
                || !thread_body_semantic_items_match(slots, statements, expressions, flow.body())
                || !flow_body_graph_matches(
                    parsed,
                    slots,
                    &validation_arenas,
                    owner,
                    attached.body(),
                    flow.body(),
                )
                || !self.validates_attached_flow_thread_body(
                    parsed,
                    slots,
                    body_owner,
                    attached.body(),
                    flow.body(),
                )
            {
                return false;
            }
        }

        let Ok(expression_entries) = expressions.try_iter_prepared(slots) else {
            return false;
        };
        for (owner, expression) in expression_entries {
            let HirExprKind::Thread(thread) = expression.kind() else {
                continue;
            };
            let Ok(metadata) = slots.resolve_prepared(owner) else {
                return false;
            };
            let crate::slot::HirOrigin::Source(source) = metadata.origin() else {
                return false;
            };
            let Ok(attached) = parsed.attached_expression(source.syntax()) else {
                return false;
            };
            let Some(syntax) = attached.thread() else {
                return false;
            };
            let Ok(attached_body) = syntax.statement_body() else {
                return false;
            };
            let invalid_name = matches!(
                attached.projection(),
                ExpressionProjection::Thread(projection)
                    if matches!(projection.name(), Some(Err(_)))
            );
            let body_owner = HirThreadBodyOwner::ThreadExpression(owner);
            if !expected.insert(body_owner)
                || !thread_body_semantic_items_match(slots, statements, expressions, thread.body())
                || !thread_expression_body_graph_matches(
                    parsed,
                    slots,
                    &validation_arenas,
                    owner,
                    expression.scope(),
                    expression,
                    invalid_name,
                    &attached_body,
                    thread.body(),
                )
                || !self.validates_attached_thread_expression_body(
                    parsed,
                    slots,
                    body_owner,
                    &attached_body,
                    thread.body(),
                )
            {
                return false;
            }
        }

        let Ok(scope_entries) = scopes.try_iter_prepared(slots) else {
            return false;
        };
        for (scope_id, scope) in scope_entries {
            let Some((body_owner @ HirThreadBodyOwner::NestedScope(_), body)) =
                crate::module::prepared_thread_body_for_scope(
                    slots,
                    items,
                    expressions,
                    statements,
                    scope_id,
                    scope,
                )
            else {
                continue;
            };
            let Ok(metadata) = slots.resolve_prepared(scope_id) else {
                return false;
            };
            let crate::slot::HirOrigin::Source(source) = metadata.origin() else {
                return false;
            };
            let attached = if let Ok(block) = parsed.typed_node::<BlockKind>(source.syntax()) {
                let Ok(body) = block.thread_flow_body() else {
                    return false;
                };
                AttachedRequiredNestedThreadFlowBody::Present(body)
            } else if let Ok(missing) = parsed.typed_node::<MissingBodyKind>(source.syntax()) {
                AttachedRequiredNestedThreadFlowBody::Missing(missing)
            } else {
                return false;
            };
            if !expected.insert(body_owner)
                || !thread_body_semantic_items_match(slots, statements, expressions, body)
                || !self.validates_attached_nested_thread_body(
                    parsed, slots, body_owner, &attached, body,
                )
            {
                return false;
            }
        }

        self.requirements
            .keys()
            .chain(self.components.keys())
            .filter_map(|query| match query {
                HirSourceQuery::ThreadBody { owner, .. } => Some(*owner),
                HirSourceQuery::Item { .. }
                | HirSourceQuery::Expr { .. }
                | HirSourceQuery::Pattern { .. }
                | HirSourceQuery::Type { .. }
                | HirSourceQuery::Stmt { .. }
                | HirSourceQuery::Scope { .. }
                | HirSourceQuery::Local { .. } => None,
            })
            .collect::<BTreeSet<_>>()
            == expected
    }

    pub(crate) fn validates_attached_flow_thread_body(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredFlowBody,
        body: &HirThreadBody,
    ) -> bool {
        if !matches!(owner, HirThreadBodyOwner::Flow(_)) {
            return false;
        }
        match attached {
            AttachedRequiredFlowBody::Present(attached) => self.thread_body_manifest_matches(
                parsed,
                slots,
                owner,
                body,
                &attached.syntax().source_span(),
                &attached.open().source_span(),
                attached.items(),
                &attached.close().source_span(),
            ),
            AttachedRequiredFlowBody::Missing {
                syntax,
                missing,
                insertion,
            } => {
                syntax.snapshot_id() == parsed.snapshot_id()
                    && missing.snapshot_id() == parsed.snapshot_id()
                    && missing.range().is_empty()
                    && missing.source_span() == *insertion
                    && self
                        .missing_thread_body_manifest_matches(parsed, slots, owner, body, insertion)
            }
        }
    }

    pub(crate) fn validates_attached_thread_expression_body(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredThreadExpressionBody,
        body: &HirThreadBody,
    ) -> bool {
        if !matches!(owner, HirThreadBodyOwner::ThreadExpression(_)) {
            return false;
        }
        match attached {
            AttachedRequiredThreadExpressionBody::Present(attached) => self
                .thread_body_manifest_matches(
                    parsed,
                    slots,
                    owner,
                    body,
                    &attached.syntax().source_span(),
                    &attached.open().source_span(),
                    attached.items(),
                    &attached.close().source_span(),
                ),
            AttachedRequiredThreadExpressionBody::Missing {
                owner: syntax,
                missing,
            } => {
                syntax.snapshot_id() == parsed.snapshot_id()
                    && missing.snapshot_id() == parsed.snapshot_id()
                    && missing.range().is_empty()
                    && self.missing_thread_body_manifest_matches(
                        parsed,
                        slots,
                        owner,
                        body,
                        &missing.source_span(),
                    )
            }
        }
    }

    pub(crate) fn validates_attached_nested_thread_body(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        owner: HirThreadBodyOwner,
        attached: &AttachedRequiredNestedThreadFlowBody,
        body: &HirThreadBody,
    ) -> bool {
        if !matches!(owner, HirThreadBodyOwner::NestedScope(scope) if scope == body.scope()) {
            return false;
        }
        match attached {
            AttachedRequiredNestedThreadFlowBody::Present(attached) => self
                .thread_body_manifest_matches(
                    parsed,
                    slots,
                    owner,
                    body,
                    &attached.syntax().source_span(),
                    &attached.open().source_span(),
                    attached.items(),
                    &attached.close().source_span(),
                ),
            AttachedRequiredNestedThreadFlowBody::Missing(missing) => {
                missing.snapshot_id() == parsed.snapshot_id()
                    && missing.range().is_empty()
                    && self.missing_thread_body_manifest_matches(
                        parsed,
                        slots,
                        owner,
                        body,
                        &missing.source_span(),
                    )
            }
        }
    }

    fn missing_thread_body_manifest_matches(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        owner: HirThreadBodyOwner,
        body: &HirThreadBody,
        insertion: &SourceSpan,
    ) -> bool {
        body.items().is_empty()
            && self.thread_body_manifest_matches(
                parsed,
                slots,
                owner,
                body,
                insertion,
                insertion,
                &[],
                insertion,
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn thread_body_manifest_matches(
        &self,
        parsed: &ParsedSource,
        slots: &SlotSnapshot,
        owner: HirThreadBodyOwner,
        body: &HirThreadBody,
        whole: &SourceSpan,
        open: &SourceSpan,
        items: &[AttachedThreadFlowItem],
        close: &SourceSpan,
    ) -> bool {
        if body.validate_module(owner.module()).is_err()
            || matches!(owner, HirThreadBodyOwner::NestedScope(scope) if scope != body.scope())
            || !thread_body_families_match(owner, body, items)
        {
            return false;
        }
        let Ok(scope_metadata) = slots.resolve_prepared(body.scope()) else {
            return false;
        };
        let Ok(whole) = HirSourceSite::from_attached_span(parsed.document(), whole) else {
            return false;
        };
        let Ok(open) = HirSourceSite::from_attached_span(parsed.document(), open) else {
            return false;
        };
        let Ok(close) = HirSourceSite::from_attached_span(parsed.document(), close) else {
            return false;
        };
        if scope_metadata.source_site() != &whole
            || !self.thread_body_component_matches(
                owner,
                HirThreadBodySourceRole::OpenDelimiter,
                &open,
            )
            || !self.thread_body_component_matches(
                owner,
                HirThreadBodySourceRole::CloseDelimiter,
                &close,
            )
        {
            return false;
        }

        let mut child_owners = BTreeSet::new();
        for (ordinal, (attached, semantic)) in items.iter().zip(body.items()).enumerate() {
            let Ok(ordinal) = u32::try_from(ordinal) else {
                return false;
            };
            let Ok(expected) = HirSourceSite::from_attached_span(
                parsed.document(),
                &attached.syntax().source_span(),
            ) else {
                return false;
            };
            let child_site_matches =
                thread_flow_item_source_site(slots, semantic).is_some_and(|site| site == &expected);
            if !child_owners.insert(semantic.owner())
                || !child_site_matches
                || !self.thread_body_component_matches(
                    owner,
                    HirThreadBodySourceRole::Item {
                        ordinal,
                        part: HirThreadFlowItemSourcePart::Whole,
                    },
                    &expected,
                )
            {
                return false;
            }
        }

        let Some(expected_component_count) = body.items().len().checked_add(2) else {
            return false;
        };
        self.requirements
            .keys()
            .filter(|query| matches!(query, HirSourceQuery::ThreadBody { owner: candidate, .. } if *candidate == owner))
            .count()
            == expected_component_count
            && self
                .components
                .keys()
                .filter(|query| matches!(query, HirSourceQuery::ThreadBody { owner: candidate, .. } if *candidate == owner))
                .count()
                == expected_component_count
    }

    fn thread_body_component_matches(
        &self,
        owner: HirThreadBodyOwner,
        role: HirThreadBodySourceRole,
        expected: &HirSourceSite,
    ) -> bool {
        let query = HirSourceQuery::ThreadBody { owner, role };
        self.requirements.get(&query) == Some(&HirSourceRequirement::Required)
            && self.components.get(&query) == Some(expected)
    }
}

fn flow_body_graph_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ItemId,
    attached: &AttachedRequiredFlowBody,
    body: &HirThreadBody,
) -> bool {
    let Ok(scope) = arenas.scopes.resolve_prepared(slots, body.scope()) else {
        return false;
    };
    let Some(parent_scope) = scope.parent() else {
        return false;
    };
    if !arenas
        .scopes
        .resolve_prepared(slots, parent_scope)
        .is_ok_and(|parent| {
            parent.kind() == crate::scope::HirScopeKind::Callable
                && parent.owner() == &HirScopeOwner::Item(owner)
        })
    {
        return false;
    }
    match attached {
        AttachedRequiredFlowBody::Present(attached) => root_thread_body_graph_matches(
            parsed,
            slots,
            arenas,
            body,
            attached.syntax().id(),
            &attached.syntax().source_span(),
            attached.items(),
            attached.is_unclosed(),
            false,
            &HirScopeOwner::Item(owner),
            parent_scope,
            crate::scope::HirScopeKind::Flow,
        )
        .is_some(),
        AttachedRequiredFlowBody::Missing { syntax, .. } => root_thread_body_graph_matches(
            parsed,
            slots,
            arenas,
            body,
            syntax.id(),
            &syntax.source_span(),
            &[],
            false,
            true,
            &HirScopeOwner::Item(owner),
            parent_scope,
            crate::scope::HirScopeKind::Flow,
        )
        .is_some(),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the validator compares one typed Thread body against its exact expression, parent scope, attachment, and recovery inputs"
)]
fn thread_expression_body_graph_matches(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    arenas: &BlockValidationArenas<'_>,
    owner: ExprId,
    parent_scope: ScopeId,
    expression: &HirExpr,
    invalid_name: bool,
    attached: &AttachedRequiredThreadExpressionBody,
    body: &HirThreadBody,
) -> bool {
    let expected_body_recovery = match attached {
        AttachedRequiredThreadExpressionBody::Present(attached) => root_thread_body_graph_matches(
            parsed,
            slots,
            arenas,
            body,
            attached.syntax().id(),
            &attached.syntax().source_span(),
            attached.items(),
            matches!(attached.close_state(), AttachedDelimiterState::Missing(_)),
            false,
            &HirScopeOwner::Expr(owner),
            parent_scope,
            crate::scope::HirScopeKind::Block,
        ),
        AttachedRequiredThreadExpressionBody::Missing { missing, .. } => {
            root_thread_body_graph_matches(
                parsed,
                slots,
                arenas,
                body,
                missing.id(),
                &missing.source_span(),
                &[],
                false,
                true,
                &HirScopeOwner::Expr(owner),
                parent_scope,
                crate::scope::HirScopeKind::Block,
            )
        }
    };
    let Some(expected_body_recovery) = expected_body_recovery else {
        return false;
    };
    let expected_recovery = invalid_name
        .then_some(HirThreadIssue::InvalidName)
        .or(expected_body_recovery);
    let expected_state = expected_recovery.map_or(HirPoisonState::Clean, |issue| {
        HirPoisonState::Poisoned(HirRecoveryIssue::InvalidThread(issue))
    });
    if expression.state() != &expected_state {
        return false;
    }

    let actual_scopes = arenas.scopes.try_iter_prepared(slots).ok().map(|scopes| {
        scopes
            .filter_map(|(scope, payload)| {
                (payload.owner() == &HirScopeOwner::Expr(owner)).then_some(scope)
            })
            .collect::<BTreeSet<_>>()
    });
    actual_scopes == Some(BTreeSet::from([body.scope()]))
}

fn thread_flow_item_source_site<'a>(
    slots: &'a SlotSnapshot,
    item: &HirThreadFlowItem,
) -> Option<&'a HirSourceSite> {
    match item {
        HirThreadFlowItem::DialogueApplication(expression) => slots
            .resolve_prepared(*expression)
            .ok()
            .map(crate::slot::HirSlotMetadata::source_site),
        HirThreadFlowItem::Statement(statement)
        | HirThreadFlowItem::Choice(statement)
        | HirThreadFlowItem::If(statement)
        | HirThreadFlowItem::IfLet(statement)
        | HirThreadFlowItem::Match(statement)
        | HirThreadFlowItem::While(statement)
        | HirThreadFlowItem::WhileLet(statement)
        | HirThreadFlowItem::For(statement)
        | HirThreadFlowItem::Select(statement)
        | HirThreadFlowItem::SourceLocale(statement)
        | HirThreadFlowItem::Scope(statement)
        | HirThreadFlowItem::Include(statement)
        | HirThreadFlowItem::Error(statement) => slots
            .resolve_prepared(*statement)
            .ok()
            .map(crate::slot::HirSlotMetadata::source_site),
    }
}

fn thread_body_families_match(
    owner: HirThreadBodyOwner,
    body: &HirThreadBody,
    attached: &[AttachedThreadFlowItem],
) -> bool {
    body.validate_module(owner.module()).is_ok()
        && attached.len() == body.items().len()
        && attached
            .iter()
            .zip(body.items())
            .all(|(attached, semantic)| {
                matches!(
                    (attached.family(), semantic),
                    (
                        AttachedThreadFlowItemFamily::Statement,
                        HirThreadFlowItem::Statement(_)
                    ) | (
                        AttachedThreadFlowItemFamily::DialogueApplication,
                        HirThreadFlowItem::DialogueApplication(_)
                    ) | (
                        AttachedThreadFlowItemFamily::Choice,
                        HirThreadFlowItem::Choice(_)
                    ) | (AttachedThreadFlowItemFamily::If, HirThreadFlowItem::If(_))
                        | (
                            AttachedThreadFlowItemFamily::IfLet,
                            HirThreadFlowItem::IfLet(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::Match,
                            HirThreadFlowItem::Match(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::While,
                            HirThreadFlowItem::While(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::WhileLet,
                            HirThreadFlowItem::WhileLet(_)
                        )
                        | (AttachedThreadFlowItemFamily::For, HirThreadFlowItem::For(_))
                        | (
                            AttachedThreadFlowItemFamily::Select,
                            HirThreadFlowItem::Select(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::SourceLocale,
                            HirThreadFlowItem::SourceLocale(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::Scope,
                            HirThreadFlowItem::Scope(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::Include,
                            HirThreadFlowItem::Include(_)
                        )
                        | (
                            AttachedThreadFlowItemFamily::Error,
                            HirThreadFlowItem::Error(_)
                        )
                )
            })
}

fn thread_body_semantic_items_match(
    slots: &SlotSnapshot,
    statements: &ArenaSnapshot<HirStmt, StmtId>,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    body: &HirThreadBody,
) -> bool {
    body.items().iter().all(|item| match item {
        HirThreadFlowItem::DialogueApplication(owner) => expressions
            .resolve_prepared(slots, *owner)
            .is_ok_and(|expression| {
                matches!(
                    expression.kind(),
                    HirExprKind::DialogueContentApplication(_)
                )
            }),
        HirThreadFlowItem::Statement(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| ordinary_thread_statement(statement.kind())),
        HirThreadFlowItem::Choice(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Choice { .. })),
        HirThreadFlowItem::If(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::If(_))),
        HirThreadFlowItem::IfLet(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::IfLet(_))),
        HirThreadFlowItem::Match(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Match(_))),
        HirThreadFlowItem::While(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::While(_))),
        HirThreadFlowItem::WhileLet(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::WhileLet(_))),
        HirThreadFlowItem::For(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::For(_))),
        HirThreadFlowItem::Select(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Select(_))),
        HirThreadFlowItem::SourceLocale(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::SourceLocale(_))),
        HirThreadFlowItem::Scope(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Scope(_))),
        HirThreadFlowItem::Include(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Include(_))),
        HirThreadFlowItem::Error(owner) => statements
            .resolve_prepared(slots, *owner)
            .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Error)),
    })
}

fn ordinary_thread_statement(kind: &HirStmtKind) -> bool {
    !matches!(
        kind,
        HirStmtKind::Choice { .. }
            | HirStmtKind::If(_)
            | HirStmtKind::IfLet(_)
            | HirStmtKind::Match(_)
            | HirStmtKind::While(_)
            | HirStmtKind::WhileLet(_)
            | HirStmtKind::For(_)
            | HirStmtKind::Select(_)
            | HirStmtKind::SourceLocale(_)
            | HirStmtKind::Scope(_)
            | HirStmtKind::Include(_)
            | HirStmtKind::Error
    )
}
