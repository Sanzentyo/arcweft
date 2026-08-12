//! Compiler-owned publication of one validated final-HIR View product.
//!
//! The legacy View AST and flattened-HIR readers are deliberately absent.
//! Authored View lowering is admitted only from the accepted arena HIR and its
//! generation-bound semantic analysis.

use std::{collections::BTreeMap, sync::Arc};

use arcweft_bundle::{
    BundleImageObject,
    resource_codec::{
        SourceMapBuildError, SourceMapSection, ValidatedViewProduct, ViewDefinitionResource,
        ViewInputResource, ViewInstructionSpan, ViewParameterResource, ViewProductBuildError,
        ViewProductValidationError, ViewProductValidationLimits, ViewProgramResource,
        ViewProgramStyleResources, ViewResourceMergeError, ViewTextBlockBounds,
        ViewTextBlockResource, ViewTextResource,
        view::{
            DialogueTextProjection, ViewDefinitionRef, ViewParameterRole, ViewProgramInstruction,
            ViewTextSourceKind, ViewTextSourceRecord, ViewTextSurface,
        },
    },
    standard_view,
};
use arcweft_id::DeclarationIdentityFamily;
use arcweft_lang_hir::{
    expr::HirCallArgument,
    identity::{ExprId, ItemId, LocalId},
    item::{HirItemKind, HirPublicIdOrigin, HirRetainedName, HirViewDeclaration},
    leaf::{HirLiteral, HirStringLiteral},
    module::HirModule,
    project::HirProject,
    source_index::{
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirViewSourceRole,
    },
    symbol::ProjectSymbolTable,
};
use arcweft_lang_sema::{
    dialogue_view::{DialogueCharacterProjection, DialogueProjectionCoordinate},
    final_analysis::{
        CheckedBindingRole, CheckedExpressionResolution, CheckedSelectResolution,
        CheckedValueResolution, CheckedViewCall, FinalSemanticAnalysis,
    },
};
use arcweft_project::sources::ProjectSources;
use arcweft_resource_model::registry::{ResourceTypeRegistry, ResourceTypeRegistryDigest};
use arcweft_source::{
    Diagnostic, DiagnosticSeverity, SourceDocumentIdentity, SourceRange, SourceSetRevision,
    SourceSpan,
};
use arcweft_view::{ViewId, ViewProgramId, style::ViewStyleSheetId};
use thiserror::Error;

use crate::style::CompiledViewStyleArtifact;

// Canonical baseline layout retained from the authored-View runtime contract.
// Explicit typed layout/modifier facts will override these values when that
// semantic slice is connected; the compiler never derives them from source
// spelling or text length.
const VIEW_ROOT_X_MILLI: i32 = 48_000;
const VIEW_ROOT_Y_MILLI: i32 = 48_000;
const VIEW_TEXT_WIDTH_MILLI: u32 = 420_000;
const VIEW_TEXT_LINE_HEIGHT_MILLI: u32 = 24_000;
const VIEW_SIBLING_GAP_MILLI: u32 = 16_000;

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

/// Final-HIR inputs for one atomic View-product publication.
pub(crate) struct ViewProjectLowerer<'a> {
    hir_project: &'a HirProject,
    semantic_analysis: &'a FinalSemanticAnalysis,
    style: &'a CompiledViewStyleArtifact,
    source_map: SourceMapSection,
    resource_types: &'a ResourceTypeRegistry,
}

