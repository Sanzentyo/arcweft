//! Final semantic expression records owned by the qualified HIR arena.
//!
//! These records retain semantic values and qualified child IDs only. Source
//! components live in the HIR source index, while liveness, lexical ownership,
//! limits, and poison-state publication remain transaction responsibilities.

mod basic;
mod call;
mod choice;
mod control;
mod for_synthetic;
mod thread;

pub use self::basic::{
    HirArrayRepeatExpr, HirAwaitBranch, HirAwaitBranchKind, HirAwaitExpr, HirBinaryExpr,
    HirBinaryOp, HirBorrowExpr, HirBorrowKind, HirBracketSequenceExpr, HirDereferenceExpr,
    HirIndexExpr, HirPipeExpr, HirPlaceholderKind, HirRangeExpr, HirRecordExpr, HirRecordField,
    HirRecordFieldIssue, HirRecordLiteralExpr, HirSelectExpr, HirSelectedMember, HirTryExpr,
    HirTupleExpr, HirUnaryExpr, HirUnaryOp,
};
pub use self::call::{
    HirAssociatedCallSyntax, HirAssociatedReceiver, HirAssociatedReceiverError,
    HirAssociatedSeparator, HirCallArgument, HirCallArgumentListTerminator, HirCallArgumentOrdinal,
    HirCallCallee, HirCallExpr, HirCallIssue, HirCallTypeApplication,
    HirCallTypeApplicationSpelling, HirCallTypeApplicationTerminator, HirCallTypeArgument,
    HirCallTypeArgumentOrdinal, HirCallValue, HirRecoveredName, HirRequiredTokenState,
};
pub(crate) use self::call::{
    HirCallArgumentOrdinalError, HirCallBuildError, HirCallChildPoison, HirCallChildStates,
};
pub(crate) use self::choice::HirChoiceRequiredExpressionSlot;
pub use self::choice::{
    HirChoiceBody, HirChoiceCompactAction, HirChoiceCompactArm, HirChoiceExpr, HirChoiceFor,
    HirChoiceIf, HirChoiceIfBranch, HirChoiceItem, HirChoiceMatch, HirChoiceMatchArm,
    HirChoiceOption, HirChoiceOptionBody, HirChoiceOptionField, HirChoiceOptionFor, HirChoicePlan,
    HirChoicePlanError, HirChoicePlanItem, HirChoiceView, HirChoiceViewEntry,
};
pub use self::control::{
    HirBlockExpr, HirClosureExpr, HirClosureParameter, HirComputationBlockExpr,
    HirComputationBlockKind, HirExprError, HirGenericExprIssue, HirIfExpr, HirIfLetExpr,
    HirLoopExpr, HirMatchArm, HirMatchExpr, HirNamedBlockExpr, HirNamedBlockName,
};
pub use self::for_synthetic::HirForSyntheticExpr;
#[cfg(test)]
pub(crate) use self::thread::HirThreadBodyInvariantError;
pub use self::thread::{
    HirThreadBody, HirThreadBodyOwner, HirThreadExpr, HirThreadFlowItem, HirThreadIssue,
    HirThreadMode,
};

use crate::dialogue_application::{
    HirDialogueContentApplication, HirDialogueIssue, HirDialogueNodeKind, HirLinePlanItem,
    HirPostfixBracket, HirPostfixBracketCandidates, HirRichTextIssue,
};
use crate::identity::{ExprId, HirModuleId, PatternId, ScopeId, StmtId, TypeId};
use crate::leaf::{
    HirCharacterLiteral, HirDurationLiteral, HirFloatLiteral, HirIdRefIssue, HirIdRefValue,
    HirIntegerLiteral, HirLifetimePathValue, HirLifetimeRegistryIssue, HirLiteral, HirLiteralIssue,
    HirNameInvariantError, HirNumericSequence, HirNumericSequenceRecovery, HirPathIssue,
    HirPathValue, HirShortVariantName, HirStringLiteral, HirTypeRegionIssue, HirUnitNumberLiteral,
};
use crate::source_index::HirExprSourceRole;
use crate::stmt::{HirThreadStmtInvariantError, HirTriggerPattern};

/// One immutable expression-arena record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpr {
    scope: ScopeId,
    kind: HirExprKind,
    state: HirPoisonState,
}

