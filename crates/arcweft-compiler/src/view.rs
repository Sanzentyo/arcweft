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
        ViewTextBlockResource, ViewTextResource, ViewValueInputResource,
        view::{
            DialogueTextProjection, ViewActionButtonActionResource, ViewActionButtonResource,
            ViewDefinitionRef, ViewFxArgumentBindingRef, ViewFxArgumentSourceRef,
            ViewParameterRole, ViewProgramInstruction, ViewRuntimeButtonBounds, ViewTextSourceKind,
            ViewTextSourceRecord, ViewTextSurface, ViewValueInputNamespace, ViewValueInputSource,
        },
    },
    standard_view,
};
use arcweft_id::DeclarationIdentityFamily;
use arcweft_lang_hir::{
    expr::HirCallArgument,
    identity::{CaptureId, ExprId, ItemId, LocalId},
    item::{HirItemKind, HirPublicIdOrigin, HirRetainedName, HirViewDeclaration},
    leaf::{HirLiteral, HirStringLiteral},
    module::HirModule,
    project::HirProject,
    scope::CaptureAccess,
    source_index::{
        HirItemSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirViewSourceRole,
    },
    symbol::ProjectSymbolTable,
};
use arcweft_lang_sema::{
    CheckedOwnershipError, CheckedOwnershipLimits,
    callable::{
        CallableValidator, CheckedCallApplicationDigest, CheckedCallArgumentSlotSource,
        CheckedCallReceiverProjection, CheckedCallSite,
    },
    dialogue_view::{DialogueCharacterProjection, DialogueProjectionCoordinate},
    effect_row::EffectRow,
    final_analysis::{
        CheckedBindingRole, CheckedExpressionResolution, CheckedFxBindingDecision,
        CheckedSelectResolution, CheckedValueResolution, CheckedViewCall, CheckedViewFxApplication,
        CheckedViewFxBinding, CheckedViewValueProgram, FinalSemanticAnalysis,
    },
    registration::RegisteredSemanticWorld,
    types::TypeKind,
};
use arcweft_presentation::fx::{
    FxDefinitionParameterType, FxId, FxRuntimeType, ValueInstruction, ValueProgramSchema,
    ValueProgramValidationError,
};
use arcweft_project::sources::ProjectSources;
use arcweft_resource_model::registry::{ResourceTypeRegistry, ResourceTypeRegistryDigest};
use arcweft_source::{
    Diagnostic, DiagnosticSeverity, SourceDocumentIdentity, SourceRange, SourceSetRevision,
    SourceSpan,
};
use arcweft_view::{
    ViewHandlerCapture, ViewHandlerProgramId, ViewHandlerResult, ViewHandlerValueTypeId, ViewId,
    ViewParameterCoordinate, ViewProgramId, ViewValueProgram, ViewValueProgramId,
    style::ViewStyleSheetId,
};
use thiserror::Error;

use crate::{fx_catalog::CompiledFxCatalog, style::CompiledViewStyleArtifact};

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
    handler_programs: Arc<[CheckedViewHandlerProgram]>,
}

/// Compiler-private checked owner of one mount-time View handler program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedViewHandlerProgram {
    id: ViewHandlerProgramId,
    event: arcweft_view::EventKind,
    closure: ExprId,
    body: ExprId,
    captures: Box<[CheckedViewHandlerCapture]>,
    result: ViewHandlerResult,
}

/// Join between one checked closure capture and its View parameter coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CheckedViewHandlerCapture {
    capture: CaptureId,
    local: LocalId,
    schema: ViewHandlerCapture,
}

/// Final-HIR inputs for one atomic View-product publication.
pub(crate) struct ViewProjectLowerer<'a> {
    hir_project: &'a HirProject,
    semantic_analysis: &'a FinalSemanticAnalysis,
    registered_world: &'a RegisteredSemanticWorld,
    style: &'a CompiledViewStyleArtifact,
    fx_catalog: &'a CompiledFxCatalog,
    source_map: SourceMapSection,
    resource_types: &'a ResourceTypeRegistry,
}

