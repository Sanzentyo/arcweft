//! Statement-owned registry addresses are metadata, never ordinary values.

use std::sync::Arc;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, StmtId},
    leaf::{HirLifetimeRegistryPath, HirLifetimeRegistryScope},
    module::HirModule,
    stmt::HirStmtKind,
};

use super::super::FinalSemanticAnalysisError;
use crate::{effects::EffectId, types::TypeKind};

/// The exact address operand owned by one lifetime-registry write.
/// Its source identity does not grant a runtime scope or a value lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRegistryAddress {
    statement: StmtId,
    target: ExprId,
    path: HirLifetimeRegistryPath,
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
        let scope = match path.scope() {
            HirLifetimeRegistryScope::Frame => "frame",
            HirLifetimeRegistryScope::Tick => "tick",
            HirLifetimeRegistryScope::Cue => "cue",
            HirLifetimeRegistryScope::Line => "line",
            HirLifetimeRegistryScope::Scene => "scene",
            HirLifetimeRegistryScope::Flow => "flow",
            HirLifetimeRegistryScope::Session => "session",
            HirLifetimeRegistryScope::Global => "global",
            HirLifetimeRegistryScope::Persistent => "persistent",
            HirLifetimeRegistryScope::Named(name) => name.as_str(),
        };
        let mut effect = format!("state.write('{scope}");
        for segment in path.segments() {
            effect.push('.');
            effect.push_str(segment.as_str());
        }
        effect.push(')');
        let effect = EffectId::parse(effect).map_err(|_| {
            FinalSemanticAnalysisError::RegistryWriteTarget {
                owner,
                target: *target,
            }
        })?;
        Ok(Self {
            statement: owner,
            target: *target,
            path: path.clone(),
            effect,
        })
    }

    pub const fn statement(&self) -> StmtId {
        self.statement
    }
    pub const fn target(&self) -> ExprId {
        self.target
    }
    pub const fn path(&self) -> &HirLifetimeRegistryPath {
        &self.path
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
