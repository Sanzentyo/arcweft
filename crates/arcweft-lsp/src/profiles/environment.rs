use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_character::catalog::CharacterCatalog;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, RegisteredTypeCheckEnv},
};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_launch::ResolvedLaunchProfile;
use std::{collections::BTreeSet, path::Path, sync::Arc};
use thiserror::Error;

use super::{
    cache::{
        AcceptedOverlaySet, AcceptedProfileCandidate, AcceptedProfileKey, AcceptedSourceAccess,
        AcceptedSourceDocumentSeed, AcceptedSourceLocator, AcceptedSourceOwnership,
    },
    uri::file_uri_from_path,
};

pub(crate) struct RegisteredProfileCandidate {
    candidate: AcceptedProfileCandidate,
    characters: CharacterCatalog,
}

pub(crate) struct LoadedEnvironmentRequest<'a> {
    pub(crate) loaded: &'a arcweft_project_loader::project::LoadedProject,
    pub(crate) project: &'a HirProject,
    pub(crate) profile: Option<&'a ResolvedLaunchProfile>,
    pub(crate) adapter_manifests: &'a [AdapterManifest],
    pub(crate) base: TypeCheckEnv,
    pub(crate) additional_documents: Vec<Arc<arcweft_source::SourceDocument>>,
    pub(crate) overlays: AcceptedOverlaySet,
    pub(crate) previous: Option<&'a RegisteredTypeCheckEnv>,
}

