//! Shared no-tail lowering for Flow declarations, Thread expressions, and
//! nested Thread/Flow control bodies.

use arcweft_lang_syntax::attachment::source_file::AttachedDelimiterState;
use arcweft_lang_syntax::attachment::{
    AttachedExpressionNode, AttachedRequiredFlowBody, AttachedRequiredNestedThreadFlowBody,
    AttachedRequiredThreadExpressionBody, AttachedThreadFlowItem, AttachedThreadFlowItemFamily,
    SyntaxNodeId,
};
use arcweft_lang_syntax::expressions::{SyntaxThreadMode, SyntaxThreadProjection};
use arcweft_source::SourceSpan;

use crate::expr::{
    HirThreadBody, HirThreadBodyOwner, HirThreadExpr, HirThreadFlowItem, HirThreadIssue,
    HirThreadMode,
};
use crate::identity::{ExprId, HirLimit, ItemId, LocalId, ScopeId};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::scope::{HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::HirSourceSite;

use super::{StagedHirModuleTransaction, require_limit};

pub(super) struct LoweredThreadBody {
    pub(super) body: HirThreadBody,
    pub(super) recovery: Option<HirThreadIssue>,
}

/// Lossless Flow-body lowering result used to build canonical Flow poison.
///
/// Other Thread-family consumers currently expose one terminal issue. A Flow
/// declaration instead retains every recovered direct child followed by the
/// missing close, so its `HirFlowPoison` can publish all related evidence.
pub(super) struct LoweredFlowBody {
    pub(super) body: HirThreadBody,
    pub(super) recoveries: Box<[HirThreadIssue]>,
}

pub(super) struct PreparedNestedThreadBody<'attached> {
    attached: &'attached AttachedRequiredNestedThreadFlowBody,
    scope: ScopeId,
    items: &'attached [AttachedThreadFlowItem],
    close_missing: bool,
    missing_body: bool,
}

impl PreparedNestedThreadBody<'_> {
    pub(super) const fn scope(&self) -> ScopeId {
        self.scope
    }
}

struct LoweredThreadFlowItems {
    items: Box<[HirThreadFlowItem]>,
    locals: Box<[LocalId]>,
    recoveries: Box<[HirThreadIssue]>,
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_flow_body(
        &mut self,
        attached: &AttachedRequiredFlowBody,
        owner: ItemId,
        scope: ScopeId,
    ) -> Result<LoweredFlowBody, HirLowerFailure> {
        let (items, recoveries) = match attached {
            AttachedRequiredFlowBody::Present(body) => {
                let lowered = self.lower_attached_thread_flow_items(body.items(), scope)?;
                self.close_thread_body_scope(scope, Box::new([]), &lowered)?;
                let mut recoveries = lowered.recoveries.into_vec();
                if matches!(body.close_state(), AttachedDelimiterState::Missing(_)) {
                    recoveries.push(HirThreadIssue::UnclosedBody);
                }
                (lowered.items, recoveries.into_boxed_slice())
            }
            AttachedRequiredFlowBody::Missing { .. } => {
                self.close_scope_members(scope, Box::new([]))?;
                (
                    Box::<[HirThreadFlowItem]>::from([]),
                    Box::<[HirThreadIssue]>::from([HirThreadIssue::MissingBody]),
                )
            }
        };
        let body = HirThreadBody::try_new(HirThreadBodyOwner::Flow(owner), scope, items)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components.stage_attached_flow_thread_body(
            self.request.source(),
            HirThreadBodyOwner::Flow(owner),
            attached,
            &body,
        )?;
        Ok(LoweredFlowBody { body, recoveries })
    }

    pub(super) fn lower_attached_thread_expression(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        parent_scope: ScopeId,
        projection: &SyntaxThreadProjection,
    ) -> Result<(HirThreadExpr, Option<HirThreadIssue>), HirLowerFailure> {
        let syntax = attached
            .thread()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let body = syntax
            .statement_body()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let lowered = self.lower_required_thread_expression_body(&body, owner, parent_scope)?;
        let (name, name_recovery) = match projection.name() {
            Some(Ok(name)) => (Some(super::name_projection::name(name)?), None),
            Some(Err(issue)) => {
                super::name_projection::require_attempted_name_limit(issue)?;
                (None, Some(HirThreadIssue::InvalidName))
            }
            None => (None, None),
        };
        let mode = match projection.mode() {
            SyntaxThreadMode::Attached => HirThreadMode::Attached,
            SyntaxThreadMode::Detached => HirThreadMode::Detached,
        };
        let expression = HirThreadExpr::new(name, mode, lowered.body);
        Ok((expression, name_recovery.or(lowered.recovery)))
    }

