//! Orchestrates view control-flow and layout lowering across responsibility modules.

mod content;
mod modifiers;
mod scroll;
mod text_controls;

use self::content::{assign_action_button_bounds, lower_button, lower_image, lower_text};
use self::modifiers::{
    lower_button_modifiers, lower_modifiers, lower_navigation_group, lower_navigation_target,
    lower_text_control_modifiers, lower_text_modifiers,
};
use self::scroll::lower_scroll_region;
use self::text_controls::{
    InputHandleBinding, lower_text_control_payload_field, lower_text_field, modifier_label,
    normalize_input_payload_ref, register_input_handle_binding, symbol_expr_name,
    text_control_selection_policy,
};

use arcweft_bundle::{
    BundleImageObject, BundleImageObjectBounds,
    container::BundleDigest,
    resource_codec::{
        ProductSourceRef, SourceMapSection, ViewActionButtonActionResource,
        ViewActionButtonResource, ViewActionPayloadResource, ViewActionTextControlPayloadField,
        ViewAwaitBranchSpan, ViewCallArgumentBindingRef, ViewDefinitionResource,
        ViewFocusDirection, ViewFocusGroupPolicy, ViewFocusGroupResource, ViewFocusInitialPolicy,
        ViewFocusNavigationEdge, ViewFocusNavigationResource, ViewFocusSkipPolicy,
        ViewFocusTargetResolution, ViewFocusWrapPolicy, ViewFxArgumentBindingRef,
        ViewInputResource, ViewInstructionSpan, ViewLayoutBoundsResource, ViewLogicalRect,
        ViewParameterResource, ViewProgramResource, ViewRuntimeButtonBounds,
        ViewRuntimeSurfaceBounds, ViewScrollAxis, ViewScrollIndicatorsPolicy,
        ViewScrollOverflowPolicy, ViewScrollOverscrollPolicy, ViewScrollRegionResource,
        ViewSurfaceResource, ViewTextBlockBounds, ViewTextBlockResource, ViewTextResource,
        view::{
            CompositionOnBlurPolicy, DialogueTextProjection, EnterKeyHint, TextAssistPolicy,
            TextCapitalization, ViewDefinitionRef, ViewElementKind, ViewExportedPart,
            ViewFocusAutoScrollPolicy, ViewInputKind, ViewInputOptions, ViewInputPurpose,
            ViewParameterRole, ViewProgramInstruction, ViewSecureInputPolicy, ViewSemanticTarget,
            ViewStyleApplicationTarget, ViewTextSelectionPolicy, ViewTextShortcutPolicy,
            ViewTextSourceKind, ViewTextSourceRecord, ViewTextSurface, ViewTextTabPolicy,
            ViewTextVerticalNavigationPolicy,
        },
    },
};
use arcweft_compiler::style::ViewStyleApplicationLookup;
use arcweft_compiler::view_part::{ViewPartLowerError, lower_view_part_exports};
use arcweft_id::{IdError, PublicId};
use arcweft_lang_sema::dialogue_view::{
    DialogueViewModel, DialogueViewModelRegistry,
    DialogueViewProjection as SemanticDialogueViewProjection,
};
use arcweft_lang_sema::view_part::CheckedViewPartCatalog;
use arcweft_lang_syntax::{
    ast::{
        common::TextRange,
        ids::{EntityRef, EntityRefSyntax},
        items::EntityDeclItem,
        view::{
            ViewAction, ViewActionPayload, ViewArg, ViewAwait, ViewAwaitBranchKind, ViewBody,
            ViewButton, ViewButtonLabel, ViewCall, ViewElement, ViewExpr, ViewForEach,
            ViewFxApplication, ViewIf, ViewImage, ViewLet, ViewMatch, ViewMatchArm, ViewModifier,
            ViewNavigationDirection, ViewNavigationInitial, ViewNavigationTarget,
            ViewNavigationTrap, ViewText, ViewTextControlPayloadField, ViewTextField,
            ViewTextFieldMode,
        },
    },
    expr::{CallArg, Expr, Literal},
    types::{TypeRef, parse_fn_signature},
};
use arcweft_presentation::fx::{FxDefinition, FxId, FxRuntimeType};
use arcweft_view::ViewElementLayoutKind;
use arcweft_view::{
    ViewId, ViewIdError, ViewPartLocalName, ViewProgramId, part::ViewPartNameError,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use super::super::bundle_view_layout::{
    VIEW_LAYOUT_GAP_MILLI, VIEW_LAYOUT_SCROLL_VIEWPORT_HEIGHT_MILLI,
    VIEW_LAYOUT_TEXT_CONTROL_WIDTH_MILLI, ViewLayoutCursor, ViewLayoutFrame, button_bounds,
    modifier_layout_length_i32, modifier_layout_length_u32, named_arg, named_layout_length_i32,
    named_layout_length_u32, text_block_frame, u32_to_i32_saturating,
};
use super::super::bundle_view_schema::{ViewValueCompileError, ViewValueProgramCompiler};

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct ViewBundleSidecars {
    pub(in crate::app) program: Option<ViewProgramResource>,
    pub(in crate::app) text: Option<ViewTextResource>,
    pub(in crate::app) input: Option<ViewInputResource>,
    pub(in crate::app) image_objects: Vec<BundleImageObject>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub(in crate::app) enum ViewSidecarError {
    #[error(
        "error[AWF0618 view::scroll_axis_both_unsupported]: `{element}` cannot use `{value}` as a Scroll axis in this cut; use `.vertical` or `.horizontal` and keep two-axis scrolling behind a future typed contract"
    )]
    UnsupportedScrollBothAxis { element: String, value: String },
    #[error(transparent)]
    ValueProgram(#[from] ViewValueCompileError),
    #[error(transparent)]
    ViewPart(#[from] ViewPartLowerError),
    #[error(transparent)]
    ViewCodec(#[from] arcweft_bundle::resource_codec::SectionCodecError),
    #[error("View call has more than 65,536 arguments")]
    TooManyViewCallArguments,
    #[error("View `{view}` has an invalid parameter signature: {message}")]
    InvalidViewSignature { view: String, message: String },
    #[error("View `{value}` has an invalid public ID: {source}")]
    InvalidViewPublicId { value: String, source: IdError },
    #[error("View `{value}` does not have a valid View-family identity: {source}")]
    InvalidViewIdentity { value: String, source: ViewIdError },
    #[error("View part `{value}` has an invalid local identity: {source}")]
    InvalidViewPartId {
        value: String,
        source: ViewPartNameError,
    },
    #[error("View `{view}` parameter {ordinal} must use one identifier binding")]
    UnsupportedViewParameter { view: String, ordinal: usize },
    #[error("View call references unknown definition `{view}`")]
    UnknownViewCall { view: String },
    #[error("View call `{view}` has invalid argument `{argument}`: {reason}")]
    InvalidViewCallArgument {
        view: String,
        argument: String,
        reason: String,
    },
    #[error("View Text source `{expression}` is not a literal or typed text projection")]
    UnsupportedTextSource { expression: String },
    #[error(
        "View action projection `{expression}` must select `primary_action` from a dialogue View parameter"
    )]
    InvalidDialogueActionProjection { expression: String },
}

#[derive(Default)]
struct ViewLoweringState {
    fx_definitions: BTreeMap<String, ViewFxDefinition>,
    view_schemas: BTreeMap<String, ViewDefinitionSchema>,
    dialogue_parameters: BTreeMap<String, DialogueViewModel>,
    definitions: Vec<ViewDefinitionResource>,
    value_compiler: ViewValueProgramCompiler,
    instructions: Vec<ViewProgramInstruction>,
    exported_parts: Vec<ViewExportedPart>,
    source_refs: Vec<ProductSourceRef>,
    text_sources: Vec<ViewTextSourceRecord>,
    input_options: Vec<ViewInputOptions>,
    semantic_targets: Vec<ViewSemanticTarget>,
    layout_bounds: Vec<ViewLayoutBoundsResource>,
    scroll_regions: Vec<ViewScrollRegionResource>,
    surfaces: Vec<ViewSurfaceResource>,
    scroll_stack: Vec<String>,
    text_blocks: Vec<ViewTextBlockResource>,
    action_buttons: Vec<ViewActionButtonResource>,
    focus_groups: Vec<ViewFocusGroupResource>,
    focus_navigation: Vec<ViewFocusNavigationResource>,
    focus_group_stack: Vec<String>,
    style_applications: ViewStyleApplicationLookup,
    active_view: Option<PublicId>,
    input_handle_bindings: Vec<InputHandleBinding>,
    source_image_objects: Vec<BundleImageObject>,
    image_objects: Vec<BundleImageObject>,
    text_counter: u32,
    input_counter: u32,
    button_counter: u32,
    scroll_counter: u32,
    text_block_counter: u32,
    image_counter: u32,
    element_counter: u32,
    group_counter: u32,
    handler_counter: u32,
}

impl ViewLoweringState {
    fn producer_styles(&self, range: TextRange) -> Vec<ViewStyleApplicationTarget> {
        self.active_view.as_ref().map_or_else(Vec::new, |view| {
            self.style_applications
                .applications_for(view, range)
                .to_vec()
        })
    }
}

#[derive(Clone)]
struct ViewFxDefinition {
    id: FxId,
    parameters: Vec<(String, FxRuntimeType)>,
}

#[derive(Clone)]
struct ViewDefinitionSchema {
    public_id: String,
    parameters: Vec<ViewParameterSchema>,
}

#[derive(Clone)]
struct ViewParameterSchema {
    name: String,
    value_type: Option<FxRuntimeType>,
    source_type: String,
    default: Option<Expr>,
    dialogue_model: Option<DialogueViewModel>,
}

pub(in crate::app) fn view_sidecars(
    views: &[&EntityDeclItem],
    dialogue_view_models: &DialogueViewModelRegistry,
    style_applications: &ViewStyleApplicationLookup,
    source_image_objects: &[BundleImageObject],
    fx_definitions: &[FxDefinition],
    view_part_catalog: &CheckedViewPartCatalog,
    source_map: &SourceMapSection,
) -> Result<ViewBundleSidecars, ViewSidecarError> {
    let mut state = ViewLoweringState {
        fx_definitions: view_fx_definitions(fx_definitions),
        view_schemas: view_definition_schemas(views, dialogue_view_models)?,
        style_applications: style_applications.clone(),
        source_image_objects: source_image_objects.to_vec(),
        ..ViewLoweringState::default()
    };
    let Some(first) = views.first() else {
        return Ok(ViewBundleSidecars::default());
    };
    for view in views {
        if let Some(body) = view.view_body().and_then(|body| body.view()) {
            let public_id = view_resource_id(view.id().body());
            let typed_view_id = ViewId::try_new(public_id.clone()).map_err(|source| {
                ViewSidecarError::InvalidViewIdentity {
                    value: public_id.clone(),
                    source,
                }
            })?;
            let typed_public_id = typed_view_id.public_id().clone();
            let schema = state.view_schemas.get(&public_id).cloned().ok_or_else(|| {
                ViewSidecarError::UnknownViewCall {
                    view: public_id.clone(),
                }
            })?;
            let parameter_slots = state.value_compiler.begin_definition(
                &public_id,
                schema.parameters.iter().filter_map(|parameter| {
                    parameter
                        .value_type
                        .map(|value_type| (parameter.name.clone(), value_type))
                }),
            )?;
            let parameters =
                compile_view_parameters(&schema, &parameter_slots, &mut state.value_compiler)?;
            state.dialogue_parameters = schema
                .parameters
                .iter()
                .filter_map(|parameter| {
                    parameter
                        .dialogue_model
                        .clone()
                        .map(|model| (parameter.name.clone(), model))
                })
                .collect();
            let root_styles = state
                .style_applications
                .root_applications_for(&typed_public_id)
                .to_vec();
            let start_instruction = usize_to_u32_saturating(state.instructions.len());
            state.active_view = Some(typed_public_id.clone());
            lower_view_body(view.id(), body, &mut state)?;
            state.active_view = None;
            state.dialogue_parameters.clear();
            let end_instruction = usize_to_u32_saturating(state.instructions.len());
            state.definitions.push(ViewDefinitionResource {
                public_id: ViewDefinitionRef::new(typed_view_id),
                styles: root_styles,
                body: ViewInstructionSpan::new(start_instruction, end_instruction),
                parameters,
                state_schema_hash: view_state_schema_hash(&schema, body),
            });
        }
    }
    let emitted_owners = state
        .definitions
        .iter()
        .map(|definition| definition.public_id.clone())
        .collect::<BTreeSet<_>>();
    (state.source_refs, state.exported_parts) =
        lower_view_part_exports(view_part_catalog, &emitted_owners, source_map)?.into_parts();
    finish_view_sidecars(first, state)
}

fn finish_view_sidecars(
    first: &EntityDeclItem,
    mut state: ViewLoweringState,
) -> Result<ViewBundleSidecars, ViewSidecarError> {
    assign_action_button_bounds(&mut state);
    let compiled_values = std::mem::take(&mut state.value_compiler).finish()?;
    if state.instructions.is_empty()
        && state.text_sources.is_empty()
        && state.input_options.is_empty()
        && state.layout_bounds.is_empty()
        && state.scroll_regions.is_empty()
        && state.surfaces.is_empty()
        && state.text_blocks.is_empty()
        && state.action_buttons.is_empty()
        && state.focus_groups.is_empty()
        && state.focus_navigation.is_empty()
        && state.exported_parts.is_empty()
        && state.image_objects.is_empty()
    {
        return Ok(ViewBundleSidecars::default());
    }
    let program = ViewProgramResource {
        program_id: ViewProgramId::try_new(format!("view.program.{}", first.id().body())).map_err(
            |source| ViewSidecarError::InvalidViewPublicId {
                value: format!("view.program.{}", first.id().body()),
                source,
            },
        )?,
        definitions: state.definitions,
        value_programs: compiled_values.programs,
        value_inputs: compiled_values.inputs,
        instructions: state.instructions,
        handlers: Vec::new(),
        source_refs: state.source_refs,
        exported_parts: state.exported_parts,
        semantic_targets: state.semantic_targets,
        layout_bounds: state.layout_bounds,
        scroll_regions: state.scroll_regions,
        surfaces: state.surfaces,
        text_blocks: state.text_blocks,
        action_buttons: state.action_buttons,
        focus_groups: state.focus_groups,
        focus_navigation: state.focus_navigation,
        adapter_requirements: Vec::new(),
    };
    Ok(ViewBundleSidecars {
        program: Some(program),
        text: (!state.text_sources.is_empty()).then(|| ViewTextResource {
            sources: state.text_sources,
            ..ViewTextResource::default()
        }),
        input: (!state.input_options.is_empty()).then(|| ViewInputResource {
            options: state.input_options,
            adapter_requirements: Vec::new(),
        }),
        image_objects: state.image_objects,
    })
}

fn view_fx_definitions(definitions: &[FxDefinition]) -> BTreeMap<String, ViewFxDefinition> {
    let mut result = BTreeMap::new();
    for definition in definitions {
        let schema = ViewFxDefinition {
            id: definition.id().clone(),
            parameters: definition
                .parameters()
                .iter()
                .map(|parameter| (parameter.name().to_owned(), parameter.value_type()))
                .collect(),
        };
        result.insert(definition.id().function().to_owned(), schema.clone());
        if let Some(name) = definition.id().function().rsplit('.').next() {
            result.insert(name.to_owned(), schema);
        }
    }
    result
}

fn view_definition_schemas(
    views: &[&EntityDeclItem],
    dialogue_view_models: &DialogueViewModelRegistry,
) -> Result<BTreeMap<String, ViewDefinitionSchema>, ViewSidecarError> {
    let mut schemas = BTreeMap::new();
    for view in views {
        let public_id = view_resource_id(view.id().body());
        let signature =
            parse_fn_signature(&format!("fn view{}", view.signature_tail())).map_err(|error| {
                ViewSidecarError::InvalidViewSignature {
                    view: public_id.clone(),
                    message: error.to_string(),
                }
            })?;
        let parameters = signature
            .param_groups()
            .iter()
            .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
            .enumerate()
            .map(|(ordinal, parameter)| {
                let name = parameter
                    .pattern()
                    .simple_binding_name()
                    .ok_or_else(|| ViewSidecarError::UnsupportedViewParameter {
                        view: public_id.clone(),
                        ordinal,
                    })?
                    .to_owned();
                Ok(ViewParameterSchema {
                    name,
                    value_type: view_scalar_type(parameter.ty()),
                    source_type: format!("{:?}", parameter.ty()),
                    default: parameter.default().cloned(),
                    dialogue_model: match parameter.ty() {
                        TypeRef::Path(type_name) => dialogue_view_models.model(type_name).cloned(),
                        _ => None,
                    },
                })
            })
            .collect::<Result<Vec<_>, ViewSidecarError>>()?;
        let schema = ViewDefinitionSchema {
            public_id: public_id.clone(),
            parameters,
        };
        if schemas.insert(public_id.clone(), schema).is_some() {
            return Err(ViewSidecarError::InvalidViewSignature {
                view: public_id,
                message: "duplicate View definition".to_owned(),
            });
        }
    }
    Ok(schemas)
}

fn compile_view_parameters(
    schema: &ViewDefinitionSchema,
    parameter_slots: &BTreeMap<String, u16>,
    compiler: &mut ViewValueProgramCompiler,
) -> Result<Vec<ViewParameterResource>, ViewSidecarError> {
    schema
        .parameters
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| {
            let ordinal =
                u16::try_from(ordinal).map_err(|_| ViewSidecarError::TooManyViewCallArguments)?;
            let default_program = parameter
                .default
                .as_ref()
                .map(|default| {
                    let expected = parameter.value_type.ok_or_else(|| {
                        ViewSidecarError::InvalidViewSignature {
                            view: schema.public_id.clone(),
                            message: format!(
                                "parameter `{}` has a non-scalar default outside ViewValueProgram",
                                parameter.name
                            ),
                        }
                    })?;
                    compiler
                        .compile(default, Some(expected))
                        .map_err(ViewSidecarError::from)
                })
                .transpose()?;
            Ok(ViewParameterResource {
                ordinal,
                name: parameter.name.clone(),
                role: if parameter.dialogue_model.is_some() {
                    ViewParameterRole::Dialogue
                } else {
                    ViewParameterRole::Value
                },
                value_type: parameter.value_type,
                value_slot: parameter_slots.get(&parameter.name).copied(),
                default_program,
            })
        })
        .collect()
}

