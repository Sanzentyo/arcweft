//! Filesystem construction of complete source-backed registration facts.

use std::sync::Arc;

use arcweft_adapter_context::manifest::{AdapterManifest, AdapterRegistrationFactsError};
use arcweft_character::{
    manifest::registration::{CharacterManifestRootField, CharacterManifestTokenPath},
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
use arcweft_source::SourceDocument;
use thiserror::Error;

use crate::{character_manifest, project::LoadedProject};

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
        source: character_manifest::LoadError,
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
    #[error("project registration facts were rejected")]
    Registration(CharacterRegistrationReport),
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

/// Loads character documents and returns the only semantic registration input shape.
#[allow(
    clippy::result_large_err,
    reason = "loader failures retain the typed path-specific manifest error as their source"
)]
pub fn load_project_registration_facts(
    request: &ProjectLoadRequest<'_>,
) -> Result<ProjectRegistrationFacts, ProjectRegistrationLoadError> {
    let root_document = request
        .loaded
        .module_document(&CanonicalModulePath::crate_root())
        .ok_or(ProjectRegistrationLoadError::MissingRootModule)?;
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

    let mut documents = request
        .loaded
        .module_documents()
        .map(|(_, document)| Arc::clone(document))
        .collect::<Vec<_>>();
    documents.push(Arc::clone(request.loaded.manifest_document()));
    documents.extend(request.additional_documents.iter().cloned());
    let mut external_facts = request.external_facts.clone();
    let mut source_backed_manifests = Vec::new();

    for (index, manifest) in request.adapter_manifests.iter().enumerate() {
        let ordinal = u64::try_from(index)
            .map_err(|_| ProjectRegistrationLoadError::AdapterOrdinalOverflow)?;
        let facts = manifest.source_backed_registration_facts(ordinal)?;
        let (document, facts) = facts.into_parts();
        documents.push(document);
        external_facts.extend(facts);
    }

    if let Some(profile) = request.profile {
        for path in profile.character_manifests() {
            let loaded = character_manifest::load_for_project(
                path,
                package_name,
                request.loaded.sources().project_root(),
            )
            .map_err(|source| ProjectRegistrationLoadError::CharacterManifest {
                path: path.clone(),
                source,
            })?;
            let (document, manifest) = loaded.into_parts();
            let owner = manifest.manifest().character().clone();
            let declaration = manifest
                .source_map()
                .token(&CharacterManifestTokenPath::Root(
                    CharacterManifestRootField::Character,
                ))
                .ok_or_else(
                    || ProjectRegistrationLoadError::MissingCharacterDeclaration {
                        path: path.clone(),
                    },
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
            external_facts.push(ExternalRegistrationFact::new(
                seed,
                RegisteredExternalOwner::Character(owner),
                declaration,
            ));
            documents.push(document);
            source_backed_manifests.push(manifest);
        }
    }

    let catalogs = if source_backed_manifests.is_empty() {
        Vec::new()
    } else {
        vec![SourceBackedCharacterCatalog::try_new(
            request.loaded.manifest_document().identity().clone(),
            source_backed_manifests,
        )?]
    };
    ProjectRegistrationFacts::try_new(world, documents, external_facts, catalogs)
        .map_err(ProjectRegistrationLoadError::Registration)
}

#[cfg(test)]
mod tests {
    use super::{ProjectLoadRequest, load_project_registration_facts};
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
        let facts = load_project_registration_facts(&ProjectLoadRequest::new(
            &loaded,
            Some(&profile),
            Vec::new(),
            Vec::new(),
        ))
        .expect("registration facts");

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
