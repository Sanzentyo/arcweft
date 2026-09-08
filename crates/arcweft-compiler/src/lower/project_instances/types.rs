//! The lexical substitution and transaction budget for one materialized instance.

use arcweft_lang_sema::{
    callable::CheckedProjectFunctionInstanceSolution,
    effect_row::EffectRow,
    effects::EffectSet,
    final_analysis::CheckedProjectNominal,
    types::{ArrayLength, TypeKind, TypeProjectionError},
};

use super::{
    ProjectInstantiationError, ProjectInstantiationOrigin, work::ProjectInstantiationWork,
};
use crate::lower::RuntimeSemanticProjectionError;

/// Every consumer of a project instance carries this complete projection
/// context. The sealed graph owns both references, so a nested closure or a
/// Content/Fx consumer cannot reset accounting or adopt another substitution.
#[derive(Clone, Copy)]
pub(in crate::lower) struct ProjectInstanceTypes<'a> {
    origin: ProjectInstantiationOrigin,
    solution: &'a CheckedProjectFunctionInstanceSolution,
    work: &'a ProjectInstantiationWork,
}

impl<'a> ProjectInstanceTypes<'a> {
    pub(super) const fn new(
        origin: ProjectInstantiationOrigin,
        solution: &'a CheckedProjectFunctionInstanceSolution,
        work: &'a ProjectInstantiationWork,
    ) -> Self {
        Self {
            origin,
            solution,
            work,
        }
    }

    pub(in crate::lower) const fn solution(self) -> &'a CheckedProjectFunctionInstanceSolution {
        self.solution
    }

    pub(in crate::lower) const fn function_type(self) -> &'a TypeKind {
        self.solution.function_type()
    }

    pub(in crate::lower) fn instantiate_type(
        self,
        ty: &TypeKind,
    ) -> Result<TypeKind, TypeProjectionError<ProjectInstantiationError>> {
        self.solution
            .instantiate_type_with_control(ty, &mut self.work.type_control(self.origin))
    }

    pub(in crate::lower) fn instantiate_array_length(
        self,
        length: &ArrayLength,
    ) -> Result<ArrayLength, TypeProjectionError<ProjectInstantiationError>> {
        self.solution
            .instantiate_array_length_with_control(length, &mut self.work.type_control(self.origin))
    }

    pub(in crate::lower) fn instantiate_effect_row(
        self,
        row: &EffectRow,
    ) -> Result<EffectSet, TypeProjectionError<ProjectInstantiationError>> {
        self.solution
            .instantiate_effect_row_with_control(row, &mut self.work.type_control(self.origin))
    }

    pub(in crate::lower) fn instantiate_project_nominal(
        self,
        nominal: &CheckedProjectNominal,
    ) -> Result<CheckedProjectNominal, TypeProjectionError<ProjectInstantiationError>> {
        self.solution.instantiate_project_nominal_with_control(
            nominal,
            &mut self.work.type_control(self.origin),
        )
    }
}

impl From<TypeProjectionError<ProjectInstantiationError>> for RuntimeSemanticProjectionError {
    fn from(error: TypeProjectionError<ProjectInstantiationError>) -> Self {
        match error {
            TypeProjectionError::Instantiation(error) => Self::TypeInstantiation(error),
            TypeProjectionError::Control(error) => Self::ProjectInstantiation(error),
        }
    }
}