/// Failure to build one complete compiler-owned View product.
#[derive(Debug, Error)]
pub(crate) enum ViewProjectLowerError {
    #[error("project source module `{module}` has no matching lowered HIR module")]
    MissingHirProjectModule { module: String },
    #[error("project source module `{module}` is bound to {actual:?}, not HIR source {expected:?}")]
    ProjectHirSourceMismatch {
        module: String,
        expected: Box<SourceDocumentIdentity>,
        actual: Box<SourceDocumentIdentity>,
    },
    #[error("final-HIR View item {owner:?} has no checked View-product projection")]
    MissingCheckedViewProjection { owner: ItemId },
    #[error("final-HIR View item {owner:?} has no valid View identity")]
    InvalidViewIdentity { owner: ItemId },
    #[error("final-HIR View item {owner:?} is missing source role {role}")]
    MissingViewSource { owner: ItemId, role: &'static str },
    #[error("final-HIR View item {owner:?} has an unsupported parameter at ordinal {ordinal}")]
    InvalidViewParameter { owner: ItemId, ordinal: usize },
    #[error("semantic analysis does not belong to the accepted HIR generation")]
    SemanticGenerationMismatch,
    #[error(transparent)]
    ProductSource(#[from] ViewProductBuildError),
    #[error(transparent)]
    Product(#[from] ViewProductValidationError),
    #[error(transparent)]
    SourceMap(#[from] SourceMapBuildError),
    #[error(transparent)]
    Merge(#[from] ViewResourceMergeError),
}

impl CompiledViewProduct {
    pub const fn product(&self) -> &Arc<ValidatedViewProduct> {
        &self.product
    }

    pub const fn text(&self) -> Option<&ViewTextResource> {
        self.text.as_ref()
    }

    pub const fn input(&self) -> Option<&ViewInputResource> {
        self.input.as_ref()
    }

    pub fn image_objects(&self) -> &[BundleImageObject] {
        &self.image_objects
    }

    pub fn view_source(&self, id: &ViewId) -> Option<&SourceSpan> {
        self.view_sources.get(id)
    }

    pub fn style_source(&self, id: &ViewStyleSheetId) -> Option<&SourceSpan> {
        self.style_sources.get(id)
    }

    pub const fn authored_source_revision(&self) -> SourceSetRevision {
        self.authored_sources
    }

    pub fn product_source_revision(&self) -> SourceSetRevision {
        self.product.source_map().source_set_revision()
    }

    pub const fn resource_type_registry_digest(&self) -> ResourceTypeRegistryDigest {
        self.resource_types
    }
}

impl ViewProjectLowerError {
    pub(crate) fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(DiagnosticSeverity::Error, self.to_string())
            .with_code("compiler.view.lower")
    }
}

impl<'a> ViewProjectLowerer<'a> {
    pub(crate) fn for_project(
        hir_project: &'a HirProject,
        semantic_analysis: &'a FinalSemanticAnalysis,
        symbols: &ProjectSymbolTable,
        style: &'a CompiledViewStyleArtifact,
        project: &ProjectSources,
        resource_types: &'a ResourceTypeRegistry,
    ) -> Result<Self, ViewProjectLowerError> {
        let project_view = hir_project.view();
        for source in project.modules() {
            let module = source.module();
            let expected = project_view.module(module).ok_or_else(|| {
                ViewProjectLowerError::MissingHirProjectModule {
                    module: module.to_string(),
                }
            })?;
            if expected.provenance().source_identity() != source.document().identity() {
                return Err(ViewProjectLowerError::ProjectHirSourceMismatch {
                    module: module.to_string(),
                    expected: Box::new(expected.provenance().source_identity().clone()),
                    actual: Box::new(source.document().identity().clone()),
                });
            }
            semantic_analysis
                .validate_module_generation(expected, symbols)
                .map_err(|_| ViewProjectLowerError::SemanticGenerationMismatch)?;
        }
        let source_map = project_source_map(project)?;
        Ok(Self {
            hir_project,
            semantic_analysis,
            style,
            source_map,
            resource_types,
        })
    }

    pub(crate) fn lower(self) -> Result<CompiledViewProduct, ViewProjectLowerError> {
        let executable = self
            .hir_project
            .executable_view()
            .map_err(|_| ViewProjectLowerError::SemanticGenerationMismatch)?;
        let authored = lower_authored_views(executable, self.semantic_analysis)?;

        let authored_sources = self.source_map.source_set_revision();
        let standard_view_source = standard_view::dialogue_view_source_document();
        let standard_style_source = standard_view::dialogue_style_source_document();
        let source_map = self
            .source_map
            .try_with_document(&standard_view_source)?
            .try_with_document(&standard_style_source)?;
        let resources =
            ViewProgramStyleResources::new(authored.program, Some(self.style.resource().clone()))
                .merge(ViewProgramStyleResources::new(
                Some(standard_view::dialogue_program()),
                Some(standard_view::dialogue_style()),
            ))?;
        let product = ValidatedViewProduct::try_new(
            Some(source_map),
            resources.program,
            resources.style,
            ViewProductValidationLimits::default(),
        )?;

        let standard_view_span = standard_view_source
            .span(SourceRange::new(0, standard_view_source.text().len()))
            .expect("the generated standard View document owns its complete UTF-8 range");
        let standard_style_span = standard_style_source
            .span(SourceRange::new(0, standard_style_source.text().len()))
            .expect("the generated standard Style document owns its complete UTF-8 range");
        let mut view_sources = authored.sources;
        if view_sources
            .insert(standard_view::dialogue_view_id(), standard_view_span)
            .is_some()
        {
            unreachable!("authored View identities cannot replace the reserved standard View")
        }
        let standard_style_id =
            ViewStyleSheetId::try_new_engine_owned(standard_view::DIALOGUE_STYLE_ID)
                .expect("the generated standard Style identity is canonical");
        let mut style_sources = self.style.sources().clone();
        if style_sources
            .insert(standard_style_id, standard_style_span)
            .is_some()
        {
            unreachable!("authored Style identities cannot replace the reserved standard Style")
        }

        let text = merge_view_text(standard_view::dialogue_text(), authored.text);
        Ok(CompiledViewProduct {
            product: Arc::new(product),
            text: Some(text),
            input: None,
            image_objects: Vec::new(),
            view_sources,
            style_sources,
            authored_sources,
            resource_types: self.resource_types.digest(),
        })
    }
}

struct AuthoredViewArtifact {
    program: Option<ViewProgramResource>,
    text: ViewTextResource,
    sources: BTreeMap<ViewId, SourceSpan>,
}

struct AuthoredViewLowering {
    definitions: Vec<ViewDefinitionResource>,
    instructions: Vec<ViewProgramInstruction>,
    text_blocks: Vec<ViewTextBlockResource>,
    text: ViewTextResource,
    sources: BTreeMap<ViewId, SourceSpan>,
}

fn lower_authored_views(
    project: arcweft_lang_hir::project::HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
) -> Result<AuthoredViewArtifact, ViewProjectLowerError> {
    let views = project
        .items()
        .filter_map(|item| match item.item().kind() {
            HirItemKind::View(view) => Some((item, view)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut output = AuthoredViewLowering {
        definitions: Vec::new(),
        instructions: Vec::new(),
        text_blocks: Vec::new(),
        text: ViewTextResource::default(),
        sources: BTreeMap::new(),
    };
    for (item, view) in views {
        lower_authored_view(item.module(), item.id(), view, analysis, &mut output)?;
    }
    let program_id = output.definitions.first().map(|first| {
        ViewProgramId::try_new(format!(
            "view.program.{}",
            first.public_id.view_id().as_str()
        ))
        .expect("an accepted View identity produces a canonical program identity")
    });
    let program = program_id.map(|program_id| ViewProgramResource {
        program_id,
        definitions: output.definitions,
        instructions: output.instructions,
        text_blocks: output.text_blocks,
        ..ViewProgramResource::default()
    });
    Ok(AuthoredViewArtifact {
        program,
        text: output.text,
        sources: output.sources,
    })
}

fn lower_authored_view(
    module: &HirModule,
    owner: ItemId,
    view: &HirViewDeclaration,
    analysis: &FinalSemanticAnalysis,
    output: &mut AuthoredViewLowering,
) -> Result<(), ViewProjectLowerError> {
    if view.header().family() != DeclarationIdentityFamily::View {
        return Err(ViewProjectLowerError::InvalidViewIdentity { owner });
    }
    let public_id = view.header().public_id();
    let view_id = match (public_id.origin(), view.header().name()) {
        (Some(HirPublicIdOrigin::Explicit), _) => ViewId::try_new(
            public_id
                .resolved()
                .ok_or(ViewProjectLowerError::InvalidViewIdentity { owner })?
                .as_str()
                .to_owned(),
        ),
        (Some(HirPublicIdOrigin::DerivedFromName), HirRetainedName::Resolved(name)) => {
            ViewId::try_from_module_name(
                module
                    .key()
                    .path()
                    .segments()
                    .iter()
                    .map(arcweft_lang_syntax::ast::module_path::ModuleSegment::as_str),
                name,
            )
        }
        _ => return Err(ViewProjectLowerError::InvalidViewIdentity { owner }),
    }
    .map_err(|_| ViewProjectLowerError::InvalidViewIdentity { owner })?;
    let source = view_source_span(module, owner, HirViewSourceRole::Whole, "whole declaration")?;
    if output.sources.insert(view_id.clone(), source).is_some() {
        return Err(ViewProjectLowerError::InvalidViewIdentity { owner });
    }
    let mut parameters = BTreeMap::new();
    let parameter_resources = view
        .parameters()
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            if parameter.default().is_some() || parameter.locals().len() != 1 {
                return Err(ViewProjectLowerError::InvalidViewParameter { owner, ordinal });
            }
            let local = parameter.locals()[0];
            let local_fact = analysis
                .local(local)
                .ok_or(ViewProjectLowerError::InvalidViewParameter { owner, ordinal })?;
            let name = module
                .resolve_local(local)
                .map_err(|_| ViewProjectLowerError::InvalidViewParameter { owner, ordinal })?
                .name()
                .as_str()
                .to_owned();
            parameters.insert(local, name.clone());
            Ok(ViewParameterResource {
                ordinal: u16::try_from(ordinal)
                    .map_err(|_| ViewProjectLowerError::InvalidViewParameter { owner, ordinal })?,
                name,
                role: if local_fact.role() == CheckedBindingRole::DialogueViewParameter {
                    ViewParameterRole::Dialogue
                } else {
                    ViewParameterRole::Value
                },
                value_type: None,
                value_slot: None,
                default_program: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let start = u32::try_from(output.instructions.len())
        .map_err(|_| ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
    {
        let mut lowerer = AuthoredViewBodyLowerer {
            module,
            owner,
            analysis,
            parameters: &parameters,
            view: &view_id,
            text_ordinal: 0,
            output,
        };
        for value in view.values() {
            lowerer.lower_value(*value)?;
        }
    }
    let end = u32::try_from(output.instructions.len())
        .map_err(|_| ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
    output.definitions.push(ViewDefinitionResource {
        public_id: ViewDefinitionRef::new(view_id.clone()),
        body: ViewInstructionSpan::new(start, end),
        styles: Vec::new(),
        parameters: parameter_resources,
        state_schema_hash: view_schema_hash(&view_id, &parameters),
    });
    Ok(())
}

struct AuthoredViewBodyLowerer<'a> {
    module: &'a HirModule,
    owner: ItemId,
    analysis: &'a FinalSemanticAnalysis,
    parameters: &'a BTreeMap<LocalId, String>,
    view: &'a ViewId,
    text_ordinal: u32,
    output: &'a mut AuthoredViewLowering,
}

impl AuthoredViewBodyLowerer<'_> {
    fn lower_value(&mut self, value: ExprId) -> Result<(), ViewProjectLowerError> {
        let expression = self.module.resolve_expr(value).map_err(|_| {
            ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner }
        })?;
        let checked = self
            .analysis
            .expression(value)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let (
            arcweft_lang_hir::expr::HirExprKind::Call(call),
            CheckedExpressionResolution::ViewCall(kind),
        ) = (expression.kind(), checked.resolution())
        else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        match kind {
            CheckedViewCall::Element(element) => {
                if !call.arguments().is_empty() {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                }
                self.output
                    .instructions
                    .push(ViewProgramInstruction::OpenElement {
                        element: *element,
                        target: None,
                        styles: Vec::new(),
                        part: None,
                        key: None,
                        source: None,
                    });
                self.output
                    .instructions
                    .push(ViewProgramInstruction::CloseElement);
            }
            CheckedViewCall::Text | CheckedViewCall::RichText => {
                let [HirCallArgument::Positional { .. }] = call.arguments() else {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                };
                let argument = call.arguments()[0].value();
                let surface = if matches!(kind, CheckedViewCall::RichText) {
                    ViewTextSurface::RichText
                } else {
                    ViewTextSurface::Text
                };
                self.lower_text(argument, surface)?;
            }
            CheckedViewCall::Modifier { .. } => {
                return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                    owner: self.owner,
                });
            }
        }
        Ok(())
    }

    fn lower_text(
        &mut self,
        value: ExprId,
        surface: ViewTextSurface,
    ) -> Result<(), ViewProjectLowerError> {
        let owner = self.owner;
        let source_kind = self.text_source_kind(value, surface)?;
        let ordinal = self.text_ordinal;
        self.text_ordinal = self
            .text_ordinal
            .checked_add(1)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
        let text_source = format!("text.{}.{}", self.view.as_str(), ordinal);
        let text_block = format!("text.block.{}.{}", self.view.as_str(), ordinal);
        self.output.text.sources.push(ViewTextSourceRecord {
            public_id: text_source.clone(),
            kind: source_kind,
            source: None,
        });
        self.output
            .instructions
            .push(ViewProgramInstruction::EmitText {
                text_source: text_source.clone(),
                text_block: text_block.clone(),
                styles: Vec::new(),
                part: None,
                source: None,
            });
        self.output.text_blocks.push(
            ViewTextBlockResource::new(
                text_block,
                Some(self.view.as_str().to_owned()),
                None,
                text_source,
                ViewTextBlockBounds::new(
                    VIEW_ROOT_X_MILLI,
                    VIEW_ROOT_Y_MILLI.saturating_add(
                        i32::try_from(ordinal).unwrap_or(i32::MAX).saturating_mul(
                            i32::try_from(
                                VIEW_TEXT_LINE_HEIGHT_MILLI.saturating_add(VIEW_SIBLING_GAP_MILLI),
                            )
                            .expect("canonical View layout increment fits i32"),
                        ),
                    ),
                    VIEW_TEXT_WIDTH_MILLI,
                    VIEW_TEXT_LINE_HEIGHT_MILLI,
                ),
            )
            .with_surface(surface),
        );
        Ok(())
    }

    fn text_source_kind(
        &self,
        value: ExprId,
        surface: ViewTextSurface,
    ) -> Result<ViewTextSourceKind, ViewProjectLowerError> {
        let owner = self.owner;
        let source_kind = match self
            .analysis
            .expression(value)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?
            .resolution()
        {
            CheckedExpressionResolution::Literal(HirLiteral::String(HirStringLiteral::Value(
                value,
            ))) => ViewTextSourceKind::Literal {
                value: value.to_string(),
            },
            CheckedExpressionResolution::Select(CheckedSelectResolution::DialogueView {
                projection,
                ..
            }) => {
                let select = self
                    .module
                    .resolve_expr(value)
                    .ok()
                    .and_then(|expr| match expr.kind() {
                        arcweft_lang_hir::expr::HirExprKind::Select(select) => Some(select),
                        _ => None,
                    })
                    .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
                let target = match projection {
                    DialogueProjectionCoordinate::Character(_) => self
                        .module
                        .resolve_expr(select.target())
                        .ok()
                        .and_then(|expression| match expression.kind() {
                            arcweft_lang_hir::expr::HirExprKind::Select(character) => {
                                Some(character.target())
                            }
                            _ => None,
                        }),
                    _ => Some(select.target()),
                }
                .and_then(|target| self.analysis.expression(target))
                .and_then(|checked| match checked.resolution() {
                    CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)) => {
                        Some(*local)
                    }
                    _ => None,
                })
                .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
                let parameter = self
                    .parameters
                    .get(&target)
                    .cloned()
                    .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
                let projection = match (surface, projection) {
                    (
                        ViewTextSurface::Text,
                        DialogueProjectionCoordinate::Character(
                            DialogueCharacterProjection::DisplayName,
                        ),
                    ) => DialogueTextProjection::CharacterDisplayName,
                    (ViewTextSurface::RichText, DialogueProjectionCoordinate::Content) => {
                        DialogueTextProjection::Content
                    }
                    _ => {
                        return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner });
                    }
                };
                ViewTextSourceKind::Dialogue {
                    parameter,
                    projection,
                }
            }
            _ => return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner }),
        };
        Ok(source_kind)
    }
}