#[derive(Debug, Error)]
pub(crate) enum RegisterProfileEnvironmentError {
    #[error("failed to load registration project: {0}")]
    Project(String),
    #[error("failed to assemble registration project: {0}")]
    ProjectAssembly(String),
    #[error("failed to load registration facts: {0}")]
    RegistrationLoad(#[source] arcweft_project_loader::environment::ProjectRegistrationLoadError),
    #[error("project registration was rejected: {0}")]
    Registration(String),
    #[error("registered character catalog was rejected: {0}")]
    Catalog(String),
    #[error("accepted profile candidate was rejected: {0}")]
    Candidate(#[source] super::cache::AcceptedProfileCandidateError),
}

impl RegisterProfileEnvironmentError {
    pub(crate) const fn registration_load(
        &self,
    ) -> Option<&arcweft_project_loader::environment::ProjectRegistrationLoadError> {
        match self {
            Self::RegistrationLoad(error) => Some(error),
            Self::Project(_)
            | Self::ProjectAssembly(_)
            | Self::Registration(_)
            | Self::Catalog(_)
            | Self::Candidate(_) => None,
        }
    }
}

impl RegisteredProfileCandidate {
    pub(crate) fn into_parts(self) -> (AcceptedProfileCandidate, CharacterCatalog) {
        (self.candidate, self.characters)
    }
}

pub(super) fn register_profile_environment(
    manifest_path: &Path,
    profile: &ResolvedLaunchProfile,
    adapter: &AdapterManifest,
    base: TypeCheckEnv,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    register_profile_environment_with_overlays(
        manifest_path,
        profile,
        adapter,
        base,
        Vec::new(),
        AcceptedOverlaySet::default(),
        previous,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "one rebuild transaction keeps its resolved profile, exact overlay documents, publication overlay set, and previous environment explicit"
)]
pub(crate) fn register_profile_environment_with_overlays(
    manifest_path: &Path,
    profile: &ResolvedLaunchProfile,
    adapter: &AdapterManifest,
    base: TypeCheckEnv,
    overlay_documents: Vec<Arc<arcweft_source::SourceDocument>>,
    overlays: AcceptedOverlaySet,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    let loaded = arcweft_project_loader::project::load(manifest_path)
        .map_err(|error| RegisterProfileEnvironmentError::Project(error.to_string()))?;
    let overlay_documents_by_id = overlay_documents
        .iter()
        .map(|document| (document.identity().id().clone(), Arc::clone(document)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let modules = loaded
        .sources()
        .modules()
        .map(|source| {
            let disk_document = loaded
                .module_document(source.module())
                .expect("loaded project retains one document per source module");
            let document = overlay_documents_by_id
                .get(disk_document.identity().id())
                .unwrap_or(disk_document);
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
            Ok(HirProjectModule::new(
                source.module().clone(),
                document.identity().clone(),
                hir,
            ))
        })
        .collect::<Result<Vec<_>, RegisterProfileEnvironmentError>>()?;
    let project = HirProject::new(
        loaded.sources().manifest().package().name().as_str(),
        modules,
    )
    .map_err(|error| RegisterProfileEnvironmentError::ProjectAssembly(error.to_string()))?;
    register_loaded_environment(LoadedEnvironmentRequest {
        loaded: &loaded,
        project: &project,
        profile: Some(profile),
        adapter_manifests: std::slice::from_ref(adapter),
        base,
        additional_documents: overlay_documents,
        overlays,
        previous,
    })
}

pub(crate) fn register_loaded_environment(
    request: LoadedEnvironmentRequest<'_>,
) -> Result<RegisteredProfileCandidate, RegisterProfileEnvironmentError> {
    let LoadedEnvironmentRequest {
        loaded,
        project,
        profile,
        adapter_manifests,
        base,
        additional_documents,
        overlays,
        previous,
    } = request;
    let additional_source_seeds = unavailable_source_seeds(&additional_documents);
    let request = arcweft_project_loader::environment::ProjectLoadRequest::new(
        loaded,
        profile,
        additional_documents,
        Vec::new(),
    )
    .with_adapter_manifests(adapter_manifests.iter().cloned());
    let registration = arcweft_project_loader::environment::load_project_registration(&request)
        .map_err(RegisterProfileEnvironmentError::RegistrationLoad)?;
    let (facts, file_documents) = registration.into_parts();
    let world = register_semantic_world(base, project, &facts, previous)?;
    let characters = registered_character_catalog(&facts)?;
    let source_seeds = accepted_source_seeds(&facts, file_documents, additional_source_seeds);
    let candidate = AcceptedProfileCandidate::try_new(
        accepted_profile_key(loaded, profile),
        world,
        source_seeds,
        overlays,
    )
    .map_err(RegisterProfileEnvironmentError::Candidate)?;
    Ok(RegisteredProfileCandidate {
        candidate,
        characters,
    })
}

fn unavailable_source_seeds(
    documents: &[Arc<arcweft_source::SourceDocument>],
) -> Vec<AcceptedSourceDocumentSeed> {
    documents
        .iter()
        .cloned()
        .map(|document| {
            AcceptedSourceDocumentSeed::new(
                document,
                AcceptedSourceLocator::Unavailable,
                AcceptedSourceOwnership::Generated,
                AcceptedSourceAccess::Unknown,
            )
        })
        .collect()
}

fn register_semantic_world(
    base: TypeCheckEnv,
    project: &HirProject,
    facts: &arcweft_lang_sema::registration::ProjectRegistrationFacts,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Result<
    Arc<arcweft_lang_sema::registration::RegisteredSemanticWorld>,
    RegisterProfileEnvironmentError,
> {
    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        project,
        facts,
        previous,
    ))
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
    additional_source_seeds: Vec<AcceptedSourceDocumentSeed>,
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
                    arcweft_project_loader::environment::LoadedDocumentOwnership::Workspace => {
                        AcceptedSourceOwnership::Workspace
                    }
                    arcweft_project_loader::environment::LoadedDocumentOwnership::Dependency => {
                        AcceptedSourceOwnership::Dependency
                    }
                },
                match file.access() {
                    arcweft_project_loader::environment::LoadedDocumentAccess::Writable => {
                        AcceptedSourceAccess::Writable
                    }
                    arcweft_project_loader::environment::LoadedDocumentAccess::ReadOnly => {
                        AcceptedSourceAccess::ReadOnly
                    }
                    arcweft_project_loader::environment::LoadedDocumentAccess::Unknown => {
                        AcceptedSourceAccess::Unknown
                    }
                },
            )
        })
        .collect::<Vec<_>>();
    source_seeds.extend(additional_source_seeds);
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
    loaded: &arcweft_project_loader::project::LoadedProject,
    profile: Option<&ResolvedLaunchProfile>,
) -> AcceptedProfileKey {
    let workspace_uri = file_uri_from_path(loaded.sources().project_root()).map_or_else(
        || loaded.sources().project_root().display().to_string(),
        |uri| uri.to_string(),
    );
    let manifest_uri = file_uri_from_path(loaded.sources().manifest_path()).map_or_else(
        || loaded.sources().manifest_path().display().to_string(),
        |uri| uri.to_string(),
    );
    let profile_id = profile.map_or("default", |profile| profile.id().as_str());
    AcceptedProfileKey::new(workspace_uri, manifest_uri, profile_id)
}