fn view_scalar_type(ty: &TypeRef) -> Option<FxRuntimeType> {
    let TypeRef::Path(path) = ty else {
        return None;
    };
    Some(match path.as_str() {
        "bool" => FxRuntimeType::Bool,
        "i32" => FxRuntimeType::I32,
        "f32" => FxRuntimeType::F32,
        "Length" => FxRuntimeType::Length,
        "Angle" => FxRuntimeType::Angle,
        "Seconds" | "Duration" => FxRuntimeType::Seconds,
        "Color" => FxRuntimeType::Color,
        "Vec2" => FxRuntimeType::Vec2,
        "Transform2D" => FxRuntimeType::Transform2D,
        _ => return None,
    })
}

fn view_state_schema_hash(schema: &ViewDefinitionSchema, body: &ViewBody) -> u64 {
    let mut canonical = String::from("arcweft.view-state-schema.v1\n");
    canonical.push_str(&schema.public_id);
    for parameter in &schema.parameters {
        canonical.push_str("\nparam:");
        canonical.push_str(&parameter.name);
        canonical.push(':');
        canonical.push_str(&parameter.source_type);
    }
    for local in body.locals() {
        canonical.push_str("\nlocal:");
        canonical.push_str(local.name());
        canonical.push(':');
        canonical.push_str(local.ty().unwrap_or("_"));
    }
    let digest = BundleDigest::of(canonical.as_bytes());
    let mut hash = [0_u8; 8];
    hash.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(hash)
}

