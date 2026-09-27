//! The lexical substitution and transaction budget for one materialized instance.

use arcweft_lang_sema::{
    callable::CheckedProjectFunctionInstanceSolution,
    checked_rich_text::CheckedDisplayConformance,
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
    solution: ProjectTypeSolution<'a>,
}

#[derive(Clone, Copy)]
enum ProjectTypeSolution<'a> {
    Function {
        solution: &'a CheckedProjectFunctionInstanceSolution,
        work: &'a ProjectInstantiationWork,
    },
    Display(&'a CheckedDisplayConformance),
}

impl<'a> ProjectInstanceTypes<'a> {
    pub(super) const fn new(
        origin: ProjectInstantiationOrigin,
        solution: &'a CheckedProjectFunctionInstanceSolution,
        work: &'a ProjectInstantiationWork,
    ) -> Self {
        Self {
            origin,
            solution: ProjectTypeSolution::Function { solution, work },
        }
    }

    pub(in crate::lower) const fn display(
        owner: arcweft_lang_hir::identity::ItemId,
        conformance: &'a CheckedDisplayConformance,
    ) -> Self {
        Self {
            origin: ProjectInstantiationOrigin::TraitMethod(owner),
            solution: ProjectTypeSolution::Display(conformance),
        }
    }

    pub(in crate::lower) const fn function_solution(
        self,
    ) -> Option<&'a CheckedProjectFunctionInstanceSolution> {
        match self.solution {
            ProjectTypeSolution::Function { solution, .. } => Some(solution),
            ProjectTypeSolution::Display(_) => None,
        }
    }

    pub(in crate::lower) fn callable_type(self) -> &'a TypeKind {
        match self.solution {
            ProjectTypeSolution::Function { solution, .. } => solution.callable_type(),
            ProjectTypeSolution::Display(_) => {
                unreachable!("DisplayText is not a project function")
            }
        }
    }

    pub(in crate::lower) fn instantiate_type(
        self,
        ty: &TypeKind,
    ) -> Result<TypeKind, RuntimeSemanticProjectionError> {
        match self.solution {
            ProjectTypeSolution::Function { solution, work } => {
                Ok(solution
                    .instantiate_type_with_control(ty, &mut work.type_control(self.origin))?)
            }
            ProjectTypeSolution::Display(conformance) => conformance
                .instantiate_type(ty)
                .map_err(|error| self.origin.error(error.to_string())),
        }
    }

    pub(in crate::lower) fn instantiate_array_length(
        self,
        length: &ArrayLength,
    ) -> Result<ArrayLength, RuntimeSemanticProjectionError> {
        match self.solution {
            ProjectTypeSolution::Function { solution, work } => Ok(solution
                .instantiate_array_length_with_control(
                    length,
                    &mut work.type_control(self.origin),
                )?),
            ProjectTypeSolution::Display(_) => match length {
                ArrayLength::Const(_) => Ok(length.clone()),
                _ => Err(self
                    .origin
                    .error("DisplayText instance has an unclosed array length")),
            },
        }
    }

    pub(in crate::lower) fn instantiate_effect_row(
        self,
        row: &EffectRow,
    ) -> Result<EffectSet, RuntimeSemanticProjectionError> {
        match self.solution {
            ProjectTypeSolution::Function { solution, work } => Ok(solution
                .instantiate_effect_row_with_control(row, &mut work.type_control(self.origin))?),
            ProjectTypeSolution::Display(_) => row
                .resolve(&arcweft_lang_sema::effect_row::EffectSubstitution::default())
                .map_err(|error| self.origin.error(error.to_string())),
        }
    }

    pub(in crate::lower) fn instantiate_project_nominal(
        self,
        nominal: &CheckedProjectNominal,
    ) -> Result<CheckedProjectNominal, RuntimeSemanticProjectionError> {
        match self.solution {
            ProjectTypeSolution::Function { solution, work } => Ok(solution
                .instantiate_project_nominal_with_control(
                    nominal,
                    &mut work.type_control(self.origin),
                )?),
            ProjectTypeSolution::Display(_) => {
                let closed = self.instantiate_type(&nominal.ty())?;
                let identity = closed
                    .semantic_identity_digest()
                    .map_err(|error| self.origin.error(error.to_string()))?;
                let TypeKind::ProjectNominal(closed) = closed else {
                    return Err(self
                        .origin
                        .error("DisplayText substitution changed a nominal constructor"));
                };
                Ok(CheckedProjectNominal::new(
                    closed.declaration().clone(),
                    nominal.owner(),
                    identity,
                    closed.arguments().to_vec(),
                ))
            }
        }
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
