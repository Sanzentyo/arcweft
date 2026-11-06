//! Checked source operands for the lifetime registry.

use arcweft_core::effect::{RuntimeRegistryDropTarget, RuntimeRegistryReadKind};
use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

use arcweft_lang_hir::{
    body_edges::HirBodyChildRole,
    expr::{
        HirExprKind, HirExpressionChildRole, HirExpressionOwnedBodyRole, HirLinePlanStatementRole,
        HirNestedExpressionPathSegment,
    },
    identity::{ExprId, StmtId},
    leaf::HirLifetimeRegistryPath,
    module::HirModule,
    project::{
        HirDeclarationBodyRootRole, HirProjectEvaluationTopology, HirSemanticPathLocation,
        HirSemanticPathOwnerId, HirSemanticPathStep,
    },
    stmt::{HirStatementBodyRole, HirStmtKind},
};

use super::super::FinalSemanticAnalysisError;
use crate::{effects::EffectId, types::TypeKind};

/// Root-relative activation that can supply authored registry type evidence.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(in crate::final_analysis) enum RegistryLexicalActivation<'a> {
    Flow,
    Line(&'a [HirSemanticPathStep]),
}

impl<'a> RegistryLexicalActivation<'a> {
    pub(in crate::final_analysis) fn at(
        steps: &'a [HirSemanticPathStep],
        scope: &LifetimeScopeKind,
    ) -> Option<Self> {
        let mut flow = matches!(
            steps.first(),
            Some(HirSemanticPathStep::DeclarationBody(
                HirDeclarationBodyRootRole::FlowBody
            ))
        );
        let mut line = None;
        // Flow control blocks also contain ThreadItems. Their enclosing
        // statement-body edge distinguishes them from an independent Thread.
        for (index, step) in steps.iter().enumerate().skip(2) {
            match (&steps[index - 1], step) {
                (
                    _,
                    HirSemanticPathStep::ExpressionOwned(
                        HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. },
                    ),
                ) => line = Some(Self::Line(&steps[..index])),
                (
                    HirSemanticPathStep::StatementBody(role),
                    HirSemanticPathStep::Body(HirBodyChildRole::ThreadItem { .. }),
                ) if *role != HirStatementBodyRole::CancelRule || line.is_some() => {}
                (
                    _,
                    HirSemanticPathStep::Body(HirBodyChildRole::ThreadItem { .. })
                    | HirSemanticPathStep::Expression(HirExpressionChildRole::ClosureBody),
                ) => {
                    flow = false;
                    line = None;
                }
                _ => {}
            }
        }
        match scope {
            LifetimeScopeKind::Flow if flow => Some(Self::Flow),
            LifetimeScopeKind::Line => line,
            _ => None,
        }
    }
}

/// Structural position of a checked registry write. Ordered bodies supply
/// precedence within their own sequence. Scheduled line items are separate
/// from the line's eager initialization sequence.
#[derive(Clone, Copy)]
pub(in crate::final_analysis) struct RegistryWritePosition<'a>(&'a [HirSemanticPathStep]);

impl<'a> RegistryWritePosition<'a> {
    pub(in crate::final_analysis) const fn new(path: &'a [HirSemanticPathStep]) -> Self {
        Self(path)
    }

    pub(in crate::final_analysis) const fn steps(self) -> &'a [HirSemanticPathStep] {
        self.0
    }

    pub(in crate::final_analysis) fn precedes(self, other: &[HirSemanticPathStep]) -> bool {
        let Some((write, parent)) = self.0.split_last() else {
            return false;
        };
        let Some(other) = other.strip_prefix(parent).and_then(|rest| rest.first()) else {
            return false;
        };
        match (write, other) {
            (
                HirSemanticPathStep::Body(HirBodyChildRole::Statement { ordinal: left }),
                HirSemanticPathStep::Body(HirBodyChildRole::Statement { ordinal: right }),
            )
            | (
                HirSemanticPathStep::Body(HirBodyChildRole::ThreadItem { ordinal: left }),
                HirSemanticPathStep::Body(HirBodyChildRole::ThreadItem { ordinal: right }),
            ) => left < right,
            (
                HirSemanticPathStep::Body(HirBodyChildRole::Statement { .. }),
                HirSemanticPathStep::Body(HirBodyChildRole::Tail),
            ) => true,
            (left, right) => eager_line_statement_position(left)
                .zip(eager_line_statement_position(right))
                .is_some_and(|(left, right)| left < right),
        }
    }
}

fn eager_line_statement_position(step: &HirSemanticPathStep) -> Option<(u32, u32)> {
    let HirSemanticPathStep::ExpressionOwned(
        HirExpressionOwnedBodyRole::DialogueLinePlanStatement { path, role },
    ) = step
    else {
        return None;
    };
    let [HirNestedExpressionPathSegment::LinePlanItem { ordinal }] = path.segments() else {
        return None;
    };
    let statement = match role {
        HirLinePlanStatementRole::Init { statement } => *statement,
        HirLinePlanStatementRole::Statement => 0,
        _ => return None,
    };
    Some((*ordinal, statement))
}