fn lower_view_body(
    view_id: &EntityRef,
    body: &ViewBody,
    state: &mut ViewLoweringState,
) -> Result<(), ViewSidecarError> {
    let mut layout = ViewLayoutCursor::root();
    lower_view_expr(view_id.body(), body.value(), state, &mut layout)?;
    Ok(())
}

fn view_resource_id(view_id: &str) -> String {
    if view_id.starts_with("view.") {
        view_id.to_owned()
    } else {
        format!("view.{view_id}")
    }
}

pub(in crate::app) fn normalize_view_call(view: &Expr) -> String {
    let source = match view {
        Expr::EntityRef(reference) => normalize_entity_ref(reference),
        _ => expr_source(view),
    };
    let source = source
        .trim()
        .trim_start_matches('@')
        .trim_start_matches("view:.")
        .trim_start_matches('.');
    view_resource_id(source)
}

fn lower_nested_view_call(
    owner_view: &str,
    call: &ViewCall,
    state: &mut ViewLoweringState,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let view = normalize_view_call(call.view());
    let schema = state
        .view_schemas
        .get(&view)
        .cloned()
        .ok_or_else(|| ViewSidecarError::UnknownViewCall { view: view.clone() })?;
    let mut bound = BTreeSet::new();
    let arguments = call
        .args()
        .iter()
        .enumerate()
        .map(|(authored_ordinal, argument)| {
            let authored_ordinal = u16::try_from(authored_ordinal)
                .map_err(|_| ViewSidecarError::TooManyViewCallArguments)?;
            let (ordinal, name, parameter, value) = match argument {
                ViewArg::Positional(value) => {
                    let parameter = schema
                        .parameters
                        .get(usize::from(authored_ordinal))
                        .ok_or_else(|| ViewSidecarError::InvalidViewCallArgument {
                            view: view.clone(),
                            argument: authored_ordinal.to_string(),
                            reason: "positional argument exceeds the parameter list".to_owned(),
                        })?;
                    (authored_ordinal, None, parameter, value)
                }
                ViewArg::Named { name, value } => {
                    let (ordinal, parameter) = schema
                        .parameters
                        .iter()
                        .enumerate()
                        .find(|(_, parameter)| parameter.name == *name)
                        .ok_or_else(|| ViewSidecarError::InvalidViewCallArgument {
                            view: view.clone(),
                            argument: name.clone(),
                            reason: "no parameter has this name".to_owned(),
                        })?;
                    let ordinal = u16::try_from(ordinal)
                        .map_err(|_| ViewSidecarError::TooManyViewCallArguments)?;
                    (ordinal, Some(name.clone()), parameter, value)
                }
            };
            if !bound.insert(ordinal) {
                return Err(ViewSidecarError::InvalidViewCallArgument {
                    view: view.clone(),
                    argument: name.clone().unwrap_or_else(|| authored_ordinal.to_string()),
                    reason: "parameter is bound more than once".to_owned(),
                });
            }
            Ok(ViewCallArgumentBindingRef {
                ordinal,
                name,
                value_program: state.value_compiler.compile(value, parameter.value_type)?,
            })
        })
        .collect::<Result<Vec<_>, ViewSidecarError>>()?;
    for (ordinal, parameter) in schema.parameters.iter().enumerate() {
        let ordinal =
            u16::try_from(ordinal).map_err(|_| ViewSidecarError::TooManyViewCallArguments)?;
        if !bound.contains(&ordinal) && parameter.default.is_none() {
            return Err(ViewSidecarError::InvalidViewCallArgument {
                view: view.clone(),
                argument: parameter.name.clone(),
                reason: "required parameter is missing".to_owned(),
            });
        }
    }
    let styles = state.producer_styles(call.range());
    state.instructions.push(ViewProgramInstruction::CallView {
        view: ViewDefinitionRef::new(ViewId::parse_public(view.clone()).map_err(|source| {
            ViewSidecarError::InvalidViewIdentity {
                value: view,
                source,
            }
        })?),
        arguments,
        styles,
        part: first_part(call.modifiers())?,
        key: None,
        source: None,
    });
    lower_modifiers(owner_view, call.modifiers(), state)?;
    Ok(ViewLayoutFrame::zero())
}

