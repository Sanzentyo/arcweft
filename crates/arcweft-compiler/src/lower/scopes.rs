//! Compiler-owned scope value/propagation projection from final checked Try facts.

use arcweft_lang_hir::identity::{ScopeId, StmtId};
use arcweft_lang_hir::scope::HirScopeKind;
use arcweft_lang_hir::stmt::HirStmtKind;
use arcweft_lang_sema::final_analysis::CheckedTryBoundaryOwner;
use arcweft_runtime_plan::semantic_facts::{RuntimeScopeContinuation, RuntimeScopeFact};

use super::{
    CheckedExpressionResolution, ExprId, FinalSemanticAnalysis, HirExprKind, ProjectInstanceTypes,
    ProjectSymbolTable, RegisteredSemanticWorld, RuntimeSemanticProjectionError, TypeKind,
    checked_expression_type, runtime_scope_identity, runtime_try_fact, runtime_type_under,
};

pub(super) struct ScopeProjection<'a, 'instance> {
    pub module: &'a arcweft_lang_hir::module::HirModule,
    pub symbols: &'a ProjectSymbolTable,
    pub world: &'a RegisteredSemanticWorld,
    pub analysis: &'a FinalSemanticAnalysis,
    pub instance: Option<ProjectInstanceTypes<'instance>>,
    pub eligible: &'a dyn Fn(ExprId) -> bool,
}

impl ScopeProjection<'_, '_> {
    pub(super) fn expression(
        &self,
        owner: ExprId,
        identity: &arcweft_lang_sema::final_analysis::CheckedScopeIdentity,
    ) -> Result<RuntimeScopeFact, RuntimeSemanticProjectionError> {
        let source = self
            .module
            .resolve_expr(owner)
            .map_err(|error| failure(error.to_string()))?;
        let HirExprKind::NamedBlock(block) = source.kind() else {
            return Err(failure("checked scope expression has the wrong HIR family"));
        };
        let expression = self
            .analysis
            .expression(owner)
            .ok_or_else(|| failure("scope expression has no final semantic value"))?;
        self.project(
            block.scope(),
            checked_expression_type(expression, owner)?,
            identity,
        )
    }

    pub(super) fn statement(
        &self,
        owner: StmtId,
        identity: &arcweft_lang_sema::final_analysis::CheckedScopeIdentity,
    ) -> Result<RuntimeScopeFact, RuntimeSemanticProjectionError> {
        let source = self
            .module
            .resolve_stmt(owner)
            .map_err(|error| failure(error.to_string()))?;
        let HirStmtKind::Scope(block) = source.kind() else {
            return Err(failure("checked scope statement has the wrong HIR family"));
        };
        self.project(block.body().scope(), &TypeKind::Unit, identity)
    }

    fn project(
        &self,
        scope: ScopeId,
        value_type: &TypeKind,
        identity: &arcweft_lang_sema::final_analysis::CheckedScopeIdentity,
    ) -> Result<RuntimeScopeFact, RuntimeSemanticProjectionError> {
        let mut shape = None;
        let mut exits = Vec::new();
        let execution = self.analysis.execution_projection();
        for (owner, expression) in self.analysis.expressions() {
            if owner.module() != self.module.module_id()
                || !(self.eligible)(owner)
                || !matches!(expression.resolution(), CheckedExpressionResolution::Try(_))
            {
                continue;
            }
            let source = self
                .module
                .resolve_expr(owner)
                .map_err(|error| failure(error.to_string()))?;
            if !self.contains_execution_scope(source.scope(), scope)? {
                continue;
            }
            let tried = execution.try_expression(owner)?;
            if matches!(
                tried.boundary().owner(),
                CheckedTryBoundaryOwner::Infallible
            ) {
                continue;
            }
            if let Some(boundary) = tried.boundary().owner().expression_boundary() {
                let boundary = self
                    .module
                    .resolve_expr(boundary.lookup_owner())
                    .map_err(|error| failure(error.to_string()))?;
                if self.contains_execution_scope(boundary.scope(), scope)? {
                    continue;
                }
            }
            let carrier = match tried.boundary().boundary_type() {
                TypeKind::Result { error, .. } => TypeKind::Result {
                    ok: Box::new(value_type.clone()),
                    error: error.clone(),
                },
                TypeKind::Option(_) => TypeKind::Option(Box::new(value_type.clone())),
                _ => {
                    return Err(failure(
                        "scope propagation boundary is not a checked carrier",
                    ));
                }
            };
            let checked = runtime_try_fact(
                owner,
                tried,
                self.symbols,
                self.world,
                self.analysis,
                self.instance,
            )?;
            let candidate = (
                runtime_type_under(
                    &carrier,
                    self.instance,
                    self.symbols,
                    self.world,
                    self.analysis,
                )?,
                checked.boundary(),
                checked.boundary_type().clone(),
            );
            if shape
                .as_ref()
                .is_some_and(|previous| previous != &candidate)
            {
                return Err(failure(
                    "one lexical scope has conflicting outward propagation boundaries",
                ));
            }
            shape = Some(candidate);
            exits.push(owner);
        }
        let continuation = shape
            .map(|(carrier, boundary, boundary_type)| {
                RuntimeScopeContinuation::try_new(
                    carrier,
                    boundary,
                    boundary_type,
                    exits.into_boxed_slice(),
                )
                .map_err(|error| failure(error.to_string()))
            })
            .transpose()?;
        Ok(RuntimeScopeFact::new(
            runtime_scope_identity(identity),
            continuation,
        ))
    }

    fn contains_execution_scope(
        &self,
        mut scope: ScopeId,
        ancestor: ScopeId,
    ) -> Result<bool, RuntimeSemanticProjectionError> {
        loop {
            if scope == ancestor {
                return Ok(true);
            }
            let current = self
                .module
                .resolve_scope(scope)
                .map_err(|error| failure(error.to_string()))?;
            if matches!(
                current.kind(),
                HirScopeKind::Callable | HirScopeKind::Flow | HirScopeKind::Closure
            ) {
                return Ok(false);
            }
            let Some(parent) = current.parent() else {
                return Ok(false);
            };
            scope = parent;
        }
    }
}

fn failure(reason: impl Into<String>) -> RuntimeSemanticProjectionError {
    RuntimeSemanticProjectionError::Type {
        reason: reason.into(),
    }
}
