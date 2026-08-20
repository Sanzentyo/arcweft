//! Direct attached block and minimal statement lowering into final HIR.
//!
//! Value blocks own one source-backed lexical scope, source-ordered statement
//! IDs, and exactly one authored or synthetic tail. Statement lowering never
//! reopens source text or routes through the detached statement model.

mod assignment;
mod keyword;
mod required_operand;
mod thread_control;

use arcweft_lang_syntax::attachment::node::{
    AssertionStatementKind, BlockKind, ChoiceStatementKind, ExpressionStatementKind,
    IfStatementKind, IncludeStatementKind, LetStatementKind, MatchArmKind, MatchStatementKind,
    PredicateBlockKind, ProofBlockKind, ProofCallStatementKind, ScopeStatementKind,
    SourceLocaleStatementKind, UnsafeLifetimeStatementKind,
};
use arcweft_lang_syntax::attachment::{
    AstKind, AstNode, AttachedAssertionMode, AttachedExpressionNode, AttachedRequiredIncludeTarget,
    AttachedRequiredNestedThreadFlowBody, AttachedSourceLocaleValue, BlockTailNode,
    IfStatementElseNode, IfStatementHeadNode, LetInitializerNode, MatchStatementArmBodyNode,
    MatchStatementBodyNode, MatchStatementExpressionNode, StatementNode, UnsafeAuditBodyNode,
    UnsafeAuditIdNode, UnsafeAuditReasonNode,
};
use arcweft_lang_syntax::expressions::ExpressionProjection;
use arcweft_lang_syntax::grammar::SyntaxKind;

use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{
    HirBlockExpr, HirExpr, HirExprError, HirExprKind, HirExpressionRecoveryIssue,
    HirGenericExprIssue, HirPoisonState, HirRecoveryIssue, HirThreadIssue,
};
use crate::identity::{
    ExprId, HirLimit, ItemId, LocalId, ScopeId, StmtId, SyntheticKey, SyntheticOwner, SyntheticRole,
};
use crate::leaf::{HirIdRefIssue, HirIdRefRecovery, HirIdRefShape, HirIdRefValue};
use crate::lowering::{HirInvariantFailure, HirLowerFailure, HirLoweringCheckpoint};
use crate::proof_return::HirProofReturnSemanticClass;
use crate::scope::{HirPatternBindingPolicy, HirScope, HirScopeKind, HirScopeOwner};
use crate::source_index::{HirExprSourceRole, HirSourceSite, HirStmtRecoveryOperandSlot};
use crate::stmt::{
    HirAssertionMode, HirConditionalElseBranch, HirContextualStmtBody, HirIfLetStmt, HirIfStmt,
    HirIncludeStmt, HirMatchStmt, HirScopeStmt, HirSourceLocaleIssue, HirSourceLocaleStmt,
    HirSourceLocaleValue, HirStatementContext, HirStmt, HirStmtChildRole, HirStmtKind,
    HirStmtMatchArm, HirStmtMatchArmBody, HirStmtPoisonState, HirStmtRecoveryIssue,
    HirThreadStmtBodyRole, HirThreadStmtRecoveryIssue, HirUnsafeAudit, HirUnsafeLifetimeBody,
};

use super::{StagedHirModuleTransaction, require_limit};

struct LoweredStatement {
    owner: StmtId,
    locals: Box<[LocalId]>,
    poisoned: bool,
}

pub(super) struct LoweredThreadFlowStatement {
    pub(super) owner: StmtId,
    pub(super) locals: Box<[LocalId]>,
    pub(super) poisoned: bool,
}

struct LoweredStatementBlock {
    body: Box<[StmtId]>,
    locals: Box<[LocalId]>,
    poisoned: bool,
    first_poisoned: Option<u32>,
}

#[derive(Clone, Copy)]
pub(super) enum OmittedValueTail {
    ImplicitUnit,
    MissingRequired,
}

pub(super) struct LoweredValueBlock {
    pub(super) scope: ScopeId,
    pub(super) statements: Box<[StmtId]>,
    pub(super) tail: ExprId,
    pub(super) recovery: Option<HirRecoveryIssue>,
}

pub(super) struct LoweredStatementOnlyBlock {
    pub(super) statements: Box<[StmtId]>,
    pub(super) poisoned: bool,
}

#[derive(Clone, Copy)]
enum ValueBlockOwner {
    Expression(ExprId),
    Item(ItemId),
}

struct ValueBlockInput {
    statements: Vec<StatementNode>,
    tail: BlockTailNode,
    owner: ValueBlockOwner,
    scope: ScopeId,
    prefix_locals: Box<[LocalId]>,
    omitted_tail: OmittedValueTail,
    statement_context: HirStatementContext,
}

impl ValueBlockOwner {
    const fn scope_owner(self) -> HirScopeOwner {
        match self {
            Self::Expression(owner) => HirScopeOwner::Expr(owner),
            Self::Item(owner) => HirScopeOwner::Item(owner),
        }
    }

    const fn module(self) -> crate::identity::HirModuleId {
        match self {
            Self::Expression(owner) => owner.module(),
            Self::Item(owner) => owner.module(),
        }
    }
}