impl HirExpr {
    pub(crate) fn try_new(
        scope: ScopeId,
        kind: HirExprKind,
        state: HirPoisonState,
    ) -> Result<Self, HirExprInvariantError> {
        kind.validate_module(scope.module())?;
        if matches!(state, HirPoisonState::Clean) && kind.contains_recovery_payload() {
            return Err(HirExprInvariantError::CleanRecoveryPayload);
        }
        if kind.requires_exact_leaf_state() {
            match (kind.leaf_recovery_issue(), &state) {
                (Some(expected), HirPoisonState::Poisoned(actual)) if actual == &expected => {}
                (Some(expected), HirPoisonState::Poisoned(actual)) => {
                    return Err(HirExprInvariantError::LeafRecoveryIssueMismatch {
                        expected,
                        actual: actual.clone(),
                    });
                }
                (Some(_), HirPoisonState::Clean) => {
                    return Err(HirExprInvariantError::CleanRecoveryPayload);
                }
                (None, HirPoisonState::Poisoned(actual)) => {
                    return Err(HirExprInvariantError::UnexpectedLeafPoison {
                        actual: actual.clone(),
                    });
                }
                (None, HirPoisonState::Clean) => {}
            }
        }
        Ok(Self { scope, kind, state })
    }

    /// Returns the lexical scope in which this expression is evaluated.
    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Returns the closed semantic expression payload.
    pub const fn kind(&self) -> &HirExprKind {
        &self.kind
    }

    /// Returns the semantic recovery state retained with this expression.
    pub const fn state(&self) -> &HirPoisonState {
        &self.state
    }

    pub const fn is_poisoned(&self) -> bool {
        self.state.is_poisoned()
    }
}

impl crate::arena::HirArenaPayload for HirExpr {
    fn is_poisoned(&self) -> bool {
        self.is_poisoned()
    }
}

/// The exact final semantic expression inventory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExprKind {
    Unit,
    Literal(HirLiteral),
    EntityReference(HirIdRefValue),
    LifetimePath(HirLifetimePathValue),
    Path(HirPathValue),
    ShortVariant(HirShortVariantName),
    Placeholder(HirPlaceholderKind),
    Tuple(HirTupleExpr),
    BracketSequence(HirBracketSequenceExpr),
    NumericBracketSequence(HirNumericSequence),
    ArrayRepeat(HirArrayRepeatExpr),
    Call(HirCallExpr),
    Select(HirSelectExpr),
    Index(HirIndexExpr),
    Pipe(HirPipeExpr),
    Try(HirTryExpr),
    Await(HirAwaitExpr),
    Thread(HirThreadExpr),
    Choice(HirChoiceExpr),
    Range(HirRangeExpr),
    Record(HirRecordExpr),
    RecordLiteral(HirRecordLiteralExpr),
    Binary(HirBinaryExpr),
    Borrow(HirBorrowExpr),
    Dereference(HirDereferenceExpr),
    Closure(HirClosureExpr),
    Unary(HirUnaryExpr),
    Block(HirBlockExpr),
    ComputationBlock(HirComputationBlockExpr),
    NamedBlock(HirNamedBlockExpr),
    Loop(HirLoopExpr),
    If(HirIfExpr),
    IfLet(HirIfLetExpr),
    Match(HirMatchExpr),
    DialogueContentApplication(HirDialogueContentApplication),
    PostfixBracket(HirPostfixBracket),
    Error(HirExprError),
    ForSynthetic(HirForSyntheticExpr),
}

/// One exact semantic slot which may own a synthetic `RecoveryOperand`.
///
/// `SyntheticOnly` is limited to accepted invalid carriers whose public schema
/// deliberately has no fabricated valid value. The synthetic key still owns
/// the recovery expression at this exact ordinal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirRecoveryOperandSlot {
    Retained(ExprId),
    SyntheticOnly,
}

impl HirExprKind {
    /// Returns every type-arena root attached directly to this expression.
    ///
    /// Nested type structure remains owned by [`crate::type_ref::HirTypeKind`].
    /// Keeping the expression-to-type edge here lets higher-level domains
    /// follow accepted typed ownership without source reconstruction.
    pub(crate) fn direct_type_roots(&self) -> Vec<TypeId> {
        match self {
            Self::Call(call) => call
                .callee()
                .associated_parts()
                .and_then(|(receiver, _, _)| receiver.type_id())
                .into_iter()
                .chain(
                    call.explicit_type_application()
                        .arguments()
                        .iter()
                        .filter_map(HirCallTypeArgument::type_id),
                )
                .collect(),
            Self::Closure(closure) => closure
                .result_type()
                .into_iter()
                .chain(
                    closure
                        .parameters()
                        .iter()
                        .filter_map(HirClosureParameter::ty),
                )
                .collect(),
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::Tuple(_)
            | Self::BracketSequence(_)
            | Self::NumericBracketSequence(_)
            | Self::ArrayRepeat(_)
            | Self::Select(_)
            | Self::Index(_)
            | Self::Pipe(_)
            | Self::Try(_)
            | Self::Await(_)
            | Self::Thread(_)
            | Self::Choice(_)
            | Self::Range(_)
            | Self::Record(_)
            | Self::RecordLiteral(_)
            | Self::Binary(_)
            | Self::Borrow(_)
            | Self::Dereference(_)
            | Self::Unary(_)
            | Self::Block(_)
            | Self::ComputationBlock(_)
            | Self::NamedBlock(_)
            | Self::Loop(_)
            | Self::If(_)
            | Self::IfLet(_)
            | Self::Match(_)
            | Self::DialogueContentApplication(_)
            | Self::PostfixBracket(_)
            | Self::Error(_)
            | Self::ForSynthetic(_) => Vec::new(),
        }
    }

