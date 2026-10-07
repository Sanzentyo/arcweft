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

    /// Complete Flow invocation signature. Declared permissions remain distinct
    /// from the narrower execution effects retained by the body ABI.
    pub fn function_type(&self) -> Option<crate::types::TypeKind> {
        let mut signature = self.body.function_type()?;
        let crate::types::TypeKind::Function { effects, .. } = &mut signature else {
            unreachable!("the accepted execution ABI issues a function signature")
        };
        *effects = crate::effect_row::EffectRow::closed(self.effects.clone());
        Some(signature)
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
