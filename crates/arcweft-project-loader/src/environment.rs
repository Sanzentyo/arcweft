//! Filesystem construction of complete source-backed registration facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use arcweft_adapter_context::manifest::AdapterManifest;
use arcweft_adapter_sema::registration::{
    AdapterRegistrationFactsError, AdapterSemanticRegistration,
};
use arcweft_character::{
    manifest::registration::{
        CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
    },
    registration_catalog::{SourceBackedCharacterCatalog, SourceBackedCharacterCatalogError},
};
use arcweft_lang_hir::symbol::{
    CallablePackageId, CallablePackageIdError, ExternalDeclarationSeed,
    ExternalDeclarationSeedError, ProjectDirectBinding, ProjectDirectBindingError,
    ProjectSymbolWorldId, ProjectSymbolWorldIdError,
};
use arcweft_lang_sema::registration::{
    CharacterRegistrationReport, ExternalRegistrationFact, ProjectRegistrationFacts,
    RegisteredExternalOwner, SourceBackedEnvironmentRegistrationInput,
};
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{
        ProjectSymbolPath, ProjectSymbolPathError, ProjectSymbolSegment, SymbolPath,
        SymbolPathError,
    },
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceDocumentIdentity};
use thiserror::Error;

use crate::{
    project::LoadedProject,
    topology::{
        LoadedDocumentAccess, LoadedDocumentOwnership, LoadedProfileTopology,
        ProfileTopologyResourceKind,
    },
};

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

/// Complete loader inputs for one semantic project world.
pub struct ProjectLoadRequest<'a> {
    loaded: &'a LoadedProject,
    additional_documents: Vec<Arc<SourceDocument>>,
    external_facts: Vec<ExternalRegistrationFact>,
    adapter_manifests: Vec<AdapterManifest>,
}

/// Exact topology input for one profile registration transaction.
pub struct ProfileRegistrationLoadRequest<'a> {
    topology: &'a LoadedProfileTopology,
    additional_adapter_manifests: &'a [AdapterManifest],
}

