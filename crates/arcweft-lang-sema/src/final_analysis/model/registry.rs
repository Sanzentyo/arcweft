//! Checked source operands for the lifetime registry.

use arcweft_id::{LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

use arcweft_lang_hir::{
    body_edges::HirBodyChildRole,
    expr::{HirExprKind, HirExpressionChildRole, HirExpressionOwnedBodyRole},
    identity::{ExprId, StmtId},
    leaf::HirLifetimeRegistryPath,
    module::HirModule,
    project::{
        HirDeclarationBodyRootRole, HirProjectEvaluationTopology, HirSemanticPathOwnerId,
        HirSemanticPathStep,
    },
    stmt::HirStmtKind,
};

use super::super::FinalSemanticAnalysisError;
use crate::{effects::EffectId, types::TypeKind};

/// The checked key and presence mode of one registry value expression.
/// The enclosing expression owns the result type. The runtime must still
/// resolve the actual activation from the current execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryRead {
    owner: ExprId,
    key: Arc<LifetimeRegistryKey>,
    kind: arcweft_core::effect::RuntimeRegistryReadKind,
}

impl CheckedRegistryRead {
    pub(crate) fn from_path(
        topology: &HirProjectEvaluationTopology,
        owner: ExprId,
        path: &HirLifetimeRegistryPath,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let location = topology
            .semantic_path(HirSemanticPathOwnerId::Expression(owner))
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let mut flow = false;
        let mut line = false;
        if let Some(location) = location {
            let steps = location.path().steps();
            flow = matches!(
                steps.first(),
                Some(HirSemanticPathStep::DeclarationBody(
                    HirDeclarationBodyRootRole::FlowBody
                ))
            );
            // The Flow body itself is represented as a ThreadItem. Only a
            // nested body can introduce a distinct execution owner.
            for step in steps.iter().skip(2) {
                match step {
                    HirSemanticPathStep::ExpressionOwned(
                        HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. },
                    ) => line = true,
                    HirSemanticPathStep::Body(HirBodyChildRole::ThreadItem { .. })
                    | HirSemanticPathStep::Expression(HirExpressionChildRole::ClosureBody) => {
                        flow = false;
                        line = false;
                    }
                    _ => {}
                }
            }
        }
        if !match path.scope() {
            LifetimeScopeKind::Flow => flow,
            LifetimeScopeKind::Line => line,
            _ => false,
        } {
            return Err(FinalSemanticAnalysisError::RegistryReadScope {
                owner,
                scope: path.scope().clone(),
            });
        }
        let kind = if path.optional() {
            arcweft_core::effect::RuntimeRegistryReadKind::Optional
        } else {
            arcweft_core::effect::RuntimeRegistryReadKind::Required
        };
        Ok(Self {
            owner,
            key: Arc::clone(path.key()),
            kind,
        })
    }

    pub fn key(&self) -> &LifetimeRegistryKey {
        &self.key
    }

    pub(in crate::final_analysis) fn shared_key(&self) -> Arc<LifetimeRegistryKey> {
        Arc::clone(&self.key)
    }

    pub const fn kind(&self) -> arcweft_core::effect::RuntimeRegistryReadKind {
        self.kind
    }

    pub fn matches_path(&self, owner: ExprId, path: &HirLifetimeRegistryPath) -> bool {
        self.owner == owner
            && self.key == *path.key()
            && path.optional()
                == matches!(
                    self.kind,
                    arcweft_core::effect::RuntimeRegistryReadKind::Optional
                )
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
