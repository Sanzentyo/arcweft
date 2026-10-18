//! Statement-owned registry addresses are metadata, never ordinary values.

use arcweft_id::{DeclarationName, LifetimeRegistryKey, LifetimeScopeKind};
use std::sync::Arc;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{ExprId, StmtId},
    leaf::HirLifetimeRegistryPath,
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
    key: LifetimeRegistryKey,
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
        let invalid = |source| FinalSemanticAnalysisError::RegistryWriteIdentity {
            owner,
            target: *target,
            source,
        };
        let scope = LifetimeScopeKind::parse(path.scope().as_str()).map_err(invalid)?;
        let segments = path
            .segments()
            .iter()
            .map(|segment| DeclarationName::try_new(segment.as_str()))
            .collect::<Result<Box<[_]>, _>>()
            .map_err(invalid)?;
        let key = LifetimeRegistryKey::new(scope, segments);
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
    pub const fn key(&self) -> &LifetimeRegistryKey {
        &self.key
    }
    pub fn matches_path(&self, path: &HirLifetimeRegistryPath) -> bool {
        !path.optional()
            && path.scope().as_str() == self.key.scope().as_str()
            && path.segments().len() == self.key.segments().len()
            && path
                .segments()
                .iter()
                .zip(self.key.segments())
                .all(|(source, key)| source.as_str() == key.as_str())
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
