use arcweft_character::catalog::CharacterCatalog;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    callable::PRODUCTION_CALLABLE_LIMITS,
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, RegisteredTypeCheckEnv},
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_launch::{LaunchProfileSelection, ProfileId};
use arcweft_project_loader::{
    environment::{ProfileRegistrationLoadRequest, load_profile_registration},
    topology::{
        LoadedProfileTopology, ProfileTopologyLoadError, ProfileTopologyLoadRequest,
        ProfileTopologyOverlaySeed, ProfileTopologyOwnerId, load_profile_topology,
    },
};
use std::{collections::BTreeSet, path::Path, sync::Arc};
use thiserror::Error;

use super::{
    accepted_project::{
        AcceptedProjectSnapshot, AcceptedSourceAccess, AcceptedSourceDocumentSeed,
        AcceptedSourceLocator, AcceptedSourceOwnership,
    },
    state::{AcceptedOverlaySet, AcceptedProfileCandidate, AcceptedProfileKey},
    uri::file_uri_from_path,
};

pub(crate) struct RegisteredProfileCandidate {
    candidate: AcceptedProfileCandidate,
    characters: CharacterCatalog,
    topology: LoadedProfileTopology,
}

pub(crate) struct LoadedEnvironmentRequest<'a> {
    pub(crate) topology: &'a LoadedProfileTopology,
    pub(crate) project: &'a Arc<HirProject>,
    pub(crate) overlays: AcceptedOverlaySet,
    pub(crate) previous: Option<&'a RegisteredTypeCheckEnv>,
}

