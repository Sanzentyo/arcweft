//! Filesystem construction of complete source-backed registration facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use arcweft_adapter_context::manifest::{AdapterManifest, AdapterRegistrationFactsError};
use arcweft_character::{
    manifest::registration::{
        CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
    },
    registration_catalog::{SourceBackedCharacterCatalog, SourceBackedCharacterCatalogError},
};
use arcweft_lang_hir::symbol::{
    CallablePackageId, CallablePackageIdError, ExternalDeclarationSeed,
    ExternalDeclarationSeedError, ProjectDirectBinding, ProjectSymbolWorldId,
    ProjectSymbolWorldIdError,
};
use arcweft_lang_sema::registration::{
    CharacterRegistrationReport, ExternalRegistrationFact, ProjectRegistrationFacts,
    RegisteredExternalOwner,
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{SymbolPath, SymbolPathError},
};
use arcweft_launch::ResolvedLaunchProfile;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceDocumentIdentity};
use thiserror::Error;

use crate::{character_manifest, project::LoadedProject};

/// Registration facts and the explicit file records that supplied them.
#[derive(Clone, Debug)]
pub struct LoadedProjectRegistration {
    facts: ProjectRegistrationFacts,
    file_documents: Vec<LoadedFileDocument>,
}

/// One exact source document paired with the actual file that was read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedFileDocument {
    document: Arc<SourceDocument>,
    path: PathBuf,
    ownership: LoadedDocumentOwnership,
    access: LoadedDocumentAccess,
}

/// Project ownership of one file-backed registration source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoadedDocumentOwnership {
    Workspace,
    Dependency,
}

/// Mutability observed for one file-backed registration source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoadedDocumentAccess {
    Writable,
    ReadOnly,
    Unknown,
}

/// Complete loader inputs for one semantic project world.
pub struct ProjectLoadRequest<'a> {
    loaded: &'a LoadedProject,
    profile: Option<&'a ResolvedLaunchProfile>,
    additional_documents: Vec<Arc<SourceDocument>>,
    external_facts: Vec<ExternalRegistrationFact>,
    adapter_manifests: Vec<AdapterManifest>,
}