    /// Returns every expression-arena edge owned directly by this payload.
    ///
    /// This inventory is source-independent: synthetic children and children
    /// without an authored span participate exactly like source-backed
    /// children. Statement IDs retained by block, Choice, or line-plan bodies,
    /// plus `FlowItem` owners retained by Thread/Choice bodies, are intentionally
    /// not expanded here; those values remain roots in their own typed owner
    /// inventory.
    pub fn direct_expression_children(&self) -> Vec<ExprId> {
        let mut children = Vec::new();
        match self {
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::NumericBracketSequence(_)
            | Self::Thread(_)
            | Self::Error(_) => {}
            Self::Tuple(expression) => children.extend_from_slice(expression.elements()),
            Self::BracketSequence(expression) => {
                children.extend_from_slice(expression.elements());
            }
            Self::ArrayRepeat(expression) => {
                children.extend([expression.value(), expression.length()]);
            }
            Self::Call(expression) => {
                children.extend(expression.callee().value_expression());
                children.extend(expression.arguments().iter().map(HirCallArgument::value));
            }
            Self::Select(expression) => children.push(expression.target()),
            Self::Index(expression) => {
                children.extend([expression.target(), expression.index()]);
            }
            Self::Pipe(expression) => {
                children.extend([expression.left(), expression.right()]);
            }
            Self::Try(expression) => children.push(expression.operand()),
            Self::Await(expression) => children.push(expression.operand()),
            Self::Choice(expression) => {
                append_choice_expression_children(expression, &mut children);
            }
            Self::Range(expression) => {
                children.extend(expression.start());
                children.extend(expression.end());
            }
            Self::Record(expression) => {
                children.extend(expression.fields().iter().filter_map(HirRecordField::value));
            }
            Self::RecordLiteral(expression) => {
                children.extend(expression.fields().iter().filter_map(HirRecordField::value));
            }
            Self::Binary(expression) => {
                children.extend([expression.left(), expression.right()]);
            }
            Self::Borrow(expression) => children.push(expression.operand()),
            Self::Dereference(expression) => children.push(expression.operand()),
            Self::Closure(expression) => children.push(expression.body()),
            Self::Unary(expression) => children.push(expression.operand()),
            Self::Block(expression) => children.push(expression.tail()),
            Self::ComputationBlock(expression) => children.push(expression.tail()),
            Self::NamedBlock(expression) => children.push(expression.tail()),
            Self::Loop(expression) => children.push(expression.tail()),
            Self::If(expression) => children.extend([
                expression.condition(),
                expression.then_branch(),
                expression.else_branch(),
            ]),
            Self::IfLet(expression) => {
                children.push(expression.scrutinee());
                children.extend(expression.guard());
                children.extend([expression.then_branch(), expression.else_branch()]);
            }
            Self::Match(expression) => {
                children.push(expression.scrutinee());
                for arm in expression.arms() {
                    children.extend(arm.guard());
                    children.push(arm.value());
                }
            }
            Self::DialogueContentApplication(expression) => {
                append_dialogue_application_children(expression, &mut children);
            }
            Self::PostfixBracket(expression) => {
                children.push(expression.target());
                if let HirPostfixBracketCandidates::Ambiguous { index, dialogue } =
                    expression.candidates()
                {
                    children.extend([*index, *dialogue]);
                }
            }
            Self::ForSynthetic(expression) => children.push(expression.input()),
        }
        children
    }