/// Failure to build one complete compiler-owned View product.
#[derive(Debug, Error)]
pub(crate) enum ViewProjectLowerError {
    #[error(transparent)]
    GenericScope(#[from] arcweft_lang_sema::types::GenericScopeError),
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
    #[error("checked View Fx application {expression:?} is inconsistent with its final authority")]
    InvalidViewFxApplication { expression: ExprId },
    #[error(
        "checked View Fx application {expression:?} references missing compiled definition `{definition}`"
    )]
    MissingCompiledFxDefinition {
        expression: ExprId,
        definition: FxId,
    },
    #[error("checked View value program {expression:?} is invalid after global input projection")]
    InvalidViewValueProgram {
        expression: ExprId,
        #[source]
        source: ValueProgramValidationError,
    },
    #[error("checked View value input count {actual} exceeds the u16 slot domain")]
    TooManyViewValueInputs { actual: usize },
    #[error("semantic analysis does not belong to the accepted HIR generation")]
    SemanticGenerationMismatch,
    #[error("View handler {owner:?} capture {capture:?} is not snapshot-retainable")]
    InvalidViewHandlerCaptureOwnership {
        owner: ItemId,
        capture: CaptureId,
        #[source]
        source: CheckedOwnershipError,
    },
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

    pub(crate) fn handler_programs(&self) -> &[CheckedViewHandlerProgram] {
        &self.handler_programs
    }
}

impl CheckedViewHandlerProgram {
    pub(crate) const fn id(&self) -> ViewHandlerProgramId {
        self.id
    }

    pub(crate) const fn closure(&self) -> ExprId {
        self.closure
    }

    pub(crate) const fn body(&self) -> ExprId {
        self.body
    }

    pub(crate) const fn captures(&self) -> &[CheckedViewHandlerCapture] {
        &self.captures
    }

    pub(crate) const fn result(&self) -> ViewHandlerResult {
        self.result
    }
}

impl CheckedViewHandlerCapture {
    pub(crate) const fn capture(self) -> CaptureId {
        self.capture
    }

    pub(crate) const fn local(self) -> LocalId {
        self.local
    }

    pub(crate) const fn schema(self) -> ViewHandlerCapture {
        self.schema
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
        registered_world: &'a RegisteredSemanticWorld,
        style: &'a CompiledViewStyleArtifact,
        fx_catalog: &'a CompiledFxCatalog,
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
            registered_world,
            style,
            fx_catalog,
            source_map,
            resource_types,
        })
    }

    pub(crate) fn lower(self) -> Result<CompiledViewProduct, ViewProjectLowerError> {
        let executable = self
            .hir_project
            .executable_view()
            .map_err(|_| ViewProjectLowerError::SemanticGenerationMismatch)?;
        let authored = lower_authored_views(
            executable,
            self.semantic_analysis,
            self.registered_world,
            self.fx_catalog,
        )?;

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
            handler_programs: authored.handlers.into(),
        })
    }
}

struct AuthoredViewArtifact {
    program: Option<ViewProgramResource>,
    text: ViewTextResource,
    sources: BTreeMap<ViewId, SourceSpan>,
    handlers: Vec<CheckedViewHandlerProgram>,
}

struct AuthoredViewLowering {
    definitions: Vec<ViewDefinitionResource>,
    value_programs: Vec<ViewValueProgram>,
    value_inputs: Vec<ViewValueInputResource>,
    global_value_inputs:
        BTreeMap<(ViewDefinitionRef, ViewParameterCoordinate), GlobalViewValueInput>,
    global_parameter_types: Vec<FxRuntimeType>,
    instructions: Vec<ViewProgramInstruction>,
    text_blocks: Vec<ViewTextBlockResource>,
    action_buttons: Vec<ViewActionButtonResource>,
    text: ViewTextResource,
    sources: BTreeMap<ViewId, SourceSpan>,
    handlers: Vec<CheckedViewHandlerProgram>,
}

struct PreparedAuthoredView<'a> {
    module: &'a HirModule,
    owner: ItemId,
    declaration: &'a HirViewDeclaration,
    id: ViewId,
    parameters: BTreeMap<LocalId, CheckedViewParameter>,
    parameter_resources: Vec<ViewParameterResource>,
    source: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalViewValueInput {
    slot: u16,
    value_type: FxRuntimeType,
}

