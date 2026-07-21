use super::{
    LoadedCharacterPackage, LoadedDocumentAccess, LoadedDocumentOwnership,
    LoadedExternalModuleMetadata, LoadedProfileTopology, LoadedProfileTopologyResource,
    LoadedProfileTopologyResourcePayload, ProfileDependencyBinaryResourceSeed,
    ProfileDependencyResourceSeed, ProfileTopologyLimits, ProfileTopologyLoadError,
    ProfileTopologyLoadRequest, ProfileTopologyLogicalPath, ProfileTopologyOwnerId,
    ProfileTopologyResourceId, ProfileTopologyResourceKind, ProfileTopologyResourceOrigin,
    ProfileTopologySeedError,
    budget::ProfileTopologyBudget,
    external::{extend_selected_adapter, validate_activity_bindings},
    model::{slash_relative_path, validate_absolute_normalized_path},
};
use crate::layout::{ContainedProjectLayout, ProjectPathRole, canonical_project_root};
use crate::{character_manifest, project};
use arcweft_adapter_context::manifest::{AdapterManifest, AdapterRegistry};
use arcweft_adapter_metadata::{AdapterTarget, SourceBackedAdapterMetadata};
use arcweft_character::{
    id::CharacterId,
    package::{CharacterLayerPayload, CharacterPackage},
};
use arcweft_lang_syntax::{
    ast::{
        common::UseTreeKind,
        module_path::{CanonicalModulePath, ModulePath},
        symbol_path::ProjectSymbolPath,
    },
    parser::parse_source,
};
use arcweft_launch::{accepted::SourceBackedManifest, resolve::ResolvedLaunchProfile};
use arcweft_manifest_model::{AdapterFamily, RawDigest};
use arcweft_project::{
    content::ProjectBinaryResource, graph::ModuleDependency, sources::ProjectSourceFile,
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
    binary_overlays: BTreeMap<PathBuf, Arc<[u8]>>,
    dependency_resources: &'a [ProfileDependencyResourceSeed],
    dependency_binary_resources: &'a [ProfileDependencyBinaryResourceSeed],
    base_adapters: AdapterRegistry,
    layout: arcweft_project::layout::ProjectLayoutSpec,
    resources: BTreeMap<ProfileTopologyResourceId, LoadedProfileTopologyResource>,
    paths: BTreeMap<PathBuf, ProfileTopologyResourceId>,
    consumed_overlays: BTreeSet<ProfileTopologyResourceId>,
    consumed_binary_overlay_paths: BTreeSet<PathBuf>,
    character_packages: BTreeMap<CharacterId, LoadedCharacterPackage>,
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

struct BoundBinary {
    bytes: Arc<[u8]>,
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
        let requested_project_root = request.manifest_path.parent().ok_or_else(|| {
            ProfileTopologyLoadError::ManifestNotFound {
                path: request.manifest_path.to_path_buf(),
            }
        })?;
        let project_root = canonical_project_root(requested_project_root)
            .map_err(|source| ProfileTopologyLoadError::ProjectLayout { source })?;
        let manifest_path = request
            .manifest_path
            .strip_prefix(requested_project_root)
            .map(|relative| project_root.join(relative))
            .map_err(|_| ProfileTopologyLoadError::ManifestNotFound {
                path: request.manifest_path.to_path_buf(),
            })?;
        let mut overlays = BTreeMap::new();
        for overlay in request.overlays {
            let path = overlay
                .path()
                .strip_prefix(requested_project_root)
                .map_or_else(
                    |_| overlay.path().to_path_buf(),
                    |relative| project_root.join(relative),
                );
            if overlays
                .insert(path.clone(), Arc::clone(overlay.source()))
                .is_some()
            {
                return Err(ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::DuplicateOverlayPath { path },
                });
            }
        }
        let mut binary_overlays = BTreeMap::new();
        for overlay in request.binary_overlays {
            let path = overlay
                .path()
                .strip_prefix(requested_project_root)
                .map_or_else(
                    |_| overlay.path().to_path_buf(),
                    |relative| project_root.join(relative),
                );
            if overlays.contains_key(&path) {
                return Err(ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::OverlayKindConflict { path },
                });
            }
            if binary_overlays
                .insert(path.clone(), Arc::clone(overlay.bytes()))
                .is_some()
            {
                return Err(ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::DuplicateOverlayPath { path },
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
        for seed in request.dependency_binary_resources {
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
            manifest_path,
            project_root,
            workspace_owner: request.workspace_owner,
            selection: request.selection,
            overlays,
            binary_overlays,
            dependency_resources: request.dependency_resources,
            dependency_binary_resources: request.dependency_binary_resources,
            base_adapters: request.base_adapters,
            layout: request.layout,
            resources: BTreeMap::new(),
            paths: BTreeMap::new(),
            consumed_overlays: BTreeSet::new(),
            consumed_binary_overlay_paths: BTreeSet::new(),
            character_packages: BTreeMap::new(),
            budget: ProfileTopologyBudget::production(),
        })
    }

    fn load(mut self) -> Result<LoadedProfileTopology, ProfileTopologyLoadError> {
        let manifest = self.load_primary_manifest()?;
        self.charge_selection_work()?;
        let selected_profile = manifest
            .resolve_profile(self.selection)
            .map_err(|source| ProfileTopologyLoadError::ProfileSelection { source })?;
        let layout = ContainedProjectLayout::try_new(
            &self.project_root,
            manifest.manifest().build(),
            &self.layout,
        )
        .map_err(|source| ProfileTopologyLoadError::ProjectLayout { source })?;
        self.project_root = layout.project_root().to_path_buf();
        let package = manifest.manifest().package().id.as_str().to_owned();
        let selected_source = layout
            .project_root()
            .join(selected_profile.source().as_path());
        let modules =
            self.load_modules(&package, layout.source_root().as_path(), &selected_source)?;
        self.load_character_resources(&package, &selected_profile, &layout)?;
        let external_modules =
            self.load_external_module_metadata(&package, &selected_profile, &layout)?;
        validate_activity_bindings(&selected_profile, &external_modules)?;
        let adapter = self.select_adapter(&selected_profile)?;
        let adapter = extend_selected_adapter(adapter, &external_modules)?;
        self.freeze(
            manifest,
            layout,
            modules,
            selected_profile,
            external_modules,
            adapter,
        )
    }

    fn load_primary_manifest(
        &mut self,
    ) -> Result<Arc<SourceBackedManifest>, ProfileTopologyLoadError> {
        let manifest_logical_path =
            ProfileTopologyLogicalPath::try_new("arcw.toml").map_err(|source| {
                ProfileTopologyLoadError::DependencySeed {
                    source: ProfileTopologySeedError::LogicalPath {
                        path: PathBuf::from("arcw.toml"),
                        source,
                    },
                }
            })?;
        let manifest_id =
            ProfileTopologyResourceId::new(self.workspace_owner.clone(), manifest_logical_path);
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
        let manifest_document = Self::document_for_claim(
            &manifest_claim,
            project::manifest_document_id(&self.manifest_path).map_err(|_| {
                ProfileTopologyLoadError::UnownedResourcePath {
                    path: self.manifest_path.clone(),
                    kind: ProfileTopologyResourceKind::Manifest,
                }
            })?,
            &manifest_text,
        )?;
        self.finalize(
            manifest_claim,
            Arc::clone(&manifest_document),
            manifest_text.origin,
        )?;
        self.budget.charge_work(1)?;
        SourceBackedManifest::decode(manifest_document)
            .map(Arc::new)
            .map_err(|source| ProfileTopologyLoadError::Manifest {
                id: Box::new(manifest_id),
                path: self.manifest_path.clone(),
                source,
            })
    }

    fn load_character_resources(
        &mut self,
        package: &str,
        selected_profile: &ResolvedLaunchProfile,
        layout: &ContainedProjectLayout,
    ) -> Result<(), ProfileTopologyLoadError> {
        for content in selected_profile.content().values() {
            for root in content.unit().roots.as_slice() {
                let reference = root.0.as_str();
                let Some(owner) = reference.strip_prefix("@character.") else {
                    continue;
                };
                let character = CharacterId::try_new(&reference[1..]).map_err(|source| {
                    ProfileTopologyLoadError::CharacterReference {
                        reference: reference.to_owned(),
                        source,
                    }
                })?;
                if self.character_packages.contains_key(&character) {
                    continue;
                }
                let mut package_path = layout.asset_root().as_path().to_path_buf();
                for segment in owner.split('.') {
                    package_path.push(segment);
                }
                package_path.set_extension("awchar");
                let path = character_manifest::manifest_path(&package_path);
                let resource = self.acquire_document(
                    package,
                    ProfileTopologyResourceKind::CharacterPackageManifest {
                        character: character.clone(),
                    },
                    &path,
                )?;
                self.budget.charge_work(1)?;
                let document = Arc::clone(resource.text_document().ok_or_else(|| {
                    ProfileTopologyLoadError::ResourceUtf8 {
                        id: Box::new(resource.id().clone()),
                        kind: resource.kind().clone(),
                        path: path.clone(),
                    }
                })?);
                let loaded = character_manifest::decode(path.clone(), Arc::clone(&document))
                    .map_err(|source| ProfileTopologyLoadError::CharacterManifest {
                        id: resource.id().clone(),
                        path: path.clone(),
                        source: Box::new(source),
                    })?;
                if loaded.manifest().manifest().character() != &character {
                    return Err(ProfileTopologyLoadError::CharacterIdentityMismatch {
                        path,
                        expected: character,
                        actual: loaded.manifest().manifest().character().clone(),
                    });
                }
                let source_manifest = Arc::new(loaded.manifest().clone());
                let mut payloads = Vec::new();
                let mut layer_paths = BTreeMap::new();
                for asset in source_manifest
                    .manifest()
                    .parts()
                    .iter()
                    .flat_map(|part| part.variants().iter().map(|variant| variant.asset()))
                {
                    let layer_path = package_path.join(asset.as_str());
                    let layer = self.acquire_binary_resource(
                        package,
                        ProfileTopologyResourceKind::CharacterLayerPayload {
                            character: character.clone(),
                            asset: asset.clone(),
                        },
                        &layer_path,
                    )?;
                    let binary = layer.binary_resource().ok_or_else(|| {
                        ProfileTopologyLoadError::UnownedResourcePath {
                            path: layer_path.clone(),
                            kind: layer.kind().clone(),
                        }
                    })?;
                    payloads.push(CharacterLayerPayload::new(
                        asset.clone(),
                        binary.shared_bytes(),
                    ));
                    layer_paths.insert(asset.clone(), layer_path);
                }
                let package_model = Arc::new(
                    CharacterPackage::from_source_backed_manifest(
                        &document,
                        &source_manifest,
                        payloads,
                    )
                    .map_err(|source| {
                        ProfileTopologyLoadError::CharacterPackage {
                            path: package_path.clone(),
                            source,
                        }
                    })?,
                );
                self.character_packages.insert(
                    character,
                    LoadedCharacterPackage::new(
                        package_model,
                        source_manifest,
                        package_path,
                        path,
                        layer_paths,
                    ),
                );
            }
        }
        Ok(())
    }

    fn load_external_module_metadata(
        &mut self,
        package: &str,
        selected_profile: &ResolvedLaunchProfile,
        layout: &ContainedProjectLayout,
    ) -> Result<Vec<LoadedExternalModuleMetadata>, ProfileTopologyLoadError> {
        let mut loaded = Vec::with_capacity(selected_profile.external_modules().len());
        for (import_id, import) in selected_profile.external_modules() {
            let path = layout
                .contain_project_path(&import.metadata, ProjectPathRole::ExternalMetadata)
                .map_err(|source| ProfileTopologyLoadError::ProjectLayout { source })?;
            let path = path.as_path().to_path_buf();
            let resource = self.acquire_document(
                package,
                ProfileTopologyResourceKind::ExternalModuleMetadata {
                    import: import_id.clone(),
                },
                &path,
            )?;
            let id = resource.id().clone();
            let document = Arc::clone(required_text_document(&resource)?);
            if RawDigest::for_bytes(document.text().as_bytes()) != import.metadata_hash {
                return Err(ProfileTopologyLoadError::ExternalModuleMetadataHash {
                    import: import_id.clone(),
                    id,
                });
            }
            self.budget.charge_work(1)?;
            let metadata =
                SourceBackedAdapterMetadata::decode(document.text()).map_err(|source| {
                    ProfileTopologyLoadError::ExternalModuleMetadataDecode {
                        import: import_id.clone(),
                        id: id.clone(),
                        source: Box::new(source),
                    }
                })?;
            let accepted = metadata.metadata();
            require_external_metadata_field(
                import_id,
                "package",
                &import.expected_package,
                &accepted.package.id,
            )?;
            require_external_metadata_field(
                import_id,
                "version",
                &import.expected_version,
                &accepted.package.version,
            )?;
            require_external_metadata_field(
                import_id,
                "module",
                &import.expected_module,
                &accepted.module.id,
            )?;
            let actual_family = match &accepted.target {
                AdapterTarget::Rust(_) => AdapterFamily::Rust,
                AdapterTarget::Wasm(_) => AdapterFamily::Wasm,
                AdapterTarget::Process(_) => AdapterFamily::Process,
            };
            require_external_metadata_field(
                import_id,
                "family",
                adapter_family_name(import.expected_family),
                adapter_family_name(actual_family),
            )?;
            require_external_metadata_field(
                import_id,
                "abi-hash",
                &import.expected_abi_hash,
                &accepted.abi_hash,
            )?;
            loaded.push(LoadedExternalModuleMetadata::new(
                import_id.clone(),
                import.clone(),
                document,
                metadata,
            ));
        }
        Ok(loaded)
    }

    fn select_adapter(
        &mut self,
        selected_profile: &ResolvedLaunchProfile,
    ) -> Result<AdapterManifest, ProfileTopologyLoadError> {
        let registry = std::mem::take(&mut self.base_adapters);
        let selected_adapter_id = selected_profile.adapter().as_str();
        registry.get(selected_adapter_id).cloned().ok_or_else(|| {
            ProfileTopologyLoadError::AdapterSelection {
                id: selected_adapter_id.to_owned(),
            }
        })
    }

    fn freeze(
        self,
        manifest: Arc<SourceBackedManifest>,
        layout: ContainedProjectLayout,
        modules: Vec<ProjectSourceFile>,
        selected_profile: ResolvedLaunchProfile,
        external_modules: Vec<LoadedExternalModuleMetadata>,
        adapter: AdapterManifest,
    ) -> Result<LoadedProfileTopology, ProfileTopologyLoadError> {
        if let Some(path) = self
            .binary_overlays
            .keys()
            .find(|path| !self.consumed_binary_overlay_paths.contains(*path))
        {
            return Err(ProfileTopologyLoadError::DependencySeed {
                source: ProfileTopologySeedError::UnconsumedBinaryOverlay { path: path.clone() },
            });
        }
        let selected_source = layout
            .project_root()
            .join(selected_profile.source().as_path());
        let root_kind = ProfileTopologyResourceKind::ArcweftModule {
            module: CanonicalModulePath::crate_root(),
        };
        let root_resource_id = self
            .resources
            .values()
            .find(|resource| resource.kind() == &root_kind)
            .map(|resource| resource.id().clone())
            .ok_or_else(|| ProfileTopologyLoadError::UnownedResourcePath {
                path: selected_source.clone(),
                kind: root_kind,
            })?;
        let loaded_project = project::LoadedProject::from_exact_documents(
            self.manifest_path.clone(),
            self.project_root.clone(),
            Arc::clone(&manifest),
            modules,
        )
        .map_err(|source| ProfileTopologyLoadError::ModuleDeclaration {
            id: root_resource_id,
            path: selected_source,
            source: Box::new(source),
        })?;
        let source_documents_revision = SourceSetRevision::try_for_identities(
            self.resources
                .values()
                .filter_map(LoadedProfileTopologyResource::text_document)
                .map(|document| document.identity()),
        )
        .map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
            kind: super::ProfileTopologyLimitKind::Resources,
        })?;
        let work = self.budget.work();
        Ok(LoadedProfileTopology::new(
            loaded_project,
            manifest,
            selected_profile,
            layout,
            external_modules,
            adapter,
            self.resources,
            self.character_packages,
            self.consumed_overlays.into_iter().collect(),
            source_documents_revision,
            work,
        ))
    }

    fn charge_selection_work(&mut self) -> Result<(), ProfileTopologyLoadError> {
        self.budget.charge_work(1)
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
                Arc::clone(required_text_document(&root_resource)?),
            ),
        )]);
        let mut queue = BTreeSet::from([root]);
        let mut dependencies = BTreeMap::<CanonicalModulePath, Vec<ModuleDependency>>::new();

        while let Some(module) = queue.pop_first() {
            let Some((path, document)) = module_resources.get(&module).cloned() else {
                return Err(ProfileTopologyLoadError::UnownedResourcePath {
                    path: module_path(source_root, &module),
                    kind: ProfileTopologyResourceKind::ArcweftModule {
                        module: module.clone(),
                    },
                });
            };
            let resource_id = self.paths.get(&path).cloned().ok_or_else(|| {
                ProfileTopologyLoadError::UnownedResourcePath {
                    path: path.clone(),
                    kind: ProfileTopologyResourceKind::ArcweftModule {
                        module: module.clone(),
                    },
                }
            })?;
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
                .map_err(|_| ProfileTopologyLoadError::ArithmeticOverflow {
                    kind: super::ProfileTopologyLimitKind::Diagnostics,
                })?;
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
                path,
                resource_id,
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
        importer_path: &Path,
        importer_id: &ProfileTopologyResourceId,
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
                    id: importer_id.clone(),
                    path: importer_path.to_path_buf(),
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
                    (
                        workspace_path,
                        Arc::clone(required_text_document(&resource)?),
                    ),
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
                    module_resources.insert(
                        candidate.clone(),
                        (path, Arc::clone(required_text_document(&resource)?)),
                    );
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
        let source_id = claim.source_id.clone().ok_or_else(|| {
            ProfileTopologyLoadError::UnownedResourcePath {
                path: claim.path.clone(),
                kind: claim.kind.clone(),
            }
        })?;
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
        let source_id = claim.source_id.clone().ok_or_else(|| {
            ProfileTopologyLoadError::UnownedResourcePath {
                path: claim.path.clone(),
                kind: claim.kind.clone(),
            }
        })?;
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
                id: Box::new(claim.id.clone()),
                kind: claim.kind.clone(),
                path: claim.path.clone(),
                source,
            })?;
        self.budget.check_source_bytes(bytes.len())?;
        let source =
            String::from_utf8(bytes).map_err(|_| ProfileTopologyLoadError::ResourceUtf8 {
                id: Box::new(claim.id.clone()),
                kind: claim.kind.clone(),
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
            payload: LoadedProfileTopologyResourcePayload::Text(document),
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

    fn acquire_binary_resource(
        &mut self,
        package: &str,
        kind: ProfileTopologyResourceKind,
        path: &Path,
    ) -> Result<LoadedProfileTopologyResource, ProfileTopologyLoadError> {
        let claim = self.claim_for_binary_path(package, kind, path)?;
        self.check_duplicate_claim(&claim)?;
        self.budget.charge_resource()?;
        self.budget.charge_work(2)?;
        let bound = if let Some(bytes) = self.binary_overlays.get(&claim.path).cloned() {
            let observed = self.budget.check_source_bytes(bytes.len())?;
            self.budget.charge_overlay_bytes(observed)?;
            self.consumed_binary_overlay_paths
                .insert(claim.path.clone());
            BoundBinary {
                bytes,
                origin: ProfileTopologyResourceOrigin::Overlay,
            }
        } else {
            self.budget.charge_work(1)?;
            let bytes =
                fs::read(&claim.path).map_err(|source| ProfileTopologyLoadError::ResourceRead {
                    id: Box::new(claim.id.clone()),
                    kind: claim.kind.clone(),
                    path: claim.path.clone(),
                    source,
                })?;
            self.budget.check_source_bytes(bytes.len())?;
            BoundBinary {
                bytes: Arc::from(bytes),
                origin: ProfileTopologyResourceOrigin::Disk,
            }
        };
        self.budget.charge_work(1)?;
        let resource = Arc::new(ProjectBinaryResource::new(bound.bytes));
        let loaded = LoadedProfileTopologyResource {
            id: claim.id.clone(),
            kind: claim.kind,
            path: claim.path.clone(),
            payload: LoadedProfileTopologyResourcePayload::Binary(resource),
            ownership: claim.ownership,
            access: claim.access,
            origin: bound.origin,
        };
        self.paths.insert(claim.path, claim.id.clone());
        if bound.origin == ProfileTopologyResourceOrigin::Overlay {
            self.consumed_overlays.insert(claim.id.clone());
        }
        self.resources.insert(claim.id, loaded.clone());
        Ok(loaded)
    }

    fn claim_for_binary_path(
        &self,
        _package: &str,
        kind: ProfileTopologyResourceKind,
        path: &Path,
    ) -> Result<ResourceClaim, ProfileTopologyLoadError> {
        if let Ok(relative) = path.strip_prefix(&self.project_root) {
            let logical = slash_relative_path(relative)
                .map_err(|source| ProfileTopologyLoadError::DependencySeed { source })?;
            return Ok(ResourceClaim {
                id: ProfileTopologyResourceId::new(
                    self.workspace_owner.clone(),
                    ProfileTopologyLogicalPath::try_new(logical).map_err(|source| {
                        ProfileTopologyLoadError::DependencySeed {
                            source: ProfileTopologySeedError::LogicalPath {
                                path: relative.to_path_buf(),
                                source,
                            },
                        }
                    })?,
                ),
                kind,
                path: path.to_path_buf(),
                source_id: None,
                ownership: LoadedDocumentOwnership::Workspace,
                access: observed_access(path, LoadedDocumentOwnership::Workspace),
            });
        }
        let seed = self
            .dependency_binary_resources
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
            source_id: None,
            ownership: LoadedDocumentOwnership::Dependency,
            access: observed_access(path, LoadedDocumentOwnership::Dependency),
        })
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
                    ProfileTopologyLogicalPath::try_new(logical).map_err(|source| {
                        ProfileTopologyLoadError::DependencySeed {
                            source: ProfileTopologySeedError::LogicalPath {
                                path: relative.to_path_buf(),
                                source,
                            },
                        }
                    })?,
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
        if let Some(first_id) = self.paths.get(&claim.path) {
            let first = self.resources.get(first_id).ok_or_else(|| {
                ProfileTopologyLoadError::UnownedResourcePath {
                    path: claim.path.clone(),
                    kind: claim.kind.clone(),
                }
            })?;
            if first.id() != &claim.id || first.kind() != &claim.kind {
                return Err(ProfileTopologyLoadError::DuplicatePath {
                    first: Box::new(first.clone()),
                    conflicting: Box::new(conflicting_resource(first, claim)),
                });
            }
        }
        if let Some(first) = self.resources.get(&claim.id) {
            return Err(ProfileTopologyLoadError::DuplicateLogicalId {
                first: Box::new(first.clone()),
                conflicting: Box::new(conflicting_resource(first, claim)),
            });
        }
        Ok(())
    }
}