/// Failure while loading or checking a complete registration-fact set.
#[derive(Debug, Error)]
pub enum ProjectRegistrationLoadError {
    #[error("failed to load character manifest `{path}`: {source}")]
    CharacterManifest {
        path: std::path::PathBuf,
        #[source]
        source: Box<character_manifest::LoadError>,
    },
    #[error(transparent)]
    Package(#[from] CallablePackageIdError),
    #[error(transparent)]
    World(#[from] ProjectSymbolWorldIdError),
    #[error(transparent)]
    SymbolPath(#[from] SymbolPathError),
    #[error(transparent)]
    ExternalDeclaration(#[from] ExternalDeclarationSeedError),
    #[error(transparent)]
    Catalog(#[from] SourceBackedCharacterCatalogError),
    #[error(transparent)]
    AdapterFacts(#[from] AdapterRegistrationFactsError),
    #[error("adapter registration-fact ordinal exceeds u64::MAX")]
    AdapterOrdinalOverflow,
    #[error("loaded project does not contain the crate root module")]
    MissingRootModule,
    #[error("loaded character manifest `{path}` has no retained character declaration span")]
    MissingCharacterDeclaration { path: std::path::PathBuf },
    #[error("overlay document id occurs with conflicting exact identities")]
    ConflictingOverlay {
        id: SourceDocumentId,
        first: Box<SourceDocumentIdentity>,
        conflicting: Box<SourceDocumentIdentity>,
    },
    #[error("project registration facts were rejected")]
    Registration(Box<CharacterRegistrationReport>),
}

impl<'a> ProjectLoadRequest<'a> {
    pub fn new(
        loaded: &'a LoadedProject,
        profile: Option<&'a ResolvedLaunchProfile>,
        additional_documents: Vec<Arc<SourceDocument>>,
        external_facts: Vec<ExternalRegistrationFact>,
    ) -> Self {
        Self {
            loaded,
            profile,
            additional_documents,
            external_facts,
            adapter_manifests: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_adapter_manifests(
        mut self,
        manifests: impl IntoIterator<Item = AdapterManifest>,
    ) -> Self {
        self.adapter_manifests.extend(manifests);
        self.adapter_manifests
            .sort_by(|left, right| left.id().cmp(right.id()));
        self
    }
}

impl LoadedProjectRegistration {
    pub const fn facts(&self) -> &ProjectRegistrationFacts {
        &self.facts
    }

    pub fn file_documents(&self) -> impl ExactSizeIterator<Item = &LoadedFileDocument> {
        self.file_documents.iter()
    }

    pub fn into_parts(self) -> (ProjectRegistrationFacts, Vec<LoadedFileDocument>) {
        (self.facts, self.file_documents)
    }
}

impl LoadedFileDocument {
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn ownership(&self) -> LoadedDocumentOwnership {
        self.ownership
    }

    pub const fn access(&self) -> LoadedDocumentAccess {
        self.access
    }
}

/// Loads character documents and returns the only semantic registration input shape.
#[allow(
    clippy::result_large_err,
    reason = "loader failures retain the typed path-specific manifest error as their source"
)]
pub fn load_project_registration(
    request: &ProjectLoadRequest<'_>,
) -> Result<LoadedProjectRegistration, ProjectRegistrationLoadError> {
    let overlays = collect_overlays(&request.additional_documents)?;
    let disk_root_document = request
        .loaded
        .module_document(&CanonicalModulePath::crate_root())
        .ok_or(ProjectRegistrationLoadError::MissingRootModule)?;
    let root_document = overlay_document(disk_root_document, &overlays);
    let package_name = request
        .loaded
        .sources()
        .manifest()
        .package()
        .name()
        .as_str();
    let package = CallablePackageId::try_new(package_name)?;
    let profile_id = request
        .profile
        .map_or("default", |profile| profile.id().as_str());
    let world =
        ProjectSymbolWorldId::try_new(package, root_document.identity().id().clone(), profile_id)?;

    let mut sources = project_registration_sources(request, &overlays);
    append_adapter_sources(&mut sources, &request.adapter_manifests)?;
    if let Some(profile) = request.profile {
        append_character_sources(
            &mut sources,
            profile,
            package_name,
            request.loaded.sources().project_root(),
            &overlays,
        )?;
    }

    let catalogs = if sources.character_manifests.is_empty() {
        Vec::new()
    } else {
        vec![SourceBackedCharacterCatalog::try_new(
            request.loaded.manifest_document().identity().clone(),
            sources.character_manifests,
        )?]
    };
    let facts = ProjectRegistrationFacts::try_new(
        world,
        sources.documents,
        sources.external_facts,
        catalogs,
    )
    .map_err(|report| ProjectRegistrationLoadError::Registration(Box::new(report)))?;
    sources.file_documents.sort_by(|left, right| {
        left.document
            .identity()
            .cmp(right.document.identity())
            .then_with(|| left.path.cmp(&right.path))
    });
    sources.file_documents.dedup();
    Ok(LoadedProjectRegistration {
        facts,
        file_documents: sources.file_documents,
    })
}

type OverlayDocuments = BTreeMap<SourceDocumentId, Arc<SourceDocument>>;

struct RegistrationSources {
    documents: Vec<Arc<SourceDocument>>,
    file_documents: Vec<LoadedFileDocument>,
    external_facts: Vec<ExternalRegistrationFact>,
    character_manifests: Vec<SourceBackedCharacterManifest>,
}

struct CharacterRegistrationSource {
    document: Arc<SourceDocument>,
    file_document: LoadedFileDocument,
    external_fact: ExternalRegistrationFact,
    manifest: SourceBackedCharacterManifest,
}

fn collect_overlays(
    documents: &[Arc<SourceDocument>],
) -> Result<OverlayDocuments, ProjectRegistrationLoadError> {
    let mut overlays = OverlayDocuments::new();
    for overlay in documents {
        if let Some(first) = overlays.get(overlay.identity().id())
            && first.identity() != overlay.identity()
        {
            return Err(ProjectRegistrationLoadError::ConflictingOverlay {
                id: overlay.identity().id().clone(),
                first: Box::new(first.identity().clone()),
                conflicting: Box::new(overlay.identity().clone()),
            });
        }
        overlays.insert(overlay.identity().id().clone(), Arc::clone(overlay));
    }
    Ok(overlays)
}

fn project_registration_sources(
    request: &ProjectLoadRequest<'_>,
    overlays: &OverlayDocuments,
) -> RegistrationSources {
    let mut documents = request
        .loaded
        .module_documents()
        .map(|(_, document)| overlay_document(document, overlays))
        .collect::<Vec<_>>();
    documents.push(overlay_document(
        request.loaded.manifest_document(),
        overlays,
    ));
    let mut admitted_ids = documents
        .iter()
        .map(|document| document.identity().id().clone())
        .collect::<BTreeSet<_>>();
    documents.extend(
        request
            .additional_documents
            .iter()
            .filter(|document| admitted_ids.insert(document.identity().id().clone()))
            .cloned(),
    );

    let mut file_documents = request
        .loaded
        .sources()
        .modules()
        .map(|source| {
            loaded_file_document(
                overlay_document(source.document(), overlays),
                source.path().to_path_buf(),
                LoadedDocumentOwnership::Workspace,
            )
        })
        .collect::<Vec<_>>();
    file_documents.push(loaded_file_document(
        overlay_document(request.loaded.manifest_document(), overlays),
        request.loaded.sources().manifest_path().to_path_buf(),
        LoadedDocumentOwnership::Workspace,
    ));

    RegistrationSources {
        documents,
        file_documents,
        external_facts: request.external_facts.clone(),
        character_manifests: Vec::new(),
    }
}

fn append_adapter_sources(
    sources: &mut RegistrationSources,
    manifests: &[AdapterManifest],
) -> Result<(), ProjectRegistrationLoadError> {
    for (index, manifest) in manifests.iter().enumerate() {
        let ordinal = u64::try_from(index)
            .map_err(|_| ProjectRegistrationLoadError::AdapterOrdinalOverflow)?;
        let facts = manifest.source_backed_registration_facts(ordinal)?;
        let (document, facts) = facts.into_parts();
        sources.documents.push(document);
        sources.external_facts.extend(facts);
    }
    Ok(())
}

fn append_character_sources(
    sources: &mut RegistrationSources,
    profile: &ResolvedLaunchProfile,
    package_name: &str,
    project_root: &Path,
    overlays: &OverlayDocuments,
) -> Result<(), ProjectRegistrationLoadError> {
    for path in profile.character_manifests() {
        let source = load_character_source(path, package_name, project_root, overlays)?;
        sources.documents.push(source.document);
        sources.file_documents.push(source.file_document);
        sources.external_facts.push(source.external_fact);
        sources.character_manifests.push(source.manifest);
    }
    Ok(())
}

fn load_character_source(
    requested_path: &Path,
    package_name: &str,
    project_root: &Path,
    overlays: &OverlayDocuments,
) -> Result<CharacterRegistrationSource, ProjectRegistrationLoadError> {
    let loaded = character_manifest::load_for_project(requested_path, package_name, project_root)
        .map_err(|source| ProjectRegistrationLoadError::CharacterManifest {
        path: requested_path.to_path_buf(),
        source: Box::new(source),
    })?;
    let (mut document, path, mut manifest) = loaded.into_parts();
    if let Some(overlay) = overlays.get(document.identity().id()) {
        document = Arc::clone(overlay);
        manifest = SourceBackedCharacterManifest::decode_registration_json(&document).map_err(
            |source| ProjectRegistrationLoadError::CharacterManifest {
                path: path.clone(),
                source: Box::new(character_manifest::LoadError::Parse(source)),
            },
        )?;
    }
    let owner = manifest.manifest().character().clone();
    let declaration = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .ok_or_else(
            || ProjectRegistrationLoadError::MissingCharacterDeclaration { path: path.clone() },
        )?
        .value()
        .clone();
    let canonical_path =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())?;
    let direct_bindings = [owner.as_str(), owner.compact_str()]
        .into_iter()
        .map(|name| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                name,
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let seed = ExternalDeclarationSeed::try_new(
        canonical_path,
        Some(Visibility::Public),
        declaration.clone(),
        direct_bindings,
    )?;
    let external_fact =
        ExternalRegistrationFact::new(seed, RegisteredExternalOwner::Character(owner), declaration);
    let file_document = loaded_file_document(
        Arc::clone(&document),
        path,
        LoadedDocumentOwnership::Workspace,
    );
    Ok(CharacterRegistrationSource {
        document,
        file_document,
        external_fact,
        manifest,
    })
}

fn overlay_document(
    disk: &Arc<SourceDocument>,
    overlays: &OverlayDocuments,
) -> Arc<SourceDocument> {
    overlays
        .get(disk.identity().id())
        .map_or_else(|| Arc::clone(disk), Arc::clone)
}

fn loaded_file_document(
    document: Arc<SourceDocument>,
    path: PathBuf,
    ownership: LoadedDocumentOwnership,
) -> LoadedFileDocument {
    let access = fs::metadata(&path).map_or(LoadedDocumentAccess::Unknown, |metadata| {
        if metadata.permissions().readonly() {
            LoadedDocumentAccess::ReadOnly
        } else {
            LoadedDocumentAccess::Writable
        }
    });
    LoadedFileDocument {
        document,
        path,
        ownership,
        access,
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectLoadRequest, load_project_registration};
    use crate::project;
    use std::{fs, path::PathBuf};

    #[test]
    fn registration_loader_returns_source_backed_manifest() {
        let fixture = TestProject::new("registration-loader-source-backed");
        fixture.write(
            "arcw.toml",
            r#"
[package]
name = "registration-loader-source-backed"
version = "0.1.0"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
character_manifests = ["characters/zundamon.awchar"]
"#,
        );
        fixture.write("src/main.arcw", "fn main() -> Unit { () }\n");
        fixture.write(
            "characters/zundamon.awchar/character.awchar.json",
            include_str!(
                "../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
            ),
        );

        let loaded = project::load(&fixture.path("arcw.toml")).expect("project loads");
        let profile = loaded
            .launch()
            .manifest()
            .resolve_profile("dev", fixture.root())
            .expect("profile resolves");
        let registration = load_project_registration(&ProjectLoadRequest::new(
            &loaded,
            Some(&profile),
            Vec::new(),
            Vec::new(),
        ))
        .expect("registration facts");
        let facts = registration.facts();

        assert!(facts.documents().any(|document| {
            document.identity().id().as_str()
                == "arcweft-project://registration-loader-source-backed/characters/zundamon.awchar/character.awchar.json"
        }));
        let catalog = facts.catalogs().next().expect("character catalog");
        let manifest = catalog.manifests().next().expect("source-backed manifest");
        assert_eq!(
            manifest.manifest().character().as_str(),
            "character.zundamon"
        );
        assert_eq!(
            manifest.source_map().document().id().as_str(),
            "arcweft-project://registration-loader-source-backed/characters/zundamon.awchar/character.awchar.json"
        );
        assert_eq!(facts.external_declarations().declarations().len(), 1);
        let manifest_file = registration
            .file_documents()
            .find(|file| file.document().identity() == manifest.source_map().document())
            .expect("manifest file ownership");
        assert_eq!(
            manifest_file.path(),
            fixture.path("characters/zundamon.awchar/character.awchar.json")
        );
    }

    struct TestProject {
        root: PathBuf,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let unique = format!(
                "arcweft-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock follows epoch")
                    .as_nanos()
            );
            let root = std::env::temp_dir().join(unique);
            fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        fn root(&self) -> &std::path::Path {
            &self.root
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture directory");
            }
            fs::write(path, contents).expect("fixture file");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
