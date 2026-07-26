use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
use arcweft_character::catalog::CharacterCatalog;
use arcweft_compiler::project::{
    AcceptedLaunchProfileInput, ProjectCompilationContext, ProjectCompileError, compile_project,
};
use arcweft_core::entry::{RootExecutionLimits, RuntimeCommandPolicy};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::RegisteredTypeCheckEnv};
use arcweft_launch::{LaunchProfileSelection, ProfileId};
use arcweft_project_loader::{
    environment::{ProfileRegistrationLoadRequest, load_profile_registration},
    topology::{
        LoadedProfileTopology, ProfileTopologyLoadError, ProfileTopologyLoadRequest,
        ProfileTopologyOverlaySeed, ProfileTopologyOwnerId, load_profile_topology,
    },
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use std::{collections::BTreeSet, path::Path, sync::Arc};
use thiserror::Error;

use super::{
    accepted_project::{
        AcceptedProjectSnapshot, AcceptedSourceAccess, AcceptedSourceDocumentSeed,
        AcceptedSourceLocator, AcceptedSourceOwnership,
    },
    state::{
        AcceptedOverlayEntry, AcceptedOverlaySet, AcceptedOverlaySetError,
        AcceptedProfileCandidate, AcceptedProfileCandidateError, AcceptedProfileKey,
    },
    uri::file_uri_from_path,
};
use crate::uri_key::LspUriKey;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AcceptedBuildWorkSnapshot {
    pub(crate) topology_loads: u64,
    pub(crate) compiler_builds: u64,
}

#[cfg(test)]
std::thread_local! {
    static ACCEPTED_BUILD_WORK: std::cell::Cell<AcceptedBuildWorkSnapshot> =
        const { std::cell::Cell::new(AcceptedBuildWorkSnapshot {
            topology_loads: 0,
            compiler_builds: 0,
        }) };
}

#[cfg(test)]
pub(crate) fn accepted_build_work_snapshot_for_test() -> AcceptedBuildWorkSnapshot {
    ACCEPTED_BUILD_WORK.get()
}

#[cfg(test)]
fn record_topology_load() {
    ACCEPTED_BUILD_WORK.with(|recorded| {
        let mut work = recorded.get();
        work.topology_loads = work
            .topology_loads
            .checked_add(1)
            .expect("test topology-load counter remains bounded");
        recorded.set(work);
    });
}

#[cfg(not(test))]
const fn record_topology_load() {}

#[cfg(test)]
fn record_compiler_build() {
    ACCEPTED_BUILD_WORK.with(|recorded| {
        let mut work = recorded.get();
        work.compiler_builds = work
            .compiler_builds
            .checked_add(1)
            .expect("test compiler-build counter remains bounded");
        recorded.set(work);
    });
}

#[cfg(not(test))]
const fn record_compiler_build() {}

pub(crate) struct RegisteredProfileCandidate {
    candidate: AcceptedProfileCandidate,
    characters: CharacterCatalog,
    topology: LoadedProfileTopology,
}

/// One live editor overlay supplied to a complete profile build.
///
/// The loader seed owns the bytes/path used by topology construction, while
/// the protocol URI/version are rebound to the resulting accepted project
/// before the publication candidate is constructed.
#[derive(Clone, Debug)]
pub(crate) struct ProfileRegistrationOverlay {
    seed: ProfileTopologyOverlaySeed,
    uri: LspUriKey,
    version: i32,
}

#[derive(Debug, Error)]
pub(crate) enum RegisterProfileEnvironmentError {
    #[error("failed to load exact launch-profile topology: {0}")]
    Topology(#[source] Box<ProfileTopologyLoadError>),
    #[error("failed to assemble registration project: {0}")]
    ProjectAssembly(String),
    #[error("failed to load registration facts: {0}")]
    RegistrationLoad(#[source] arcweft_project_loader::environment::ProjectRegistrationLoadError),
    #[error("project compilation was rejected: {details}")]
    Compile {
        details: String,
        #[source]
        source: Box<ProjectCompileError>,
    },
    #[error("registered character catalog was rejected: {0}")]
    Catalog(String),
    #[error("accepted profile candidate was rejected: {0}")]
    Candidate(#[source] Box<super::state::AcceptedProfileCandidateError>),
    #[error("accepted profile overlays were rejected: {0}")]
    Overlay(#[source] AcceptedOverlaySetError),
    #[error("accepted project snapshot was rejected: {0}")]
    AcceptedProject(#[source] Box<super::accepted_project::AcceptedProjectSnapshotError>),
}

impl RegisteredProfileCandidate {
    pub(crate) const fn candidate(&self) -> &AcceptedProfileCandidate {
        &self.candidate
    }

    pub(crate) const fn metadata(&self) -> (&CharacterCatalog, &LoadedProfileTopology) {
        (&self.characters, &self.topology)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        AcceptedProfileCandidate,
        CharacterCatalog,
        LoadedProfileTopology,
    ) {
        (self.candidate, self.characters, self.topology)
    }
}

impl ProfileRegistrationOverlay {
    pub(crate) fn new(seed: ProfileTopologyOverlaySeed, uri: LspUriKey, version: i32) -> Self {
        Self { seed, uri, version }
    }

    const fn seed(&self) -> &ProfileTopologyOverlaySeed {
        &self.seed
    }

    const fn uri(&self) -> &LspUriKey {
        &self.uri
    }

    const fn version(&self) -> i32 {
        self.version
    }
}

pub(crate) fn register_profile_environment_with_overlays(
    manifest_path: &Path,
    profile_id: &ProfileId,
    overlays: &[ProfileRegistrationOverlay],
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    register_profile_environment(
        manifest_path,
        LaunchProfileSelection::Explicit(profile_id.as_str()),
        overlays,
        previous,
    )
}

pub(crate) fn register_profile_environment(
    manifest_path: &Path,
    selection: LaunchProfileSelection<'_>,
    overlays: &[ProfileRegistrationOverlay],
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    record_topology_load();
    let workspace_owner = workspace_owner(manifest_path)?;
    let overlay_seeds = overlays
        .iter()
        .map(ProfileRegistrationOverlay::seed)
        .cloned()
        .collect::<Vec<_>>();
    let topology = load_profile_topology(ProfileTopologyLoadRequest::new(
        manifest_path,
        workspace_owner,
        selection,
        &overlay_seeds,
        arcweft_adapter_context::standard::standard_registry(),
    ))
    .map_err(|error| RegisterProfileEnvironmentError::Topology(Box::new(error)))?;
    let (candidate, characters) = register_loaded_environment(&topology, overlays, previous)?;
    Ok(RegisteredProfileCandidate {
        candidate,
        characters,
        topology,
    })
}

pub(crate) fn register_loaded_environment(
    topology: &LoadedProfileTopology,
    overlays: &[ProfileRegistrationOverlay],
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<(AcceptedProfileCandidate, CharacterCatalog), RegisterProfileEnvironmentError> {
    let registration = load_profile_registration(&ProfileRegistrationLoadRequest::new(topology))
        .map_err(RegisterProfileEnvironmentError::RegistrationLoad)?;
    let (facts, file_documents) = registration.into_parts();
    let facts = Arc::new(facts);
    let base = AdapterSemanticRegistration::new(topology.adapter())
        .declare_effects(TypeCheckEnv::standard());
    let characters = registered_character_catalog(&facts)?;
    let source_seeds = accepted_source_seeds(&facts, file_documents);
    let resource_types = Arc::new(ResourceTypeRegistry::empty());
    let accepted_launch = AcceptedLaunchProfileInput::new(
        Arc::clone(topology.manifest()),
        topology.selected_profile().id().clone(),
        topology.selected_profile().clone(),
        topology.source_documents_revision(),
        Arc::clone(&resource_types),
    );
    let context = ProjectCompilationContext::new(
        Arc::new(base),
        Arc::clone(&facts),
        resource_types,
        previous.cloned().map(Arc::new),
        None,
    )
    .with_accepted_launch_profile(accepted_launch);
    record_compiler_build();
    let compiled = Arc::new(
        compile_project(
            topology.loaded_project().sources(),
            &context,
            &RuntimePlanLowerOptions::default()
                .with_package_identity(topology.loaded_project().sources().package().id.as_str())
                .with_command_policy(RuntimeCommandPolicy::deny_all(
                    RootExecutionLimits::engine_default(),
                )),
        )
        .map_err(|error| {
            let details = error
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.diagnostic().message())
                .collect::<Vec<_>>()
                .join("; ");
            RegisterProfileEnvironmentError::Compile {
                details,
                source: Box::new(error),
            }
        })?,
    );
    let world = compiled.registered_world_arc();
    let project = Arc::new(
        AcceptedProjectSnapshot::try_new(
            Arc::clone(compiled.hir_project()),
            world.as_ref(),
            source_seeds,
        )
        .map_err(|error| RegisterProfileEnvironmentError::AcceptedProject(Box::new(error)))?,
    );
    let mut overlay_entries = Vec::with_capacity(overlays.len());
    for overlay in overlays {
        let Some(identity) = project.source_identity_by_uri(overlay.uri()) else {
            return Err(RegisterProfileEnvironmentError::Candidate(Box::new(
                AcceptedProfileCandidateError::UnknownOverlayUri {
                    uri: overlay.uri().clone(),
                },
            )));
        };
        overlay_entries.push((
            overlay.uri().clone(),
            AcceptedOverlayEntry::new(overlay.version(), identity.clone()),
        ));
    }
    let overlays = AcceptedOverlaySet::try_new(overlay_entries)
        .map_err(RegisterProfileEnvironmentError::Overlay)?;
    let candidate = AcceptedProfileCandidate::try_new(
        accepted_profile_key(topology)?,
        compiled,
        project,
        overlays,
    )
    .map_err(|error| RegisterProfileEnvironmentError::Candidate(Box::new(error)))?;
    Ok((candidate, characters))
}

fn registered_character_catalog(
    facts: &arcweft_lang_sema::registration::ProjectRegistrationFacts,
) -> Result<CharacterCatalog, RegisterProfileEnvironmentError> {
    CharacterCatalog::try_from_manifests(facts.catalogs().flat_map(|catalog| {
        catalog
            .manifests()
            .map(|manifest| manifest.manifest().clone())
    }))
    .map_err(|error| RegisterProfileEnvironmentError::Catalog(error.to_string()))
}

fn accepted_source_seeds(
    facts: &arcweft_lang_sema::registration::ProjectRegistrationFacts,
    file_documents: Vec<arcweft_project_loader::environment::LoadedFileDocument>,
) -> Vec<AcceptedSourceDocumentSeed> {
    let mut source_seeds = file_documents
        .into_iter()
        .map(|file| {
            let locator =
                file_uri_from_path(file.path()).map_or(AcceptedSourceLocator::Unavailable, |uri| {
                    AcceptedSourceLocator::File {
                        path: file.path().to_path_buf(),
                        uri,
                    }
                });
            AcceptedSourceDocumentSeed::new(
                Arc::clone(file.document()),
                locator,
                match file.ownership() {
                    arcweft_project_loader::topology::LoadedDocumentOwnership::Workspace => {
                        AcceptedSourceOwnership::Workspace
                    }
                    arcweft_project_loader::topology::LoadedDocumentOwnership::Dependency => {
                        AcceptedSourceOwnership::Dependency
                    }
                },
                match file.access() {
                    arcweft_project_loader::topology::LoadedDocumentAccess::Writable => {
                        AcceptedSourceAccess::Writable
                    }
                    arcweft_project_loader::topology::LoadedDocumentAccess::ReadOnly => {
                        AcceptedSourceAccess::ReadOnly
                    }
                    arcweft_project_loader::topology::LoadedDocumentAccess::Unknown => {
                        AcceptedSourceAccess::Unknown
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    let mut seeded = source_seeds
        .iter()
        .map(|seed| seed.document().identity().clone())
        .collect::<BTreeSet<_>>();
    source_seeds.extend(
        facts
            .documents()
            .filter(|document| seeded.insert(document.identity().clone()))
            .map(|document| {
                let locator = document
                    .identity()
                    .id()
                    .as_str()
                    .parse()
                    .map_or(AcceptedSourceLocator::Unavailable, |uri| {
                        AcceptedSourceLocator::Uri { uri }
                    });
                AcceptedSourceDocumentSeed::new(
                    Arc::clone(document),
                    locator,
                    AcceptedSourceOwnership::Generated,
                    AcceptedSourceAccess::Unknown,
                )
            }),
    );
    source_seeds
}

fn accepted_profile_key(
    topology: &LoadedProfileTopology,
) -> Result<AcceptedProfileKey, RegisterProfileEnvironmentError> {
    let loaded = topology.loaded_project();
    let workspace_uri = file_uri_from_path(loaded.sources().project_root()).ok_or_else(|| {
        RegisterProfileEnvironmentError::ProjectAssembly(
            "project root cannot be represented as an LSP file URI".to_owned(),
        )
    })?;
    let manifest_uri = file_uri_from_path(loaded.sources().manifest_path()).ok_or_else(|| {
        RegisterProfileEnvironmentError::ProjectAssembly(
            "project manifest cannot be represented as an LSP file URI".to_owned(),
        )
    })?;
    let profile_id = topology.selected_profile().id().clone();
    Ok(AcceptedProfileKey::new(
        &workspace_uri,
        &manifest_uri,
        profile_id,
    ))
}

fn workspace_owner(
    manifest_path: &Path,
) -> Result<ProfileTopologyOwnerId, RegisterProfileEnvironmentError> {
    let project_root = manifest_path.parent().ok_or_else(|| {
        RegisterProfileEnvironmentError::ProjectAssembly(
            "project manifest has no workspace parent".to_owned(),
        )
    })?;
    let workspace_uri = file_uri_from_path(project_root).ok_or_else(|| {
        RegisterProfileEnvironmentError::ProjectAssembly(
            "project root cannot be represented as an LSP file URI".to_owned(),
        )
    })?;
    let manifest_uri = file_uri_from_path(manifest_path).ok_or_else(|| {
        RegisterProfileEnvironmentError::ProjectAssembly(
            "project manifest cannot be represented as an LSP file URI".to_owned(),
        )
    })?;
    ProfileTopologyOwnerId::workspace(
        workspace_uri.as_str().to_owned(),
        manifest_uri.as_str().to_owned(),
    )
    .map_err(|error| RegisterProfileEnvironmentError::ProjectAssembly(error.to_string()))
}
