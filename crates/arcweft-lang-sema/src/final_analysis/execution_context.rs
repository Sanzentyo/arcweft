//! One accepted lexical owner, frozen environment and local-use authority.

use std::sync::Arc;

use arcweft_lang_hir::{
    identity::ExprId,
    project::{HirAnalysisProjectView, HirSemanticPathRoot},
    symbol::{CallableDeclarationKey, ProjectSymbolTable},
};

use crate::types::{TypeKind, constraints::ClosedTypeInstantiation};

use super::{
    CheckedExecutionBodyOwner, CheckedLocalUseAuthority, CheckedLocalUseCatalog,
    CheckedLocalUseInstantiation, FinalSemanticAnalysis, FinalSemanticAnalysisError,
};

/// Selects the value-creation or body-invocation boundary. Acceptance remains
/// report/context issued; constructing this selector does not admit execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedExecutionSource {
    EvaluateValue(ExprId),
    /// Executes a binding and exports its owned values in canonical binding order.
    ExportBinding(arcweft_lang_hir::identity::StmtId),
    InvokeBody(CheckedExecutionBodyOwner),
}

impl From<ExprId> for CheckedExecutionSource {
    fn from(source: ExprId) -> Self {
        Self::EvaluateValue(source)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CheckedExecutionContextError {
    #[error("callable {declaration:?} is outside the execution environment's lexical owner")]
    ForeignCallableOwner {
        declaration: Box<crate::callable::CheckedCallableDeclaration>,
    },
    #[error(transparent)]
    Semantic(#[from] FinalSemanticAnalysisError),
    #[error(transparent)]
    LocalUse(#[from] super::CheckedLocalUseError),
    #[error("execution context needs a closed instance of {declaration:?}")]
    OpenDeclaration {
        declaration: Box<CallableDeclarationKey>,
    },
    #[error("source {owner:?} is outside the execution context's lexical owner")]
    ScopeMismatch { owner: Box<CheckedExecutionSource> },
    #[error("execution input evidence belongs to another callable authority")]
    ForeignAuthority,
    #[error("execution input evidence belongs to another closed instance")]
    InstanceMismatch,
}

#[derive(Debug)]
enum ExecutionTypeEnvironment {
    Monomorphic,
    ProjectFunction(Arc<crate::callable::CheckedProjectFunctionInstanceSolution>),
    DisplayText(Arc<crate::checked_rich_text::CheckedDisplayConformance>),
    Declaration(crate::types::GenericDeclarationBinder),
}

/// Owned snapshot of the exact environment that issued an execution admission.
/// Its substitution and complete local-use authority are sealed together.
#[derive(Debug)]
pub struct CheckedExecutionEnvironment {
    authority: crate::callable::CheckedCallableAuthorityLease,
    scope: HirSemanticPathRoot,
    types: ExecutionTypeEnvironment,
    local_uses: CheckedLocalUseAuthority,
}

impl CheckedExecutionEnvironment {
    pub fn validate_callable_owner(
        &self,
        id: &crate::callable::CheckedCallableId,
        topology: &arcweft_lang_hir::project::HirProjectEvaluationTopology,
    ) -> Result<(), CheckedExecutionContextError> {
        if !self.authority.admits_generation(topology.generation())
            || !self.authority.admits_callable_id(id)
        {
            return Err(CheckedExecutionContextError::ForeignAuthority);
        }
        let crate::callable::CheckedCallableDeclaration::Project(declaration) = id.declaration()
        else {
            return Err(CheckedExecutionContextError::ForeignAuthority);
        };
        let owns = match self.scope() {
            HirSemanticPathRoot::Declaration(expected) => expected == declaration,
            HirSemanticPathRoot::Item { item, .. } => topology
                .declaration(declaration)
                .is_ok_and(|view| view.body().source_item() == *item),
        };
        if !owns {
            return Err(CheckedExecutionContextError::ForeignCallableOwner {
                declaration: Box::new(id.declaration().clone()),
            });
        }
        Ok(())
    }

    pub const fn scope(&self) -> &HirSemanticPathRoot {
        &self.scope
    }

    pub fn instantiation(&self) -> Option<CheckedLocalUseInstantiation<'_>> {
        match &self.types {
            ExecutionTypeEnvironment::ProjectFunction(instance) => {
                Some(CheckedLocalUseInstantiation::ProjectFunction(instance))
            }
            ExecutionTypeEnvironment::DisplayText(instance) => {
                Some(CheckedLocalUseInstantiation::DisplayText(instance))
            }
            ExecutionTypeEnvironment::Monomorphic | ExecutionTypeEnvironment::Declaration(_) => {
                None
            }
        }
    }

    /// Lexical type scope owned by the exact declaration environment.
    /// Closed instances and monomorphic roots have the root scope.
    pub fn type_scope(&self) -> crate::types::GenericScope {
        match &self.types {
            ExecutionTypeEnvironment::Declaration(binder) => binder.scope().clone(),
            _ => crate::types::GenericScope::default(),
        }
    }

    /// Gives projected terms their canonical identity in this owned type scope.
    pub fn semantic_type_identity(
        &self,
        ty: &TypeKind,
    ) -> Result<crate::types::SemanticTypeDigest, crate::types::GenericScopeError> {
        ty.semantic_identity_digest()
            .or_else(|_| ty.semantic_identity_digest_in_scope(&self.type_scope()))
    }

    pub const fn local_uses(&self) -> &CheckedLocalUseAuthority {
        &self.local_uses
    }

    pub fn instance_identity(&self) -> Option<&super::CheckedLocalUseInstanceIdentity> {
        self.local_uses.instance_identity()
    }

    pub fn instantiate_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, CheckedExecutionContextError> {
        if let ExecutionTypeEnvironment::Declaration(binder) = &self.types {
            return binder
                .project_with_control(ty, &mut crate::types::UnmeteredTypeProjection)
                .map(|ty| ty.view().value().clone())
                .map_err(crate::types::TypeProjectionError::into_instantiation)
                .map_err(super::CheckedLocalUseError::from)
                .map_err(CheckedExecutionContextError::from);
        }
        match self.instantiation() {
            Some(instance) => Ok(instance.instantiate_type(ty)?),
            None => ClosedTypeInstantiation::default()
                .instantiate_type(ty)
                .map_err(super::CheckedLocalUseError::from)
                .map_err(CheckedExecutionContextError::from),
        }
    }

    pub fn validate_analysis(
        &self,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<(), CheckedExecutionContextError> {
        if !self.authority.admits(analysis.checked_callables()) {
            return Err(CheckedExecutionContextError::ForeignAuthority);
        }
        Ok(())
    }

    pub fn validate_project(
        &self,
        project: HirAnalysisProjectView<'_>,
    ) -> Result<(), CheckedExecutionContextError> {
        if !self.authority.admits_hir(project) {
            return Err(CheckedExecutionContextError::ForeignAuthority);
        }
        Ok(())
    }
}

/// Report-issued lexical context for admitting executable roots. The frozen
/// substitution and its transfer/Copy/place certificates cannot be supplied
/// independently. Monomorphic contexts remain bound to their lexical owner;
/// they never provide fallback evidence for unbound declaration type/const
/// parameters. A value root need not invoke latent callback effects in the
/// owner's signature; its actual input/result types still must close.
pub struct CheckedClosedExecutionContext<'analysis> {
    analysis: &'analysis FinalSemanticAnalysis,
    project: HirAnalysisProjectView<'analysis>,
    environment: Arc<CheckedExecutionEnvironment>,
}

impl CheckedClosedExecutionContext<'_> {
    pub fn scope(&self) -> &HirSemanticPathRoot {
        self.environment.scope()
    }

    /// Projects a semantic type using exactly the environment which issued
    /// this context's local-use certificates.
    pub fn instantiate_type(
        &self,
        ty: &TypeKind,
    ) -> Result<TypeKind, CheckedExecutionContextError> {
        self.environment.instantiate_type(ty)
    }

    pub const fn environment(&self) -> &Arc<CheckedExecutionEnvironment> {
        &self.environment
    }

    pub(super) const fn analysis(&self) -> &FinalSemanticAnalysis {
        self.analysis
    }

    pub(super) const fn project(&self) -> HirAnalysisProjectView<'_> {
        self.project
    }

    pub(super) fn local_uses(&self) -> &CheckedLocalUseCatalog {
        match self.environment.local_uses() {
            CheckedLocalUseAuthority::Global(catalog) => catalog,
            CheckedLocalUseAuthority::Instance(catalog) => catalog.catalog(),
        }
    }

    pub(super) fn instance_identity(&self) -> Option<&super::CheckedLocalUseInstanceIdentity> {
        self.environment.instance_identity()
    }

    pub(super) fn admit_root(
        &self,
        source: &CheckedExecutionSource,
    ) -> Result<(), CheckedExecutionContextError> {
        let scope = self.analysis.execution_root_scope(source)?;
        if &scope != self.scope() {
            return Err(CheckedExecutionContextError::ScopeMismatch {
                owner: Box::new(source.clone()),
            });
        }
        Ok(())
    }
}

impl FinalSemanticAnalysis {
    fn execution_root_scope(
        &self,
        source: &CheckedExecutionSource,
    ) -> Result<HirSemanticPathRoot, FinalSemanticAnalysisError> {
        match source {
            CheckedExecutionSource::EvaluateValue(source) => self.execution_source_scope(*source),
            CheckedExecutionSource::ExportBinding(source) => {
                let location = self
                    .hir_topology()
                    .semantic_path((*source).into())
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                    .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
                self.accepted_root_catalog()
                    .root_for_hir(location.root())
                    .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                Ok(location.root().clone())
            }
            CheckedExecutionSource::InvokeBody(owner) => {
                if !self.has_execution_body(owner) {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                match owner {
                    CheckedExecutionBodyOwner::CallableValue(source) => {
                        self.execution_source_scope(*source)
                    }
                    CheckedExecutionBodyOwner::Declaration { declaration, .. } => {
                        let scope = HirSemanticPathRoot::Declaration(declaration.clone());
                        self.accepted_root_catalog()
                            .root_for_hir(&scope)
                            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
                        Ok(scope)
                    }
                }
            }
        }
    }
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
        project: HirAnalysisProjectView<'analysis>,
        symbols: &ProjectSymbolTable,
        source: impl Into<CheckedExecutionSource>,
        instance: Option<CheckedLocalUseInstantiation<'analysis>>,
    ) -> Result<CheckedClosedExecutionContext<'analysis>, CheckedExecutionContextError> {
        self.validate_generation(project, symbols)?;
        let source = source.into();
        let scope = self.execution_root_scope(&source)?;
        let local_uses = if let Some(instance) = instance {
            if scope != HirSemanticPathRoot::Declaration(instance.declaration()) {
                return Err(CheckedExecutionContextError::ScopeMismatch {
                    owner: Box::new(source),
                });
            }
            CheckedLocalUseAuthority::Instance(Arc::new(
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
            CheckedLocalUseAuthority::Global(Arc::clone(self.checked_local_uses()))
        };
        let types = match instance {
            Some(instance) => match instance {
                CheckedLocalUseInstantiation::ProjectFunction(instance) => {
                    ExecutionTypeEnvironment::ProjectFunction(Arc::new(instance.clone()))
                }
                CheckedLocalUseInstantiation::DisplayText(instance) => {
                    ExecutionTypeEnvironment::DisplayText(Arc::new(instance.clone()))
                }
            },
            None => match &scope {
                HirSemanticPathRoot::Declaration(declaration)
                    if declaration.owner()
                        == arcweft_lang_hir::symbol::CallableDeclarationOwner::View =>
                {
                    let checked = self
                        .checked_callables()
                        .project_callable(declaration)
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                    ExecutionTypeEnvironment::Declaration(
                        checked
                            .signature()
                            .function_value_binder()
                            .map_err(crate::types::TypeInstantiationError::from)
                            .map_err(super::CheckedLocalUseError::from)?,
                    )
                }
                _ => ExecutionTypeEnvironment::Monomorphic,
            },
        };
        Ok(CheckedClosedExecutionContext {
            analysis: self,
            project,
            environment: Arc::new(CheckedExecutionEnvironment {
                authority: self.checked_callables().authority_lease(),
                scope,
                types,
                local_uses,
            }),
        })
    }
}