#[derive(Debug, Error)]
pub(crate) enum RegisterProfileEnvironmentError {
    #[error("failed to load exact launch-profile topology: {0}")]
    Topology(#[source] Box<ProfileTopologyLoadError>),
    #[error("failed to assemble registration project: {0}")]
    ProjectAssembly(String),
    #[error("failed to load registration facts: {0}")]
    RegistrationLoad(#[source] arcweft_project_loader::environment::ProjectRegistrationLoadError),
    #[error("project registration was rejected: {0}")]
    Registration(String),
    #[error("registered character catalog was rejected: {0}")]
    Catalog(String),
    #[error("accepted profile candidate was rejected: {0}")]
    Candidate(#[source] Box<super::state::AcceptedProfileCandidateError>),
    #[error("accepted project snapshot was rejected: {0}")]
    AcceptedProject(#[source] Box<super::accepted_project::AcceptedProjectSnapshotError>),
}

impl RegisteredProfileCandidate {
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

pub(crate) fn register_profile_environment_with_overlays(
    manifest_path: &Path,
    profile_id: &ProfileId,
    overlay_seeds: &[ProfileTopologyOverlaySeed],
    overlays: AcceptedOverlaySet,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    register_profile_environment(
        manifest_path,
        LaunchProfileSelection::Explicit(profile_id.as_str()),
        overlay_seeds,
        overlays,
        previous,
    )
}

pub(crate) fn register_profile_environment(
    manifest_path: &Path,
    selection: LaunchProfileSelection<'_>,
    overlay_seeds: &[ProfileTopologyOverlaySeed],
    overlays: AcceptedOverlaySet,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    let workspace_owner = workspace_owner(manifest_path)?;
    let topology = load_profile_topology(ProfileTopologyLoadRequest::new(
        manifest_path,
        workspace_owner,
        selection,
        overlay_seeds,
        arcweft_adapter_context::standard::standard_registry(),
    ))
    .map_err(|error| RegisterProfileEnvironmentError::Topology(Box::new(error)))?;
    let modules = topology
        .loaded_project()
        .sources()
        .modules()
        .map(|source| {
            let document = topology
                .loaded_project()
                .module_document(source.module())
                .ok_or_else(|| {
                    RegisterProfileEnvironmentError::ProjectAssembly(format!(
                        "exact topology omitted module document `{}`",
                        source.module()
                    ))
                })?;
            let parsed = parse_source(document.text());
            if !parsed.errors().is_empty() {
                return Err(RegisterProfileEnvironmentError::ProjectAssembly(format!(
                    "source `{}` has syntax errors: {:?}",
                    source.path().display(),
                    parsed.errors()
                )));
            }
            let hir = lower_document_to_hir(document, parsed.typed_tree()).map_err(|errors| {
                RegisterProfileEnvironmentError::ProjectAssembly(format!(
                    "HIR lowering failed for `{}`: {errors:?}",
                    source.path().display()
                ))
            })?;
            HirProjectModule::try_new(source.module().clone(), document.identity().clone(), hir)
                .map_err(|error| {
                    RegisterProfileEnvironmentError::ProjectAssembly(error.to_string())
                })
        })
        .collect::<Result<Vec<_>, RegisterProfileEnvironmentError>>()?;
    let project = Arc::new(
        HirProject::new(
            topology
                .loaded_project()
                .sources()
                .manifest()
                .package()
                .name()
                .as_str(),
            modules,
        )
        .map_err(|error| RegisterProfileEnvironmentError::ProjectAssembly(error.to_string()))?,
    );
    let (candidate, characters) = register_loaded_environment(LoadedEnvironmentRequest {
        topology: &topology,
        project: &project,
        overlays,
        previous,
    })?;
    Ok(RegisteredProfileCandidate {
        candidate,
        characters,
        topology,
    })
}

pub(crate) fn register_loaded_environment(
    request: LoadedEnvironmentRequest<'_>,
) -> Result<(AcceptedProfileCandidate, CharacterCatalog), RegisterProfileEnvironmentError> {
    let LoadedEnvironmentRequest {
        topology,
        project,
        overlays,
        previous,
    } = request;
    let registration = load_profile_registration(&ProfileRegistrationLoadRequest::new(topology))
        .map_err(RegisterProfileEnvironmentError::RegistrationLoad)?;
    let (facts, file_documents) = registration.into_parts();
    let base = topology.adapter().apply_to_env(TypeCheckEnv::standard());
    let mut callable_publications =
        arcweft_adapter_context::standard::callable_publications(&PRODUCTION_CALLABLE_LIMITS)
            .map_err(|error| RegisterProfileEnvironmentError::Registration(error.to_string()))?;
    if arcweft_adapter_context::standard::manifest_source(topology.adapter().id().as_str())
        .is_none()
    {
        callable_publications.push(
            topology
                .adapter()
                .try_callable_publication(
                    arcweft_adapter_context::publication::AdapterManifestSource::SelectedAdapter,
                    &PRODUCTION_CALLABLE_LIMITS,
                )
                .map_err(|error| {
                    RegisterProfileEnvironmentError::Registration(error.to_string())
                })?,
        );
    }
    let world = register_semantic_world(
        base,
        project.as_ref(),
        &facts,
        previous,
        callable_publications,
    )?;
    let characters = registered_character_catalog(&facts)?;
    let source_seeds = accepted_source_seeds(&facts, file_documents);
    let project = Arc::new(
        AcceptedProjectSnapshot::try_new(Arc::clone(project), world.as_ref(), source_seeds)
            .map_err(|error| RegisterProfileEnvironmentError::AcceptedProject(Box::new(error)))?,
    );
    let candidate = AcceptedProfileCandidate::try_new(
        accepted_profile_key(topology)?,
        world,
        project,
        overlays,
    )
    .map_err(|error| RegisterProfileEnvironmentError::Candidate(Box::new(error)))?;
    Ok((candidate, characters))
}

fn register_semantic_world(
    base: TypeCheckEnv,
    project: &HirProject,
    facts: &arcweft_lang_sema::registration::ProjectRegistrationFacts,
    previous: Option<&RegisteredTypeCheckEnv>,
    callable_publications: Vec<arcweft_lang_sema::callable::EnvironmentCallablePublication>,
) -> Result<
    Arc<arcweft_lang_sema::registration::RegisteredSemanticWorld>,
    RegisterProfileEnvironmentError,
> {
    let request = callable_publications.into_iter().fold(
        CharacterRegistrationRequest::new(Arc::new(base), project, facts, previous),
        CharacterRegistrationRequest::with_callable_publication,
    );
    CharacterRegistrar::register(request)
        .map(Arc::new)
        .map_err(|report| {
            let details = report
                .diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.diagnostic().message().to_owned())
                .collect::<Vec<_>>()
                .join("; ");
            RegisterProfileEnvironmentError::Registration(details)
        })
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
                AcceptedSourceDocumentSeed::new(
                    Arc::clone(document),
                    AcceptedSourceLocator::Unavailable,
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