fn collect_view_fx_inputs(
    owner: ItemId,
    root: ExprId,
    analysis: &FinalSemanticAnalysis,
    view: &ViewDefinitionRef,
    inputs: &mut BTreeMap<(ViewDefinitionRef, ViewParameterCoordinate), FxRuntimeType>,
) -> Result<(), ViewProjectLowerError> {
    let mut expression = root;
    loop {
        let checked = analysis
            .expression(expression)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
        if let CheckedExpressionResolution::ViewFxApplication(application) = checked.resolution() {
            for argument in application.arguments() {
                let CheckedFxBindingDecision::Explicit(CheckedViewFxBinding::Reactive(program)) =
                    argument.decision()
                else {
                    continue;
                };
                if program.inputs().len() != program.program().schema().parameter_types().len() {
                    return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
                }
                for input in program.inputs() {
                    let key = (view.clone(), input.parameter());
                    match inputs.entry(key) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(input.value_type());
                        }
                        std::collections::btree_map::Entry::Occupied(entry)
                            if *entry.get() == input.value_type() => {}
                        std::collections::btree_map::Entry::Occupied(_) => {
                            return Err(ViewProjectLowerError::InvalidViewFxApplication {
                                expression,
                            });
                        }
                    }
                }
            }
        }
        if !matches!(
            checked.resolution(),
            CheckedExpressionResolution::ViewFxApplication(_) | CheckedExpressionResolution::Call
        ) {
            return Ok(());
        }
        let projection = checked_view_modifier_projection(analysis, owner, expression)?;
        if projection.receiver == expression {
            return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
        }
        expression = projection.receiver;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedViewModifierProjection {
    application: CheckedCallApplicationDigest,
    receiver: ExprId,
}

fn checked_view_modifier_projection(
    analysis: &FinalSemanticAnalysis,
    owner: ItemId,
    expression: ExprId,
) -> Result<CheckedViewModifierProjection, ViewProjectLowerError> {
    let application = analysis
        .call(expression)
        .and_then(arcweft_lang_sema::callable::CallTargetFacts::selected_application)
        .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner })?;
    if application.core().application_site().raw() != CheckedCallSite::HirCall(expression) {
        return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner });
    }
    let CheckedCallReceiverProjection::Operand { source, .. } =
        application.core().execution().receiver()
    else {
        return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner });
    };
    let CheckedCallArgumentSlotSource::Expression(receiver) = source.raw() else {
        return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner });
    };
    if let Some(fx) =
        analysis
            .expression(expression)
            .and_then(|checked| match checked.resolution() {
                CheckedExpressionResolution::ViewFxApplication(application) => Some(application),
                _ => None,
            })
    {
        if fx.outer_application() != application.digest()
            || fx.receiver_expression() != Some(receiver)
        {
            return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
        }
    }
    Ok(CheckedViewModifierProjection {
        application: application.digest(),
        receiver,
    })
}

