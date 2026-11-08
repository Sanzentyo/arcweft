//! Accepted Flow permission contract and its exact body invocation authority.

use std::sync::Arc;

use arcweft_lang_hir::{
    identity::ItemId,
    project::{HirAnalysisProjectView, HirDeclarationBodyRootRole},
    symbol::{CallableDeclarationKey, FlowDeclarationId, ProjectSymbolTable},
};

use super::{
    CheckedExecutionBodyOwner, CheckedExecutionContextError, CheckedExecutionInputAbi,
    CheckedExecutionSource, CheckedItemRole, FinalSemanticAnalysis, FinalSemanticAnalysisError,
};
use crate::effects::EffectSet;

impl crate::callable::CheckedCallableCatalog {
    /// Bare goto and compact Choice arms supply no arguments. Check their
    /// complete declaration ABI, including unused and wildcard formals, before
    /// publishing semantic success. Stateful Entry startup is a separate typed
    /// invocation and must not enter this zero-input boundary.
    pub(super) fn validate_empty_flow_transfer(
        &self,
        target: &super::CheckedProjectItem,
        owner: arcweft_lang_hir::identity::ExprId,
        source_span: arcweft_source::SourceSpan,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let (declaration, _) = target
            .flow_owner()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let callable = self
            .project_callable(declaration)
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        let expected = callable
            .signature()
            .groups()
            .iter()
            .try_fold(0usize, |count, group| {
                count
                    .checked_add(group.parameters().len())
                    .ok_or(FinalSemanticAnalysisError::AccountingOverflow)
            })?;
        if expected != 0 {
            return Err(FinalSemanticAnalysisError::FlowTransferArgumentCount {
                owner,
                expected,
                actual: 0,
                source_span,
            });
        }
        Ok(())
    }
}

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

    /// Original declaration selected with this definition's accepted body.
    /// The private constructor always requests the same Flow body source.
    pub fn declaration(&self) -> &FlowDeclarationId {
        match self.body.source() {
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration: CallableDeclarationKey::Flow(declaration),
                role: HirDeclarationBodyRootRole::FlowBody,
            }) => declaration,
            _ => unreachable!("checked Flow definitions retain their original declaration body"),
        }
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
