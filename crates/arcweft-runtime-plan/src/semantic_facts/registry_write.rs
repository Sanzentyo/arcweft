//! Registry writes retain the checked address and the instantiated value domain.

use std::collections::BTreeMap;

use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::{HirModuleId, StmtId},
    module::HirModule,
    stmt::HirStmtKind,
};
use arcweft_lang_sema::final_analysis::{CheckedRegistryAddress, CheckedRegistryWrite};

use super::{RuntimeExpressionTypeView, RuntimeNormalizedType, RuntimeSemanticFactsError};

/// A projected write awaiting executable registry storage admission. The source
/// address is checked metadata and grants no runtime scope or value ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRegistryWriteFact {
    source: CheckedRegistryWrite,
    value_type: RuntimeNormalizedType,
}

impl RuntimeRegistryWriteFact {
    pub const fn new(source: CheckedRegistryWrite, value_type: RuntimeNormalizedType) -> Self {
        Self { source, value_type }
    }

    pub fn address(&self) -> &CheckedRegistryAddress {
        self.source.address()
    }

    pub const fn value(&self) -> arcweft_lang_hir::identity::ExprId {
        self.source.value()
    }

    pub const fn value_type(&self) -> &RuntimeNormalizedType {
        &self.value_type
    }

    pub(super) fn validate(
        &self,
        modules: &BTreeMap<HirModuleId, &HirModule>,
        expression_types: &RuntimeExpressionTypeView<'_>,
        statement: StmtId,
    ) -> Result<(), RuntimeSemanticFactsError> {
        let invalid = || RuntimeSemanticFactsError::InvalidRegistryWriteFact { statement };
        let HirStmtKind::LifetimeSet { target, value } = super::resolve_stmt(modules, statement)?
        else {
            return Err(invalid());
        };
        let target_expression = super::module_for(modules, target.module())?
            .resolve_expr(*target)
            .map_err(|_| invalid())?;
        let HirExprKind::LifetimePath(path) = target_expression.kind() else {
            return Err(invalid());
        };
        if self.address().statement() != statement
            || self.address().target() != *target
            || self.value() != *value
            || !path
                .as_resolved()
                .is_some_and(|path| self.address().matches_path(path))
            || expression_types.get(target).is_some()
            || expression_types.get(value) != Some(self.value_type())
        {
            return Err(invalid());
        }
        super::validate_normalized_type(modules, self.value_type())
    }
}
