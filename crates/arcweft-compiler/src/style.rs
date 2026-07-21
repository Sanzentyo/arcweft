//! Compiler-owned lowering of checked View Style data.
//!
//! Semantic checking stays in `arcweft-lang-sema`, canonical Style records
//! stay in `arcweft-view`, and this module adds deterministic bundle
//! source maps plus the typed application lookup consumed by View-program
//! lowering. No source parser is reopened at the bundle or runtime boundary.

use arcweft_bundle::resource_codec::view::ViewStyleResource;
use arcweft_bundle::resource_codec::{ProductSourceRef, SourceMapSection, SourceRangeRef};
use arcweft_id::{IdError, PublicId};
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_hir::project::HirProject;
use arcweft_lang_sema::style::{
    CheckedStyleEnvironmentClause, CheckedStyleEnvironmentPath, CheckedViewStyleCatalog,
    CheckedViewStyleDeclaration, CheckedViewStylePatch, CheckedViewStyleRule,
    CheckedViewStyleSheet,
};
use arcweft_lang_syntax::ast::common::TextRange;
use arcweft_lang_syntax::ast::ids::EntityRefSyntax;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::ast::style::StylePatch as SyntaxStylePatch;
use arcweft_lang_syntax::ast::view::{ViewBody, ViewExpr, ViewModifier, ViewStyleModifier};
use arcweft_project::sources::ProjectSources;
use arcweft_source::{SourceDocument, SourceRange, SourceSpan, SourceSpanError};
use arcweft_view::style::{
    ViewEnvironmentClause, ViewEnvironmentCondition, ViewEnvironmentConditionError,
    ViewEnvironmentWrapperIndex, ViewEnvironmentWrapperSource, ViewStyleApplicationTarget,
    ViewStyleAssignOp, ViewStyleDeclaration, ViewStyleModelError, ViewStylePatch, ViewStylePatchId,
    ViewStyleProgram, ViewStyleRule, ViewStyleSheet, ViewStyleSheetId, ViewStyleSheetIdError,
    ViewStyleSourceId, ViewStyleToken,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Complete compiler-owned Style output for one source or linked project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledViewStyleArtifact {
    resource: ViewStyleResource,
    applications: ViewStyleApplicationLookup,
    sources: BTreeMap<ViewStyleSheetId, SourceSpan>,
}

/// Typed Style applications indexed inside their owning View declaration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ViewStyleApplicationLookup {
    views: BTreeMap<PublicId, ViewStyleViewApplications>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ViewStyleViewApplications {
    root: Vec<ViewStyleApplicationTarget>,
    sites: Vec<ViewStyleApplicationSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ViewStyleApplicationSite {
    range: TextRange,
    applications: Vec<ViewStyleApplicationTarget>,
}

/// Checked Style lowering failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewStyleLowerError {
    #[error("invalid View public ID `{value}`: {source}")]
    InvalidViewId { value: String, source: IdError },
    #[error("invalid Style sheet ID `{value}`: {source}")]
    InvalidSheetId {
        value: String,
        source: ViewStyleSheetIdError,
    },
    #[error("duplicate source origin for Style sheet `{0}`")]
    DuplicateSheetOrigin(String),
    #[error("Style sheet {sheet:?} has an invalid source range: {source}")]
    InvalidSheetSource {
        sheet: ViewStyleSheetId,
        source: SourceSpanError,
    },
    #[error("duplicate Style application inventory for View `{0}`")]
    DuplicateView(String),
    #[error("duplicate Style application site in View `{view}` at {range:?}")]
    DuplicateApplicationSite { view: String, range: TextRange },
    #[error("Style sheet `{0}` has no source origin")]
    MissingSheetOrigin(String),
    #[error("inline Style patch {0} has no source origin")]
    MissingPatchOrigin(u32),
    #[error("project source for module `{0}` is missing")]
    MissingProjectSource(String),
    #[error("HIR project module `{0}` is missing")]
    MissingHirProjectModule(String),
    #[error("Style source range {range:?} exceeds its {source_len}-byte source document")]
    SourceRangeOutOfBounds { range: TextRange, source_len: usize },
    #[error("Style source range {range:?} cannot be represented by the bundle source map")]
    SourceRangeTooLarge { range: TextRange },
    #[error("Style resource contains more source ranges than a u32 ID can address")]
    TooManySourceRanges,
    #[error(transparent)]
    ProductSource(#[from] arcweft_bundle::resource_codec::ViewProductBuildError),
    #[error(transparent)]
    SourceMap(#[from] arcweft_bundle::resource_codec::SourceMapBuildError),
    #[error("View Style application references missing sheet `{sheet}` at {range:?}")]
    UnknownSheetApplication { sheet: String, range: TextRange },
    #[error(
        "View Style application contains invalid sheet reference `{sheet}` at {range:?}: {source}"
    )]
    InvalidSheetApplication {
        sheet: String,
        range: TextRange,
        source: ViewStyleSheetIdError,
    },
    #[error("View Style application in module `{module}` at {range:?} has no checked inline patch")]
    MissingInlinePatch {
        module: CanonicalModulePath,
        range: TextRange,
    },
    #[error("multiple checked inline Style patches share module `{module}` source range {range:?}")]
    DuplicateCheckedInlinePatchRange {
        module: CanonicalModulePath,
        range: TextRange,
    },
    #[error("checked Style catalog contains duplicate inline patch ID {0}")]
    DuplicateCheckedInlinePatchId(u32),
    #[error("linked inline Style patch {0} has no checked catalog entry")]
    MissingCheckedInlinePatch(u32),
    #[error(
        "module-preserving Style patch inventory has {source_count} entries, but linked HIR has {linked_count}"
    )]
    LinkedInlinePatchCountMismatch {
        source_count: usize,
        linked_count: usize,
    },
    #[error(
        "View Style application in module `{module}` at {application:?} does not match checked patch {patch:?}"
    )]
    InlinePatchRangeMismatch {
        module: CanonicalModulePath,
        application: TextRange,
        patch: TextRange,
    },
    #[error("checked Style catalog contains {remaining} unreferenced inline patches")]
    UnreferencedInlinePatches { remaining: usize },
    #[error(transparent)]
    Model(#[from] ViewStyleModelError),
    #[error(transparent)]
    EnvironmentCondition(#[from] ViewEnvironmentConditionError),
    #[error(transparent)]
    Bundle(#[from] arcweft_bundle::resource_codec::SectionCodecError),
}

impl CompiledViewStyleArtifact {
    pub const fn resource(&self) -> &ViewStyleResource {
        &self.resource
    }

    pub const fn applications(&self) -> &ViewStyleApplicationLookup {
        &self.applications
    }

    /// Exact source-bound declaration span for each authored Style sheet.
    pub const fn sources(&self) -> &BTreeMap<ViewStyleSheetId, SourceSpan> {
        &self.sources
    }

    pub fn into_parts(
        self,
    ) -> (
        ViewStyleResource,
        ViewStyleApplicationLookup,
        BTreeMap<ViewStyleSheetId, SourceSpan>,
    ) {
        (self.resource, self.applications, self.sources)
    }
}

impl ViewStyleApplicationLookup {
    /// Applications that establish the owning View's root Style scope.
    pub fn root_applications_for(&self, view: &PublicId) -> &[ViewStyleApplicationTarget] {
        self.views
            .get(view)
            .map_or(&[], |applications| applications.root.as_slice())
    }

    /// Ordered node-local applications for one typed source node.
    pub fn applications_for(
        &self,
        view: &PublicId,
        node_range: TextRange,
    ) -> &[ViewStyleApplicationTarget] {
        self.views
            .get(view)
            .and_then(|applications| {
                applications
                    .sites
                    .iter()
                    .find(|site| site.range == node_range)
            })
            .map_or(&[], |site| site.applications.as_slice())
    }

    pub fn view_ids(&self) -> impl Iterator<Item = &PublicId> {
        self.views.keys()
    }
}

/// Lowers one already typechecked source into canonical Style product data.
pub(crate) fn lower_source_view_styles(
    hir: &HirModule,
    catalog: &CheckedViewStyleCatalog,
    source: &SourceDocument,
) -> Result<CompiledViewStyleArtifact, ViewStyleLowerError> {
    let origins = StyleSourceOrigins::for_source(hir, source)?;
    let module = CanonicalModulePath::crate_root();
    lower_view_styles(&[(module, hir)], hir, catalog, &origins)
}

/// Lowers a linked project while retaining each module's source identity.
pub fn lower_project_view_styles(
    hir_project: &HirProject,
    linked_hir: &HirModule,
    catalog: &CheckedViewStyleCatalog,
    project: &ProjectSources,
) -> Result<CompiledViewStyleArtifact, ViewStyleLowerError> {
    let origins = StyleSourceOrigins::for_project(hir_project, project)?;
    let root = CanonicalModulePath::crate_root();
    let root_hir = hir_project
        .module(&root)
        .ok_or_else(|| ViewStyleLowerError::MissingHirProjectModule(root.to_string()))?;
    let modules = std::iter::once((root.clone(), root_hir))
        .chain(
            hir_project
                .modules()
                .filter(|(path, _)| *path != &root)
                .map(|(path, hir)| (path.clone(), hir)),
        )
        .collect::<Vec<_>>();
    lower_view_styles(&modules, linked_hir, catalog, &origins)
}

fn lower_view_styles(
    modules: &[(CanonicalModulePath, &HirModule)],
    linked_hir: &HirModule,
    catalog: &CheckedViewStyleCatalog,
    origins: &StyleSourceOrigins,
) -> Result<CompiledViewStyleArtifact, ViewStyleLowerError> {
    let resource = lower_style_resource(catalog, origins)?;
    let applications = lower_style_applications(modules, linked_hir, catalog)?;
    Ok(CompiledViewStyleArtifact {
        resource,
        applications,
        sources: origins.sheet_spans.clone(),
    })
}

#[derive(Clone, Debug)]
struct StyleSourceDocument {
    source: ProductSourceRef,
    len: usize,
    document: SourceDocument,
}

#[derive(Default)]
struct StyleSourceOrigins {
    sheets: BTreeMap<ViewStyleSheetId, StyleSourceDocument>,
    sheet_spans: BTreeMap<ViewStyleSheetId, SourceSpan>,
    patches: Vec<StyleSourceDocument>,
}

impl StyleSourceDocument {
    fn new(source: &SourceDocument) -> Result<Self, ViewStyleLowerError> {
        let section = SourceMapSection::try_from_documents(&[source])?;
        let document = section
            .documents()
            .next()
            .expect("a one-document source map retains its input");
        Ok(Self {
            source: ProductSourceRef::from_document(document),
            len: source.text().len(),
            document: source.clone(),
        })
    }
}

impl StyleSourceOrigins {
    fn for_source(hir: &HirModule, source: &SourceDocument) -> Result<Self, ViewStyleLowerError> {
        let document = StyleSourceDocument::new(source)?;
        let mut origins = Self::default();
        origins.register_module(hir, &document)?;
        Ok(origins)
    }

    fn for_project(
        hir_project: &HirProject,
        project: &ProjectSources,
    ) -> Result<Self, ViewStyleLowerError> {
        let root = arcweft_lang_syntax::ast::module_path::CanonicalModulePath::crate_root();
        let mut origins = Self::default();
        origins.register_project_module(&root, hir_project, project)?;
        for (path, _) in hir_project.modules().filter(|(path, _)| *path != &root) {
            origins.register_project_module(path, hir_project, project)?;
        }
        Ok(origins)
    }

    fn register_project_module(
        &mut self,
        path: &arcweft_lang_syntax::ast::module_path::CanonicalModulePath,
        hir_project: &HirProject,
        project: &ProjectSources,
    ) -> Result<(), ViewStyleLowerError> {
        let hir = hir_project
            .module(path)
            .expect("HIR project iteration only returns owned modules");
        let source = project
            .module(path)
            .ok_or_else(|| ViewStyleLowerError::MissingProjectSource(path.to_string()))?;
        self.register_module(hir, &StyleSourceDocument::new(source.document())?)
    }

    fn register_module(
        &mut self,
        hir: &HirModule,
        document: &StyleSourceDocument,
    ) -> Result<(), ViewStyleLowerError> {
        for declaration in hir.declarations() {
            let HirTopLevelDecl::Style(style) = declaration else {
                continue;
            };
            let value = style.id().body().to_owned();
            let id = ViewStyleSheetId::try_new(value.clone())
                .map_err(|source| ViewStyleLowerError::InvalidSheetId { value, source })?;
            if self.sheets.contains_key(&id) {
                return Err(ViewStyleLowerError::DuplicateSheetOrigin(
                    id.public_id().as_str().to_owned(),
                ));
            }
            let range = style.range();
            let span = document
                .document
                .span(SourceRange::new(range.start(), range.end()))
                .map_err(|source| ViewStyleLowerError::InvalidSheetSource {
                    sheet: id.clone(),
                    source,
                })?;
            self.sheets.insert(id.clone(), document.clone());
            self.sheet_spans.insert(id, span);
        }
        self.patches
            .extend(hir.style_patches().iter().map(|_| document.clone()));
        Ok(())
    }
}

struct PendingSourceRange {
    source: ProductSourceRef,
    range: TextRange,
}

#[derive(Default)]
struct StyleSourceRangeBuilder {
    ranges: Vec<PendingSourceRange>,
}

impl StyleSourceRangeBuilder {
    fn add(
        &mut self,
        _owner: &str,
        document: &StyleSourceDocument,
        range: TextRange,
    ) -> Result<ViewStyleSourceId, ViewStyleLowerError> {
        if range.start() > range.end() || range.end() > document.len {
            return Err(ViewStyleLowerError::SourceRangeOutOfBounds {
                range,
                source_len: document.len,
            });
        }
        let id = u32::try_from(self.ranges.len())
            .map(ViewStyleSourceId::new)
            .map_err(|_| ViewStyleLowerError::TooManySourceRanges)?;
        self.ranges.push(PendingSourceRange {
            source: document.source.clone(),
            range,
        });
        Ok(id)
    }

    fn finish(self) -> Result<(Vec<ProductSourceRef>, Vec<SourceRangeRef>), ViewStyleLowerError> {
        let mut source_refs = self
            .ranges
            .iter()
            .map(|pending| pending.source.clone())
            .collect::<Vec<_>>();
        source_refs.sort();
        source_refs.dedup();
        let ranges = self
            .ranges
            .into_iter()
            .map(|pending| {
                let start_byte = u32::try_from(pending.range.start()).map_err(|_| {
                    ViewStyleLowerError::SourceRangeTooLarge {
                        range: pending.range,
                    }
                })?;
                let end_byte = u32::try_from(pending.range.end()).map_err(|_| {
                    ViewStyleLowerError::SourceRangeTooLarge {
                        range: pending.range,
                    }
                })?;
                SourceRangeRef::try_for_source(&source_refs, &pending.source, start_byte, end_byte)
                    .map_err(ViewStyleLowerError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((source_refs, ranges))
    }
}

const VIEW_STYLE_PROGRAM_ID: &str = "view.style.program";

fn lower_style_resource(
    catalog: &CheckedViewStyleCatalog,
    origins: &StyleSourceOrigins,
) -> Result<ViewStyleResource, ViewStyleLowerError> {
    let mut ranges = StyleSourceRangeBuilder::default();
    let sheets = catalog
        .sheets()
        .iter()
        .map(|sheet| lower_sheet(sheet, origins, &mut ranges))
        .collect::<Result<Vec<_>, _>>()?;
    let patches = catalog
        .inline_patches()
        .iter()
        .map(|patch| lower_patch(patch, origins, &mut ranges))
        .collect::<Result<Vec<_>, _>>()?;
    let mut resource = ViewStyleResource {
        style_program_id: VIEW_STYLE_PROGRAM_ID.to_owned(),
        program: ViewStyleProgram::try_new(sheets, patches)?,
        source_refs: Vec::new(),
        source_map_refs: Vec::new(),
        adapter_requirements: Vec::new(),
    };
    (resource.source_refs, resource.source_map_refs) = ranges.finish()?;
    Ok(resource)
}

fn lower_sheet(
    checked: &CheckedViewStyleSheet,
    origins: &StyleSourceOrigins,
    ranges: &mut StyleSourceRangeBuilder,
) -> Result<ViewStyleSheet, ViewStyleLowerError> {
    let owner = checked.id().public_id().as_str();
    let document = origins
        .sheets
        .get(checked.id())
        .ok_or_else(|| ViewStyleLowerError::MissingSheetOrigin(owner.to_owned()))?;
    let tokens = checked
        .tokens()
        .iter()
        .map(|token| {
            ViewStyleToken::new(
                token.id().clone(),
                token.value_kind(),
                token.value().clone(),
                ranges.add(owner, document, token.range())?,
            )
            .map_err(ViewStyleLowerError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rules = checked
        .rules()
        .iter()
        .map(|rule| lower_rule(rule, owner, document, ranges))
        .collect::<Result<Vec<_>, _>>()?;
    ViewStyleSheet::new(checked.id().clone(), tokens, rules).map_err(ViewStyleLowerError::from)
}

fn lower_rule(
    checked: &CheckedViewStyleRule,
    owner: &str,
    document: &StyleSourceDocument,
    ranges: &mut StyleSourceRangeBuilder,
) -> Result<ViewStyleRule, ViewStyleLowerError> {
    let source = ranges.add(owner, document, checked.range())?;
    let environment = checked
        .environment()
        .map(|environment| lower_environment(environment, owner, document, ranges))
        .transpose()?;
    let declarations = checked
        .declarations()
        .iter()
        .map(|declaration| lower_declaration(declaration, owner, document, ranges))
        .collect::<Result<Vec<_>, _>>()?;
    ViewStyleRule::new(
        checked.selector().clone(),
        environment,
        declarations,
        checked.source_order(),
        source,
    )
    .map_err(ViewStyleLowerError::from)
}

fn lower_environment(
    checked: &CheckedStyleEnvironmentPath,
    owner: &str,
    document: &StyleSourceDocument,
    ranges: &mut StyleSourceRangeBuilder,
) -> Result<ViewEnvironmentCondition, ViewStyleLowerError> {
    let wrappers = checked
        .wrappers()
        .iter()
        .map(|wrapper| {
            Ok(ViewEnvironmentWrapperSource::new(
                ranges.add(owner, document, wrapper.predicate_range())?,
                ranges.add(owner, document, wrapper.body_range())?,
                ranges.add(owner, document, wrapper.scope_range())?,
            ))
        })
        .collect::<Result<Vec<_>, ViewStyleLowerError>>()?;
    let clauses = checked
        .clauses()
        .iter()
        .map(|clause| {
            let clause_source = ranges.add(owner, document, clause.range())?;
            let wrapper = ViewEnvironmentWrapperIndex::new(clause.wrapper().value());
            Ok(match clause {
                CheckedStyleEnvironmentClause::ColorScheme { value, .. } => {
                    ViewEnvironmentClause::color_scheme(*value, wrapper, clause_source)
                }
                CheckedStyleEnvironmentClause::Contrast { value, .. } => {
                    ViewEnvironmentClause::contrast(*value, wrapper, clause_source)
                }
                CheckedStyleEnvironmentClause::ReducedMotion { value, .. } => {
                    ViewEnvironmentClause::reduced_motion(*value, wrapper, clause_source)
                }
                CheckedStyleEnvironmentClause::TextScale {
                    comparison, value, ..
                } => ViewEnvironmentClause::text_scale(*comparison, *value, wrapper, clause_source),
            })
        })
        .collect::<Result<Vec<_>, ViewStyleLowerError>>()?;
    ViewEnvironmentCondition::try_new(wrappers, clauses).map_err(ViewStyleLowerError::from)
}

fn lower_declaration(
    checked: &CheckedViewStyleDeclaration,
    owner: &str,
    document: &StyleSourceDocument,
    ranges: &mut StyleSourceRangeBuilder,
) -> Result<ViewStyleDeclaration, ViewStyleLowerError> {
    ViewStyleDeclaration::new(
        checked.property(),
        checked.value().clone(),
        if checked.is_append() {
            ViewStyleAssignOp::Append
        } else {
            ViewStyleAssignOp::Replace
        },
        ranges.add(owner, document, checked.range())?,
    )
    .map_err(ViewStyleLowerError::from)
}

fn lower_patch(
    checked: &CheckedViewStylePatch,
    origins: &StyleSourceOrigins,
    ranges: &mut StyleSourceRangeBuilder,
) -> Result<ViewStylePatch, ViewStyleLowerError> {
    let patch_id = checked.id();
    let patch_index = usize::try_from(patch_id.value())
        .map_err(|_| ViewStyleLowerError::MissingPatchOrigin(patch_id.value()))?;
    let document = origins
        .patches
        .get(patch_index)
        .ok_or(ViewStyleLowerError::MissingPatchOrigin(patch_id.value()))?;
    let declarations = checked
        .declarations()
        .iter()
        .map(|declaration| lower_declaration(declaration, VIEW_STYLE_PROGRAM_ID, document, ranges))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ViewStylePatch::new(patch_id, declarations))
}

fn lower_style_applications(
    modules: &[(CanonicalModulePath, &HirModule)],
    linked_hir: &HirModule,
    catalog: &CheckedViewStyleCatalog,
) -> Result<ViewStyleApplicationLookup, ViewStyleLowerError> {
    let sheets = catalog
        .sheets()
        .iter()
        .map(|sheet| sheet.id().clone())
        .collect::<BTreeSet<_>>();
    let mut patches = CheckedPatchInventory::new(modules, linked_hir, catalog.inline_patches())?;
    let mut views = BTreeMap::new();
    for (module, hir) in modules {
        for declaration in hir.declarations() {
            let HirTopLevelDecl::EntityDecl(entity) = declaration else {
                continue;
            };
            let Some(body) = entity.view_body().and_then(|body| body.view()) else {
                continue;
            };
            let value = entity.id().body().to_owned();
            let view = PublicId::try_new(value.clone())
                .map_err(|source| ViewStyleLowerError::InvalidViewId { value, source })?;
            let applications = lower_view_applications(module, &view, body, &sheets, &mut patches)?;
            if views.insert(view.clone(), applications).is_some() {
                return Err(ViewStyleLowerError::DuplicateView(view.as_str().to_owned()));
            }
        }
    }
    patches.finish()?;
    Ok(ViewStyleApplicationLookup { views })
}

fn lower_view_applications(
    module: &CanonicalModulePath,
    view: &PublicId,
    body: &ViewBody,
    sheets: &BTreeSet<ViewStyleSheetId>,
    patches: &mut CheckedPatchInventory<'_>,
) -> Result<ViewStyleViewApplications, ViewStyleLowerError> {
    let root = body
        .stylesheets()
        .iter()
        .map(|reference| lower_named_application(reference, sheets))
        .collect::<Result<Vec<_>, _>>()?;
    let mut applications = ViewStyleViewApplications {
        root,
        sites: Vec::new(),
    };
    collect_application_sites(
        module,
        view,
        body.value(),
        sheets,
        patches,
        &mut applications,
    )?;
    Ok(applications)
}

fn collect_application_sites(
    module: &CanonicalModulePath,
    view: &PublicId,
    expr: &ViewExpr,
    sheets: &BTreeSet<ViewStyleSheetId>,
    patches: &mut CheckedPatchInventory<'_>,
    output: &mut ViewStyleViewApplications,
) -> Result<(), ViewStyleLowerError> {
    if let Some((range, modifiers)) = producer_style_site(expr) {
        let applications = modifiers
            .iter()
            .filter_map(|modifier| match modifier {
                ViewModifier::Style(style) => Some(style),
                _ => None,
            })
            .map(|style| match style {
                ViewStyleModifier::Named(reference) => lower_named_application(reference, sheets),
                ViewStyleModifier::Inline(patch) => patches.lower_application(module, patch),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !applications.is_empty() {
            if output.sites.iter().any(|site| site.range == range) {
                return Err(ViewStyleLowerError::DuplicateApplicationSite {
                    view: view.as_str().to_owned(),
                    range,
                });
            }
            output.sites.push(ViewStyleApplicationSite {
                range,
                applications,
            });
        }
    }

    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_application_sites(module, view, child, sheets, patches, output)?;
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_application_sites(module, view, child, sheets, patches, output)?;
            }
        }
        ViewExpr::If(branch) => {
            collect_application_sites(module, view, branch.then_branch(), sheets, patches, output)?;
            if let Some(branch) = branch.else_branch() {
                collect_application_sites(module, view, branch, sheets, patches, output)?;
            }
        }
        ViewExpr::Match(branch) => {
            for arm in branch.arms() {
                collect_application_sites(module, view, arm.value(), sheets, patches, output)?;
            }
        }
        ViewExpr::ForEach(loop_expr) => {
            collect_application_sites(module, view, loop_expr.body(), sheets, patches, output)?;
        }
        ViewExpr::Await(await_expr) => {
            for branch in await_expr.branches() {
                collect_application_sites(module, view, branch.value(), sheets, patches, output)?;
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
    Ok(())
}

fn producer_style_site(expr: &ViewExpr) -> Option<(TextRange, &[ViewModifier])> {
    match expr {
        ViewExpr::Element(element) => Some((element.range(), element.modifiers())),
        ViewExpr::ViewCall(call) => Some((call.range(), call.modifiers())),
        ViewExpr::Text(text) => Some((text.range(), text.modifiers())),
        ViewExpr::Image(image) => Some((image.range(), image.modifiers())),
        ViewExpr::TextField(field) => Some((field.range(), field.modifiers())),
        ViewExpr::Button(button) => Some((button.range(), button.modifiers())),
        ViewExpr::Fragment(_)
        | ViewExpr::Let(_)
        | ViewExpr::If(_)
        | ViewExpr::Match(_)
        | ViewExpr::ForEach(_)
        | ViewExpr::Await(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => None,
    }
}

fn lower_named_application(
    reference: &EntityRefSyntax,
    sheets: &BTreeSet<ViewStyleSheetId>,
) -> Result<ViewStyleApplicationTarget, ViewStyleLowerError> {
    let value = reference.canonical_body();
    let id = ViewStyleSheetId::try_new(value.clone()).map_err(|source| {
        ViewStyleLowerError::InvalidSheetApplication {
            sheet: value.clone(),
            range: *reference.range(),
            source,
        }
    })?;
    if !sheets.contains(&id) {
        return Err(ViewStyleLowerError::UnknownSheetApplication {
            sheet: value,
            range: *reference.range(),
        });
    }
    Ok(ViewStyleApplicationTarget::named(id))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StylePatchSiteKey {
    module: CanonicalModulePath,
    start: usize,
    end: usize,
}

struct CheckedPatchInventory<'a> {
    by_site: BTreeMap<StylePatchSiteKey, &'a CheckedViewStylePatch>,
}

impl<'a> CheckedPatchInventory<'a> {
    fn new(
        modules: &[(CanonicalModulePath, &HirModule)],
        linked_hir: &HirModule,
        patches: &'a [CheckedViewStylePatch],
    ) -> Result<Self, ViewStyleLowerError> {
        let mut checked_by_id = BTreeMap::new();
        for patch in patches {
            if checked_by_id.insert(patch.id(), patch).is_some() {
                return Err(ViewStyleLowerError::DuplicateCheckedInlinePatchId(
                    patch.id().value(),
                ));
            }
        }

        let source_sites = modules
            .iter()
            .flat_map(|(module, hir)| hir.style_patches().iter().map(move |patch| (module, patch)))
            .collect::<Vec<_>>();
        let linked_patches = linked_hir.style_patches();
        if source_sites.len() != linked_patches.len() {
            return Err(ViewStyleLowerError::LinkedInlinePatchCountMismatch {
                source_count: source_sites.len(),
                linked_count: linked_patches.len(),
            });
        }

        let mut by_site = BTreeMap::new();
        for ((module, source_patch), linked_patch) in source_sites.into_iter().zip(linked_patches) {
            if source_patch.range() != linked_patch.range() {
                return Err(ViewStyleLowerError::InlinePatchRangeMismatch {
                    module: module.clone(),
                    application: source_patch.range(),
                    patch: linked_patch.range(),
                });
            }
            let patch_id = ViewStylePatchId::new(linked_patch.ordinal());
            let checked = checked_by_id.remove(&patch_id).ok_or(
                ViewStyleLowerError::MissingCheckedInlinePatch(patch_id.value()),
            )?;
            if checked.range() != linked_patch.range() {
                return Err(ViewStyleLowerError::InlinePatchRangeMismatch {
                    module: module.clone(),
                    application: linked_patch.range(),
                    patch: checked.range(),
                });
            }
            let key = StylePatchSiteKey::new(module.clone(), source_patch.range());
            if by_site.insert(key, checked).is_some() {
                return Err(ViewStyleLowerError::DuplicateCheckedInlinePatchRange {
                    module: module.clone(),
                    range: source_patch.range(),
                });
            }
        }
        if !checked_by_id.is_empty() {
            return Err(ViewStyleLowerError::UnreferencedInlinePatches {
                remaining: checked_by_id.len(),
            });
        }
        Ok(Self { by_site })
    }

    fn lower_application(
        &mut self,
        module: &CanonicalModulePath,
        application: &SyntaxStylePatch,
    ) -> Result<ViewStyleApplicationTarget, ViewStyleLowerError> {
        let range = application.range();
        let checked = self
            .by_site
            .remove(&StylePatchSiteKey::new(module.clone(), range))
            .ok_or_else(|| ViewStyleLowerError::MissingInlinePatch {
                module: module.clone(),
                range,
            })?;
        Ok(ViewStyleApplicationTarget::inline(checked.id()))
    }

    fn finish(self) -> Result<(), ViewStyleLowerError> {
        if self.by_site.is_empty() {
            Ok(())
        } else {
            Err(ViewStyleLowerError::UnreferencedInlinePatches {
                remaining: self.by_site.len(),
            })
        }
    }
}

impl StylePatchSiteKey {
    fn new(module: CanonicalModulePath, range: TextRange) -> Self {
        Self {
            module,
            start: range.start(),
            end: range.end(),
        }
    }
}