impl StagedHirModuleTransaction<'_> {
    pub(super) fn lower_attached_thread_flow_statement(
        &mut self,
        statement: &StatementNode,
        scope: ScopeId,
    ) -> Result<LoweredThreadFlowStatement, HirLowerFailure> {
        let lowered =
            self.lower_attached_statement(statement, scope, HirStatementContext::Thread)?;
        Ok(LoweredThreadFlowStatement {
            owner: lowered.owner,
            locals: lowered.locals,
            poisoned: lowered.poisoned,
        })
    }

    pub(super) fn allocate_expression_owned_block_scope<K: AstKind>(
        &mut self,
        block: &AstNode<K>,
        owner: ExprId,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        self.allocate_block_scope(
            block.id(),
            block.source_span(),
            ValueBlockOwner::Expression(owner),
            parent,
        )
    }

    pub(super) fn lower_attached_statement_only_block(
        &mut self,
        block: &AstNode<BlockKind>,
        scope: ScopeId,
    ) -> Result<LoweredStatementOnlyBlock, HirLowerFailure> {
        self.lower_attached_statement_only_block_with_prefix(block, scope, Box::new([]))
    }

    pub(super) fn lower_attached_statement_only_block_with_prefix(
        &mut self,
        block: &AstNode<BlockKind>,
        scope: ScopeId,
        prefix_locals: Box<[LocalId]>,
    ) -> Result<LoweredStatementOnlyBlock, HirLowerFailure> {
        let lowered =
            self.lower_attached_statement_block(block, scope, HirStatementContext::Ordinary)?;
        let mut locals = Vec::with_capacity(prefix_locals.len() + lowered.locals.len());
        locals.extend(prefix_locals);
        locals.extend(lowered.locals);
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(scope, locals.into_boxed_slice())?;
        Ok(LoweredStatementOnlyBlock {
            statements: lowered.body,
            poisoned: lowered.poisoned,
        })
    }

    pub(super) fn lower_attached_value_block(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        parent_scope: ScopeId,
    ) -> Result<(HirBlockExpr, Option<HirRecoveryIssue>), HirLowerFailure> {
        let lowered = self.lower_attached_value_block_parts(
            attached,
            owner,
            parent_scope,
            OmittedValueTail::ImplicitUnit,
        )?;
        Ok((
            HirBlockExpr::new(lowered.scope, lowered.statements, lowered.tail),
            lowered.recovery,
        ))
    }

    pub(super) fn lower_attached_value_block_parts(
        &mut self,
        attached: &AttachedExpressionNode,
        owner: ExprId,
        parent_scope: ScopeId,
        omitted_tail: OmittedValueTail,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let block = attached
            .block()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let tail = block
            .tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let scope = self.allocate_block_scope(
            block.id(),
            block.source_span(),
            ValueBlockOwner::Expression(owner),
            parent_scope,
        )?;
        self.lower_value_block(ValueBlockInput {
            statements,
            tail,
            owner: ValueBlockOwner::Expression(owner),
            scope,
            prefix_locals: Box::new([]),
            omitted_tail,
            statement_context: HirStatementContext::Ordinary,
        })
    }

    pub(super) fn lower_attached_predicate_block(
        &mut self,
        block: &AstNode<PredicateBlockKind>,
        owner: ItemId,
        body_scope: ScopeId,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let tail = block
            .tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_value_block(ValueBlockInput {
            statements,
            tail,
            owner: ValueBlockOwner::Item(owner),
            scope: body_scope,
            prefix_locals: Box::new([]),
            omitted_tail: OmittedValueTail::MissingRequired,
            statement_context: HirStatementContext::Predicate,
        })
    }

    pub(super) fn lower_attached_proof_block(
        &mut self,
        block: &AstNode<ProofBlockKind>,
        owner: ItemId,
        body_scope: ScopeId,
        return_semantic_class: HirProofReturnSemanticClass,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let tail = block
            .tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_value_block(ValueBlockInput {
            statements,
            tail,
            owner: ValueBlockOwner::Item(owner),
            scope: body_scope,
            prefix_locals: Box::new([]),
            omitted_tail: if return_semantic_class.admits_implicit_unit_tail() {
                OmittedValueTail::ImplicitUnit
            } else {
                OmittedValueTail::MissingRequired
            },
            statement_context: HirStatementContext::Proof,
        })
    }

    pub(super) fn lower_attached_function_block(
        &mut self,
        block: &AstNode<BlockKind>,
        owner: ItemId,
        body_scope: ScopeId,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let tail = block
            .tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_value_block(ValueBlockInput {
            statements,
            tail,
            owner: ValueBlockOwner::Item(owner),
            scope: body_scope,
            prefix_locals: Box::new([]),
            omitted_tail: OmittedValueTail::ImplicitUnit,
            statement_context: HirStatementContext::Ordinary,
        })
    }

    /// Lowers a Trait/Impl default body directly in its one callable scope.
    /// Parameter locals remain the lexical prefix and the scope is closed once
    /// after source-ordered body locals are appended.
    pub(super) fn lower_attached_method_block(
        &mut self,
        block: &AstNode<BlockKind>,
        owner: ItemId,
        callable_scope: ScopeId,
        parameter_locals: Box<[LocalId]>,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let tail = block
            .tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_value_block(ValueBlockInput {
            statements,
            tail,
            owner: ValueBlockOwner::Item(owner),
            scope: callable_scope,
            prefix_locals: parameter_locals,
            omitted_tail: OmittedValueTail::ImplicitUnit,
            statement_context: HirStatementContext::Ordinary,
        })
    }

    fn lower_value_block(
        &mut self,
        input: ValueBlockInput,
    ) -> Result<LoweredValueBlock, HirLowerFailure> {
        let ValueBlockInput {
            statements,
            tail,
            owner,
            scope,
            prefix_locals,
            omitted_tail,
            statement_context,
        } = input;
        require_limit(HirLimit::Statements, statements.len())?;
        let mut statement_ids = Vec::with_capacity(statements.len());
        let mut locals = prefix_locals.into_vec();
        let mut recovery = None;
        for (ordinal, statement) in statements.iter().enumerate() {
            let lowered = self.lower_attached_statement(statement, scope, statement_context)?;
            if lowered.poisoned {
                recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::RecoveredChild {
                        role: HirExprSourceRole::Statement {
                            ordinal: u32::try_from(ordinal)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        },
                    },
                ));
            }
            statement_ids.push(lowered.owner);
            locals.extend(lowered.locals);
        }
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(scope, locals.into_boxed_slice())?;

        let tail = match tail {
            BlockTailNode::Expression(expression) => {
                let semantic = expression
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let tail = self.lower_attached_expression(&semantic, scope)?;
                if self.staged_expression_is_poisoned(tail)? {
                    recovery.get_or_insert(HirRecoveryIssue::InvalidExpression(
                        HirExpressionRecoveryIssue::RecoveredChild {
                            role: HirExprSourceRole::Tail,
                        },
                    ));
                }
                tail
            }
            BlockTailNode::Omitted(omitted) => match omitted_tail {
                OmittedValueTail::ImplicitUnit => match owner {
                    ValueBlockOwner::Expression(owner) => {
                        self.lower_implicit_unit_tail(owner, scope, &omitted.source_span())?
                    }
                    ValueBlockOwner::Item(_) => {
                        self.lower_implicit_unit_tail_for_scope(scope, &omitted.source_span())?
                    }
                },
                OmittedValueTail::MissingRequired => {
                    recovery.get_or_insert(HirRecoveryIssue::MissingRequiredTail);
                    match owner {
                        ValueBlockOwner::Expression(owner) => {
                            self.lower_missing_required_tail(owner, scope, &omitted.source_span())?
                        }
                        ValueBlockOwner::Item(_) => self
                            .lower_missing_required_tail_for_scope(scope, &omitted.source_span())?,
                    }
                }
            },
        };

        Ok(LoweredValueBlock {
            scope,
            statements: statement_ids.into_boxed_slice(),
            tail,
            recovery,
        })
    }

    fn allocate_block_scope(
        &mut self,
        block_id: arcweft_lang_syntax::attachment::SyntaxNodeId,
        block_source: arcweft_source::SourceSpan,
        owner: ValueBlockOwner,
        parent: ScopeId,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            block_id,
            HirSourceSite::Span(block_source),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                HirScopeKind::Block,
                Some(parent),
                owner.scope_owner(),
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
        if retained.kind() == HirScopeKind::Block
            && retained.parent() == Some(parent)
            && retained.owner() == &owner.scope_owner()
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    pub(super) fn append_scope_child(
        &mut self,
        parent: ScopeId,
        child: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        let retained = self
            .arenas
            .scopes()
            .resolve_staged(&self.slots, parent)?
            .clone();
        let mut children = retained.children().to_vec();
        if !children.contains(&child) {
            children.push(child);
        }
        let revised = retained
            .try_with_members(
                children.into_boxed_slice(),
                retained.locals().to_vec().into_boxed_slice(),
            )
            .map_err(|_| HirInvariantFailure::InvalidScopeParent)?;
        self.arenas
            .scopes()
            .revise_finalized(&mut self.slots, parent, revised)?;
        Ok(())
    }

    pub(super) fn close_scope_members(
        &mut self,
        scope: ScopeId,
        locals: Box<[LocalId]>,
    ) -> Result<(), HirLowerFailure> {
        let retained = self
            .arenas
            .scopes()
            .resolve_staged(&self.slots, scope)?
            .clone();
        let revised = retained
            .try_with_members(retained.children().to_vec().into_boxed_slice(), locals)
            .map_err(|_| HirInvariantFailure::InvalidLocalTimeline)?;
        self.arenas
            .scopes()
            .revise_finalized(&mut self.slots, scope, revised)?;
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed statement family is one exhaustive typed lowering and source-staging transaction"
    )]
    fn lower_attached_statement(
        &mut self,
        attached: &StatementNode,
        scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<LoweredStatement, HirLowerFailure> {
        self.validate_attached_statement(attached, scope)?;
        let reservation = self.arenas.statements().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(attached.source_span()),
        )?;
        let owner = reservation.id();
        self.control
            .checkpoint(HirLoweringCheckpoint::ChildReserved)?;
        if !reservation.is_first_touch() {
            let retained = self
                .arenas
                .statements()
                .resolve_staged(&self.slots, owner)?;
            if retained.scope() != scope {
                return Err(HirInvariantFailure::InvalidArenaCommit.into());
            }
            return Ok(LoweredStatement {
                owner,
                locals: retained.kind().post_statement_locals().into(),
                poisoned: retained.is_poisoned(),
            });
        }

        let (kind, locals, recovery) = match attached.kind() {
            SyntaxKind::AssertionStatement => {
                let statement = attached
                    .cast::<AssertionStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let assertion = statement
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                require_limit(HirLimit::AssertionConditions, assertion.conditions().len())?;
                let mut condition_recovery = false;
                let conditions = assertion
                    .conditions()
                    .iter()
                    .map(|condition| {
                        let condition = self.lower_attached_expression(condition, scope)?;
                        condition_recovery |= self.staged_expression_is_poisoned(condition)?;
                        Ok(condition)
                    })
                    .collect::<Result<Vec<_>, HirLowerFailure>>()?
                    .into_boxed_slice();
                let mode = match assertion.mode() {
                    AttachedAssertionMode::Resolved { value, .. } => {
                        HirAssertionMode::Resolved(*value)
                    }
                    AttachedAssertionMode::Recovered { .. } => HirAssertionMode::Recovered,
                };
                let recovery = if matches!(mode, HirAssertionMode::Recovered) {
                    Some(HirStmtRecoveryIssue::InvalidAssertionMode)
                } else if conditions.is_empty() {
                    Some(HirStmtRecoveryIssue::MissingAssertionCondition)
                } else if condition_recovery {
                    Some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Condition,
                    })
                } else if assertion.has_recovery() {
                    Some(HirStmtRecoveryIssue::MalformedAssertion)
                } else {
                    None
                };
                (
                    HirStmtKind::Assertion { mode, conditions },
                    Box::<[LocalId]>::from([]),
                    recovery,
                )
            }
            SyntaxKind::ProofCallStatement => {
                let statement = attached
                    .cast::<ProofCallStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let call = statement
                    .callee()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let call = self.lower_attached_expression(&call, scope)?;
                let recovery = self.staged_expression_is_poisoned(call)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    },
                );
                (
                    HirStmtKind::ProofCall { call },
                    Box::<[LocalId]>::from([]),
                    recovery,
                )
            }
            SyntaxKind::LetStatement => {
                let statement = attached
                    .cast::<LetStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let initializer = match statement
                    .initializer()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                {
                    Some(LetInitializerNode::Expression(initializer)) => {
                        let initializer = initializer
                            .semantic()
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        self.lower_attached_expression(&initializer, scope)?
                    }
                    Some(LetInitializerNode::Missing(missing)) => self
                        .lower_missing_statement_expression(
                            owner,
                            scope,
                            HirStmtRecoveryOperandSlot::LetInitializer {
                                insertion: missing.range().start(),
                            },
                        )?,
                    None => self.lower_missing_statement_expression(
                        owner,
                        scope,
                        HirStmtRecoveryOperandSlot::LetInitializer {
                            insertion: attached.range().end(),
                        },
                    )?,
                };
                let initializer_poisoned = self.staged_expression_is_poisoned(initializer)?;
                let pattern = statement
                    .pattern()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let lowered = self.lower_attached_pattern_binding(
                    &pattern,
                    scope,
                    context.let_binding_policy(),
                )?;
                let pattern_poisoned = lowered.poisoned;
                let locals = lowered.locals;
                (
                    HirStmtKind::Let {
                        pattern: lowered.owner,
                        annotation: None,
                        initializer,
                        locals: locals.clone(),
                    },
                    locals,
                    if pattern_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Pattern,
                        })
                    } else if initializer_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Initializer,
                        })
                    } else {
                        None
                    },
                )
            }
            SyntaxKind::AssignmentStatement | SyntaxKind::LifetimeSetStatement => {
                let (kind, recovery) =
                    self.lower_attached_assignment_statement(attached, owner, scope)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ChoiceStatement => {
                Self::require_thread_statement_context(context)?;
                let statement = attached
                    .cast::<ChoiceStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantics()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let expression = statement
                    .expression()
                    .expression_node()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let choice = self.lower_attached_expression(&expression, scope)?;
                let recovery = self.staged_expression_is_poisoned(choice)?.then_some(
                    HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    },
                );
                (
                    HirStmtKind::Choice { choice },
                    Box::<[LocalId]>::from([]),
                    recovery,
                )
            }
            SyntaxKind::ReturnStatement
            | SyntaxKind::YieldStatement
            | SyntaxKind::WaitStatement
            | SyntaxKind::CloseStatement => {
                let (kind, recovery) =
                    self.lower_attached_required_operand_statement(attached, owner, scope)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::WhileStatement
            | SyntaxKind::WhileLetStatement
            | SyntaxKind::ForStatement
            | SyntaxKind::SelectStatement => {
                let (kind, recovery) =
                    self.lower_attached_thread_control_statement(attached, owner, scope, context)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ExpressionStatement => {
                let statement = attached
                    .cast::<ExpressionStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let expression = statement
                    .expression()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let expression = self.lower_attached_expression(&expression, scope)?;
                let poisoned = self.staged_expression_is_poisoned(expression)?;
                (
                    HirStmtKind::Expression { expression },
                    Box::<[LocalId]>::from([]),
                    poisoned.then_some(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Expression,
                    }),
                )
            }
            SyntaxKind::OutStatement
            | SyntaxKind::GotoStatement
            | SyntaxKind::DeferStatement
            | SyntaxKind::SignalStatement
            | SyntaxKind::BreakStatement
            | SyntaxKind::ContinueStatement => {
                let (kind, recovery) =
                    self.lower_attached_keyword_statement(attached, owner, scope)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::IfStatement => {
                let statement = attached
                    .cast::<IfStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) =
                    self.lower_attached_if_statement(&statement, owner, scope, context)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::MatchStatement => {
                let statement = attached
                    .cast::<MatchStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) =
                    self.lower_attached_match_statement(&statement, owner, scope, context)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::SourceLocaleStatement => {
                Self::require_thread_statement_context(context)?;
                let statement = attached
                    .cast::<SourceLocaleStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) =
                    self.lower_attached_source_locale_statement(&statement, owner, scope)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ScopeStatement => {
                Self::require_thread_statement_context(context)?;
                let statement = attached
                    .cast::<ScopeStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) =
                    self.lower_attached_scope_statement(&statement, owner, scope)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::IncludeStatement => {
                Self::require_thread_statement_context(context)?;
                let statement = attached
                    .cast::<IncludeStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) = Self::lower_attached_include_statement(&statement)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::UnsafeLifetimeStatement => {
                let statement = attached
                    .cast::<UnsafeLifetimeStatementKind>()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (kind, recovery) = self
                    .lower_attached_unsafe_lifetime_statement(&statement, owner, scope, context)?;
                (kind, Box::<[LocalId]>::from([]), recovery)
            }
            SyntaxKind::ErrorStatement => (
                HirStmtKind::Error,
                Box::<[LocalId]>::from([]),
                Some(HirStmtRecoveryIssue::UnclassifiedSyntax),
            ),
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };
        let state = recovery.map_or(HirStmtPoisonState::Clean, HirStmtPoisonState::Poisoned);
        let payload = HirStmt::try_new_with_state(scope, kind, state)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.source_components.stage_attached_stmt(
            self.request.source(),
            owner,
            attached,
            &payload,
        )?;
        if payload.is_poisoned() {
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                SyntheticOwner::Stmt(owner),
                HirRecoveryPrimary::owner_whole(SyntheticOwner::Stmt(owner)),
                HirSourceSite::Span(attached.source_span()),
            ));
        }
        let poisoned = payload.is_poisoned();
        self.arenas
            .statements()
            .finalize(&mut self.slots, reservation, payload)?;
        Ok(LoweredStatement {
            owner,
            locals,
            poisoned,
        })
    }

    fn require_thread_statement_context(
        context: HirStatementContext,
    ) -> Result<(), HirLowerFailure> {
        if matches!(context, HirStatementContext::Thread) {
            Ok(())
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }

    fn lower_attached_source_locale_statement(
        &mut self,
        attached: &AstNode<SourceLocaleStatementKind>,
        owner: StmtId,
        outer_scope: ScopeId,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let attached = attached
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let (locale, locale_recovery) = match attached.locale() {
            AttachedSourceLocaleValue::Authored {
                value: Ok(value), ..
            } => (HirSourceLocaleValue::Resolved(value.clone()), None),
            AttachedSourceLocaleValue::Authored { value: Err(_), .. } => (
                HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Invalid),
                Some(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::InvalidSourceLocale(HirSourceLocaleIssue::Invalid),
                )),
            ),
            AttachedSourceLocaleValue::Missing(_) => (
                HirSourceLocaleValue::Recovered(HirSourceLocaleIssue::Missing),
                Some(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::InvalidSourceLocale(HirSourceLocaleIssue::Missing),
                )),
            ),
        };
        let lowered = self.lower_attached_nested_thread_body(
            attached.body(),
            HirScopeOwner::Stmt(owner),
            outer_scope,
        )?;
        let body_recovery = nested_thread_body_recovery(
            lowered.recovery.as_ref(),
            HirThreadStmtBodyRole::SourceLocale,
        )?;
        let body = HirContextualStmtBody::try_thread(lowered.body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let statement = HirSourceLocaleStmt::try_new(locale, body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((
            HirStmtKind::SourceLocale(statement),
            locale_recovery.or(body_recovery),
        ))
    }

    fn lower_attached_scope_statement(
        &mut self,
        attached: &AstNode<ScopeStatementKind>,
        owner: StmtId,
        outer_scope: ScopeId,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let attached = attached
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let (name, name_recovery) = match attached.name() {
            None => (None, None),
            Some(name) => match name.value() {
                Ok(value) => (Some(super::name_projection::name(value)?), None),
                Err(issue) => {
                    super::name_projection::require_attempted_name_limit(issue)?;
                    (
                        None,
                        Some(HirStmtRecoveryIssue::Thread(
                            HirThreadStmtRecoveryIssue::InvalidScopeName(
                                super::name_projection::name_issue(issue),
                            ),
                        )),
                    )
                }
            },
        };
        let lowered = self.lower_attached_nested_thread_body(
            attached.body(),
            HirScopeOwner::Stmt(owner),
            outer_scope,
        )?;
        let body_recovery =
            nested_thread_body_recovery(lowered.recovery.as_ref(), HirThreadStmtBodyRole::Scope)?;
        let body = HirContextualStmtBody::try_thread(lowered.body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let statement = HirScopeStmt::try_new(name, body)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((
            HirStmtKind::Scope(statement),
            name_recovery.or(body_recovery),
        ))
    }

    fn lower_attached_include_statement(
        attached: &AstNode<IncludeStatementKind>,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let attached = attached
            .semantics()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let target = match attached.target() {
            AttachedRequiredIncludeTarget::Reference(reference) => {
                super::id_ref_projection::id_ref(reference.value())?
            }
            AttachedRequiredIncludeTarget::Missing(_) => HirIdRefValue::Recovered(
                HirIdRefRecovery::new(HirIdRefShape::Missing, HirIdRefIssue::Missing),
            ),
        };
        let recovery = target
            .recovery_issue()
            .map(|issue| {
                HirStmtRecoveryIssue::Thread(HirThreadStmtRecoveryIssue::InvalidIncludeTarget(
                    issue,
                ))
            })
            .or_else(|| {
                attached
                    .trailing_recovery()
                    .map(|_| HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Target,
                    })
            });
        Ok((HirStmtKind::Include(HirIncludeStmt::new(target)), recovery))
    }

    fn lower_attached_match_statement(
        &mut self,
        attached: &AstNode<MatchStatementKind>,
        owner: StmtId,
        outer_scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let scrutinee = match attached
            .scrutinee()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            MatchStatementExpressionNode::Expression(scrutinee) => {
                let scrutinee = scrutinee
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                self.lower_attached_expression(&scrutinee, outer_scope)?
            }
            MatchStatementExpressionNode::Missing(missing) => self
                .lower_missing_statement_expression(
                    owner,
                    outer_scope,
                    HirStmtRecoveryOperandSlot::MatchScrutinee {
                        insertion: missing.range().start(),
                    },
                )?,
        };
        let mut recovery = self.staged_expression_is_poisoned(scrutinee)?.then_some(
            HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::Scrutinee,
            },
        );

        let attached_body = attached
            .body_or_missing()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let attached_arms = attached_body
            .arms()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let body_unclosed = match &attached_body {
            MatchStatementBodyNode::Missing(_) => {
                recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                    HirThreadStmtRecoveryIssue::MissingBody {
                        role: HirThreadStmtBodyRole::Match,
                    },
                ));
                false
            }
            MatchStatementBodyNode::Block(block) => block
                .close_delimiter()
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                .range()
                .is_empty(),
        };
        let mut arms = Vec::with_capacity(attached_arms.len());
        for (arm_ordinal, attached_arm) in attached_arms.iter().enumerate() {
            let arm_ordinal =
                u32::try_from(arm_ordinal).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            let (arm, pattern_poisoned, guard_poisoned, body_poisoned) = self
                .lower_attached_match_arm(attached_arm, owner, arm_ordinal, outer_scope, context)?;
            if pattern_poisoned {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmPattern { arm: arm_ordinal },
                });
            }
            if guard_poisoned {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmGuard { arm: arm_ordinal },
                });
            }
            if body_poisoned {
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::MatchArmBody { arm: arm_ordinal },
                });
            }
            arms.push(arm);
        }
        if arms.is_empty() && !matches!(attached_body, MatchStatementBodyNode::Missing(_)) {
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::EmptyMatch,
            ));
        }
        if body_unclosed {
            recovery.get_or_insert(HirStmtRecoveryIssue::Thread(
                HirThreadStmtRecoveryIssue::UnclosedBody {
                    role: HirThreadStmtBodyRole::Match,
                },
            ));
        }

        Ok((
            HirStmtKind::Match(
                HirMatchStmt::try_new(scrutinee, arms.into_boxed_slice())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            ),
            recovery,
        ))
    }

    fn lower_attached_match_arm(
        &mut self,
        attached: &AstNode<MatchArmKind>,
        owner: StmtId,
        arm_ordinal: u32,
        outer_scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<(HirStmtMatchArm, bool, bool, bool), HirLowerFailure> {
        let body = attached
            .body()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        if let (HirStatementContext::Thread, MatchStatementArmBodyNode::Block(block)) =
            (context, &body)
        {
            let attached_body = AttachedRequiredNestedThreadFlowBody::Present(
                block
                    .thread_flow_body()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
            let prepared = self.prepare_attached_nested_thread_body(
                &attached_body,
                HirScopeOwner::Stmt(owner),
                outer_scope,
            )?;
            let scope = prepared.scope();
            let (pattern, pattern_poisoned) = self.lower_match_arm_pattern(attached, scope)?;
            let (guard, guard_poisoned) =
                self.lower_match_arm_guard(attached, owner, arm_ordinal, scope)?;
            let lowered_body =
                self.finish_attached_nested_thread_body(prepared, pattern.locals.clone())?;
            let body_poisoned = lowered_body.recovery.is_some();
            let body = HirStmtMatchArmBody::Body(
                HirContextualStmtBody::try_thread(lowered_body.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
            );
            let arm = HirStmtMatchArm::try_new(scope, pattern.owner, guard, body, pattern.locals)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
            return Ok((arm, pattern_poisoned, guard_poisoned, body_poisoned));
        }

        let scope =
            self.allocate_statement_scope(attached, owner, outer_scope, HirScopeKind::MatchArm)?;
        let (pattern, pattern_poisoned) = self.lower_match_arm_pattern(attached, scope)?;
        let (guard, guard_poisoned) =
            self.lower_match_arm_guard(attached, owner, arm_ordinal, scope)?;
        let mut locals = pattern.locals.to_vec();
        let (body, body_poisoned) = match body {
            MatchStatementArmBodyNode::Expression(expression) => {
                let expression = expression
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let expression = self.lower_attached_expression(&expression, scope)?;
                let poisoned = self.staged_expression_is_poisoned(expression)?;
                (HirStmtMatchArmBody::Expression(expression), poisoned)
            }
            MatchStatementArmBodyNode::Missing(missing) => {
                let expression =
                    self.lower_missing_required_tail_for_scope(scope, &missing.source_span())?;
                (HirStmtMatchArmBody::Expression(expression), true)
            }
            MatchStatementArmBodyNode::Statement(statement) => {
                let lowered = self.lower_attached_statement(&statement, scope, context)?;
                locals.extend(lowered.locals);
                let body = HirContextualStmtBody::try_ordinary(
                    scope,
                    Box::<[StmtId]>::from([lowered.owner]),
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (HirStmtMatchArmBody::Body(body), lowered.poisoned)
            }
            MatchStatementArmBodyNode::Block(block) => {
                let lowered = self.lower_attached_statement_block(&block, scope, context)?;
                locals.extend(lowered.locals);
                let body = HirContextualStmtBody::try_ordinary(scope, lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                (HirStmtMatchArmBody::Body(body), lowered.poisoned)
            }
        };
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        self.close_scope_members(scope, locals.into_boxed_slice())?;
        let arm = HirStmtMatchArm::try_new(scope, pattern.owner, guard, body, pattern.locals)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        Ok((arm, pattern_poisoned, guard_poisoned, body_poisoned))
    }

    fn lower_match_arm_pattern(
        &mut self,
        attached: &AstNode<MatchArmKind>,
        scope: ScopeId,
    ) -> Result<(super::pattern_lowering::LoweredPattern, bool), HirLowerFailure> {
        let pattern = attached
            .pattern()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .semantic()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let lowered = self.lower_attached_pattern_binding(
            &pattern,
            scope,
            HirPatternBindingPolicy::MatchBinding,
        )?;
        let poisoned = lowered.poisoned;
        Ok((lowered, poisoned))
    }

    fn lower_match_arm_guard(
        &mut self,
        attached: &AstNode<MatchArmKind>,
        owner: StmtId,
        arm_ordinal: u32,
        scope: ScopeId,
    ) -> Result<(Option<ExprId>, bool), HirLowerFailure> {
        let guard = match attached
            .guard()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            None => None,
            Some(MatchStatementExpressionNode::Expression(guard)) => {
                let guard = guard
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Some(self.lower_attached_expression(&guard, scope)?)
            }
            Some(MatchStatementExpressionNode::Missing(missing)) => {
                Some(self.lower_missing_statement_expression(
                    owner,
                    scope,
                    HirStmtRecoveryOperandSlot::MatchArmGuard {
                        insertion: missing.range().start(),
                        arm: arm_ordinal,
                    },
                )?)
            }
        };
        let poisoned = guard
            .map(|guard| self.staged_expression_is_poisoned(guard))
            .transpose()?
            .unwrap_or(false);
        Ok((guard, poisoned))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "If and IfLet share one closed branch, scope, local, and recovery lowering matrix"
    )]
    fn lower_attached_if_statement(
        &mut self,
        attached: &AstNode<IfStatementKind>,
        owner: StmtId,
        outer_scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        match attached
            .head()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            IfStatementHeadNode::Condition(condition) => {
                let condition = condition
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let condition = self.lower_attached_expression(&condition, outer_scope)?;
                let condition_poisoned = self.staged_expression_is_poisoned(condition)?;

                let then_block = attached
                    .then_branch()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (then_body, then_poisoned) = self.lower_contextual_statement_body(
                    &then_block,
                    owner,
                    outer_scope,
                    HirScopeKind::Conditional,
                    context,
                )?;

                let (else_branch, else_poisoned) = match attached
                    .else_branch()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                {
                    None => (None, false),
                    Some(IfStatementElseNode::Block(block)) => {
                        let (body, poisoned) = self.lower_contextual_statement_body(
                            &block,
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            context,
                        )?;
                        (Some(HirConditionalElseBranch::body(body)), poisoned)
                    }
                    Some(IfStatementElseNode::If(statement)) => {
                        let nested =
                            self.lower_attached_statement(&statement, outer_scope, context)?;
                        (
                            Some(HirConditionalElseBranch::else_if(nested.owner)),
                            nested.poisoned,
                        )
                    }
                };

                let statement = HirIfStmt::try_new(condition, then_body, else_branch)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

                Ok((
                    HirStmtKind::If(statement),
                    if condition_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Condition,
                        })
                    } else if then_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ThenBranch,
                        })
                    } else if else_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ElseBranch,
                        })
                    } else {
                        None
                    },
                ))
            }
            IfStatementHeadNode::Let {
                pattern,
                scrutinee,
                guard,
            } => {
                let scrutinee = scrutinee
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let scrutinee = self.lower_attached_expression(&scrutinee, outer_scope)?;
                let scrutinee_poisoned = self.staged_expression_is_poisoned(scrutinee)?;

                let then_block = attached
                    .then_branch()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let pattern = pattern
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let (lowered_pattern, guard, then_body, then_poisoned) = match context {
                    HirStatementContext::Thread => {
                        let body = AttachedRequiredNestedThreadFlowBody::Present(
                            then_block
                                .thread_flow_body()
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                        );
                        let prepared = self.prepare_attached_nested_thread_body(
                            &body,
                            HirScopeOwner::Stmt(owner),
                            outer_scope,
                        )?;
                        let then_scope = prepared.scope();
                        let lowered_pattern = self.lower_attached_pattern_binding(
                            &pattern,
                            then_scope,
                            HirPatternBindingPolicy::PatternBinding,
                        )?;
                        let guard = guard
                            .map(|guard| {
                                let guard = guard
                                    .semantic()
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                                self.lower_attached_expression(&guard, then_scope)
                            })
                            .transpose()?;
                        let lowered_body = self.finish_attached_nested_thread_body(
                            prepared,
                            lowered_pattern.locals.clone(),
                        )?;
                        let poisoned = lowered_body.recovery.is_some();
                        let body = HirContextualStmtBody::try_thread(lowered_body.body)
                            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        (lowered_pattern, guard, body, poisoned)
                    }
                    HirStatementContext::Ordinary
                    | HirStatementContext::Predicate
                    | HirStatementContext::Proof => {
                        let then_scope = self.allocate_statement_scope(
                            &then_block,
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                        )?;
                        let lowered_pattern = self.lower_attached_pattern_binding(
                            &pattern,
                            then_scope,
                            HirPatternBindingPolicy::PatternBinding,
                        )?;
                        let guard = guard
                            .map(|guard| {
                                let guard = guard
                                    .semantic()
                                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                                self.lower_attached_expression(&guard, then_scope)
                            })
                            .transpose()?;
                        let lowered_body =
                            self.lower_attached_statement_block(&then_block, then_scope, context)?;
                        let mut locals = lowered_pattern.locals.to_vec();
                        locals.extend(lowered_body.locals);
                        require_limit(HirLimit::LocalsPerScope, locals.len())?;
                        self.close_scope_members(then_scope, locals.into_boxed_slice())?;
                        let poisoned = lowered_body.poisoned;
                        let body =
                            HirContextualStmtBody::try_ordinary(then_scope, lowered_body.body)
                                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                        (lowered_pattern, guard, body, poisoned)
                    }
                };
                let pattern_poisoned = lowered_pattern.poisoned;
                let pattern_locals = lowered_pattern.locals;
                let guard_poisoned = guard
                    .map(|guard| self.staged_expression_is_poisoned(guard))
                    .transpose()?
                    .unwrap_or(false);

                let (else_branch, else_poisoned) = match attached
                    .else_branch()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                {
                    None => (None, false),
                    Some(IfStatementElseNode::Block(block)) => {
                        let (body, poisoned) = self.lower_contextual_statement_body(
                            &block,
                            owner,
                            outer_scope,
                            HirScopeKind::Conditional,
                            context,
                        )?;
                        (Some(HirConditionalElseBranch::body(body)), poisoned)
                    }
                    Some(IfStatementElseNode::If(statement)) => {
                        let nested =
                            self.lower_attached_statement(&statement, outer_scope, context)?;
                        (
                            Some(HirConditionalElseBranch::else_if(nested.owner)),
                            nested.poisoned,
                        )
                    }
                };
                let if_let = HirIfLetStmt::try_new(
                    lowered_pattern.owner,
                    scrutinee,
                    guard,
                    then_body,
                    pattern_locals,
                    else_branch,
                )
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((
                    HirStmtKind::IfLet(if_let),
                    if pattern_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Pattern,
                        })
                    } else if scrutinee_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Scrutinee,
                        })
                    } else if guard_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::Guard,
                        })
                    } else if then_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ThenBranch,
                        })
                    } else if else_poisoned {
                        Some(HirStmtRecoveryIssue::RecoveredChild {
                            role: HirStmtChildRole::ElseBranch,
                        })
                    } else {
                        None
                    },
                ))
            }
        }
    }

    fn lower_attached_unsafe_lifetime_statement(
        &mut self,
        attached: &AstNode<UnsafeLifetimeStatementKind>,
        owner: StmtId,
        outer_scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<(HirStmtKind, Option<HirStmtRecoveryIssue>), HirLowerFailure> {
        let audit_id = match attached
            .audit_id()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            UnsafeAuditIdNode::Reference(reference) => {
                let semantic = reference
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let ExpressionProjection::EntityReference(reference) = semantic.projection() else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                super::id_ref_projection::id_ref(reference)?
            }
            UnsafeAuditIdNode::Missing(_) => HirIdRefValue::Recovered(HirIdRefRecovery::new(
                HirIdRefShape::Missing,
                HirIdRefIssue::Missing,
            )),
        };
        let mut recovery = audit_id
            .recovery_issue()
            .map(HirStmtRecoveryIssue::InvalidAuditId);

        let reason = match attached
            .reason()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            None => None,
            Some(UnsafeAuditReasonNode::Expression(reason)) => {
                let reason = reason
                    .semantic()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                let reason = self.lower_attached_expression(&reason, outer_scope)?;
                if self.staged_expression_is_poisoned(reason)? {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::Reason,
                    });
                }
                Some(reason)
            }
            Some(UnsafeAuditReasonNode::Missing(missing)) => {
                let reason = self.lower_missing_statement_expression(
                    owner,
                    outer_scope,
                    HirStmtRecoveryOperandSlot::UnsafeAuditReason {
                        insertion: missing.range().start(),
                    },
                )?;
                recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                    role: HirStmtChildRole::Reason,
                });
                Some(reason)
            }
        };

        let (body, has_safety_doc) = match attached
            .body_or_missing()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
        {
            UnsafeAuditBodyNode::Missing(_) => {
                recovery.get_or_insert(HirStmtRecoveryIssue::MissingBody);
                (HirUnsafeLifetimeBody::Missing, false)
            }
            UnsafeAuditBodyNode::Block(block) => {
                let body_scope =
                    self.allocate_statement_scope(&block, owner, outer_scope, HirScopeKind::Block)?;
                let lowered = self.lower_attached_statement_block(&block, body_scope, context)?;
                self.close_scope_members(body_scope, lowered.locals)?;
                if let Some(ordinal) = lowered.first_poisoned {
                    recovery.get_or_insert(HirStmtRecoveryIssue::RecoveredChild {
                        role: HirStmtChildRole::BodyStatement { ordinal },
                    });
                }
                let close = block
                    .close_delimiter()
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                if close.range().is_empty() {
                    recovery.get_or_insert(HirStmtRecoveryIssue::UnclosedBody);
                }
                (
                    HirUnsafeLifetimeBody::Block {
                        scope: body_scope,
                        statements: lowered.body,
                    },
                    !attached
                        .safety_documentation()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
                        .is_empty(),
                )
            }
        };

        Ok((
            HirStmtKind::UnsafeLifetime {
                audit: HirUnsafeAudit::new(audit_id, reason, has_safety_doc),
                body,
            },
            recovery,
        ))
    }

    fn lower_contextual_statement_body(
        &mut self,
        block: &AstNode<BlockKind>,
        owner: StmtId,
        parent_scope: ScopeId,
        ordinary_kind: HirScopeKind,
        context: HirStatementContext,
    ) -> Result<(HirContextualStmtBody, bool), HirLowerFailure> {
        match context {
            HirStatementContext::Thread => {
                let attached = AttachedRequiredNestedThreadFlowBody::Present(
                    block
                        .thread_flow_body()
                        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                );
                let lowered = self.lower_attached_nested_thread_body(
                    &attached,
                    HirScopeOwner::Stmt(owner),
                    parent_scope,
                )?;
                let poisoned = lowered.recovery.is_some();
                let body = HirContextualStmtBody::try_thread(lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((body, poisoned))
            }
            HirStatementContext::Ordinary
            | HirStatementContext::Predicate
            | HirStatementContext::Proof => {
                let scope =
                    self.allocate_statement_scope(block, owner, parent_scope, ordinary_kind)?;
                let lowered = self.lower_attached_statement_block(block, scope, context)?;
                self.close_scope_members(scope, lowered.locals)?;
                let body = HirContextualStmtBody::try_ordinary(scope, lowered.body)
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                Ok((body, lowered.poisoned))
            }
        }
    }

    fn allocate_statement_scope<K: AstKind>(
        &mut self,
        source: &AstNode<K>,
        owner: StmtId,
        parent: ScopeId,
        kind: HirScopeKind,
    ) -> Result<ScopeId, HirLowerFailure> {
        let reservation = self.arenas.scopes().reserve_source(
            &mut self.slots,
            source.id(),
            HirSourceSite::Span(source.source_span()),
        )?;
        let scope = reservation.id();
        if reservation.is_first_touch() {
            let payload = HirScope::try_new(
                owner.module(),
                kind,
                Some(parent),
                HirScopeOwner::Stmt(owner),
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
            && retained.owner() == &HirScopeOwner::Stmt(owner)
        {
            Ok(scope)
        } else {
            Err(HirInvariantFailure::InvalidScopeParent.into())
        }
    }

    fn lower_attached_statement_block(
        &mut self,
        block: &AstNode<BlockKind>,
        scope: ScopeId,
        context: HirStatementContext,
    ) -> Result<LoweredStatementBlock, HirLowerFailure> {
        if block
            .optional_tail()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .is_some()
        {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let statements = block
            .statements()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        require_limit(HirLimit::Statements, statements.len())?;
        let mut body = Vec::with_capacity(statements.len());
        let mut locals = Vec::new();
        let mut poisoned = false;
        let mut first_poisoned = None;
        for (ordinal, statement) in statements.iter().enumerate() {
            let lowered = self.lower_attached_statement(statement, scope, context)?;
            body.push(lowered.owner);
            locals.extend(lowered.locals);
            poisoned |= lowered.poisoned;
            if lowered.poisoned && first_poisoned.is_none() {
                first_poisoned = Some(
                    u32::try_from(ordinal).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
                );
            }
        }
        require_limit(HirLimit::LocalsPerScope, locals.len())?;
        Ok(LoweredStatementBlock {
            body: body.into_boxed_slice(),
            locals: locals.into_boxed_slice(),
            poisoned,
            first_poisoned,
        })
    }

    fn validate_attached_statement(
        &mut self,
        attached: &StatementNode,
        scope: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        if attached.snapshot_id() != self.request.source().snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: self.request.source().snapshot_id().clone(),
                supplied: attached.snapshot_id().clone(),
            });
        }
        let source = attached.source_span();
        if source.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: source.source().clone(),
            });
        }
        if self
            .arenas
            .scopes()
            .resolve_staged(&self.slots, scope)
            .is_err()
        {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok(())
    }

    pub(super) fn lower_implicit_unit_tail(
        &mut self,
        parent: ExprId,
        scope: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        self.lower_implicit_unit_tail_for_owner(SyntheticOwner::Expr(parent), scope, source)
    }

    pub(super) fn lower_implicit_unit_tail_for_scope(
        &mut self,
        owner: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        self.lower_implicit_unit_tail_for_owner(SyntheticOwner::Scope(owner), owner, source)
    }

    fn lower_implicit_unit_tail_for_owner(
        &mut self,
        parent: SyntheticOwner,
        scope: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        let site = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        let key = SyntheticKey::try_new(parent, SyntheticRole::ImplicitUnitTail, 0)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site)?;
        let tail = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(tail, scope);
        }
        let payload = HirExpr::try_new(scope, HirExprKind::Unit, HirPoisonState::Clean)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    pub(super) fn lower_missing_required_tail(
        &mut self,
        parent: ExprId,
        scope: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        self.lower_missing_required_tail_for_owner(SyntheticOwner::Expr(parent), scope, source)
    }

    pub(super) fn lower_missing_required_tail_for_scope(
        &mut self,
        owner: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        self.lower_missing_required_tail_for_owner(SyntheticOwner::Scope(owner), owner, source)
    }

    fn lower_missing_required_tail_for_owner(
        &mut self,
        parent: SyntheticOwner,
        scope: ScopeId,
        source: &arcweft_source::SourceSpan,
    ) -> Result<ExprId, HirLowerFailure> {
        let site = HirSourceSite::from_attached_span(self.request.source().document(), source)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        if !matches!(site, HirSourceSite::Insertion(_)) {
            return Err(HirInvariantFailure::InvalidSourceSpan.into());
        }
        let key = SyntheticKey::try_new(parent, SyntheticRole::MissingRequiredTail, 0)
            .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let reservation =
            self.arenas
                .expressions()
                .reserve_synthetic(&mut self.slots, key, site.clone())?;
        let tail = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_expression(tail, scope);
        }
        let payload = HirExpr::try_new(
            scope,
            HirExprKind::Error(HirExprError::new(
                HirGenericExprIssue::TransactionalChildFailure,
            )),
            HirPoisonState::Poisoned(HirRecoveryIssue::MissingRequiredTail),
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
            SyntheticOwner::Expr(tail),
            HirRecoveryPrimary::owner_whole(SyntheticOwner::Expr(tail)),
            site,
        ));
        self.arenas
            .expressions()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn lower_missing_statement_expression(
        &mut self,
        parent: StmtId,
        scope: ScopeId,
        slot: HirStmtRecoveryOperandSlot,
    ) -> Result<ExprId, HirLowerFailure> {
        let insertion = crate::source_index::HirInsertionPoint::try_new(
            self.request.source().document(),
            slot.insertion(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        self.lower_missing_owned_expression(
            SyntheticOwner::Stmt(parent),
            scope,
            HirSourceSite::Insertion(insertion),
            slot.ordinal()
                .ok_or(HirInvariantFailure::InvalidSlotCommit)?,
            slot.source_role(),
        )
    }
}

fn nested_thread_body_recovery(
    recovery: Option<&HirThreadIssue>,
    role: HirThreadStmtBodyRole,
) -> Result<Option<HirStmtRecoveryIssue>, HirLowerFailure> {
    let recovery = match recovery {
        None => None,
        Some(HirThreadIssue::MissingBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::MissingBody { role },
        )),
        Some(HirThreadIssue::UnclosedBody) => Some(HirStmtRecoveryIssue::Thread(
            HirThreadStmtRecoveryIssue::UnclosedBody { role },
        )),
        Some(HirThreadIssue::RecoveredBodyChild { ordinal }) => {
            Some(HirStmtRecoveryIssue::RecoveredChild {
                role: HirStmtChildRole::BodyStatement { ordinal: *ordinal },
            })
        }
        Some(
            HirThreadIssue::InvalidName
            | HirThreadIssue::DetachedBorrowedCapture { .. }
            | HirThreadIssue::DetachedEphemeralRegistryAccess,
        ) => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
    };
    Ok(recovery)
}