fn lower_view_expr(
    view_id: &str,
    expr: &ViewExpr,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    Ok(match expr {
        ViewExpr::Element(element) => lower_element(view_id, element, state, layout)?,
        ViewExpr::Text(text) => lower_text(view_id, text, state, *layout)?,
        ViewExpr::TextField(field) => lower_text_field(view_id, field, state, layout)?,
        ViewExpr::Button(button) => lower_button(view_id, button, state, *layout)?,
        ViewExpr::Image(image) => lower_image(view_id, image, state, *layout)?,
        ViewExpr::Let(view_let) => lower_view_let(view_let, state)?,
        ViewExpr::Fragment(children) => lower_layout_column(view_id, children, state, *layout)?,
        ViewExpr::ViewCall(call) => lower_nested_view_call(view_id, call, state)?,
        ViewExpr::Raw(raw) => {
            state.instructions.push(ViewProgramInstruction::EmitCustom {
                element: raw.clone(),
                styles: Vec::new(),
                part: None,
                source: None,
            });
            ViewLayoutFrame::zero()
        }
        ViewExpr::If(branch) => lower_view_if(view_id, branch, state, layout)?,
        ViewExpr::Match(view_match) => lower_view_match(view_id, view_match, state, layout)?,
        ViewExpr::ForEach(view_for_each) => {
            lower_view_for_each(view_id, view_for_each, state, layout)?
        }
        ViewExpr::Await(view_await) => lower_view_await(view_id, view_await, state, layout)?,
        ViewExpr::Expr(_) => ViewLayoutFrame::zero(),
    })
}

