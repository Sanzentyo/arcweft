//! One accepted lexical owner, frozen environment and local-use authority.

use arcweft_lang_hir::{
    identity::ExprId,
    project::{HirAnalysisProjectView, HirSemanticPathRoot},
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};

use crate::types::{TypeKind, constraints::ClosedTypeInstantiation};

use super::{
    CheckedLocalUseCatalog, CheckedLocalUseInstanceCatalog, CheckedLocalUseInstantiation,
    FinalSemanticAnalysis, FinalSemanticAnalysisError,
};

#[derive(Debug, thiserror::Error)]
pub enum CheckedExecutionContextError {
    #[error(transparent)]
    Semantic(#[from] FinalSemanticAnalysisError),
    #[error(transparent)]
    LocalUse(#[from] super::CheckedLocalUseError),
    #[error("execution context needs a closed instance of {declaration:?}")]
    OpenDeclaration {
        declaration: Box<CallableDeclarationKey>,
    },
    #[error("expression {owner:?} is outside the execution context's lexical owner")]
    ScopeMismatch { owner: ExprId },
    #[error("execution input evidence belongs to another callable authority")]
    ForeignAuthority,
    #[error("execution input evidence belongs to another closed instance")]
    InstanceMismatch,
}

enum ClosedLocalUseEvidence<'analysis> {
    Global(&'analysis CheckedLocalUseCatalog),
    Instance(Box<CheckedLocalUseInstanceCatalog>),
}

/// Report-issued lexical context for admitting executable roots. The frozen
/// substitution and its transfer/Copy/place certificates cannot be supplied
/// independently. Monomorphic contexts remain bound to their lexical owner;
/// they never provide fallback evidence for unbound declaration type/const
/// parameters. A value root need not invoke latent callback effects in the
/// owner's signature; its actual input/result types still must close.
pub struct CheckedClosedExecutionContext<'analysis> {
    analysis: &'analysis FinalSemanticAnalysis,
    scope: HirSemanticPathRoot,
    instance: Option<CheckedLocalUseInstantiation<'analysis>>,
    local_uses: ClosedLocalUseEvidence<'analysis>,
}

impl CheckedClosedExecutionContext<'_> {
    pub const fn scope(&self) -> &HirSemanticPathRoot {
        &self.scope
    }

    /// Projects a semantic type using exactly the environment which issued
    /// this context's local-use certificates.
    pub fn instantiate_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, CheckedExecutionContextError> {
        match self.instance {
            Some(instance) => Ok(instance.instantiate_type(ty)?),
            None => ClosedTypeInstantiation::default()
                .instantiate_type(ty)
                .map_err(super::CheckedLocalUseError::from)
                .map_err(CheckedExecutionContextError::from),
        }
    }

    pub(super) const fn analysis(&self) -> &FinalSemanticAnalysis {
        self.analysis
    }

    pub(super) fn local_uses(&self) -> &CheckedLocalUseCatalog {
        match &self.local_uses {
            ClosedLocalUseEvidence::Global(catalog) => catalog,
            ClosedLocalUseEvidence::Instance(catalog) => catalog.catalog(),
        }
    }

    pub(super) fn instance_identity(&self) -> Option<&super::CheckedLocalUseInstanceIdentity> {
        match &self.local_uses {
            ClosedLocalUseEvidence::Global(_) => None,
            ClosedLocalUseEvidence::Instance(catalog) => Some(catalog.identity()),
        }
    }

    pub(super) fn admit_source(&self, source: ExprId) -> Result<(), CheckedExecutionContextError> {
        let scope = self.analysis.execution_source_scope(source)?;
        if scope != self.scope {
            return Err(CheckedExecutionContextError::ScopeMismatch { owner: source });
        }
        Ok(())
    }
}

impl FinalSemanticAnalysis {
    fn execution_source_scope(
        &self,
        source: ExprId,
    ) -> Result<HirSemanticPathRoot, FinalSemanticAnalysisError> {
        let location = self
            .hir_topology()
            .semantic_path(source.into())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        self.accepted_root_catalog()
            .root_for_hir(location.root())
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        Ok(location.root().clone())
    }

    /// Authenticates a source owner and issues its complete closed environment
    /// together with the ownership seal. An instance must belong to both this
    /// report's callable authority and the source's lexical declaration.
    pub fn checked_execution_context<'analysis>(
        &'analysis self,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        source: ExprId,
        instance: Option<CheckedLocalUseInstantiation<'analysis>>,
    ) -> Result<CheckedClosedExecutionContext<'analysis>, CheckedExecutionContextError> {
        self.validate_generation(project, symbols)?;
        let scope = self.execution_source_scope(source)?;
        let local_uses = if let Some(instance) = instance {
            if scope != HirSemanticPathRoot::Declaration(instance.declaration()) {
                return Err(CheckedExecutionContextError::ScopeMismatch { owner: source });
            }
            ClosedLocalUseEvidence::Instance(Box::new(
                self.checked_local_uses_for_instance(project, symbols, instance)?,
            ))
        } else {
            if let HirSemanticPathRoot::Declaration(declaration) = &scope {
                let checked = self
                    .checked_callables()
                    .project_callable(declaration)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let inventory = checked.signature().generic_inventory();
                if !inventory.types().is_empty() || !inventory.consts().is_empty() {
                    return Err(CheckedExecutionContextError::OpenDeclaration {
                        declaration: Box::new(declaration.clone()),
                    });
                }
            }
            ClosedLocalUseEvidence::Global(self.checked_local_uses())
        };
        Ok(CheckedClosedExecutionContext {
            analysis: self,
            scope,
            instance,
            local_uses,
        })
    }
}
