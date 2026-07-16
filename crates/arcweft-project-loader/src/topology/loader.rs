use super::{
    LoadedDocumentAccess, LoadedDocumentOwnership, LoadedProfileTopology,
    LoadedProfileTopologyResource, ProfileDependencyResourceSeed, ProfileTopologyLimits,
    ProfileTopologyLoadError, ProfileTopologyLoadRequest, ProfileTopologyLogicalPath,
    ProfileTopologyOwnerId, ProfileTopologyResourceId, ProfileTopologyResourceKind,
    ProfileTopologyResourceOrigin, ProfileTopologySeedError,
    budget::ProfileTopologyBudget,
    model::{slash_relative_path, validate_absolute_normalized_path},
};
use crate::{adapter_manifest, character_manifest, project, rust_metadata};
use arcweft_adapter_context::{manifest::AdapterRegistry, standard::SANS_IO_ADAPTER_ID};
use arcweft_lang_syntax::{
    ast::{
        common::UseTreeKind,
        module_path::{CanonicalModulePath, ModulePath},
        symbol_path::ProjectSymbolPath,
    },
    parser::parse_source,
};
use arcweft_launch::{ResolvedLaunchProfile, SourceBackedLaunchManifest};
use arcweft_project::{
    graph::ModuleDependency, manifest::ProjectManifest, sources::ProjectSourceFile,
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSetRevision,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Loads one complete profile topology without directory enumeration or fallback publication.
pub fn load_profile_topology(
    request: ProfileTopologyLoadRequest<'_>,
) -> Result<LoadedProfileTopology, ProfileTopologyLoadError> {
    TopologyBuilder::new(request)?.load()
}

struct TopologyBuilder<'a> {
    manifest_path: PathBuf,
    project_root: PathBuf,
    workspace_owner: ProfileTopologyOwnerId,
    selection: arcweft_launch::LaunchProfileSelection<'a>,
    overlays: BTreeMap<PathBuf, Arc<str>>,
    dependency_resources: &'a [ProfileDependencyResourceSeed],
    base_adapters: AdapterRegistry,
    resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
    paths: BTreeMap<PathBuf, ProfileTopologyResourceId>,
    consumed_overlays: BTreeSet<ProfileTopologyResourceId>,
    budget: ProfileTopologyBudget,
}

#[derive(Clone)]
struct ResourceClaim {
    id: ProfileTopologyResourceId,
    kind: ProfileTopologyResourceKind,
    path: PathBuf,
    source_id: Option<SourceDocumentId>,
    ownership: LoadedDocumentOwnership,
    access: LoadedDocumentAccess,
}

struct BoundText {
    source: Arc<str>,
    origin: ProfileTopologyResourceOrigin,
}