    /// Resolves a `RecoveryOperand` ordinal through the semantic owner rather
    /// than duplicating child order in source-index validation.
    pub(crate) fn recovery_operand_slot(&self, ordinal: u32) -> Option<HirRecoveryOperandSlot> {
        let ordinal = usize::try_from(ordinal).ok()?;
        let retained = HirRecoveryOperandSlot::Retained;
        match self {
            Self::Tuple(expression) => expression.elements().get(ordinal).copied().map(retained),
            Self::BracketSequence(expression) => {
                expression.elements().get(ordinal).copied().map(retained)
            }
            Self::ArrayRepeat(expression) => match ordinal {
                0 => Some(retained(expression.value())),
                1 => Some(retained(expression.length())),
                _ => None,
            },
            Self::Call(expression) => match ordinal {
                0 => expression.callee().value_expression().map(retained),
                _ => expression
                    .arguments()
                    .get(ordinal.checked_sub(1)?)
                    .map(HirCallArgument::value)
                    .map(retained),
            },
            Self::Select(expression) => (ordinal == 0).then(|| retained(expression.target())),
            Self::Index(expression) => match ordinal {
                0 => Some(retained(expression.target())),
                1 => Some(retained(expression.index())),
                _ => None,
            },
            Self::Pipe(expression) => match ordinal {
                0 => Some(retained(expression.left())),
                1 => Some(retained(expression.right())),
                _ => None,
            },
            Self::Try(expression) => (ordinal == 0).then(|| retained(expression.operand())),
            Self::Await(expression) => (ordinal == 0).then(|| retained(expression.operand())),
            Self::Choice(expression) => expression
                .required_expression_slots()
                .get(ordinal)
                .copied()
                .and_then(|slot| match slot {
                    HirChoiceRequiredExpressionSlot::Retained(expression) => {
                        Some(retained(expression))
                    }
                    HirChoiceRequiredExpressionSlot::UnretainedInvalidAssignmentValue => None,
                }),
            Self::Range(expression) => match ordinal {
                0 => expression.start().map(retained),
                1 => expression.end().map(retained),
                _ => None,
            },
            Self::Record(expression) => record_recovery_operand_slot(expression.fields(), ordinal),
            Self::RecordLiteral(expression) => {
                record_recovery_operand_slot(expression.fields(), ordinal)
            }
            Self::Binary(expression) => match ordinal {
                0 => Some(retained(expression.left())),
                1 => Some(retained(expression.right())),
                _ => None,
            },
            Self::Borrow(expression) => (ordinal == 0).then(|| retained(expression.operand())),
            Self::Dereference(expression) => (ordinal == 0).then(|| retained(expression.operand())),
            Self::Closure(expression) => (ordinal == 0).then(|| retained(expression.body())),
            Self::Unary(expression) => (ordinal == 0).then(|| retained(expression.operand())),
            Self::Loop(expression) => (ordinal == 0).then(|| retained(expression.tail())),
            Self::If(expression) => match ordinal {
                0 => Some(retained(expression.condition())),
                1 => Some(retained(expression.then_branch())),
                2 => Some(retained(expression.else_branch())),
                _ => None,
            },
            Self::IfLet(expression) => match ordinal {
                0 => Some(retained(expression.scrutinee())),
                1 => expression.guard().map(retained),
                2 => Some(retained(expression.then_branch())),
                3 => Some(retained(expression.else_branch())),
                _ => None,
            },
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::NumericBracketSequence(_)
            | Self::Thread(_)
            | Self::NamedBlock(_)
            | Self::Block(_)
            | Self::ComputationBlock(_)
            | Self::Match(_)
            | Self::DialogueContentApplication(_)
            | Self::PostfixBracket(_)
            | Self::Error(_)
            | Self::ForSynthetic(_) => None,
        }
    }

