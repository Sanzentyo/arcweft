use super::{ProjectCompileError, ProjectCompileStage, linked_error_with_registration_sources};
use arcweft_id::PublicId;
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationDiagnostic, CharacterRegistrationRequest,
        ProjectRegistrationFacts, RegisteredSemanticWorld, RegisteredTypeCheckEnv,
    },
};
use arcweft_launch::{accepted::SourceBackedManifest, resolve::ResolvedLaunchProfile};
use arcweft_manifest_model::ProfileId;
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::SourceSetRevision;
use std::sync::Arc;

/// Exact accepted launch objects supplied to one compiler transaction.
///
/// This carrier contains no path or TOML text. Catalog-aware dialogue profile
/// admission consumes these same immutable objects after View lowering.
#[derive(Clone)]
pub struct AcceptedLaunchProfileInput {
    manifest: Arc<SourceBackedManifest>,
    profile_id: ProfileId,
    resolved_profile: ResolvedLaunchProfile,
    topology_source_revision: SourceSetRevision,
    resource_types: Arc<ResourceTypeRegistry>,
}

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
    resource_types: Arc<ResourceTypeRegistry>,
    previous: Option<Arc<RegisteredTypeCheckEnv>>,
    entry_selection: Option<ProjectEntrySelection>,
    accepted_launch_profile: Option<AcceptedLaunchProfileInput>,
}

impl AcceptedLaunchProfileInput {
    pub const fn new(
        manifest: Arc<SourceBackedManifest>,
        profile_id: ProfileId,
        resolved_profile: ResolvedLaunchProfile,
        topology_source_revision: SourceSetRevision,
        resource_types: Arc<ResourceTypeRegistry>,
    ) -> Self {
        Self {
            manifest,
            profile_id,
            resolved_profile,
            topology_source_revision,
            resource_types,
        }
    }

    pub const fn manifest(&self) -> &Arc<SourceBackedManifest> {
        &self.manifest
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub const fn resolved_profile(&self) -> &ResolvedLaunchProfile {
        &self.resolved_profile
    }

    pub const fn topology_source_revision(&self) -> SourceSetRevision {
        self.topology_source_revision
    }

    pub const fn resource_types(&self) -> &Arc<ResourceTypeRegistry> {
        &self.resource_types
    }
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
        resource_types: Arc<ResourceTypeRegistry>,
        previous: Option<Arc<RegisteredTypeCheckEnv>>,
        entry_selection: Option<ProjectEntrySelection>,
    ) -> Self {
        Self {
            base,
            facts,
            resource_types,
            previous,
            entry_selection,
            accepted_launch_profile: None,
        }
    }

    /// Supplies the immutable launch-profile objects accepted by the loader.
    #[must_use]
    pub fn with_accepted_launch_profile(mut self, input: AcceptedLaunchProfileInput) -> Self {
        self.accepted_launch_profile = Some(input);
        self
    }
}

impl ProjectCompilationContext {
    pub(super) fn facts(&self) -> &ProjectRegistrationFacts {
        &self.facts
    }

    pub(super) const fn entry_selection(&self) -> Option<&ProjectEntrySelection> {
        self.entry_selection.as_ref()
    }

    /// Accepted launch-profile input retained for the later compiler admission stages.
    pub const fn accepted_launch_profile(&self) -> Option<&AcceptedLaunchProfileInput> {
        self.accepted_launch_profile.as_ref()
    }

    /// Exact configured-resource registry used by this compiler transaction.
    pub const fn resource_types(&self) -> &Arc<ResourceTypeRegistry> {
        &self.resource_types
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

#[cfg(test)]
mod tests {
    use super::AcceptedLaunchProfileInput;
    use arcweft_launch::{LaunchProfileSelection, accepted::SourceBackedManifest};
    use arcweft_manifest_model::ProfileId;
    use arcweft_resource_model::registry::{
        RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION, ResourceRegistryPublication, ResourceTypeRegistry,
    };
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use std::sync::Arc;

    #[test]
    fn accepted_launch_input_retains_one_exact_typed_object_graph() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("project-manifest").expect("document id"),
                SourceName::Memory,
                r#"schema = 1
[package]
id = "org.arcweft.test"
version = "1.0.0"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
"#,
            )
            .expect("document"),
        );
        let manifest = Arc::new(
            SourceBackedManifest::decode(Arc::clone(&document)).expect("accepted manifest"),
        );
        let profile_id = ProfileId::new("dev").expect("profile id");
        let resolved = manifest
            .resolve_profile(LaunchProfileSelection::Explicit(profile_id.as_str()))
            .expect("resolved profile");
        let topology_source_revision = SourceSetRevision::try_for_identities([document.identity()])
            .expect("topology revision");
        let resource_types = Arc::new(
            ResourceTypeRegistry::publish(ResourceRegistryPublication::new(
                RESOURCE_TYPE_MANIFEST_SCHEMA_VERSION,
                [],
                [],
                [],
            ))
            .expect("registry"),
        );

        let input = AcceptedLaunchProfileInput::new(
            Arc::clone(&manifest),
            profile_id.clone(),
            resolved.clone(),
            topology_source_revision,
            Arc::clone(&resource_types),
        );

        assert!(Arc::ptr_eq(input.manifest(), &manifest));
        assert_eq!(input.profile_id(), &profile_id);
        assert_eq!(input.resolved_profile(), &resolved);
        assert_eq!(input.topology_source_revision(), topology_source_revision);
        assert!(Arc::ptr_eq(input.resource_types(), &resource_types));
    }
}
