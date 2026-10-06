//! Accepted Flow permission contract and its exact body invocation authority.

use std::sync::Arc;

use arcweft_lang_hir::{
    identity::ItemId,
    project::{HirAnalysisProjectView, HirDeclarationBodyRootRole},
    symbol::ProjectSymbolTable,
};

use super::{
    CheckedExecutionBodyOwner, CheckedExecutionContextError, CheckedExecutionInputAbi,
    CheckedExecutionSource, CheckedItemRole, FinalSemanticAnalysis, FinalSemanticAnalysisError,
};
use crate::effects::EffectSet;

/// One accepted Flow contract. Exposed permissions can include unused or scoped
/// members; body execution effects are retained independently in its ABI.
#[derive(Debug)]
pub struct CheckedFlowExecutionDefinition {
    owner: ItemId,
    body: Arc<CheckedExecutionInputAbi>,
    effects: EffectSet,
}

impl CheckedFlowExecutionDefinition {
    pub const fn owner(&self) -> ItemId {
        self.owner
    }

    pub const fn body(&self) -> &Arc<CheckedExecutionInputAbi> {
        &self.body
    }

    pub const fn effects(&self) -> &EffectSet {
        &self.effects
    }
}

impl FinalSemanticAnalysis {
    pub fn checked_flow_execution_definition(
        &self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        owner: ItemId,
    ) -> Result<CheckedFlowExecutionDefinition, CheckedExecutionContextError> {
        self.validate_generation(project, symbols)?;
        let item = self
            .item(owner)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        if !matches!(item.role(), CheckedItemRole::Flow { .. }) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
        }
        let declaration = symbols
            .flow_symbol_for_item(owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let source = CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
            declaration: declaration.declaration().clone(),
            role: HirDeclarationBodyRootRole::FlowBody,
        });
        let context = self.checked_execution_context(project, symbols, source.clone(), None)?;
        let body = Arc::new(context.checked_execution_input_abi(source)?);
        Ok(CheckedFlowExecutionDefinition {
            owner,
            body,
            effects: item.effects().clone(),
        })
    }
}
