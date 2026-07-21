//! Compiler-owned lowering of authored Views into one validated product.

mod layout;
mod lowering;
mod schema;

use std::{collections::BTreeMap, sync::Arc};

use arcweft_bundle::{
    BundleImageObject,
    resource_codec::{
        SourceMapBuildError, SourceMapSection, ValidatedViewProduct, ViewInputResource,
        ViewProductValidationError, ViewProductValidationLimits, ViewProgramStyleResources,
        ViewResourceMergeError, ViewTextResource,
    },
    standard_view,
};
use arcweft_id::{IdError, PublicId};
use arcweft_lang_hir::{model::HirModule, project::HirProject};
use arcweft_lang_sema::{
    check::TypeCheckReport,
    dialogue_view::{DialogueViewModelError, DialogueViewModelRegistry},
};
use arcweft_presentation::{fx::FxDefinition, image::ImageObjectId};
use arcweft_project::sources::ProjectSources;
use arcweft_resource_model::registry::{ResourceTypeRegistry, ResourceTypeRegistryDigest};
use arcweft_source::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, SourceDocument, SourceDocumentIdentity,
    SourceRange, SourceSetRevision, SourceSpan, SourceSpanError,
};
use arcweft_view::{ViewId, ViewIdError, style::ViewStyleSheetId};
use thiserror::Error;

use self::lowering::{ViewBundleSidecars, view_resource_id, view_sidecars};
use crate::{image::CompiledImageCatalog, style::CompiledViewStyleArtifact};

/// One compiler candidate containing the only accepted View/Style catalog.
#[derive(Clone, Debug)]
pub struct CompiledViewProduct {
    product: Arc<ValidatedViewProduct>,
    text: Option<ViewTextResource>,
    input: Option<ViewInputResource>,
    image_objects: Vec<BundleImageObject>,
    view_sources: BTreeMap<ViewId, SourceSpan>,
    style_sources: BTreeMap<ViewStyleSheetId, SourceSpan>,
    authored_sources: SourceSetRevision,
    resource_types: ResourceTypeRegistryDigest,
}

/// Compiler-owned context for one source-bound View product candidate.
pub struct ViewProjectLowerer<'a> {
    linked_hir: &'a HirModule,
    typecheck: &'a TypeCheckReport,
    style: &'a CompiledViewStyleArtifact,
    source_map: SourceMapSection,
    source_image_objects: &'a [BundleImageObject],
    source_image_catalog: Option<&'a CompiledImageCatalog>,
    fx_definitions: &'a [FxDefinition],
    resource_types: &'a ResourceTypeRegistry,
    view_sources: BTreeMap<ViewId, SourceSpan>,
}