/// The checked key and presence mode of one registry value expression.
/// The enclosing expression owns the result type. The runtime must still
/// resolve the actual activation from the current execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryRead {
    owner: ExprId,
    key: Arc<LifetimeRegistryKey>,
    kind: RuntimeRegistryReadKind,
}

impl CheckedRegistryRead {
    pub(in crate::final_analysis) fn from_path<'a>(
        topology: &'a HirProjectEvaluationTopology,
        owner: ExprId,
        path: &HirLifetimeRegistryPath,
    ) -> Result<
        (
            Self,
            HirSemanticPathLocation<'a>,
            RegistryLexicalActivation<'a>,
        ),
        FinalSemanticAnalysisError,
    > {
        let location = topology
            .semantic_path(HirSemanticPathOwnerId::Expression(owner))
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let Some((location, activation)) = location.and_then(|location| {
            RegistryLexicalActivation::at(location.path().steps(), path.scope())
                .map(|activation| (location, activation))
        }) else {
            return Err(FinalSemanticAnalysisError::RegistryReadScope {
                owner,
                scope: path.scope().clone(),
            });
        };
        let kind = if path.optional() {
            RuntimeRegistryReadKind::Optional
        } else {
            RuntimeRegistryReadKind::Required
        };
        Ok((
            Self {
                owner,
                key: Arc::clone(path.key()),
                kind,
            },
            location,
            activation,
        ))
    }

    pub fn key(&self) -> &LifetimeRegistryKey {
        &self.key
    }

    pub(in crate::final_analysis) fn shared_key(&self) -> Arc<LifetimeRegistryKey> {
        Arc::clone(&self.key)
    }

    pub const fn kind(&self) -> RuntimeRegistryReadKind {
        self.kind
    }

    pub fn matches_path(&self, owner: ExprId, path: &HirLifetimeRegistryPath) -> bool {
        self.owner == owner
            && self.key == *path.key()
            && path.optional() == matches!(self.kind, RuntimeRegistryReadKind::Optional)
    }
}

/// The checked address and original item type of a destructive registry operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryDrop {
    address: ExprId,
    argument: crate::callable::CheckedCallExecutionSource,
    target: RuntimeRegistryDropTarget<TypeKind>,
}

impl CheckedRegistryDrop {
    pub(in crate::final_analysis) fn new(
        read: &CheckedRegistryRead,
        argument: crate::callable::CheckedCallExecutionSource,
        item: TypeKind,
    ) -> Self {
        Self {
            address: read.owner,
            argument,
            target: RuntimeRegistryDropTarget::new(read.key().clone(), read.kind(), item),
        }
    }

    pub const fn address(&self) -> ExprId {
        self.address
    }

    pub const fn argument(&self) -> &crate::callable::CheckedCallExecutionSource {
        &self.argument
    }

    pub fn uses_pipe_address(&self) -> bool {
        self.address != self.argument.owner()
    }

    pub const fn target(&self) -> &RuntimeRegistryDropTarget<TypeKind> {
        &self.target
    }

    pub fn effect(&self) -> EffectId {
        EffectId::state_write(self.target.key())
    }
}

/// The exact address operand owned by one lifetime-registry write.
/// Its source identity does not grant a runtime scope or a value lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryAddress {
    statement: StmtId,
    target: ExprId,
    key: Arc<LifetimeRegistryKey>,
    effect: EffectId,
}

impl CheckedRegistryAddress {
    pub(crate) fn from_statement(
        module: &HirModule,
        owner: StmtId,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let statement = module
            .resolve_stmt(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirStmtKind::LifetimeSet { target, .. } = statement.kind() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let expression = module
            .resolve_expr(*target)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirExprKind::LifetimePath(path) = expression.kind() else {
            return Err(FinalSemanticAnalysisError::RegistryWriteTarget {
                owner,
                target: *target,
            });
        };
        let path = path.as_resolved().filter(|path| !path.optional()).ok_or(
            FinalSemanticAnalysisError::RegistryWriteTarget {
                owner,
                target: *target,
            },
        )?;
        let key = Arc::clone(path.key());
        let effect = EffectId::state_write(&key);
        Ok(Self {
            statement: owner,
            target: *target,
            key,
            effect,
        })
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }
    pub const fn target(&self) -> ExprId {
        self.target
    }
    pub fn key(&self) -> &LifetimeRegistryKey {
        &self.key
    }
    pub(in crate::final_analysis) fn shared_key(&self) -> Arc<LifetimeRegistryKey> {
        Arc::clone(&self.key)
    }
    pub fn matches_path(&self, path: &HirLifetimeRegistryPath) -> bool {
        !path.optional() && self.key == *path.key()
    }
    pub const fn effect(&self) -> &EffectId {
        &self.effect
    }
}

/// A checked value entering the address's actual registry storage role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryWrite {
    address: Arc<CheckedRegistryAddress>,
    value: ExprId,
    value_type: TypeKind,
}

impl CheckedRegistryWrite {
    pub(crate) const fn new(
        address: Arc<CheckedRegistryAddress>,
        value: ExprId,
        value_type: TypeKind,
    ) -> Self {
        Self {
            address,
            value,
            value_type,
        }
    }
    pub fn address(&self) -> &CheckedRegistryAddress {
        &self.address
    }
    pub const fn value(&self) -> ExprId {
        self.value
    }
    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}