fn lower_view_let(
    view_let: &ViewLet,
    state: &mut ViewLoweringState,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let (binding, value_program) = state
        .value_compiler
        .compile_local(view_let.pattern(), view_let.value())?;
    state.instructions.push(ViewProgramInstruction::BindLocal {
        binding,
        value_program,
        source: None,
    });
    register_input_handle_binding(view_let, state);
    Ok(ViewLayoutFrame::zero())
}

fn lower_view_if(
    view_id: &str,
    branch: &ViewIf,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let condition_program = state.value_compiler.compile_condition(branch.condition())?;
    let branch_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Branch {
        condition_program,
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(view_id, branch.then_branch(), state, &mut then_layout)?;
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) = if let Some(branch) = branch.else_branch() {
        let else_start = state.instructions.len();
        let mut else_layout = *layout;
        let frame = lower_view_expr(view_id, branch, state, &mut else_layout)?;
        let span = usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
        (frame, Some(span))
    } else {
        (ViewLayoutFrame::zero(), None)
    };

    state.instructions[branch_index] = ViewProgramInstruction::Branch {
        condition_program,
        then_span,
        else_span,
        source: None,
    };
    Ok(ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    ))
}

fn lower_view_match(
    view_id: &str,
    view_match: &ViewMatch,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    lower_view_match_arms(
        view_id,
        view_match.scrutinee(),
        view_match.arms(),
        state,
        layout,
    )
}

