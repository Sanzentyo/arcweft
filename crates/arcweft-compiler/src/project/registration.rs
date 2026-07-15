use super::{ProjectCompileError, ProjectCompileStage, linked_error_with_registration_sources};
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationDiagnostic, CharacterRegistrationRequest,
        ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv,
    },
};
use std::sync::Arc;

/// Immutable semantic inputs for one project compilation transaction.
#[derive(Clone)]
pub struct ProjectCompilationContext {
    base: Arc<TypeCheckEnv>,
    facts: Arc<ProjectRegistrationFacts>,
    previous: Option<Arc<RegisteredTypeCheckEnv>>,
}

impl ProjectCompilationContext {
    pub fn new(
        base: Arc<TypeCheckEnv>,
        facts: Arc<ProjectRegistrationFacts>,
        previous: Option<Arc<RegisteredTypeCheckEnv>>,
    ) -> Self {
        Self {
            base,
            facts,
            previous,
        }
    }
}

impl ProjectCompilationContext {
    pub(super) fn facts(&self) -> &ProjectRegistrationFacts {
        &self.facts
    }
}

pub(super) fn register(
    project: &HirProject,
    context: &ProjectCompilationContext,
) -> Result<Arc<RegisteredSemanticWorld>, ProjectCompileError> {
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::clone(&context.base),
        project,
        &context.facts,
        context.previous.as_deref(),
    ))
    .map(Arc::new)
    .map_err(|failure| {
        linked_error_with_registration_sources(
            ProjectCompileStage::Registration,
            &context.facts,
            failure
                .diagnostics()
                .iter()
                .map(CharacterRegistrationDiagnostic::diagnostic),
        )
    })
}