fn lower_authored_views(
    project: arcweft_lang_hir::project::HirExecutableProjectView<'_>,
    analysis: &FinalSemanticAnalysis,
    registered_world: &RegisteredSemanticWorld,
    fx_catalog: &CompiledFxCatalog,
) -> Result<AuthoredViewArtifact, ViewProjectLowerError> {
    let mut views = project
        .items()
        .filter_map(|item| match item.item().kind() {
            HirItemKind::View(view) => Some(prepare_authored_view(
                item.module(),
                item.id(),
                view,
                analysis,
            )),
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut input_types = BTreeMap::new();
    for view in &views {
        let definition = ViewDefinitionRef::new(view.id.clone());
        for value in view.declaration.values() {
            collect_view_fx_inputs(view.owner, *value, analysis, &definition, &mut input_types)?;
        }
    }
    let global_value_inputs = input_types
        .into_iter()
        .enumerate()
        .map(|(slot, (key, value_type))| {
            let slot = u16::try_from(slot)
                .map_err(|_| ViewProjectLowerError::TooManyViewValueInputs { actual: slot + 1 })?;
            Ok::<_, ViewProjectLowerError>((key, GlobalViewValueInput { slot, value_type }))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for view in &mut views {
        let definition = ViewDefinitionRef::new(view.id.clone());
        for (ordinal, parameter) in view.parameter_resources.iter_mut().enumerate() {
            let coordinate = ViewParameterCoordinate::try_from_index(ordinal).ok_or(
                ViewProjectLowerError::InvalidViewParameter {
                    owner: view.owner,
                    ordinal,
                },
            )?;
            if let Some(input) = global_value_inputs.get(&(definition.clone(), coordinate)) {
                if parameter.role == ViewParameterRole::Dialogue {
                    return Err(ViewProjectLowerError::InvalidViewParameter {
                        owner: view.owner,
                        ordinal,
                    });
                }
                parameter.value_type = Some(input.value_type);
                parameter.value_slot = Some(input.slot);
            }
        }
    }
    let global_parameter_types = global_value_inputs
        .values()
        .map(|input| input.value_type)
        .collect::<Vec<_>>();
    let value_inputs = global_value_inputs
        .iter()
        .map(|((view, parameter), input)| ViewValueInputResource {
            namespace: ViewValueInputNamespace::Parameter,
            slot: input.slot,
            value_type: input.value_type,
            source: ViewValueInputSource::DefinitionParameter {
                view: view.clone(),
                parameter: *parameter,
            },
        })
        .collect();
    let mut output = AuthoredViewLowering {
        definitions: Vec::new(),
        value_programs: Vec::new(),
        value_inputs,
        global_value_inputs,
        global_parameter_types,
        instructions: Vec::new(),
        text_blocks: Vec::new(),
        action_buttons: Vec::new(),
        text: ViewTextResource::default(),
        sources: BTreeMap::new(),
        handlers: Vec::new(),
    };
    for view in &views {
        lower_authored_view(view, analysis, registered_world, fx_catalog, &mut output)?;
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
        value_programs: output.value_programs,
        value_inputs: output.value_inputs,
        instructions: output.instructions,
        handlers: output
            .handlers
            .iter()
            .map(
                |handler| arcweft_bundle::resource_codec::view::ViewHandlerRef {
                    program: handler.id,
                    captures: handler
                        .captures
                        .iter()
                        .map(|capture| capture.schema)
                        .collect(),
                    result: handler.result,
                },
            )
            .collect(),
        text_blocks: output.text_blocks,
        action_buttons: output.action_buttons,
        ..ViewProgramResource::default()
    });
    Ok(AuthoredViewArtifact {
        program,
        text: output.text,
        sources: output.sources,
        handlers: output.handlers,
    })
}

fn prepare_authored_view<'a>(
    module: &'a HirModule,
    owner: ItemId,
    view: &'a HirViewDeclaration,
    analysis: &FinalSemanticAnalysis,
) -> Result<PreparedAuthoredView<'a>, ViewProjectLowerError> {
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
            let coordinate = ViewParameterCoordinate::try_from_index(ordinal)
                .ok_or(ViewProjectLowerError::InvalidViewParameter { owner, ordinal })?;
            let semantic_type = ViewHandlerValueTypeId::from_semantic_digest(
                *local_fact.ty().semantic_identity_digest()?.as_bytes(),
            );
            parameters.insert(
                local,
                CheckedViewParameter {
                    coordinate,
                    name: name.clone(),
                    value_type: semantic_type,
                },
            );
            Ok(ViewParameterResource {
                ordinal: u16::try_from(ordinal)
                    .map_err(|_| ViewProjectLowerError::InvalidViewParameter { owner, ordinal })?,
                name,
                role: if local_fact.role() == CheckedBindingRole::DialogueViewParameter {
                    ViewParameterRole::Dialogue
                } else {
                    ViewParameterRole::Value
                },
                semantic_type,
                value_type: None,
                value_slot: None,
                default_program: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedAuthoredView {
        module,
        owner,
        declaration: view,
        id: view_id,
        parameters,
        parameter_resources,
        source,
    })
}

fn lower_authored_view(
    view: &PreparedAuthoredView<'_>,
    analysis: &FinalSemanticAnalysis,
    registered_world: &RegisteredSemanticWorld,
    fx_catalog: &CompiledFxCatalog,
    output: &mut AuthoredViewLowering,
) -> Result<(), ViewProjectLowerError> {
    if output
        .sources
        .insert(view.id.clone(), view.source.clone())
        .is_some()
    {
        return Err(ViewProjectLowerError::InvalidViewIdentity { owner: view.owner });
    }
    let start = u32::try_from(output.instructions.len())
        .map_err(|_| ViewProjectLowerError::MissingCheckedViewProjection { owner: view.owner })?;
    {
        let mut lowerer = AuthoredViewBodyLowerer {
            module: view.module,
            owner: view.owner,
            analysis,
            registered_world,
            fx_catalog,
            parameters: &view.parameters,
            view: &view.id,
            text_ordinal: 0,
            element_ordinal: 0,
            output,
        };
        for value in view.declaration.values() {
            lowerer.lower_value(*value)?;
        }
    }
    let end = u32::try_from(output.instructions.len())
        .map_err(|_| ViewProjectLowerError::MissingCheckedViewProjection { owner: view.owner })?;
    output.definitions.push(ViewDefinitionResource {
        public_id: ViewDefinitionRef::new(view.id.clone()),
        body: ViewInstructionSpan::new(start, end),
        styles: Vec::new(),
        parameters: view.parameter_resources.clone(),
        state_schema_hash: view_schema_hash(&view.id, &view.parameters),
    });
    Ok(())
}

struct AuthoredViewBodyLowerer<'a> {
    module: &'a HirModule,
    owner: ItemId,
    analysis: &'a FinalSemanticAnalysis,
    registered_world: &'a RegisteredSemanticWorld,
    fx_catalog: &'a CompiledFxCatalog,
    parameters: &'a BTreeMap<LocalId, CheckedViewParameter>,
    view: &'a ViewId,
    text_ordinal: u32,
    element_ordinal: u32,
    output: &'a mut AuthoredViewLowering,
}

#[derive(Clone, Debug)]
struct CheckedViewParameter {
    coordinate: ViewParameterCoordinate,
    name: String,
    value_type: ViewHandlerValueTypeId,
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
        let arcweft_lang_hir::expr::HirExprKind::Call(call) = expression.kind() else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let CheckedExpressionResolution::ViewCall(kind) = checked.resolution() else {
            return match checked.resolution() {
                CheckedExpressionResolution::ViewFxApplication(application) => {
                    self.lower_fx_modifier(value, application)
                }
                CheckedExpressionResolution::Call => self.lower_handler_modifier(value, call),
                _ => Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner }),
            };
        };
        match kind {
            CheckedViewCall::Element(element) => {
                if !call.arguments().is_empty() {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                }
                let ordinal = self.element_ordinal;
                self.element_ordinal = self.element_ordinal.checked_add(1).ok_or(
                    ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner },
                )?;
                let target = format!("node.{}.{}", self.view.as_str(), ordinal);
                self.output
                    .instructions
                    .push(ViewProgramInstruction::OpenElement {
                        element: *element,
                        target: Some(target.clone()),
                        styles: Vec::new(),
                        part: None,
                        key: None,
                        source: None,
                    });
                if element.is_action_control() {
                    let label = format!("button.label.{}.{}", self.view.as_str(), ordinal);
                    self.output.text.sources.push(ViewTextSourceRecord {
                        public_id: label.clone(),
                        kind: ViewTextSourceKind::Literal {
                            value: String::new(),
                        },
                        source: None,
                    });
                    self.output.action_buttons.push(ViewActionButtonResource {
                        public_id: target.clone(),
                        view: Some(self.view.as_str().to_owned()),
                        containing_scroll_region: None,
                        label_text_source: label,
                        enabled: true,
                        action: ViewActionButtonActionResource::Noop,
                        bounds: ViewRuntimeButtonBounds::new(
                            VIEW_ROOT_X_MILLI,
                            VIEW_ROOT_Y_MILLI,
                            VIEW_TEXT_WIDTH_MILLI,
                            VIEW_TEXT_LINE_HEIGHT_MILLI,
                        ),
                        source: None,
                    });
                }
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
        }
        Ok(())
    }

    fn lower_fx_modifier(
        &mut self,
        expression: ExprId,
        application: &CheckedViewFxApplication,
    ) -> Result<(), ViewProjectLowerError> {
        let definition_id = application.definition().definition();
        let definition = self.fx_catalog.get(definition_id).ok_or_else(|| {
            ViewProjectLowerError::MissingCompiledFxDefinition {
                expression,
                definition: definition_id.clone(),
            }
        })?;
        if definition.id() != definition_id
            || definition.parameter_layout().digest() != application.definition().layout()
        {
            return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
        }

        let mut arguments = Vec::new();
        for argument in application.arguments() {
            let abi_parameter = self
                .analysis
                .checked_fx_definitions()
                .abi_parameter(application.definition(), argument.parameter())
                .map_err(|_| ViewProjectLowerError::InvalidViewFxApplication { expression })?;
            let CheckedFxBindingDecision::Explicit(binding) = argument.decision() else {
                continue;
            };
            match binding {
                CheckedViewFxBinding::Closed(binding) => {
                    let Some(value) = binding.abi_value() else {
                        if abi_parameter.is_some() {
                            return Err(ViewProjectLowerError::InvalidViewFxApplication {
                                expression,
                            });
                        }
                        continue;
                    };
                    let parameter = abi_parameter
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    let definition_parameter = definition
                        .parameters()
                        .get(usize::from(parameter.get()))
                        .filter(|row| row.index() == parameter)
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    if definition_parameter.parameter_type() != value.parameter_type() {
                        return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
                    }
                    arguments.push(ViewFxArgumentBindingRef {
                        parameter,
                        source: ViewFxArgumentSourceRef::Closed(value.clone()),
                    });
                }
                CheckedViewFxBinding::Reactive(program) => {
                    let parameter = abi_parameter
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    let definition_parameter = definition
                        .parameters()
                        .get(usize::from(parameter.get()))
                        .filter(|row| row.index() == parameter)
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    if definition_parameter.parameter_type()
                        != FxDefinitionParameterType::Runtime(program.return_type())
                    {
                        return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
                    }
                    let program = self.lower_view_value_program(expression, program)?;
                    arguments.push(ViewFxArgumentBindingRef {
                        parameter,
                        source: ViewFxArgumentSourceRef::Reactive(program),
                    });
                }
            }
        }
        arguments.sort_by_key(|argument| argument.parameter);
        if arguments
            .windows(2)
            .any(|pair| pair[0].parameter == pair[1].parameter)
        {
            return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
        }

        let projection = checked_view_modifier_projection(self.analysis, self.owner, expression)?;
        if projection.application != application.outer_application() {
            return Err(ViewProjectLowerError::InvalidViewFxApplication { expression });
        }
        self.lower_value(projection.receiver)?;
        self.output
            .instructions
            .push(ViewProgramInstruction::ApplyFx {
                fx: definition_id.clone(),
                parameter_layout: application.definition().layout(),
                arguments,
                key_program: None,
                application_ordinal: application.ordinal().get(),
                source: None,
            });
        Ok(())
    }

    fn lower_view_value_program(
        &mut self,
        expression: ExprId,
        checked: &CheckedViewValueProgram,
    ) -> Result<ViewValueProgramId, ViewProjectLowerError> {
        let id = ViewValueProgramId(
            u32::try_from(self.output.value_programs.len())
                .map_err(|_| ViewProjectLowerError::InvalidViewFxApplication { expression })?,
        );
        let schema = ValueProgramSchema::new(
            self.output.global_parameter_types.clone(),
            Vec::new(),
            checked.return_type(),
        );
        let view = ViewDefinitionRef::new(self.view.clone());
        let mut instructions = Vec::with_capacity(checked.program().instructions().len());
        for instruction in checked.program().instructions() {
            let instruction = match instruction {
                ValueInstruction::LoadParameter { parameter } => {
                    let input = checked
                        .inputs()
                        .get(usize::from(parameter.slot().get()))
                        .filter(|input| input.value_type() == parameter.runtime_type())
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    let global = self
                        .output
                        .global_value_inputs
                        .get(&(view.clone(), input.parameter()))
                        .filter(|global| global.value_type == input.value_type())
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    let parameter = schema
                        .parameter_ref(usize::from(global.slot))
                        .filter(|parameter| parameter.runtime_type() == input.value_type())
                        .ok_or(ViewProjectLowerError::InvalidViewFxApplication { expression })?;
                    ValueInstruction::LoadParameter { parameter }
                }
                instruction => instruction.clone(),
            };
            instructions.push(instruction);
        }
        let program = ViewValueProgram::validate(id, schema, instructions).map_err(|source| {
            ViewProjectLowerError::InvalidViewValueProgram { expression, source }
        })?;
        self.output.value_programs.push(program);
        Ok(id)
    }

    fn lower_handler_modifier(
        &mut self,
        owner: ExprId,
        call: &arcweft_lang_hir::expr::HirCallInvocation,
    ) -> Result<(), ViewProjectLowerError> {
        let application = self
            .analysis
            .call(owner)
            .and_then(arcweft_lang_sema::callable::CallTargetFacts::selected_application)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let CallableValidator::ViewModifier(modifier) = application
            .core()
            .candidates()
            .selected()
            .schema()
            .validator()
        else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let modifier = *modifier;
        let event = modifier
            .event()
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let handler_result_role = modifier
            .handler_result_role()
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let handler_result_type = modifier
            .handler_result_type()
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let arcweft_lang_hir::expr::HirCallCallee::Value { value: callee } = call.callee() else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let select = self
            .module
            .resolve_expr(*callee)
            .ok()
            .and_then(|callee| match callee.kind() {
                arcweft_lang_hir::expr::HirExprKind::Select(select) => Some(select),
                _ => None,
            })
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        if !matches!(
            self.analysis
                .expression(*callee)
                .map(|checked| checked.resolution()),
            Some(CheckedExpressionResolution::Select(
                CheckedSelectResolution::Method(_)
            ))
        ) {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        }
        let [argument] = application.core().execution().arguments() else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let [slot] = argument.slots() else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let handler_source = slot.source().owner();
        if call.arguments().len() != 1 || call.arguments()[0].value() != handler_source {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        }

        let checked_handler = self
            .analysis
            .expression(handler_source)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let CheckedExpressionResolution::Closure(closure) = checked_handler.resolution() else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let arcweft_lang_hir::expr::HirExprKind::Closure(hir_closure) = self
            .module
            .resolve_expr(handler_source)
            .map_err(|_| ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?
            .kind()
        else {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        };
        let handler_type = checked_handler
            .value_type()
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        let expected_handler = TypeKind::function_with_effects(
            [],
            handler_result_type.clone(),
            EffectRow::closed(arcweft_lang_sema::effects::EffectSet::new()),
        );
        if closure.owner() != handler_source || handler_type != &expected_handler {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        }
        if hir_closure.captures().len() != closure.captures().len() {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        }
        let captures = hir_closure
            .captures()
            .iter()
            .copied()
            .zip(closure.captures())
            .map(|(capture_id, capture)| {
                if capture.mode() != CaptureAccess::Read {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                }
                let parameter = self.parameters.get(&capture.local()).ok_or(
                    ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner },
                )?;
                let capture_fact = self.analysis.capture(capture_id).ok_or(
                    ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner },
                )?;
                if capture_fact.ty().semantic_identity_digest()?.as_bytes()
                    != parameter.value_type.as_bytes()
                {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                }
                self.registered_world
                    .checked_ownership(
                        self.analysis,
                        capture_fact.ty(),
                        CheckedOwnershipLimits::PRODUCTION,
                    )
                    .map_err(|source| {
                        ViewProjectLowerError::InvalidViewHandlerCaptureOwnership {
                            owner: self.owner,
                            capture: capture_id,
                            source,
                        }
                    })?;
                let hir_capture = self.module.resolve_capture(capture_id).map_err(|_| {
                    ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner }
                })?;
                if hir_capture.closure() != handler_source
                    || hir_capture.local() != capture.local()
                    || hir_capture.access() != capture.mode()
                {
                    return Err(ViewProjectLowerError::MissingCheckedViewProjection {
                        owner: self.owner,
                    });
                }
                Ok(CheckedViewHandlerCapture {
                    capture: capture_id,
                    local: capture.local(),
                    schema: ViewHandlerCapture::new(parameter.coordinate, parameter.value_type),
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let program_id = modifier
            .handler_program_id(application)
            .ok_or(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner })?;
        if self
            .output
            .handlers
            .iter()
            .any(|handler| handler.id == program_id)
        {
            return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner: self.owner });
        }
        self.output.handlers.push(CheckedViewHandlerProgram {
            id: program_id,
            event,
            closure: handler_source,
            body: hir_closure.body(),
            captures,
            result: ViewHandlerResult::new(
                handler_result_role,
                ViewHandlerValueTypeId::from_semantic_digest(
                    *handler_result_type.semantic_identity_digest()?.as_bytes(),
                ),
            ),
        });

        self.lower_value(select.target())?;
        self.output
            .instructions
            .push(ViewProgramInstruction::BindHandler {
                event,
                handler: program_id,
                source: None,
            });
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
                    parameter: parameter.name,
                    projection,
                }
            }
            _ => return Err(ViewProjectLowerError::MissingCheckedViewProjection { owner }),
        };
        Ok(source_kind)
    }
}

fn view_schema_hash(view: &ViewId, parameters: &BTreeMap<LocalId, CheckedViewParameter>) -> u64 {
    let mut hasher = blake3::Hasher::new_derive_key("arcweft.view.state-schema.v1");
    hasher.update(view.as_str().as_bytes());
    for parameter in parameters.values() {
        hasher.update(&[0]);
        hasher.update(parameter.name.as_bytes());
        hasher.update(&parameter.coordinate.value().to_le_bytes());
        hasher.update(parameter.value_type.as_bytes());
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