    pub(crate) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Thread(expression) => {
                (expression.body().scope() == scope).then(|| expression.body())
            }
            Self::Await(expression) => expression.thread_body_for_scope(scope),
            Self::Choice(expression) => expression.thread_body_for_scope(scope),
            _ => None,
        }
    }

    fn contains_recovery_payload(&self) -> bool {
        match self {
            Self::Literal(literal) => literal_contains_recovery(literal),
            Self::EntityReference(reference) => reference.recovery().is_some(),
            Self::LifetimePath(path) => path.recovery().is_some(),
            Self::Path(path) => path.recovery().is_some(),
            Self::ShortVariant(name) => name.recovery_issue().is_some(),
            Self::NumericBracketSequence(sequence) => {
                !matches!(sequence.recovery(), HirNumericSequenceRecovery::Complete)
            }
            Self::Record(expression) => record_fields_contain_recovery(expression.fields()),
            Self::RecordLiteral(expression) => record_fields_contain_recovery(expression.fields()),
            Self::Call(expression) => expression.contains_recovery_payload(),
            Self::Select(expression) => {
                matches!(expression.member(), HirSelectedMember::Missing)
            }
            Self::DialogueContentApplication(expression) => expression.has_recovery(),
            Self::PostfixBracket(expression) => expression.has_recovery(),
            Self::Choice(expression) => expression.has_recovery(),
            Self::Error(_) => true,
            Self::Unit
            | Self::Placeholder(_)
            | Self::Tuple(_)
            | Self::BracketSequence(_)
            | Self::ArrayRepeat(_)
            | Self::Index(_)
            | Self::Pipe(_)
            | Self::Try(_)
            | Self::Thread(_)
            | Self::Range(_)
            | Self::Binary(_)
            | Self::Borrow(_)
            | Self::Dereference(_)
            | Self::Closure(_)
            | Self::Unary(_)
            | Self::Block(_)
            | Self::ComputationBlock(_)
            | Self::Loop(_)
            | Self::If(_)
            | Self::IfLet(_)
            | Self::Match(_)
            | Self::ForSynthetic(_) => false,
            Self::Await(expression) => expression
                .branches()
                .iter()
                .any(|branch| matches!(branch.kind(), crate::expr::HirAwaitBranchKind::Recovered)),
            Self::NamedBlock(expression) => expression.name().recovery_issue().is_some(),
        }
    }

    fn requires_exact_leaf_state(&self) -> bool {
        matches!(
            self,
            Self::Unit
                | Self::Literal(_)
                | Self::EntityReference(_)
                | Self::LifetimePath(_)
                | Self::Path(_)
                | Self::ShortVariant(_)
                | Self::Placeholder(_)
                | Self::NumericBracketSequence(_)
        ) || matches!(
            self,
            Self::Error(error) if error.issue() == HirGenericExprIssue::UnclassifiedSyntax
        )
    }

    fn leaf_recovery_issue(&self) -> Option<HirRecoveryIssue> {
        match self {
            Self::Literal(literal) => {
                literal_recovery_issue(literal).map(HirRecoveryIssue::MalformedLiteral)
            }
            Self::EntityReference(reference) => {
                reference.recovery_issue().map(HirRecoveryIssue::InvalidId)
            }
            Self::LifetimePath(path) => path
                .recovery()
                .map(|recovery| HirRecoveryIssue::InvalidLifetimeRegistry(recovery.issue())),
            Self::Path(path) => path
                .recovery()
                .map(|recovery| HirRecoveryIssue::InvalidPath(recovery.issue().clone())),
            Self::ShortVariant(name) => name.recovery_issue().map(HirRecoveryIssue::InvalidName),
            Self::NumericBracketSequence(sequence) => {
                (!matches!(sequence.recovery(), HirNumericSequenceRecovery::Complete))
                    .then_some(HirRecoveryIssue::InvalidNumericSequence)
            }
            Self::Error(error) if error.issue() == HirGenericExprIssue::UnclassifiedSyntax => {
                Some(HirRecoveryIssue::InvalidExpression(
                    HirExpressionRecoveryIssue::Generic(HirGenericExprIssue::UnclassifiedSyntax),
                ))
            }
            _ => None,
        }
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Unit
            | Self::Literal(_)
            | Self::EntityReference(_)
            | Self::LifetimePath(_)
            | Self::Path(_)
            | Self::ShortVariant(_)
            | Self::Placeholder(_)
            | Self::NumericBracketSequence(_)
            | Self::Error(_) => Ok(()),
            Self::Tuple(expression) => validate_exprs(expected, expression.elements()),
            Self::BracketSequence(expression) => validate_exprs(expected, expression.elements()),
            Self::ArrayRepeat(expression) => {
                validate_expr(expected, expression.value())?;
                validate_expr(expected, expression.length())
            }
            Self::Call(expression) => expression.validate_module(expected),
            Self::Select(expression) => validate_expr(expected, expression.target()),
            Self::Index(expression) => {
                validate_expr(expected, expression.target())?;
                validate_expr(expected, expression.index())
            }
            Self::Pipe(expression) => {
                validate_expr(expected, expression.left())?;
                validate_expr(expected, expression.right())
            }
            Self::Try(expression) => validate_expr(expected, expression.operand()),
            Self::Await(expression) => {
                validate_expr(expected, expression.operand())?;
                expression
                    .branches()
                    .iter()
                    .try_for_each(|branch| branch.validate_module(expected))
                    .map_err(HirExprInvariantError::InvalidAwaitBranch)
            }
            Self::Thread(expression) => expression.validate_module(expected),
            Self::Choice(expression) => expression.validate_module(expected),
            Self::Range(expression) => {
                validate_optional_expr(expected, expression.start())?;
                validate_optional_expr(expected, expression.end())
            }
            Self::Record(expression) => validate_record_fields(expected, expression.fields()),
            Self::RecordLiteral(expression) => {
                validate_record_fields(expected, expression.fields())
            }
            Self::Binary(expression) => {
                validate_expr(expected, expression.left())?;
                validate_expr(expected, expression.right())
            }
            Self::Borrow(expression) => validate_expr(expected, expression.operand()),
            Self::Dereference(expression) => validate_expr(expected, expression.operand()),
            Self::Closure(expression) => expression.validate_module(expected),
            Self::Unary(expression) => validate_expr(expected, expression.operand()),
            Self::Block(expression) => {
                validate_scope(expected, expression.scope())?;
                validate_statements(expected, expression.statements())?;
                validate_expr(expected, expression.tail())
            }
            Self::ComputationBlock(expression) => {
                validate_scope(expected, expression.scope())?;
                validate_statements(expected, expression.statements())?;
                validate_expr(expected, expression.tail())
            }
            Self::NamedBlock(expression) => {
                validate_scope(expected, expression.scope())?;
                validate_statements(expected, expression.statements())?;
                validate_expr(expected, expression.tail())
            }
            Self::Loop(expression) => expression.validate_module(expected),
            Self::If(expression) => {
                validate_expr(expected, expression.condition())?;
                validate_expr(expected, expression.then_branch())?;
                validate_expr(expected, expression.else_branch())
            }
            Self::IfLet(expression) => expression.validate_module(expected),
            Self::Match(expression) => expression.validate_module(expected),
            Self::ForSynthetic(expression) => expression.validate_module(expected),
            Self::DialogueContentApplication(expression) => expression
                .validate_module(expected)
                .map_err(|actual| HirExprInvariantError::ForeignChild { expected, actual }),
            Self::PostfixBracket(expression) => expression
                .validate_module(expected)
                .map_err(|actual| HirExprInvariantError::ForeignChild { expected, actual }),
        }
    }
}