/// Failure to build one complete compiler-owned View product.
#[derive(Debug, Error)]
pub enum ViewProjectLowerError {
    #[error("type checking produced {count} blocking diagnostic(s) before View lowering")]
    UncheckedTypeReport { count: usize },
    #[error("dialogue View model validation failed: {errors:?}")]
    DialogueModels { errors: Vec<DialogueViewModelError> },
    #[error("View `{view}` occurs more than once in the project source inventory")]
    DuplicateViewSource { view: ViewId },
    #[error("View `{view}` has an invalid source range: {source}")]
    InvalidViewSource {
        view: ViewId,
        source: SourceSpanError,
    },
    #[error("View `{value}` has an invalid nominal identity: {source}")]
    InvalidViewIdentity { value: String, source: ViewIdError },
    #[error("image object `{value}` has an invalid nominal identity: {source}")]
    InvalidImageIdentity { value: String, source: IdError },
    #[error(
        "image object `{image}` is owned by both a top-level image declaration and View `{view}`"
    )]
    DuplicateImageObject {
        image: String,
        view: ViewId,
        top_level: Option<SourceSpan>,
        generated: SourceSpan,
    },
    #[error("View-generated image object `{image}` names unknown View owner `{view}`")]
    MissingGeneratedImageOwner { image: String, view: ViewId },
    #[error("HIR project module `{module}` has no matching project source document")]
    MissingProjectSource { module: String },
    #[error("project source module `{module}` has no matching lowered HIR module")]
    MissingHirProjectModule { module: String },
    #[error("project source module `{module}` is bound to {actual:?}, not HIR source {expected:?}")]
    ProjectHirSourceMismatch {
        module: String,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("View lowering requires HIR bound to an exact source document revision")]
    UnboundHirSource,
    #[error("View lowering HIR is bound to {actual:?}, not supplied document {expected:?}")]
    HirSourceMismatch {
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error(transparent)]
    Lower(#[from] ViewSidecarError),
    #[error("View `{view}` lowering failed: {error}")]
    AuthoredViewLower {
        view: ViewId,
        span: SourceSpan,
        #[source]
        error: ViewSidecarError,
    },
    #[error(transparent)]
    Merge(#[from] ViewResourceMergeError),
    #[error(transparent)]
    Product(#[from] ViewProductValidationError),
    #[error(transparent)]
    SourceMap(#[from] SourceMapBuildError),
}

impl CompiledViewProduct {
    /// Shared accepted product used by admission, bundle, runtime, and tooling.
    pub const fn product(&self) -> &Arc<ValidatedViewProduct> {
        &self.product
    }

    /// View-owned text data accepted with the product.
    pub const fn text(&self) -> Option<&ViewTextResource> {
        self.text.as_ref()
    }

    /// View-owned input metadata accepted with the product.
    pub const fn input(&self) -> Option<&ViewInputResource> {
        self.input.as_ref()
    }

    /// Image objects visible to the accepted View catalog.
    pub fn image_objects(&self) -> &[BundleImageObject] {
        &self.image_objects
    }

    /// Exact declaration owner for an accepted View identity.
    pub fn view_source(&self, id: &ViewId) -> Option<&SourceSpan> {
        self.view_sources.get(id)
    }

    /// Exact declaration owner for an accepted Style identity.
    pub fn style_source(&self, id: &ViewStyleSheetId) -> Option<&SourceSpan> {
        self.style_sources.get(id)
    }

    /// Revision of authored project modules before engine-generated standard
    /// View/Style provenance is added to the complete product source set.
    pub const fn authored_source_revision(&self) -> SourceSetRevision {
        self.authored_sources
    }

    /// Complete product revision, including engine-generated standard sources.
    pub fn product_source_revision(&self) -> SourceSetRevision {
        self.product.source_map().source_set_revision()
    }

    /// Digest of the exact resource-type registry used while lowering.
    pub const fn resource_type_registry_digest(&self) -> ResourceTypeRegistryDigest {
        self.resource_types
    }
}

impl ViewProjectLowerError {
    /// Converts a View-product failure without discarding typed source owners.
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::DuplicateImageObject {
                image,
                view,
                top_level,
                generated,
            } => {
                let mut diagnostic = Diagnostic::new(
                    DiagnosticSeverity::Error,
                    format!(
                        "image object `{image}` is owned by both a top-level image declaration and View `{view}`"
                    ),
                )
                .with_code("compiler.view.duplicate_image_object")
                .with_label(DiagnosticLabel::primary(
                    generated.clone(),
                    Some("View materializes this colliding image identity".to_owned()),
                ));
                if let Some(top_level) = top_level {
                    diagnostic = diagnostic.with_label(DiagnosticLabel::secondary(
                        top_level.clone(),
                        Some("top-level image identity is declared here".to_owned()),
                    ));
                }
                diagnostic
            }
            Self::AuthoredViewLower { view, span, error } => Diagnostic::new(
                DiagnosticSeverity::Error,
                format!("View `{view}` lowering failed: {error}"),
            )
            .with_code("compiler.view.lower")
            .with_label(DiagnosticLabel::primary(
                span.clone(),
                Some("this authored View contains the rejected value".to_owned()),
            )),
            _ => Diagnostic::new(DiagnosticSeverity::Error, self.to_string())
                .with_code("compiler.view.lower"),
        }
    }
}

impl<'a> ViewProjectLowerer<'a> {
    /// Creates a lowerer for one exact source document and its linked HIR.
    #[allow(clippy::too_many_arguments)]
    pub fn for_source(
        hir: &'a HirModule,
        typecheck: &'a TypeCheckReport,
        style: &'a CompiledViewStyleArtifact,
        source: &SourceDocument,
        source_image_objects: &'a [BundleImageObject],
        fx_definitions: &'a [FxDefinition],
        resource_types: &'a ResourceTypeRegistry,
    ) -> Result<Self, ViewProjectLowerError> {
        validate_hir_source(hir, source)?;
        let source_map = SourceMapSection::try_from_documents(&[source])?;
        let view_sources = collect_module_view_sources(hir, source)?;
        Ok(Self {
            linked_hir: hir,
            typecheck,
            style,
            source_map,
            source_image_objects,
            source_image_catalog: None,
            fx_definitions,
            resource_types,
            view_sources,
        })
    }

    /// Creates a lowerer from the exact module HIR and documents in one project.
    #[allow(clippy::too_many_arguments)]
    pub fn for_project(
        hir_project: &HirProject,
        linked_hir: &'a HirModule,
        typecheck: &'a TypeCheckReport,
        style: &'a CompiledViewStyleArtifact,
        project: &ProjectSources,
        source_image_catalog: &'a CompiledImageCatalog,
        fx_definitions: &'a [FxDefinition],
        resource_types: &'a ResourceTypeRegistry,
    ) -> Result<Self, ViewProjectLowerError> {
        for source in project.modules() {
            let module = source.module();
            let expected = hir_project.source(module).ok_or_else(|| {
                ViewProjectLowerError::MissingHirProjectModule {
                    module: module.to_string(),
                }
            })?;
            if expected != source.document().identity() {
                return Err(ViewProjectLowerError::ProjectHirSourceMismatch {
                    module: module.to_string(),
                    expected: Box::new(expected.clone()),
                    actual: Box::new(source.document().identity().clone()),
                });
            }
        }
        let source_map = project_source_map(project)?;
        let mut view_sources = BTreeMap::new();
        for (module, hir) in hir_project.modules() {
            let source = project.module(module).ok_or_else(|| {
                ViewProjectLowerError::MissingProjectSource {
                    module: module.to_string(),
                }
            })?;
            merge_view_sources(
                &mut view_sources,
                collect_module_view_sources(hir, source.document())?,
            )?;
        }
        Ok(Self {
            linked_hir,
            typecheck,
            style,
            source_map,
            source_image_objects: source_image_catalog.objects(),
            source_image_catalog: Some(source_image_catalog),
            fx_definitions,
            resource_types,
            view_sources,
        })
    }

    fn validate_authored_views(&self) -> Result<(), ViewProjectLowerError> {
        for view in self.linked_hir.view_declarations() {
            let public_id = view_resource_id(view.id().body());
            let Some(body) = view.view_body() else {
                return Err(ViewSidecarError::RecoveredViewSyntax { view: public_id }.into());
            };
            if body.has_recovery() {
                return Err(ViewSidecarError::RecoveredViewSyntax { view: public_id }.into());
            }
            let Some(signature) = body.signature() else {
                return Err(ViewSidecarError::RecoveredViewSyntax { view: public_id }.into());
            };
            if signature.return_type().is_some() {
                return Err(ViewSidecarError::InvalidViewSignature {
                    view: public_id,
                    message: "View declarations cannot declare a return type".to_owned(),
                }
                .into());
            }
            let Some(body) = body.view() else {
                return Err(ViewSidecarError::RecoveredViewSyntax { view: public_id }.into());
            };
            if body.contains_recovered_syntax() {
                return Err(ViewSidecarError::RecoveredViewSyntax { view: public_id }.into());
            }
        }
        Ok(())
    }

    fn lower_authored_sidecars(
        &self,
        dialogue_view_models: &DialogueViewModelRegistry,
    ) -> Result<ViewBundleSidecars, ViewProjectLowerError> {
        let views = self.linked_hir.view_declarations().collect::<Vec<_>>();
        view_sidecars(
            &views,
            dialogue_view_models,
            self.style.applications(),
            self.source_image_objects,
            self.fx_definitions,
            &self.typecheck.view_part_catalog,
            &self.source_map,
        )
        .map_err(|error| {
            let source = error
                .authored_view()
                .and_then(|value| ViewId::try_new(value.to_owned()).ok())
                .and_then(|view| {
                    self.view_sources
                        .get(&view)
                        .cloned()
                        .map(|span| (view, span))
                });
            match source {
                Some((view, span)) => {
                    ViewProjectLowerError::AuthoredViewLower { view, span, error }
                }
                None => ViewProjectLowerError::Lower(error),
            }
        })
    }

    /// Lowers and validates one atomic View/Style candidate.
    ///
    /// # Panics
    ///
    /// Panics only if an engine-owned standard View/Style identity or its
    /// generated source stops satisfying its compile-time invariants after the
    /// typed standard resources have already been constructed successfully.
    pub fn lower(self) -> Result<CompiledViewProduct, ViewProjectLowerError> {
        self.validate_authored_views()?;
        if !self.typecheck.diagnostics.is_empty() {
            return Err(ViewProjectLowerError::UncheckedTypeReport {
                count: self.typecheck.diagnostics.len(),
            });
        }
        let dialogue_view_models = DialogueViewModelRegistry::from_hir(self.linked_hir)
            .map_err(|errors| ViewProjectLowerError::DialogueModels { errors })?;
        let ViewBundleSidecars {
            program,
            text,
            input,
            image_objects,
        } = self.lower_authored_sidecars(&dialogue_view_models)?;
        reject_image_object_collisions(
            self.source_image_objects,
            &image_objects,
            &self.view_sources,
            self.source_image_catalog,
        )?;
        let authored_style = (!self.style.resource().program.sheets().is_empty()
            || !self.style.resource().program.patches().is_empty())
        .then(|| self.style.resource().clone());
        let resources = ViewProgramStyleResources::new(program, authored_style).merge(
            ViewProgramStyleResources::new(
                Some(standard_view::dialogue_program()),
                Some(standard_view::dialogue_style()),
            ),
        )?;
        let authored_sources = self.source_map.source_set_revision();
        let standard_view_source = standard_view::dialogue_view_source_document();
        let standard_style_source = standard_view::dialogue_style_source_document();
        let source_map = self
            .source_map
            .try_with_document(&standard_view_source)?
            .try_with_document(&standard_style_source)?;
        let product = ValidatedViewProduct::try_new(
            Some(source_map),
            resources.program,
            resources.style,
            ViewProductValidationLimits::default(),
        )?;
        let mut view_sources = self.view_sources;
        let standard_view_id = standard_view::dialogue_view_id();
        let standard_view_span = standard_view_source
            .span(SourceRange::new(0, standard_view_source.text().len()))
            .expect("the generated standard View document owns its complete UTF-8 range");
        if view_sources
            .insert(standard_view_id.clone(), standard_view_span)
            .is_some()
        {
            return Err(ViewProjectLowerError::DuplicateViewSource {
                view: standard_view_id,
            });
        }
        let mut style_sources = self.style.sources().clone();
        let standard_style_id =
            ViewStyleSheetId::try_new_engine_owned(standard_view::DIALOGUE_STYLE_ID)
                .expect("the generated standard Style identity is canonical");
        let standard_style_span = standard_style_source
            .span(SourceRange::new(0, standard_style_source.text().len()))
            .expect("the generated standard Style document owns its complete UTF-8 range");
        if style_sources
            .insert(standard_style_id, standard_style_span)
            .is_some()
        {
            unreachable!("authored Style identities cannot replace the reserved standard Style")
        }
        Ok(CompiledViewProduct {
            product: Arc::new(product),
            text,
            input,
            image_objects,
            view_sources,
            style_sources,
            authored_sources,
            resource_types: self.resource_types.digest(),
        })
    }
}

fn project_source_map(project: &ProjectSources) -> Result<SourceMapSection, SourceMapBuildError> {
    let documents = std::iter::once(project.root_module().document().as_ref())
        .chain(
            project
                .modules()
                .filter(|source| !source.module().is_crate_root())
                .map(|source| source.document().as_ref()),
        )
        .collect::<Vec<_>>();
    SourceMapSection::try_from_documents(&documents)
}

fn reject_image_object_collisions(
    top_level: &[BundleImageObject],
    generated: &[BundleImageObject],
    view_sources: &BTreeMap<ViewId, SourceSpan>,
    catalog: Option<&CompiledImageCatalog>,
) -> Result<(), ViewProjectLowerError> {
    let top_level_ids = top_level
        .iter()
        .map(|object| object.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for object in generated {
        if !top_level_ids.contains(object.id.as_str()) {
            continue;
        }
        let view_value = object.view.as_deref().unwrap_or_default().to_owned();
        let view = ViewId::try_new(view_value.clone()).map_err(|source| {
            ViewProjectLowerError::InvalidViewIdentity {
                value: view_value,
                source,
            }
        })?;
        let generated = view_sources.get(&view).cloned().ok_or_else(|| {
            ViewProjectLowerError::MissingGeneratedImageOwner {
                image: object.id.clone(),
                view: view.clone(),
            }
        })?;
        let typed_image =
            ImageObjectId::new(PublicId::try_new(object.id.clone()).map_err(|source| {
                ViewProjectLowerError::InvalidImageIdentity {
                    value: object.id.clone(),
                    source,
                }
            })?);
        return Err(ViewProjectLowerError::DuplicateImageObject {
            image: object.id.clone(),
            view,
            top_level: catalog
                .and_then(|catalog| catalog.source(&typed_image))
                .cloned(),
            generated,
        });
    }
    Ok(())
}

fn collect_module_view_sources(
    hir: &HirModule,
    source: &SourceDocument,
) -> Result<BTreeMap<ViewId, SourceSpan>, ViewProjectLowerError> {
    validate_hir_source(hir, source)?;
    let mut result = BTreeMap::new();
    for view in hir.view_declarations() {
        let value = view_resource_id(view.id().body());
        let id = ViewId::try_new(value.clone())
            .map_err(|source| ViewProjectLowerError::InvalidViewIdentity { value, source })?;
        let range = view.range();
        let span = source
            .span(SourceRange::new(range.start(), range.end()))
            .map_err(|source| ViewProjectLowerError::InvalidViewSource {
                view: id.clone(),
                source,
            })?;
        if result.insert(id.clone(), span).is_some() {
            return Err(ViewProjectLowerError::DuplicateViewSource { view: id });
        }
    }
    Ok(result)
}

fn validate_hir_source(
    hir: &HirModule,
    source: &SourceDocument,
) -> Result<(), ViewProjectLowerError> {
    let actual = hir
        .source_identity()
        .ok_or(ViewProjectLowerError::UnboundHirSource)?;
    if actual != source.identity() {
        return Err(ViewProjectLowerError::HirSourceMismatch {
            expected: Box::new(source.identity().clone()),
            actual: Box::new(actual.clone()),
        });
    }
    Ok(())
}

fn merge_view_sources(
    target: &mut BTreeMap<ViewId, SourceSpan>,
    sources: BTreeMap<ViewId, SourceSpan>,
) -> Result<(), ViewProjectLowerError> {
    for (view, source) in sources {
        if target.insert(view.clone(), source).is_some() {
            return Err(ViewProjectLowerError::DuplicateViewSource { view });
        }
    }
    Ok(())
}

pub use lowering::ViewSidecarError;
pub use schema::ViewValueCompileError;

#[cfg(test)]
mod tests {
    use super::project_source_map;
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
    use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
    use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::{path::PathBuf, sync::Arc};

    #[test]
    fn project_source_map_keeps_root_as_primary_before_canonical_sorting() {
        let document = |id: &str, name: &str| {
            Arc::new(
                SourceDocument::try_new(
                    SourceDocumentId::try_new(id).expect("source ID"),
                    SourceName::path(name),
                    "view Empty() { Panel() }",
                )
                .expect("source document"),
            )
        };
        let root = document("z-root", "src/main.arcw");
        let child = document("a-child", "src/child.arcw");
        let manifest = document("manifest", "arcw.toml");
        let child_module = CanonicalModulePath::crate_root()
            .join(ModuleSegment::new("child").expect("module segment"));
        let project = ProjectSources::new(
            PathBuf::from("arcw.toml"),
            PathBuf::new(),
            PackageSpec {
                id: PackageId::new("org.arcweft.view-source-map").expect("package ID"),
                version: PackageVersion::new("0.1.0").expect("package version"),
            },
            BuildSpec::default(),
            manifest,
            [
                ProjectSourceFile::new(child_module, PathBuf::from("src/child.arcw"), child, []),
                ProjectSourceFile::new(
                    CanonicalModulePath::crate_root(),
                    PathBuf::from("src/main.arcw"),
                    Arc::clone(&root),
                    [],
                ),
            ],
        )
        .expect("project sources");

        let source_map = project_source_map(&project).expect("source map");
        assert_eq!(source_map.primary_document_id(), Some(root.identity().id()));
    }
}