fn require_external_metadata_field<T>(
    import: &arcweft_manifest_model::ExternalModuleImportId,
    field: &'static str,
    expected: &T,
    actual: &T,
) -> Result<(), ProfileTopologyLoadError>
where
    T: std::fmt::Display + PartialEq + ?Sized,
{
    if expected != actual {
        return Err(
            ProfileTopologyLoadError::ExternalModuleMetadataExpectation {
                import: import.clone(),
                field,
                expected: expected.to_string(),
                actual: actual.to_string(),
            },
        );
    }
    Ok(())
}

const fn adapter_family_name(family: AdapterFamily) -> &'static str {
    match family {
        AdapterFamily::Rust => "rust",
        AdapterFamily::Wasm => "wasm",
        AdapterFamily::Process => "process",
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
        payload: first.payload.clone(),
        ownership: claim.ownership,
        access: claim.access,
        origin: first.origin(),
    }
}

fn required_text_document(
    resource: &LoadedProfileTopologyResource,
) -> Result<&Arc<SourceDocument>, ProfileTopologyLoadError> {
    resource
        .text_document()
        .ok_or_else(|| ProfileTopologyLoadError::ResourceUtf8 {
            id: Box::new(resource.id().clone()),
            kind: resource.kind().clone(),
            path: resource.path().to_path_buf(),
        })
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