fn record_recovery_operand_slot(
    fields: &[HirRecordField],
    ordinal: usize,
) -> Option<HirRecoveryOperandSlot> {
    matches!(
        fields.get(ordinal),
        Some(HirRecordField::Invalid {
            issue: HirRecordFieldIssue::MissingValue,
        })
    )
    .then_some(HirRecoveryOperandSlot::SyntheticOnly)
}

fn append_dialogue_application_children(
    application: &HirDialogueContentApplication,
    children: &mut Vec<ExprId>,
) {
    children.push(application.target());
    children.extend(
        application
            .coordinates()
            .iter()
            .map(crate::dialogue_application::HirDialogueCoordinate::value),
    );
    children.extend(application.content().nodes().iter().filter_map(|node| {
        let HirDialogueNodeKind::Interpolation(expression) = node.kind() else {
            return None;
        };
        Some(*expression)
    }));
    children.extend(
        application
            .content()
            .tags()
            .iter()
            .filter_map(|tag| tag.payload().expression()),
    );
    if let Some(plan) = application.plan() {
        append_line_plan_expression_children(plan.items(), children);
    }
}

fn append_line_plan_expression_children(items: &[HirLinePlanItem], children: &mut Vec<ExprId>) {
    let mut pending = vec![items];
    while let Some(items) = pending.pop() {
        for item in items {
            match item {
                HirLinePlanItem::Option { value, .. }
                | HirLinePlanItem::Let { value, .. }
                | HirLinePlanItem::Out(value)
                | HirLinePlanItem::TimelineAssert {
                    condition: value, ..
                }
                | HirLinePlanItem::Expression(value) => children.push(*value),
                HirLinePlanItem::TimedCue { anchor, body } => {
                    children.extend([*anchor, *body]);
                }
                HirLinePlanItem::StartGroup(items) | HirLinePlanItem::TogetherGroup(items) => {
                    pending.push(items);
                }
                HirLinePlanItem::Init(_)
                | HirLinePlanItem::Thread(_)
                | HirLinePlanItem::On(_)
                | HirLinePlanItem::Statement(_)
                | HirLinePlanItem::CancelRule(_)
                | HirLinePlanItem::Error(_) => {}
            }
        }
    }
}

fn append_choice_expression_children(expression: &HirChoiceExpr, children: &mut Vec<ExprId>) {
    append_choice_body_expression_children(expression.body(), children);
    if let Some(plan) = expression.plan() {
        for item in plan.items() {
            match item {
                HirChoicePlanItem::Assignment { value, .. } => children.push(*value),
                HirChoicePlanItem::Timeout { duration, .. } => children.push(*duration),
                HirChoicePlanItem::Cancel { trigger, .. } => {
                    append_trigger_expression_children(trigger, children);
                }
                HirChoicePlanItem::OnSelect { .. } | HirChoicePlanItem::Error(_) => {}
            }
        }
    }
}