fn view_schema_hash(view: &ViewId, parameters: &BTreeMap<LocalId, String>) -> u64 {
    let mut hasher = blake3::Hasher::new_derive_key("arcweft.view.state-schema.v1");
    hasher.update(view.as_str().as_bytes());
    for name in parameters.values() {
        hasher.update(&[0]);
        hasher.update(name.as_bytes());
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(
        digest.as_bytes()[..8]
            .try_into()
            .expect("BLAKE3 digest has eight bytes"),
    )
}

fn view_source_span(
    module: &HirModule,
    owner: ItemId,
    role: HirViewSourceRole,
    label: &'static str,
) -> Result<SourceSpan, ViewProjectLowerError> {
    let lookup = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Item {
                owner,
                role: HirItemSourceRole::View(role),
            },
        )
        .map_err(|_| ViewProjectLowerError::MissingViewSource { owner, role: label })?;
    match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Ok(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => {
            Err(ViewProjectLowerError::MissingViewSource { owner, role: label })
        }
    }
}

fn merge_view_text(mut standard: ViewTextResource, authored: ViewTextResource) -> ViewTextResource {
    standard.sources.extend(authored.sources);
    standard.localized.extend(authored.localized);
    standard
        .rich_text_documents
        .extend(authored.rich_text_documents);
    standard.display_frames.extend(authored.display_frames);
    standard.source_ranges.extend(authored.source_ranges);
    standard.reveal_policies.extend(authored.reveal_policies);
    standard.cursor_policies.extend(authored.cursor_policies);
    standard.redactions.extend(authored.redactions);
    standard
}

fn project_source_map(project: &ProjectSources) -> Result<SourceMapSection, SourceMapBuildError> {
    let documents = project
        .modules()
        .map(|source| source.document().as_ref())
        .collect::<Vec<_>>();
    SourceMapSection::try_from_documents(&documents)
}
