use super::{ProjectCompileError, ProjectCompileStage, linked_error_with_registration_sources};
use arcweft_id::PublicId;
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_sema::{
    callable::EnvironmentCallablePublication,
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationDiagnostic, CharacterRegistrationRequest,
        ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv,
    },
};
use std::sync::Arc;

/// Launch surface expected for one selected source entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectEntrySelectionKind {
    Game,
    Editor,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Agent,
}

/// Exact source entry selected by a project compilation transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntrySelection {
    id: PublicId,
    kind: ProjectEntrySelectionKind,
}

/// Immutable semantic inputs for one project compilation transaction.
#[derive(Clone)]
pub struct ProjectCompilationContext {
    base: Arc<TypeCheckEnv>,
    facts: Arc<ProjectRegistrationFacts>,
    previous: Option<Arc<RegisteredTypeCheckEnv>>,
    entry_selection: Option<ProjectEntrySelection>,
    callable_publications: Vec<EnvironmentCallablePublication>,
}

impl ProjectEntrySelection {
    pub const fn new(id: PublicId, kind: ProjectEntrySelectionKind) -> Self {
        Self { id, kind }
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    pub const fn kind(&self) -> ProjectEntrySelectionKind {
        self.kind
    }
}

impl ProjectEntrySelectionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Editor => "editor",
            Self::Cli => "cli",
            Self::Server => "server",
            Self::Activity => "activity",
            Self::Test => "test",
            Self::Bench => "bench",
            Self::Agent => "agent",
        }
    }
}

impl ProjectCompilationContext {
    pub fn new(
        base: Arc<TypeCheckEnv>,
        facts: Arc<ProjectRegistrationFacts>,
        previous: Option<Arc<RegisteredTypeCheckEnv>>,
        entry_selection: Option<ProjectEntrySelection>,
        callable_publications: Vec<EnvironmentCallablePublication>,
    ) -> Self {
        Self {
            base,
            facts,
            previous,
            entry_selection,
            callable_publications,
        }
    }
}

impl ProjectCompilationContext {
    pub(super) fn facts(&self) -> &ProjectRegistrationFacts {
        &self.facts
    }

    pub(super) const fn entry_selection(&self) -> Option<&ProjectEntrySelection> {
        self.entry_selection.as_ref()
    }
}

pub(super) fn register(
    project: &HirProject,
    context: &ProjectCompilationContext,
) -> Result<Arc<RegisteredSemanticWorld>, ProjectCompileError> {
    let request = CharacterRegistrationRequest::new(
        Arc::clone(&context.base),
        project,
        &context.facts,
        context.previous.as_deref(),
    );
    let request = context.callable_publications.iter().cloned().fold(
        request,
        CharacterRegistrationRequest::with_callable_publication,
    );
    CharacterRegistrar::register(request)
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