fn lower_view_match_arms(
    view_id: &str,
    scrutinee: &Expr,
    arms: &[ViewMatchArm],
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let Some((arm, remaining)) = arms.split_first() else {
        return Ok(ViewLayoutFrame::zero());
    };
    let condition_program = state
        .value_compiler
        .compile_match_condition(scrutinee, arm)?;
    let branch_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Branch {
        condition_program,
        then_span: 0,
        else_span: None,
        source: None,
    });

    let then_start = state.instructions.len();
    let mut then_layout = *layout;
    let then_frame = lower_view_expr(view_id, arm.value(), state, &mut then_layout)?;
    let then_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(then_start));

    let (else_frame, else_span) = if remaining.is_empty() {
        (ViewLayoutFrame::zero(), None)
    } else {
        let else_start = state.instructions.len();
        let frame = lower_view_match_arms(view_id, scrutinee, remaining, state, layout)?;
        let span = usize_to_u32_saturating(state.instructions.len().saturating_sub(else_start));
        (frame, Some(span))
    };

    state.instructions[branch_index] = ViewProgramInstruction::Branch {
        condition_program,
        then_span,
        else_span,
        source: None,
    };
    Ok(ViewLayoutFrame::new(
        then_frame.width_milli.max(else_frame.width_milli),
        then_frame.height_milli.max(else_frame.height_milli),
    ))
}

fn lower_view_for_each(
    view_id: &str,
    view_for_each: &ViewForEach,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let source_program = state
        .value_compiler
        .compile_repeat_source(view_for_each.source())?;
    let key_program = state.value_compiler.compile_repeat_key(view_for_each)?;
    let repeat_index = state.instructions.len();
    state
        .instructions
        .push(ViewProgramInstruction::RepeatKeyed {
            source_program,
            key_program,
            body_span: 0,
            source: None,
        });
    let body_start = state.instructions.len();
    let body_frame = lower_view_expr(view_id, view_for_each.body(), state, layout)?;
    let body_span = usize_to_u32_saturating(state.instructions.len().saturating_sub(body_start));
    state.instructions[repeat_index] = ViewProgramInstruction::RepeatKeyed {
        source_program,
        key_program,
        body_span,
        source: None,
    };
    Ok(body_frame)
}