    fn lower_required_thread_expression_body(
        &mut self,
        attached: &AttachedRequiredThreadExpressionBody,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<LoweredThreadBody, HirLowerFailure> {
        let lowered = match attached {
            AttachedRequiredThreadExpressionBody::Present(body) => {
                let scope = self.allocate_thread_body_scope(
                    body.syntax().id(),
                    &body.syntax().source_span(),
                    HirScopeKind::Block,
                    HirScopeOwner::Expr(owner),
                    parent_scope,
                )?;
                let lowered_items = self.lower_attached_thread_flow_items(body.items(), scope)?;
                self.close_thread_body_scope(scope, Box::new([]), &lowered_items)?;
                let recovery = lowered_items
                    .recoveries
                    .first()
                    .cloned()
                    .or_else(|| body.is_unclosed().then_some(HirThreadIssue::UnclosedBody));
                let body = HirThreadBody::try_new(
                    HirThreadBodyOwner::ThreadExpression(owner),
                    scope,
                    lowered_items.items,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                LoweredThreadBody { body, recovery }
            }
            AttachedRequiredThreadExpressionBody::Missing {
                owner: syntax,
                missing,
            } => {
                if syntax.id() != owner_syntax_id(owner, self)? {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                let scope = self.allocate_thread_body_scope(
                    missing.id(),
                    &missing.source_span(),
                    HirScopeKind::Block,
                    HirScopeOwner::Expr(owner),
                    parent_scope,
                )?;
                self.close_scope_members(scope, Box::new([]))?;
                let body = HirThreadBody::try_new(
                    HirThreadBodyOwner::ThreadExpression(owner),
                    scope,
                    Box::new([]),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                LoweredThreadBody {
                    body,
                    recovery: Some(HirThreadIssue::MissingBody),
                }
            }
        };
        self.source_components
            .stage_attached_thread_expression_body(
                self.request.source(),
                HirThreadBodyOwner::ThreadExpression(owner),
                attached,
                &lowered.body,
            )?;
        Ok(lowered)
    }

    pub(super) fn lower_attached_nested_thread_body(
        &mut self,
        attached: &AttachedRequiredNestedThreadFlowBody,
        scope_owner: HirScopeOwner,
        parent_scope: ScopeId,
    ) -> Result<LoweredThreadBody, HirLowerFailure> {
        let prepared =
            self.prepare_attached_nested_thread_body(attached, scope_owner, parent_scope)?;
        self.finish_attached_nested_thread_body(prepared, Box::new([]))
    }

    pub(super) fn prepare_attached_nested_thread_body<'attached>(
        &mut self,
        attached: &'attached AttachedRequiredNestedThreadFlowBody,
        scope_owner: HirScopeOwner,
        parent_scope: ScopeId,
    ) -> Result<PreparedNestedThreadBody<'attached>, HirLowerFailure> {
        let (syntax, source, items, close_missing, missing_body) = match attached {
            AttachedRequiredNestedThreadFlowBody::Present(body) => (
                body.syntax().id(),
                body.syntax().source_span(),
                body.items(),
                body.is_unclosed(),
                false,
            ),
            AttachedRequiredNestedThreadFlowBody::Missing(missing) => {
                (missing.id(), missing.source_span(), &[][..], false, true)
            }
        };
        let scope = self.allocate_thread_body_scope(
            syntax,
            &source,
            HirScopeKind::Block,
            scope_owner,
            parent_scope,
        )?;
        Ok(PreparedNestedThreadBody {
            attached,
            scope,
            items,
            close_missing,
            missing_body,
        })
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the prepared body is a single-use transaction capability consumed by finalization"
    )]
    pub(super) fn finish_attached_nested_thread_body(
        &mut self,
        prepared: PreparedNestedThreadBody<'_>,
        prefix_locals: Box<[LocalId]>,
    ) -> Result<LoweredThreadBody, HirLowerFailure> {
        let lowered_items =
            self.lower_attached_thread_flow_items(prepared.items, prepared.scope)?;
        self.close_thread_body_scope(prepared.scope, prefix_locals, &lowered_items)?;
        let recovery = if prepared.missing_body {
            Some(HirThreadIssue::MissingBody)
        } else {
            lowered_items.recoveries.first().cloned().or(prepared
                .close_missing
                .then_some(HirThreadIssue::UnclosedBody))
        };
        let body = HirThreadBody::try_new(
            HirThreadBodyOwner::NestedScope(prepared.scope),
            prepared.scope,
            lowered_items.items,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components.stage_attached_nested_thread_body(
            self.request.source(),
            HirThreadBodyOwner::NestedScope(prepared.scope),
            prepared.attached,
            &body,
        )?;
        Ok(LoweredThreadBody { body, recovery })
    }

    fn lower_attached_thread_flow_items(
        &mut self,
        attached: &[AttachedThreadFlowItem],
        scope: ScopeId,
    ) -> Result<LoweredThreadFlowItems, HirLowerFailure> {
        require_limit(HirLimit::ThreadFlowItems, attached.len())?;
        let mut items = Vec::with_capacity(attached.len());
        let mut locals = Vec::<LocalId>::new();
        let mut recoveries = Vec::new();
        for (ordinal, item) in attached.iter().enumerate() {
            let (lowered, poisoned) = if let Some(expression) = item.dialogue_application() {
                let expression = self.lower_attached_expression(&expression, scope)?;
                (
                    HirThreadFlowItem::DialogueApplication(expression),
                    self.staged_expression_is_poisoned(expression)?,
                )
            } else {
                let statement = item
                    .statement()
                    .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                let statement = self.lower_attached_thread_flow_statement(&statement, scope)?;
                locals.extend(statement.locals);
                (
                    thread_flow_statement(item.family(), statement.owner)?,
                    statement.poisoned,
                )
            };
            if poisoned || item.has_recovery() {
                recoveries.push(HirThreadIssue::RecoveredBodyChild {
                    ordinal: u32::try_from(ordinal)
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                });
            }
            items.push(lowered);
        }
        Ok(LoweredThreadFlowItems {
            items: items.into_boxed_slice(),
            locals: locals.into_boxed_slice(),
            recoveries: recoveries.into_boxed_slice(),
        })
    }

    fn close_thread_body_scope(
        &mut self,
        scope: ScopeId,
        prefix_locals: Box<[LocalId]>,
        lowered: &LoweredThreadFlowItems,
    ) -> Result<(), HirLowerFailure> {
        let mut locals = Vec::with_capacity(prefix_locals.len() + lowered.locals.len());
        locals.extend(prefix_locals);
        locals.extend_from_slice(&lowered.locals);
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(scope, locals.into_boxed_slice())
    }

    fn allocate_thread_body_scope(
        &mut self,
        syntax: SyntaxNodeId,
        source: &SourceSpan,
        kind: HirScopeKind,
        owner: HirScopeOwner,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        let source = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation = self
            .arenas
            .scopes()
            .reserve_source(&mut self.slots, syntax, source)?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                scope.module(),
                kind,
                Some(parent),
                owner,
                Box::new([]),
                Box::new([]),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
            self.arenas
                .scopes()
                .finalize(&mut self.slots, reservation, payload)?;
            self.append_scope_child(parent, scope)?;
            return Ok(scope);
        }
        let retained = self.arenas.scopes().resolve_staged(&self.slots, scope)?;
        if retained.kind() == kind
            && retained.parent() == Some(parent)
            && retained.owner() == &owner
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }
}

fn thread_flow_statement(
    family: AttachedThreadFlowItemFamily,
    owner: crate::identity::StmtId,
) -> Result<HirThreadFlowItem, HirLowerFailure> {
    Ok(match family {
        AttachedThreadFlowItemFamily::Statement => HirThreadFlowItem::Statement(owner),
        AttachedThreadFlowItemFamily::DialogueApplication => {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        AttachedThreadFlowItemFamily::Choice => HirThreadFlowItem::Choice(owner),
        AttachedThreadFlowItemFamily::If => HirThreadFlowItem::If(owner),
        AttachedThreadFlowItemFamily::IfLet => HirThreadFlowItem::IfLet(owner),
        AttachedThreadFlowItemFamily::Match => HirThreadFlowItem::Match(owner),
        AttachedThreadFlowItemFamily::While => HirThreadFlowItem::While(owner),
        AttachedThreadFlowItemFamily::WhileLet => HirThreadFlowItem::WhileLet(owner),
        AttachedThreadFlowItemFamily::For => HirThreadFlowItem::For(owner),
        AttachedThreadFlowItemFamily::Select => HirThreadFlowItem::Select(owner),
        AttachedThreadFlowItemFamily::SourceLocale => HirThreadFlowItem::SourceLocale(owner),
        AttachedThreadFlowItemFamily::Scope => HirThreadFlowItem::Scope(owner),
        AttachedThreadFlowItemFamily::Include => HirThreadFlowItem::Include(owner),
        AttachedThreadFlowItemFamily::Error => HirThreadFlowItem::Error(owner),
    })
}

fn owner_syntax_id(
    owner: ExprId,
    transaction: &StagedHirModuleTransaction<'_>,
) -> Result<SyntaxNodeId, HirLowerFailure> {
    let metadata = transaction.slots.resolve_staged(owner)?;
    let crate::slot::HirOrigin::Source(key) = metadata.origin() else {
        return Err(HirInvariantFailure::InvalidArenaCommit.into());
    };
    Ok(key.syntax())
}
