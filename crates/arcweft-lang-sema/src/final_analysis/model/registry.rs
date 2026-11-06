//! Checked source operands for the lifetime registry.

use arcweft_core::effect::{RuntimeRegistryDropTarget, RuntimeRegistryReadKind};
use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

use arcweft_lang_hir::{
    body_edges::HirBodyChildRole,
    expr::{HirExprKind, HirExpressionChildRole, HirExpressionOwnedBodyRole},
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