fn lower_view_await(
    view_id: &str,
    view_await: &ViewAwait,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let source_program = state
        .value_compiler
        .compile_await_source(view_await.source())?;
    let await_index = state.instructions.len();
    state.instructions.push(ViewProgramInstruction::Await {
        source_program,
        pending_branch: None,
        ready_branch: None,
        error_branch: None,
        denied_branch: None,
        source: None,
    });

    let mut pending_branch = None;
    let mut ready_branch = None;
    let mut error_branch = None;
    let mut denied_branch = None;
    let mut frame = ViewLayoutFrame::zero();

    for branch in view_await.branches() {
        let start = state.instructions.len();
        let mut branch_layout = *layout;
        let branch_frame = lower_view_expr(view_id, branch.value(), state, &mut branch_layout)?;
        let branch_span = ViewAwaitBranchSpan {
            start_offset: usize_to_u32_saturating(start.saturating_sub(await_index + 1)),
            body_span: usize_to_u32_saturating(state.instructions.len().saturating_sub(start)),
        };
        match branch.kind() {
            ViewAwaitBranchKind::Pending => pending_branch = Some(branch_span),
            ViewAwaitBranchKind::Ready => ready_branch = Some(branch_span),
            ViewAwaitBranchKind::Error => error_branch = Some(branch_span),
            ViewAwaitBranchKind::Denied => denied_branch = Some(branch_span),
        }
        frame = ViewLayoutFrame::new(
            frame.width_milli.max(branch_frame.width_milli),
            frame.height_milli.max(branch_frame.height_milli),
        );
    }

    state.instructions[await_index] = ViewProgramInstruction::Await {
        source_program,
        pending_branch,
        ready_branch,
        error_branch,
        denied_branch,
        source: None,
    };
    Ok(frame)
}

fn lower_element(
    view_id: &str,
    element: &ViewElement,
    state: &mut ViewLoweringState,
    layout: &mut ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    if let Some(kind) = ViewElementKind::from_source_name(element.callee()) {
        let origin = ViewLayoutCursor {
            x_milli: named_layout_length_i32(element.args(), &["x"]).unwrap_or(layout.x_milli),
            y_milli: named_layout_length_i32(element.args(), &["y"]).unwrap_or(layout.y_milli),
        };
        let target = next_element_id(view_id, state);
        let part = element_part(element)?;
        let styles = state.producer_styles(element.range());
        let open_instruction = state.instructions.len();
        state
            .instructions
            .push(ViewProgramInstruction::OpenElement {
                element: kind,
                target: Some(target.clone()),
                styles,
                part,
                key: None,
                source: None,
            });
        let pushed_group = lower_navigation_group(view_id, element, state);
        lower_modifiers(view_id, element.modifiers(), state)?;
        let frame = match kind.layout_kind() {
            Some(ViewElementLayoutKind::Row) => {
                lower_layout_row(view_id, element.children(), state, origin)?
            }
            Some(ViewElementLayoutKind::Column) => {
                lower_layout_column(view_id, element.children(), state, origin)?
            }
            Some(ViewElementLayoutKind::Scroll) => {
                lower_scroll_region(view_id, element, state, origin, open_instruction)?
            }
            Some(ViewElementLayoutKind::Stack) => {
                lower_layout_stack(view_id, element.children(), state, origin)?
            }
            None if kind.is_action_control() => ViewLayoutFrame::action_button(),
            None => ViewInputKind::from_element(kind)
                .map_or(ViewLayoutFrame::zero(), ViewLayoutFrame::text_control),
        };
        let frame = ViewLayoutFrame::new(
            named_layout_length_u32(element.args(), &["width", "w"]).unwrap_or(frame.width_milli),
            named_layout_length_u32(element.args(), &["height", "h"]).unwrap_or(frame.height_milli),
        );
        if matches!(kind, ViewElementKind::Panel | ViewElementKind::Box) && !frame.is_empty() {
            state.surfaces.push(ViewSurfaceResource {
                public_id: target,
                view: Some(view_resource_id(view_id)),
                containing_scroll_region: state.scroll_stack.last().cloned(),
                element: kind,
                bounds: ViewRuntimeSurfaceBounds::new(
                    origin.x_milli,
                    origin.y_milli,
                    frame.width_milli,
                    frame.height_milli,
                ),
                source: None,
            });
        }
        if pushed_group {
            state.focus_group_stack.pop();
        }
        state
            .instructions
            .push(ViewProgramInstruction::CloseElement);
        Ok(frame)
    } else {
        let styles = state.producer_styles(element.range());
        state.instructions.push(ViewProgramInstruction::EmitCustom {
            element: element.callee().to_owned(),
            styles,
            part: first_part(element.modifiers())?,
            source: None,
        });
        lower_modifiers(view_id, element.modifiers(), state)?;
        Ok(ViewLayoutFrame::zero())
    }
}

fn next_element_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("element.{view_id}.{}", state.element_counter);
    state.element_counter = state.element_counter.saturating_add(1);
    id
}

fn element_part(element: &ViewElement) -> Result<Option<ViewPartLocalName>, ViewSidecarError> {
    first_part(element.modifiers())
}

fn lower_layout_column(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.y_milli = cursor.y_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        if frame.is_empty() {
            continue;
        }
        width_milli = width_milli.max(frame.width_milli);
        height_milli = height_milli
            .saturating_add(if placed {
                VIEW_LAYOUT_GAP_MILLI as u32
            } else {
                0
            })
            .saturating_add(frame.height_milli);
        cursor.y_milli = cursor
            .y_milli
            .saturating_add(u32_to_i32_saturating(frame.height_milli));
        placed = true;
    }
    Ok(ViewLayoutFrame::new(width_milli, height_milli))
}