fn append_choice_body_expression_children(body: &HirChoiceBody, children: &mut Vec<ExprId>) {
    let mut pending = vec![body];
    while let Some(body) = pending.pop() {
        for item in body.items() {
            match item {
                HirChoiceItem::Let(_) | HirChoiceItem::Error => {}
                HirChoiceItem::If(expression) => {
                    for branch in expression.branches() {
                        children.push(branch.condition());
                        pending.push(branch.body());
                    }
                    pending.extend(expression.else_body());
                }
                HirChoiceItem::For(expression) => {
                    children.push(expression.source());
                    pending.push(expression.body());
                }
                HirChoiceItem::Match(expression) => {
                    children.push(expression.scrutinee());
                    for arm in expression.arms() {
                        children.extend(arm.guard());
                        pending.push(arm.body());
                    }
                }
                HirChoiceItem::Option(expression) => {
                    children.push(expression.id());
                    append_choice_option_expression_children(expression.body(), children);
                }
                HirChoiceItem::OptionFor(expression) => {
                    children.push(expression.source());
                    append_choice_option_expression_children(expression.body(), children);
                }
                HirChoiceItem::CompactArm(expression) => {
                    children.push(expression.label());
                    children.extend(expression.condition());
                    if let HirChoiceCompactAction::Out(value) = expression.action() {
                        children.push(*value);
                    }
                }
            }
        }
    }
}

fn append_choice_option_expression_children(
    body: &HirChoiceOptionBody,
    children: &mut Vec<ExprId>,
) {
    for field in body.fields() {
        match field {
            HirChoiceOptionField::Label { value, .. }
            | HirChoiceOptionField::Id(value)
            | HirChoiceOptionField::Value(value)
            | HirChoiceOptionField::Visible(value)
            | HirChoiceOptionField::Enabled(value)
            | HirChoiceOptionField::Order(value)
            | HirChoiceOptionField::Hotkey(value) => children.push(*value),
            HirChoiceOptionField::View(view) => {
                for entry in view.entries() {
                    children.extend([entry.key(), entry.value()]);
                }
            }
            HirChoiceOptionField::Select(_)
            | HirChoiceOptionField::Let(_)
            | HirChoiceOptionField::Error => {}
        }
    }
}

fn append_trigger_expression_children(trigger: &HirTriggerPattern, children: &mut Vec<ExprId>) {
    match trigger {
        HirTriggerPattern::Signal { target, .. }
        | HirTriggerPattern::Timeout(target)
        | HirTriggerPattern::Expr(target) => children.push(*target),
        HirTriggerPattern::Input(_)
        | HirTriggerPattern::Event(_)
        | HirTriggerPattern::Mark(_)
        | HirTriggerPattern::Select(_)
        | HirTriggerPattern::Task(_)
        | HirTriggerPattern::Scope(_) => {}
    }
}

/// Construction invariant rejected before an expression-arena publication.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirExprInvariantError {
    ForeignChild {
        expected: HirModuleId,
        actual: HirModuleId,
    },
    CleanRecoveryPayload,
    LeafRecoveryIssueMismatch {
        expected: HirRecoveryIssue,
        actual: HirRecoveryIssue,
    },
    UnexpectedLeafPoison {
        actual: HirRecoveryIssue,
    },
    DuplicateMatchArmScope {
        scope: ScopeId,
    },
    InvalidAwaitBranch(HirThreadStmtInvariantError),
}

/// Publication state for one semantic HIR expression.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirPoisonState {
    Clean,
    Poisoned(HirRecoveryIssue),
}

impl HirPoisonState {
    pub const fn is_poisoned(&self) -> bool {
        matches!(self, Self::Poisoned(_))
    }
}

/// Typed reason that a semantic HIR record retains recovery payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirRecoveryIssue {
    MissingOperand { role: HirExprSourceRole },
    MissingRequiredTail,
    MalformedLiteral(HirLiteralIssue),
    InvalidName(HirNameInvariantError),
    InvalidPath(HirPathIssue),
    InvalidId(HirIdRefIssue),
    InvalidType(crate::type_ref::HirGenericTypeIssue),
    InvalidTypeRegion(HirTypeRegionIssue),
    InvalidLifetimeRegistry(HirLifetimeRegistryIssue),
    InvalidNumericSequence,
    InvalidExpression(HirExpressionRecoveryIssue),
    InvalidMatch(HirMatchRecoveryIssue),
    InvalidCall(HirCallIssue),
    InvalidThread(HirThreadIssue),
    InvalidDialogue(HirDialogueIssue),
    InvalidRichText(HirRichTextIssue),
    InvalidPattern(crate::pattern::HirPatternRecoveryIssue),
    StaleSource,
    ForeignSource,
}

