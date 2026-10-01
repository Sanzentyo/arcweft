//! Execution instances are issued from this report's joined source evidence.

use arcweft_lang_hir::{
    identity::ExprId, project::HirSemanticPathRoot, symbol::CallableDeclarationKey,
};

use crate::{
    callable::{
        CheckedProjectFunctionCallableSource, CheckedProjectFunctionInstanceProjectionError,
        CheckedProjectFunctionInstanceSolution, CheckedProjectFunctionRootRuntimeSelection,
        CheckedProjectFunctionRuntimeSelection, CheckedProjectFunctionRuntimeSelectionError,
        select_project_function_root_runtime, select_project_function_runtime,
        select_project_function_value_runtime,
    },
    types::TypeProjectionControl,
};

use super::FinalSemanticAnalysis;

impl FinalSemanticAnalysis {
    /// Selects one application's executable instance using only the source
    /// application, join and callable authority retained by this report.
    pub fn project_function_runtime(
        &self,
        owner: ExprId,
    ) -> Result<
        Option<CheckedProjectFunctionRuntimeSelection>,
        CheckedProjectFunctionRuntimeSelectionError,
    > {
        let application = self
            .call(owner)
            .and_then(|call| call.selected_application())
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::JoinMismatch)?;
        let join = self
            .checked_callable_join(owner)
            .map_err(|_| CheckedProjectFunctionRuntimeSelectionError::JoinMismatch)?;
        let location = self
            .hir_topology()
            .semantic_path(owner.into())
            .map_err(|_| CheckedProjectFunctionRuntimeSelectionError::ForeignAuthority)?
            .ok_or(CheckedProjectFunctionRuntimeSelectionError::ForeignAuthority)?;
        let caller = match location.root() {
            HirSemanticPathRoot::Declaration(declaration) => Some(declaration.clone()),
            HirSemanticPathRoot::Item { .. } => None,
        };
        select_project_function_runtime(application, join, self.checked_callables(), caller)
    }

    /// Selects a closed declaration for an already checked runtime ingress.
    pub fn project_function_root_runtime(
        &self,
        declaration: &CallableDeclarationKey,
    ) -> Result<
        CheckedProjectFunctionRootRuntimeSelection,
        CheckedProjectFunctionRuntimeSelectionError,
    > {
        select_project_function_root_runtime(declaration, self.checked_callables())
    }

    /// Selects a declaration value in this report's accepted callable world.
    pub fn project_function_value_runtime_with_control<C: TypeProjectionControl>(
        &self,
        declaration: &CallableDeclarationKey,
        enclosing: Option<&CheckedProjectFunctionInstanceSolution>,
        control: &mut C,
    ) -> Result<
        CheckedProjectFunctionCallableSource,
        CheckedProjectFunctionInstanceProjectionError<C::Error>,
    > {
        select_project_function_value_runtime(
            declaration,
            self.checked_callables(),
            enclosing,
            control,
        )
    }
}