impl<'a> TopologyBuilder<'a> {
    fn new(request: ProfileTopologyLoadRequest<'a>) -> Result<Self, ProfileTopologyLoadError> {
        validate_absolute_normalized_path(request.manifest_path, "manifest path")
            .map_err(|source| ProfileTopologyLoadError::DependencySeed { source })?;
        if !matches!(
            request.workspace_owner,
            ProfileTopologyOwnerId::Workspace { .. }
        ) {
            return Err(ProfileTopologyLoadError::DependencySeed {
                source: ProfileTopologySeedError::DependencyOwnerRequired,
            });
        }
        let project_root = request
            .manifest_path
            .parent()
            .ok_or_else(|| ProfileTopologyLoadError::ManifestNotFound {
                path: request.manifest_path.to_path_buf(),
            })?
            .to_path_buf();
        let mut overlays = BTreeMap::new();
        for overlay in request.overlays {
            if overlays
                .insert(overlay.path().to_path_buf(), Arc::clone(overlay.source()))
                .is_some()
            {
                return Err(ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::DuplicateOverlayPath {
                        path: overlay.path().to_path_buf(),
                    },
                });
            }
        }
        let mut dependency_keys = BTreeSet::new();
        for seed in request.dependency_resources {
            let key = (seed.path().to_path_buf(), seed.kind().clone());
            if !dependency_keys.insert(key) {
                return Err(ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::DuplicateDependencySeed {
                        path: seed.path().to_path_buf(),
                    },
                });
            }
        }
        Ok(Self {
            manifest_path: request.manifest_path.to_path_buf(),
            project_root,
            workspace_owner: request.workspace_owner,
            selection: request.selection,
            overlays,
            dependency_resources: request.dependency_resources,
            base_adapters: request.base_adapters,
            resources: BTreeMap::new(),
            paths: BTreeMap::new(),
            consumed_overlays: BTreeSet::new(),
            budget: ProfileTopologyBudget::production(),
        })
    }

    fn load(mut self) -> Result<LoadedProfileTopology, ProfileTopologyLoadError> {
        let (project_manifest, manifest_document, launch) = self.load_primary_manifest()?;
        self.charge_selection_work(launch.manifest())?;
        let profile_id = launch
            .manifest()
            .select_profile_id(self.selection)
            .map_err(|source| ProfileTopologyLoadError::ProfileSelection { source })?;
        let selected_profile = launch
            .manifest()
            .resolve_profile(profile_id, &self.project_root)
            .map_err(|source| ProfileTopologyLoadError::ProfileSelection { source })?;
        let package = project_manifest.package().name().as_str().to_owned();
        let source_root = project_manifest.source_root(&self.project_root);
        let modules = self.load_modules(&package, &source_root, selected_profile.source())?;
        self.load_character_resources(&package, &selected_profile)?;
        let (adapter_sources, adapter, rust_metadata_sources) =
            self.load_adapter_resources(&package, &selected_profile)?;
        self.freeze(
            project_manifest,
            manifest_document,
            launch,
            modules,
            selected_profile,
            adapter_sources,
            adapter,
            rust_metadata_sources,
        )
    }

    fn load_primary_manifest(
        &mut self,
    ) -> Result<
        (
            ProjectManifest,
            Arc<SourceDocument>,
            SourceBackedLaunchManifest,
        ),
        ProfileTopologyLoadError,
    > {
        let manifest_id = ProfileTopologyResourceId::new(
            self.workspace_owner.clone(),
            ProfileTopologyLogicalPath::try_new("arcw.toml")
                .expect("the canonical manifest logical path is valid"),
        );
        let manifest_claim = ResourceClaim {
            id: manifest_id.clone(),
            kind: ProfileTopologyResourceKind::Manifest,
            path: self.manifest_path.clone(),
            source_id: None,
            ownership: LoadedDocumentOwnership::Workspace,
            access: observed_access(&self.manifest_path, LoadedDocumentOwnership::Workspace),
        };
        let manifest_text = self.acquire_manifest_text(&manifest_claim)?;
        self.budget.charge_work(1)?;
        let project_manifest =
            ProjectManifest::parse_toml(&manifest_text.source).map_err(|source| {
                ProfileTopologyLoadError::ProjectManifest {
                    id: manifest_id.clone(),
                    path: self.manifest_path.clone(),
                    source: Box::new(source),
                }
            })?;
        let package = project_manifest.package().name().as_str().to_owned();
        let manifest_document = Self::document_for_claim(
            &manifest_claim,
            project::project_document_id(&package, &self.project_root, &self.manifest_path)
                .map_err(|_| ProfileTopologyLoadError::UnownedResourcePath {
                    path: self.manifest_path.clone(),
                    kind: ProfileTopologyResourceKind::Manifest,
                })?,
            &manifest_text,
        )?;
        self.finalize(
            manifest_claim,
            Arc::clone(&manifest_document),
            manifest_text.origin,
        )?;
        self.budget.charge_work(1)?;
        let launch =
            SourceBackedLaunchManifest::parse_document(&manifest_document).map_err(|source| {
                ProfileTopologyLoadError::LaunchManifest {
                    id: manifest_id,
                    path: self.manifest_path.clone(),
                    source: Box::new(source),
                }
            })?;
        Ok((project_manifest, manifest_document, launch))
    }

    fn load_character_resources(
        &mut self,
        package: &str,
        selected_profile: &ResolvedLaunchProfile,
    ) -> Result<(), ProfileTopologyLoadError> {
        for path in selected_profile.character_manifests() {
            let path = character_manifest::manifest_path(path);
            let resource = self.acquire_document(
                package,
                ProfileTopologyResourceKind::CharacterManifest,
                &path,
            )?;
            self.budget.charge_work(1)?;
            character_manifest::decode(path.clone(), Arc::clone(resource.document())).map_err(
                |source| ProfileTopologyLoadError::CharacterManifest {
                    id: resource.id().clone(),
                    path,
                    source: Box::new(source),
                },
            )?;
        }
        Ok(())
    }

    fn load_adapter_resources(
        &mut self,
        package: &str,
        selected_profile: &ResolvedLaunchProfile,
    ) -> Result<
        (
            Vec<adapter_manifest::LoadedAdapterManifest>,
            arcweft_adapter_context::manifest::AdapterManifest,
            Vec<rust_metadata::LoadedRustMetadata>,
        ),
        ProfileTopologyLoadError,
    > {
        let mut adapter_sources = Vec::new();
        let mut registry = std::mem::take(&mut self.base_adapters);
        for path in selected_profile.adapter_manifests() {
            let resource =
                self.acquire_document(package, ProfileTopologyResourceKind::AdapterManifest, path)?;
            self.budget.charge_work(1)?;
            let loaded = adapter_manifest::decode(path.clone(), Arc::clone(resource.document()))
                .map_err(|source| ProfileTopologyLoadError::AdapterManifest {
                    id: resource.id().clone(),
                    path: path.clone(),
                    source: Box::new(source),
                })?;
            self.budget.charge_work(1)?;
            registry = registry.try_with_manifest(loaded.manifest().clone())?;
            adapter_sources.push(loaded);
        }

        let selected_adapter_id = selected_profile.adapter().unwrap_or(SANS_IO_ADAPTER_ID);
        let mut adapter = registry.get(selected_adapter_id).cloned().ok_or_else(|| {
            ProfileTopologyLoadError::AdapterSelection {
                id: selected_adapter_id.to_owned(),
            }
        })?;

        let mut rust_metadata_sources = Vec::new();
        for path in selected_profile.rust_metadata() {
            let resource =
                self.acquire_document(package, ProfileTopologyResourceKind::RustMetadata, path)?;
            self.budget.charge_work(1)?;
            let loaded = rust_metadata::decode(path.clone(), Arc::clone(resource.document()))
                .map_err(|source| ProfileTopologyLoadError::RustMetadata {
                    id: resource.id().clone(),
                    path: path.clone(),
                    source: Box::new(source),
                })?;
            rust_metadata_sources.push(loaded);
        }
        for loaded in &rust_metadata_sources {
            self.budget.charge_work(1)?;
            adapter = adapter
                .try_with_rust_manifest(loaded.manifest())
                .map_err(|source| ProfileTopologyLoadError::RustCallableModel {
                    path: loaded.path().to_path_buf(),
                    source,
                })?;
        }
        Ok((adapter_sources, adapter, rust_metadata_sources))
    }

    #[allow(clippy::too_many_arguments)]
    fn freeze(
        self,
        project_manifest: ProjectManifest,
        manifest_document: Arc<SourceDocument>,
        launch: SourceBackedLaunchManifest,
        modules: Vec<ProjectSourceFile>,
        selected_profile: ResolvedLaunchProfile,
        adapter_sources: Vec<adapter_manifest::LoadedAdapterManifest>,
        adapter: arcweft_adapter_context::manifest::AdapterManifest,
        rust_metadata_sources: Vec<rust_metadata::LoadedRustMetadata>,
    ) -> Result<LoadedProfileTopology, ProfileTopologyLoadError> {
        let loaded_project = project::LoadedProject::from_exact_documents(
            self.manifest_path.clone(),
            self.project_root.clone(),
            project_manifest,
            manifest_document,
            launch,
            modules,
        )
        .map_err(|source| ProfileTopologyLoadError::ModuleDeclaration {
            id: self
                .resources
                .values()
                .find(|resource| {
                    matches!(
                        resource.kind(),
                        ProfileTopologyResourceKind::ArcweftModule { module }
                            if module.is_crate_root()
                    )
                })
                .map_or_else(
                    || {
                        ProfileTopologyResourceId::new(
                            self.workspace_owner.clone(),
                            ProfileTopologyLogicalPath::try_new("unknown.arcw")
                                .expect("fallback logical path is valid"),
                        )
                    },
                    |resource| resource.id().clone(),
                ),
            path: selected_profile.source().to_path_buf(),
            source: Box::new(source),
        })?;
        let source_revision = SourceSetRevision::try_for_identities(
            self.resources
                .values()
                .map(|resource| resource.document().identity()),
        )
        .map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
            kind: super::ProfileTopologyLimitKind::Resources,
        })?;
        let work = self.budget.work();
        Ok(LoadedProfileTopology::new(
            loaded_project,
            selected_profile,
            adapter_sources,
            adapter,
            rust_metadata_sources,
            self.resources,
            self.consumed_overlays.into_iter().collect(),
            source_revision,
            work,
        ))
    }

    fn charge_selection_work(
        &mut self,
        manifest: &arcweft_launch::LaunchProfileManifest,
    ) -> Result<(), ProfileTopologyLoadError> {
        if matches!(
            self.selection,
            arcweft_launch::LaunchProfileSelection::Explicit(_)
        ) {
            return Ok(());
        }
        if manifest.default_profile().is_some() {
            return self.budget.charge_work(1);
        }
        if let arcweft_launch::LaunchProfileSelection::Automatic { previous: Some(_) } =
            self.selection
        {
            self.budget.charge_work(1)?;
        }
        if !manifest.profiles().is_empty() {
            self.budget.charge_work(1)?;
        }
        Ok(())
    }

    fn load_modules(
        &mut self,
        package: &str,
        source_root: &Path,
        selected_source: &Path,
    ) -> Result<Vec<ProjectSourceFile>, ProfileTopologyLoadError> {
        let root = CanonicalModulePath::crate_root();
        let root_resource = self.acquire_document(
            package,
            ProfileTopologyResourceKind::ArcweftModule {
                module: root.clone(),
            },
            selected_source,
        )?;
        let mut module_resources = BTreeMap::from([(
            root.clone(),
            (
                selected_source.to_path_buf(),
                Arc::clone(root_resource.document()),
            ),
        )]);
        let mut queue = BTreeSet::from([root]);
        let mut dependencies = BTreeMap::<CanonicalModulePath, Vec<ModuleDependency>>::new();

        while let Some(module) = queue.pop_first() {
            let (path, document) = module_resources
                .get(&module)
                .cloned()
                .expect("queued modules have retained source documents");
            let resource_id = self
                .paths
                .get(&path)
                .cloned()
                .expect("module path has a topology resource ID");
            let module_dependencies = self.load_module_dependencies(
                package,
                source_root,
                &module,
                &path,
                &document,
                &resource_id,
                &mut module_resources,
                &mut queue,
            )?;
            dependencies.insert(module, module_dependencies);
        }

        Ok(module_resources
            .into_iter()
            .map(|(module, (path, document))| {
                let module_dependencies = dependencies.remove(&module).unwrap_or_default();
                ProjectSourceFile::new(module, path, document, module_dependencies)
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    fn load_module_dependencies(
        &mut self,
        package: &str,
        source_root: &Path,
        module: &CanonicalModulePath,
        path: &Path,
        document: &SourceDocument,
        resource_id: &ProfileTopologyResourceId,
        module_resources: &mut BTreeMap<CanonicalModulePath, (PathBuf, Arc<SourceDocument>)>,
        queue: &mut BTreeSet<CanonicalModulePath>,
    ) -> Result<Vec<ModuleDependency>, ProfileTopologyLoadError> {
        self.budget.charge_work(1)?;
        let parsed = parse_source(document.text());
        if !parsed.errors().is_empty() {
            let maximum = usize::try_from(ProfileTopologyLimits::PRODUCTION.diagnostics())
                .expect("the production diagnostic limit fits usize");
            let truncated = parsed.errors().len() > maximum;
            let retained = if truncated { maximum - 1 } else { maximum };
            let mut diagnostics = parsed
                .errors()
                .iter()
                .take(retained)
                .map(|error| error.message().to_owned())
                .collect::<Vec<_>>();
            if truncated {
                diagnostics.push(format!(
                    "topology diagnostic limit exceeded: retained {}, observed at least {}",
                    maximum,
                    maximum + 1
                ));
            }
            self.budget
                .charge_diagnostics(u64::try_from(diagnostics.len()).map_err(|_| {
                    ProfileTopologyLoadError::ArithmeticOverflow {
                        kind: super::ProfileTopologyLimitKind::Diagnostics,
                    }
                })?)?;
            return Err(ProfileTopologyLoadError::ModuleSyntax {
                id: resource_id.clone(),
                path: path.to_path_buf(),
                diagnostics: diagnostics.into_boxed_slice(),
                truncated,
            });
        }
        let tree = parsed.typed_tree();
        Self::validate_module_declaration(module, path, tree, resource_id)?;
        let mut dependencies = Vec::new();
        for item in tree.uses() {
            self.budget.charge_work(1)?;
            let spanned = match item.tree().kind() {
                UseTreeKind::Path { path, .. } => path,
                UseTreeKind::Glob { module } | UseTreeKind::Group { module, .. } => module,
            };
            let target = self.resolve_import(
                package,
                source_root,
                module,
                spanned.path(),
                module_resources,
                queue,
            )?;
            let Some(target) = target else {
                let span = document
                    .span(SourceRange::new(
                        spanned.range().start(),
                        spanned.range().end(),
                    ))
                    .ok();
                return Err(ProfileTopologyLoadError::ModuleImport {
                    id: Box::new(resource_id.clone()),
                    path: path.to_path_buf(),
                    module: Box::new(module.clone()),
                    import: spanned.path().to_string().into_boxed_str(),
                    span: span.map(Box::new),
                });
            };
            if &target != module {
                self.budget.charge_work(1)?;
                dependencies.push(ModuleDependency::new(target));
            }
        }
        Ok(dependencies)
    }

    fn validate_module_declaration(
        expected: &CanonicalModulePath,
        path: &Path,
        tree: &arcweft_lang_syntax::ast::items::TypedSyntaxTree,
        id: &ProfileTopologyResourceId,
    ) -> Result<(), ProfileTopologyLoadError> {
        match tree.module() {
            Some(declaration) => {
                let declared = declaration
                    .module_path()
                    .and_then(|module| module.resolve_declaration_for(expected))
                    .map_err(|source| ProfileTopologyLoadError::ModuleDeclaration {
                        id: id.clone(),
                        path: path.to_path_buf(),
                        source: Box::new(project::ProjectLoadError::ModulePath(source)),
                    })?;
                if &declared != expected {
                    return Err(ProfileTopologyLoadError::ModuleDeclaration {
                        id: id.clone(),
                        path: path.to_path_buf(),
                        source: Box::new(project::ProjectLoadError::ModulePathMismatch {
                            path: path.to_path_buf(),
                            declared,
                            expected: expected.clone(),
                        }),
                    });
                }
            }
            None if expected.is_crate_root() => {}
            None => {
                return Err(ProfileTopologyLoadError::ModuleDeclaration {
                    id: id.clone(),
                    path: path.to_path_buf(),
                    source: Box::new(project::ProjectLoadError::MissingModuleDeclaration {
                        path: path.to_path_buf(),
                        expected: expected.clone(),
                    }),
                });
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_import(
        &mut self,
        package: &str,
        source_root: &Path,
        importer: &CanonicalModulePath,
        import: &ProjectSymbolPath,
        module_resources: &mut BTreeMap<CanonicalModulePath, (PathBuf, Arc<SourceDocument>)>,
        queue: &mut BTreeSet<CanonicalModulePath>,
    ) -> Result<Option<CanonicalModulePath>, ProfileTopologyLoadError> {
        let mut segments = Vec::new();
        for segment in import.segments() {
            let Ok(segment) = segment.try_as_module_segment() else {
                break;
            };
            segments.push(segment);
        }
        for length in (1..=segments.len()).rev() {
            self.budget.charge_work(1)?;
            let candidate = ModulePath::new(import.root(), segments[..length].iter().cloned())
                .and_then(|module| module.resolve_from(importer))
                .map_err(|source| ProfileTopologyLoadError::ModuleDeclaration {
                    id: self
                        .resources
                        .values()
                        .find(|resource| {
                            matches!(resource.kind(), ProfileTopologyResourceKind::ArcweftModule { module } if module == importer)
                        })
                        .expect("importer resource exists")
                        .id()
                        .clone(),
                    path: module_resources
                        .get(importer)
                        .expect("importer path exists")
                        .0
                        .clone(),
                    source: Box::new(project::ProjectLoadError::ModulePath(source)),
                })?;
            if module_resources.contains_key(&candidate) {
                return Ok(Some(candidate));
            }
            let workspace_path = module_path(source_root, &candidate);
            let kind = ProfileTopologyResourceKind::ArcweftModule {
                module: candidate.clone(),
            };
            if let Some(resource) =
                self.probe_and_acquire_document(package, &kind, &workspace_path)?
            {
                module_resources.insert(
                    candidate.clone(),
                    (workspace_path, Arc::clone(resource.document())),
                );
                queue.insert(candidate.clone());
                return Ok(Some(candidate));
            }
            let dependency_paths = self
                .dependency_resources
                .iter()
                .filter(|seed| seed.kind() == &kind)
                .map(|seed| seed.path().to_path_buf())
                .collect::<Vec<_>>();
            for path in dependency_paths {
                if let Some(resource) = self.probe_and_acquire_document(package, &kind, &path)? {
                    module_resources
                        .insert(candidate.clone(), (path, Arc::clone(resource.document())));
                    queue.insert(candidate.clone());
                    return Ok(Some(candidate));
                }
            }
        }
        Ok(None)
    }

    fn acquire_manifest_text(
        &mut self,
        claim: &ResourceClaim,
    ) -> Result<BoundText, ProfileTopologyLoadError> {
        self.check_duplicate_claim(claim)?;
        self.budget.charge_resource()?;
        self.budget.charge_work(1)?;
        self.budget.charge_work(1)?;
        if let Some(source) = self.overlays.get(&claim.path).cloned() {
            let bytes = self.budget.check_source_bytes(source.len())?;
            self.budget.charge_overlay_bytes(bytes)?;
            return Ok(BoundText {
                source,
                origin: ProfileTopologyResourceOrigin::Overlay,
            });
        }
        self.budget.charge_work(1)?;
        if !claim.path.is_file() {
            return Err(ProfileTopologyLoadError::ManifestNotFound {
                path: claim.path.clone(),
            });
        }
        self.read_disk(claim)
    }

    fn acquire_document(
        &mut self,
        package: &str,
        kind: ProfileTopologyResourceKind,
        path: &Path,
    ) -> Result<LoadedProfileTopologyResource, ProfileTopologyLoadError> {
        let claim = self.claim_for_path(package, kind, path)?;
        let bound = self.acquire_required_text(&claim)?;
        let source_id = claim
            .source_id
            .clone()
            .expect("non-manifest claims have source document IDs");
        let document = Self::document_for_claim(&claim, source_id, &bound)?;
        self.finalize(claim, document, bound.origin)
    }

    fn probe_and_acquire_document(
        &mut self,
        package: &str,
        kind: &ProfileTopologyResourceKind,
        path: &Path,
    ) -> Result<Option<LoadedProfileTopologyResource>, ProfileTopologyLoadError> {
        let claim = match self.claim_for_path(package, kind.clone(), path) {
            Ok(claim) => claim,
            Err(ProfileTopologyLoadError::UnownedResourcePath { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        self.budget.charge_work(1)?;
        self.budget.charge_work(1)?;
        let bound = if let Some(source) = self.overlays.get(path).cloned() {
            let bytes = self.budget.check_source_bytes(source.len())?;
            self.budget.charge_overlay_bytes(bytes)?;
            Some(BoundText {
                source,
                origin: ProfileTopologyResourceOrigin::Overlay,
            })
        } else {
            self.budget.charge_work(1)?;
            if path.is_file() {
                Some(self.read_disk(&claim)?)
            } else {
                None
            }
        };
        let Some(bound) = bound else {
            return Ok(None);
        };
        self.check_duplicate_claim(&claim)?;
        self.budget.charge_resource()?;
        self.budget.charge_work(1)?;
        let source_id = claim
            .source_id
            .clone()
            .expect("module claims have source document IDs");
        let document = Self::document_for_claim(&claim, source_id, &bound)?;
        self.finalize(claim, document, bound.origin).map(Some)
    }

    fn acquire_required_text(
        &mut self,
        claim: &ResourceClaim,
    ) -> Result<BoundText, ProfileTopologyLoadError> {
        self.check_duplicate_claim(claim)?;
        self.budget.charge_resource()?;
        self.budget.charge_work(1)?;
        self.budget.charge_work(1)?;
        if let Some(source) = self.overlays.get(&claim.path).cloned() {
            let bytes = self.budget.check_source_bytes(source.len())?;
            self.budget.charge_overlay_bytes(bytes)?;
            return Ok(BoundText {
                source,
                origin: ProfileTopologyResourceOrigin::Overlay,
            });
        }
        self.budget.charge_work(1)?;
        self.read_disk(claim)
    }

    fn read_disk(&self, claim: &ResourceClaim) -> Result<BoundText, ProfileTopologyLoadError> {
        let bytes =
            fs::read(&claim.path).map_err(|source| ProfileTopologyLoadError::ResourceRead {
                id: claim.id.clone(),
                path: claim.path.clone(),
                source,
            })?;
        self.budget.check_source_bytes(bytes.len())?;
        let source =
            String::from_utf8(bytes).map_err(|_| ProfileTopologyLoadError::ResourceUtf8 {
                id: claim.id.clone(),
                path: claim.path.clone(),
            })?;
        Ok(BoundText {
            source: Arc::from(source),
            origin: ProfileTopologyResourceOrigin::Disk,
        })
    }

    fn document_for_claim(
        claim: &ResourceClaim,
        source_id: SourceDocumentId,
        bound: &BoundText,
    ) -> Result<Arc<SourceDocument>, ProfileTopologyLoadError> {
        SourceDocument::try_new(
            source_id,
            SourceName::path(claim.path.display().to_string()),
            Arc::clone(&bound.source),
        )
        .map(Arc::new)
        .map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
            kind: super::ProfileTopologyLimitKind::SourceBytes,
        })
    }

    fn finalize(
        &mut self,
        claim: ResourceClaim,
        document: Arc<SourceDocument>,
        origin: ProfileTopologyResourceOrigin,
    ) -> Result<LoadedProfileTopologyResource, ProfileTopologyLoadError> {
        self.budget.charge_work(1)?;
        let resource = LoadedProfileTopologyResource {
            id: claim.id.clone(),
            kind: claim.kind,
            path: claim.path.clone(),
            document,
            ownership: claim.ownership,
            access: claim.access,
            origin,
        };
        self.paths.insert(claim.path, claim.id.clone());
        if origin == ProfileTopologyResourceOrigin::Overlay {
            self.consumed_overlays.insert(claim.id.clone());
        }
        self.resources.insert(claim.id, resource.clone());
        Ok(resource)
    }

    fn claim_for_path(
        &self,
        package: &str,
        kind: ProfileTopologyResourceKind,
        path: &Path,
    ) -> Result<ResourceClaim, ProfileTopologyLoadError> {
        if let Ok(relative) = path.strip_prefix(&self.project_root) {
            let logical = slash_relative_path(relative)
                .map_err(|source| ProfileTopologyLoadError::DependencySeed { source })?;
            let source_id = project::project_document_id(package, &self.project_root, path)
                .map_err(|_| ProfileTopologyLoadError::UnownedResourcePath {
                    path: path.to_path_buf(),
                    kind: kind.clone(),
                })?;
            return Ok(ResourceClaim {
                id: ProfileTopologyResourceId::new(
                    self.workspace_owner.clone(),
                    ProfileTopologyLogicalPath::try_new(logical)
                        .expect("workspace relative paths were validated"),
                ),
                kind,
                path: path.to_path_buf(),
                source_id: Some(source_id),
                ownership: LoadedDocumentOwnership::Workspace,
                access: observed_access(path, LoadedDocumentOwnership::Workspace),
            });
        }
        let seed = self
            .dependency_resources
            .iter()
            .find(|seed| seed.path() == path && seed.kind() == &kind)
            .ok_or_else(|| ProfileTopologyLoadError::UnownedResourcePath {
                path: path.to_path_buf(),
                kind: kind.clone(),
            })?;
        Ok(ResourceClaim {
            id: seed.id().clone(),
            kind,
            path: path.to_path_buf(),
            source_id: Some(seed.source_id().clone()),
            ownership: LoadedDocumentOwnership::Dependency,
            access: observed_access(path, LoadedDocumentOwnership::Dependency),
        })
    }

    fn check_duplicate_claim(&self, claim: &ResourceClaim) -> Result<(), ProfileTopologyLoadError> {
        if let Some(first) = self.resources.get(&claim.id) {
            return Err(ProfileTopologyLoadError::DuplicateLogicalId {
                first: Box::new(first.clone()),
                conflicting: Box::new(conflicting_resource(first, claim)),
            });
        }
        if let Some(first_id) = self.paths.get(&claim.path)
            && first_id != &claim.id
        {
            let first = self
                .resources
                .get(first_id)
                .expect("path index refers to a retained resource");
            return Err(ProfileTopologyLoadError::DuplicatePath {
                first: Box::new(first.clone()),
                conflicting: Box::new(conflicting_resource(first, claim)),
            });
        }
        Ok(())
    }
}

fn conflicting_resource(
    first: &LoadedProfileTopologyResource,
    claim: &ResourceClaim,
) -> LoadedProfileTopologyResource {
    LoadedProfileTopologyResource {
        id: claim.id.clone(),
        kind: claim.kind.clone(),
        path: claim.path.clone(),
        document: Arc::clone(first.document()),
        ownership: claim.ownership,
        access: claim.access,
        origin: first.origin(),
    }
}

fn module_path(source_root: &Path, module: &CanonicalModulePath) -> PathBuf {
    let mut path = source_root.to_path_buf();
    for segment in module.segments() {
        path.push(segment.as_str());
    }
    path.set_extension("arcw");
    path
}

fn observed_access(path: &Path, ownership: LoadedDocumentOwnership) -> LoadedDocumentAccess {
    if ownership == LoadedDocumentOwnership::Dependency {
        return if path.exists() {
            LoadedDocumentAccess::ReadOnly
        } else {
            LoadedDocumentAccess::Unknown
        };
    }
    fs::metadata(path).map_or(LoadedDocumentAccess::Unknown, |metadata| {
        if metadata.permissions().readonly() {
            LoadedDocumentAccess::ReadOnly
        } else {
            LoadedDocumentAccess::Writable
        }
    })
}