/// Parser-owned Match body recovery retained without inventing a body child
/// or a source role that is absent from the accepted E32 schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirMatchRecoveryIssue {
    MissingBody,
    UnclosedBody,
}

/// Typed propagation retained when a source-backed expression is generic
/// recovery or when a known expression family owns a poisoned authored child.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirExpressionRecoveryIssue {
    Generic(HirGenericExprIssue),
    RecoveredChild { role: HirExprSourceRole },
}

pub(crate) const fn literal_recovery_issue(literal: &HirLiteral) -> Option<HirLiteralIssue> {
    match literal {
        HirLiteral::String(HirStringLiteral::Invalid(issue)) => {
            Some(HirLiteralIssue::String(*issue))
        }
        HirLiteral::Character(HirCharacterLiteral::Invalid(issue)) => {
            Some(HirLiteralIssue::Character(*issue))
        }
        HirLiteral::Integer(HirIntegerLiteral::Invalid(issue)) => {
            Some(HirLiteralIssue::Integer(*issue))
        }
        HirLiteral::Float(HirFloatLiteral::Invalid(issue)) => Some(HirLiteralIssue::Float(*issue)),
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(issue)) => {
            Some(HirLiteralIssue::UnitNumber(*issue))
        }
        HirLiteral::Duration(HirDurationLiteral::Invalid(issue)) => {
            Some(HirLiteralIssue::Duration(*issue))
        }
        HirLiteral::Boolean(_)
        | HirLiteral::String(HirStringLiteral::Value(_))
        | HirLiteral::Character(HirCharacterLiteral::Value(_))
        | HirLiteral::Integer(HirIntegerLiteral::Value { .. })
        | HirLiteral::Float(HirFloatLiteral::Value { .. })
        | HirLiteral::UnitNumber(HirUnitNumberLiteral::Value { .. })
        | HirLiteral::Duration(HirDurationLiteral::Value(_)) => None,
    }
}

pub(crate) const fn literal_contains_recovery(literal: &HirLiteral) -> bool {
    literal_recovery_issue(literal).is_some()
}

fn record_fields_contain_recovery(fields: &[HirRecordField]) -> bool {
    fields
        .iter()
        .any(|field| matches!(field, HirRecordField::Invalid { .. }))
}

fn validate_record_fields(
    expected: HirModuleId,
    fields: &[HirRecordField],
) -> Result<(), HirExprInvariantError> {
    for field in fields {
        match field {
            HirRecordField::Explicit { value, .. } => validate_expr(expected, *value)?,
            HirRecordField::Shorthand { local, .. } => {
                validate_module(expected, local.module())?;
            }
            HirRecordField::Invalid { .. } => {}
        }
    }
    Ok(())
}

fn validate_exprs(
    expected: HirModuleId,
    expressions: &[ExprId],
) -> Result<(), HirExprInvariantError> {
    for expression in expressions {
        validate_expr(expected, *expression)?;
    }
    Ok(())
}

pub(super) fn validate_statements(
    expected: HirModuleId,
    statements: &[StmtId],
) -> Result<(), HirExprInvariantError> {
    for statement in statements {
        validate_module(expected, statement.module())?;
    }
    Ok(())
}

fn validate_expr(expected: HirModuleId, expression: ExprId) -> Result<(), HirExprInvariantError> {
    validate_module(expected, expression.module())
}

fn validate_optional_expr(
    expected: HirModuleId,
    expression: Option<ExprId>,
) -> Result<(), HirExprInvariantError> {
    if let Some(expression) = expression {
        validate_expr(expected, expression)?;
    }
    Ok(())
}

fn validate_pattern(
    expected: HirModuleId,
    pattern: PatternId,
) -> Result<(), HirExprInvariantError> {
    validate_module(expected, pattern.module())
}

fn validate_optional_type(
    expected: HirModuleId,
    ty: Option<TypeId>,
) -> Result<(), HirExprInvariantError> {
    if let Some(ty) = ty {
        validate_module(expected, ty.module())?;
    }
    Ok(())
}

fn validate_scope(expected: HirModuleId, scope: ScopeId) -> Result<(), HirExprInvariantError> {
    validate_module(expected, scope.module())
}

fn validate_module(
    expected: HirModuleId,
    actual: HirModuleId,
) -> Result<(), HirExprInvariantError> {
    if expected == actual {
        Ok(())
    } else {
        Err(HirExprInvariantError::ForeignChild { expected, actual })
    }
}

#[cfg(test)]
#[path = "expr/tests.rs"]
mod tests;