/// Failure while loading or checking a complete registration-fact set.
#[derive(Debug, Error)]
pub enum ProjectRegistrationLoadError {
    #[error(transparent)]
    Package(#[from] CallablePackageIdError),
    #[error(transparent)]
    World(#[from] ProjectSymbolWorldIdError),
    #[error(transparent)]
    SymbolPath(#[from] SymbolPathError),
    #[error(transparent)]
    ProjectSymbolPath(#[from] ProjectSymbolPathError),
    #[error(transparent)]
    ProjectDirectBinding(#[from] ProjectDirectBindingError),
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
    #[error("loaded profile topology has no `{kind:?}` resource at `{path}`")]
    MissingTopologyResource {
        path: PathBuf,
        kind: ProfileTopologyResourceKind,
    },
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
        additional_documents: Vec<Arc<SourceDocument>>,
        external_facts: Vec<ExternalRegistrationFact>,
    ) -> Self {
        Self {
            loaded,
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

impl<'a> ProfileRegistrationLoadRequest<'a> {
    pub const fn new(topology: &'a LoadedProfileTopology) -> Self {
        Self {
            topology,
            additional_adapter_manifests: &[],
        }
    }

    #[must_use]
    pub const fn with_adapter_manifests(mut self, manifests: &'a [AdapterManifest]) -> Self {
        self.additional_adapter_manifests = manifests;
        self
    }
}

impl LoadedProjectRegistration {
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
    let package_name = request.loaded.sources().package().id.as_str();
    let package = CallablePackageId::try_new(package_name)?;
    let world =
        ProjectSymbolWorldId::try_new(package, root_document.identity().id().clone(), "default")?;

    let mut sources = project_registration_sources(request, &overlays);
    append_adapter_sources(&mut sources, request.adapter_manifests.iter())?;

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
        sources.environment_inputs,
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

/// Builds profile registration facts solely from one immutable loaded topology.
#[allow(
    clippy::result_large_err,
    reason = "loader failures retain typed resource and registration errors"
)]
pub fn load_profile_registration(
    request: &ProfileRegistrationLoadRequest<'_>,
) -> Result<LoadedProjectRegistration, ProjectRegistrationLoadError> {
    let topology = request.topology;
    let loaded = topology.loaded_project();
    let root_document = loaded
        .module_document(&CanonicalModulePath::crate_root())
        .ok_or(ProjectRegistrationLoadError::MissingRootModule)?;
    let package_name = loaded.sources().package().id.as_str();
    let package = CallablePackageId::try_new(package_name)?;
    let world = ProjectSymbolWorldId::try_new(
        package,
        root_document.identity().id().clone(),
        topology.selected_profile().id().as_str(),
    )?;

    let mut sources = RegistrationSources {
        documents: topology
            .resources()
            .filter_map(|resource| resource.text_document().map(Arc::clone))
            .collect(),
        file_documents: topology
            .resources()
            .filter_map(|resource| {
                Some(LoadedFileDocument {
                    document: Arc::clone(resource.text_document()?),
                    path: resource.path().to_path_buf(),
                    ownership: resource.ownership(),
                    access: resource.access(),
                })
            })
            .collect(),
        external_facts: Vec::new(),
        character_manifests: Vec::new(),
        environment_inputs: Vec::new(),
    };
    append_adapter_sources(
        &mut sources,
        topology
            .registration_adapter_manifests()
            .iter()
            .chain(request.additional_adapter_manifests),
    )?;
    append_topology_character_sources(&mut sources, topology)?;

    let catalogs = if sources.character_manifests.is_empty() {
        Vec::new()
    } else {
        vec![SourceBackedCharacterCatalog::try_new(
            loaded.manifest_document().identity().clone(),
            sources.character_manifests,
        )?]
    };
    let facts = ProjectRegistrationFacts::try_new(
        world,
        sources.documents,
        sources.external_facts,
        catalogs,
        sources.environment_inputs,
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
    environment_inputs: Vec<SourceBackedEnvironmentRegistrationInput>,
}

struct CharacterRegistrationSource {
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
        environment_inputs: Vec::new(),
    }
}

fn append_adapter_sources<'a>(
    sources: &mut RegistrationSources,
    manifests: impl IntoIterator<Item = &'a AdapterManifest>,
) -> Result<(), ProjectRegistrationLoadError> {
    for (index, manifest) in manifests.into_iter().enumerate() {
        let ordinal = u64::try_from(index)
            .map_err(|_| ProjectRegistrationLoadError::AdapterOrdinalOverflow)?;
        let parts = AdapterSemanticRegistration::new(manifest)
            .source_backed_facts(ordinal)?
            .into_parts();
        sources.documents.push(parts.document);
        sources.external_facts.extend(parts.externals);
        sources.environment_inputs.push(parts.environment);
    }
    Ok(())
}

fn append_topology_character_sources(
    sources: &mut RegistrationSources,
    topology: &LoadedProfileTopology,
) -> Result<(), ProjectRegistrationLoadError> {
    for (_, package) in topology.character_packages() {
        let source = character_registration_source(
            package.manifest_path(),
            package.source_manifest().as_ref().clone(),
        )?;
        sources.external_facts.push(source.external_fact);
        sources.character_manifests.push(source.manifest);
    }
    Ok(())
}

fn character_registration_source(
    path: &Path,
    manifest: SourceBackedCharacterManifest,
) -> Result<CharacterRegistrationSource, ProjectRegistrationLoadError> {
    let owner = manifest.manifest().character().clone();
    let declaration = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .ok_or_else(
            || ProjectRegistrationLoadError::MissingCharacterDeclaration {
                path: path.to_path_buf(),
            },
        )?
        .value()
        .clone();
    let canonical_path =
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())?;
    let compact_segments = owner
        .compact_segments()
        .map(|segment| ProjectSymbolSegment::try_new(segment.to_owned()))
        .collect::<Result<Vec<_>, ProjectSymbolPathError>>()?;
    let (qualified_path, compact_path) = character_publication_paths(compact_segments)?;
    let direct_bindings = vec![
        ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            qualified_path,
            Some(Visibility::Public),
            declaration.clone(),
            false,
        )?,
        ProjectDirectBinding::try_new(
            CanonicalModulePath::crate_root(),
            compact_path,
            Some(Visibility::Public),
            declaration.clone(),
            false,
        )?,
    ];
    let seed = ExternalDeclarationSeed::try_new(
        canonical_path,
        Some(Visibility::Public),
        declaration.clone(),
        direct_bindings,
    )?;
    let external_fact =
        ExternalRegistrationFact::new(seed, RegisteredExternalOwner::Character(owner), declaration);
    Ok(CharacterRegistrationSource {
        external_fact,
        manifest,
    })
}

fn character_publication_paths(
    compact_segments: impl IntoIterator<Item = ProjectSymbolSegment>,
) -> Result<(ProjectSymbolPath, ProjectSymbolPath), ProjectSymbolPathError> {
    let compact_segments = compact_segments.into_iter().collect::<Vec<_>>();
    let qualified_path = ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        std::iter::once(ProjectSymbolSegment::try_new("character")?)
            .chain(compact_segments.iter().cloned()),
    )?;
    ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact_segments)
        .map(|compact_path| (qualified_path, compact_path))
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
    use super::{
        ProfileRegistrationLoadRequest, character_publication_paths, load_profile_registration,
    };
    use crate::topology::{
        ProfileTopologyLoadRequest, ProfileTopologyOwnerId, load_profile_topology,
    };
    use arcweft_adapter_context::standard::standard_registry;
    use arcweft_lang_syntax::ast::symbol_path::{ProjectSymbolPathError, ProjectSymbolSegment};
    use arcweft_lang_syntax::incremental::SyntaxDatabase;
    use arcweft_launch::LaunchProfileSelection;
    use std::{fs, path::PathBuf};

    #[test]
    fn registration_loader_returns_source_backed_manifest() {
        let fixture = TestProject::new("registration-loader-source-backed");
        fixture.write(
            "arcw.toml",
            r#"
schema = 1

[package]
id = "org.arcweft.test.registration-loader-source-backed"
version = "0.1.0"

[content-units.characters]
roots = ["@character.zundamon"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#,
        );
        fixture.write("src/main.arcw", "fn main() -> Unit { () }\n");
        fixture.write(
            "assets/zundamon.awchar/character.awchar.json",
            include_str!(
                "../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
            ),
        );
        fixture.write_character_layers("assets/zundamon.awchar");

        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let topology = load_profile_topology(
            &mut syntax,
            ProfileTopologyLoadRequest::new(
                &fixture.path("arcw.toml"),
                ProfileTopologyOwnerId::workspace(
                    format!("file:///{}", slash(fixture.root())),
                    format!("file:///{}", slash(&fixture.path("arcw.toml"))),
                )
                .expect("workspace owner"),
                LaunchProfileSelection::Explicit("dev"),
                &[],
                standard_registry(),
            ),
        )
        .expect("topology loads");
        let registration =
            load_profile_registration(&ProfileRegistrationLoadRequest::new(&topology))
                .expect("registration facts");
        let (facts, file_documents) = registration.into_parts();

        assert!(facts.documents().any(|document| {
            document.identity().id().as_str()
                == "arcweft-project://org.arcweft.test.registration-loader-source-backed/assets/zundamon.awchar/character.awchar.json"
        }));
        let catalog = facts.catalogs().next().expect("character catalog");
        let manifest = catalog.manifests().next().expect("source-backed manifest");
        assert_eq!(
            manifest.manifest().character().as_str(),
            "character.zundamon"
        );
        assert_eq!(
            manifest.source_map().document().id().as_str(),
            "arcweft-project://org.arcweft.test.registration-loader-source-backed/assets/zundamon.awchar/character.awchar.json"
        );
        assert_eq!(facts.external_declarations().declarations().len(), 1);
        let (_, seed) = facts
            .external_declarations()
            .declarations()
            .next()
            .expect("character declaration");
        assert_eq!(seed.canonical_path().leaf(), "character.zundamon");
        assert_eq!(
            seed.direct_bindings()
                .iter()
                .map(|binding| {
                    binding
                        .path()
                        .segments()
                        .iter()
                        .map(ProjectSymbolSegment::as_str)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            [vec!["character", "zundamon"], vec!["zundamon"]]
        );
        let manifest_file = file_documents
            .iter()
            .find(|file| file.document().identity() == manifest.source_map().document())
            .expect("manifest file ownership");
        assert_eq!(
            manifest_file.path(),
            fs::canonicalize(fixture.path("assets/zundamon.awchar/character.awchar.json"))
                .expect("fixture manifest path canonicalizes")
        );
    }

    #[test]
    fn malformed_compact_character_path_fails_before_direct_binding_construction() {
        let error = character_publication_paths([
            ProjectSymbolSegment::try_new("2d").expect("valid external segment")
        ])
        .expect_err("numeric compact root must fail typed character publication");

        assert_eq!(
            error,
            ProjectSymbolPathError::InvalidImplicitRoot {
                segment: "2d".to_owned(),
            }
        );
    }

    #[test]
    fn profile_registration_uses_only_retained_topology_documents() {
        let fixture = TestProject::new("profile-registration-topology-only");
        fixture.write(
            "arcw.toml",
            r#"
schema = 1

[package]
id = "org.arcweft.test.profile-registration-topology-only"
version = "0.1.0"

[content-units.characters]
roots = ["@character.zundamon"]
visibility = "package"
demand = "required"

[profiles.dev]
kind = "game"
entry = "@entry.game.main"
source = "src/main.arcw"

[profiles.dev.content.characters]
residency = "startup"
placement = "embedded"
compression = "none"
"#,
        );
        fixture.write("src/main.arcw", "fn main() -> Unit { () }\n");
        fixture.write(
            "assets/zundamon.awchar/character.awchar.json",
            include_str!(
                "../../arcweft-character/tests/fixtures/zundamon.awchar/character.awchar.json"
            ),
        );
        fixture.write_character_layers("assets/zundamon.awchar");
        let manifest_path = fixture.path("arcw.toml");
        let owner = ProfileTopologyOwnerId::workspace(
            format!("file:///{}", slash(fixture.root())),
            format!("file:///{}", slash(&manifest_path)),
        )
        .expect("workspace owner");
        let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
        let topology = load_profile_topology(
            &mut syntax,
            ProfileTopologyLoadRequest::new(
                &manifest_path,
                owner,
                LaunchProfileSelection::Explicit("dev"),
                &[],
                standard_registry(),
            ),
        )
        .expect("topology loads");

        fs::remove_file(&manifest_path).expect("manifest removed");
        fs::remove_file(fixture.path("src/main.arcw")).expect("module removed");
        fs::remove_file(fixture.path("assets/zundamon.awchar/character.awchar.json"))
            .expect("character removed");

        let registration =
            load_profile_registration(&ProfileRegistrationLoadRequest::new(&topology))
                .expect("registration uses retained documents");
        let (facts, file_documents) = registration.into_parts();

        assert_eq!(file_documents.len(), 3);
        assert_eq!(facts.catalogs().count(), 1);
        assert!(file_documents.iter().all(|file| {
            topology.resources().any(|resource| {
                resource.path() == file.path()
                    && resource
                        .text_document()
                        .is_some_and(|document| document.identity() == file.document().identity())
            })
        }));
    }

    fn slash(path: &std::path::Path) -> String {
        path.to_string_lossy().replace('\\', "/")
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
            self.write_bytes(relative, contents.as_bytes());
        }

        fn write_bytes(&self, relative: &str, contents: &[u8]) {
            let path = self.path(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture directory");
            }
            fs::write(path, contents).expect("fixture file");
        }

        fn write_character_layers(&self, package_root: &str) {
            const LAYERS: [(&str, &[u8]); 5] = [
                (
                    "layers/body--default.png",
                    include_bytes!(
                        "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/body--default.png"
                    ),
                ),
                (
                    "layers/eyes--normal.png",
                    include_bytes!(
                        "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/eyes--normal.png"
                    ),
                ),
                (
                    "layers/eyes--smile.png",
                    include_bytes!(
                        "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/eyes--smile.png"
                    ),
                ),
                (
                    "layers/mouth--neutral.png",
                    include_bytes!(
                        "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/mouth--neutral.png"
                    ),
                ),
                (
                    "layers/mouth--smile.png",
                    include_bytes!(
                        "../../arcweft-character/tests/fixtures/zundamon.awchar/layers/mouth--smile.png"
                    ),
                ),
            ];
            for (relative, bytes) in LAYERS {
                self.write_bytes(&format!("{package_root}/{relative}"), bytes);
            }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