fn lower_layout_row(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut cursor = origin;
    let mut width_milli = 0_u32;
    let mut height_milli = 0_u32;
    let mut placed = false;
    for child in children {
        if placed {
            cursor.x_milli = cursor.x_milli.saturating_add(VIEW_LAYOUT_GAP_MILLI);
        }
        let frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        if frame.is_empty() {
            continue;
        }
        width_milli = width_milli
            .saturating_add(if placed {
                VIEW_LAYOUT_GAP_MILLI as u32
            } else {
                0
            })
            .saturating_add(frame.width_milli);
        height_milli = height_milli.max(frame.height_milli);
        cursor.x_milli = cursor
            .x_milli
            .saturating_add(u32_to_i32_saturating(frame.width_milli));
        placed = true;
    }
    Ok(ViewLayoutFrame::new(width_milli, height_milli))
}

fn lower_layout_stack(
    view_id: &str,
    children: &[ViewExpr],
    state: &mut ViewLoweringState,
    origin: ViewLayoutCursor,
) -> Result<ViewLayoutFrame, ViewSidecarError> {
    let mut frame = ViewLayoutFrame::zero();
    for child in children {
        let mut cursor = origin;
        let child_frame = lower_view_expr(view_id, child, state, &mut cursor)?;
        frame = ViewLayoutFrame::new(
            frame.width_milli.max(child_frame.width_milli),
            frame.height_milli.max(child_frame.height_milli),
        );
    }
    Ok(frame)
}

fn usize_to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn first_part(modifiers: &[ViewModifier]) -> Result<Option<ViewPartLocalName>, ViewSidecarError> {
    modifiers
        .iter()
        .find_map(|modifier| match modifier {
            ViewModifier::Part(part) => Some(part.local_name().text()),
            _ => None,
        })
        .map(|value| {
            ViewPartLocalName::try_new(value.to_owned()).map_err(|source| {
                ViewSidecarError::InvalidViewPartId {
                    value: value.to_owned(),
                    source,
                }
            })
        })
        .transpose()
}

fn next_focus_group_id(view_id: &str, state: &mut ViewLoweringState) -> String {
    let id = format!("group.{view_id}.{}", state.group_counter);
    state.group_counter += 1;
    id
}

fn normalize_entity_ref(reference: &EntityRefSyntax) -> String {
    reference.canonical_body()
}

pub(in crate::app) fn expr_source(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(value)) | Expr::Raw(value) => value.clone(),
        Expr::Path(value) => value.as_label().to_owned(),
        Expr::ShortVariant(value) => format!(".{value}"),
        Expr::EntityRef(reference) => normalize_entity_ref(reference),
        other => format!("{other:?}"),
    }
}

fn lower_fx_application(
    application: &ViewFxApplication,
    state: &mut ViewLoweringState,
) -> Result<Option<ViewProgramInstruction>, ViewSidecarError> {
    let Expr::Call(call) = application.call() else {
        return Ok(None);
    };
    let Some(function) = call.callee().dotted_selector_label() else {
        return Ok(None);
    };
    let name = function.rsplit('.').next().unwrap_or(&function);
    let Some(definition) = state.fx_definitions.get(name).cloned() else {
        return Ok(None);
    };
    let arguments = call
        .args()
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            let (name, value, expected) = match argument {
                CallArg::Named { name, value } => {
                    let expected = definition
                        .parameters
                        .iter()
                        .find_map(|(parameter, ty)| (parameter == name).then_some(*ty))
                        .ok_or_else(|| ViewValueCompileError::UnsupportedExpression {
                            expression: format!(
                                "Fx `{}` has no typed parameter `{name}`",
                                definition.id
                            ),
                        })?;
                    (name.clone(), value.as_ref(), expected)
                }
                CallArg::Positional(value) => {
                    let (name, expected) = definition.parameters.get(ordinal).ok_or_else(|| {
                        ViewValueCompileError::UnsupportedExpression {
                            expression: format!(
                                "Fx `{}` has no positional parameter {ordinal}",
                                definition.id
                            ),
                        }
                    })?;
                    (name.clone(), value, *expected)
                }
                CallArg::Spread { value } => {
                    return Err(ViewValueCompileError::UnsupportedExpression {
                        expression: format!(
                            "Fx `{}` does not accept spread View argument `{value:?}`",
                            definition.id
                        ),
                    }
                    .into());
                }
            };
            Ok(ViewFxArgumentBindingRef {
                parameter: name,
                value_program: state.value_compiler.compile(value, Some(expected))?,
            })
        })
        .collect::<Result<Vec<_>, ViewSidecarError>>()?;
    let key_program = application
        .key()
        .map(|key| state.value_compiler.compile(key, Some(FxRuntimeType::I32)))
        .transpose()?;
    Ok(Some(ViewProgramInstruction::ApplyFx {
        fx: definition.id,
        arguments,
        key_program,
        application_ordinal: application.ordinal().get(),
        source: None,
    }))
}
