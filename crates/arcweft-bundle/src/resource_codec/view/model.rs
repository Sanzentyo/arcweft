use super::runtime_control_style::ViewRuntimeControlVisualStyle;
use crate::resource_codec::types::{CrossSectionRef, SourceRangeRef};
use arcweft_presentation::appearance::{
    PresentationColor, PresentationEnvironmentOverrides, SystemColor, SystemPalette,
    SystemPaletteSet,
};
use arcweft_presentation::fx::{FxId, FxRuntimeType};
use arcweft_source::ProductSourceRef;
use arcweft_text_model::{LineDisplayFrame, RichTextDocument};
pub use arcweft_view::ViewProgramId;
pub use arcweft_view::program::{EventKind, ViewElementKind};
use arcweft_view::program::{ViewElementTextInputKind, ViewVirtualAxis};
use arcweft_view::{
    ViewHandlerCapture, ViewHandlerProgramId, ViewHandlerResult, ViewHandlerValueTypeId,
    ViewPartLocalName, ViewValueProgram, ViewValueProgramId,
};
use core::fmt;
use serde::{Deserialize, Serialize};

mod part;
pub use part::*;

mod source;

mod input;
pub use input::*;

mod style;
pub use style::*;

/// Product View program section decoded from `ViewProgram`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewProgramResource {
    pub program_id: ViewProgramId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<ProductSourceRef>,
    pub definitions: Vec<ViewDefinitionResource>,
    pub value_programs: Vec<ViewValueProgram>,
    pub value_inputs: Vec<ViewValueInputResource>,
    pub instructions: Vec<ViewProgramInstruction>,
    pub handlers: Vec<ViewHandlerRef>,
    pub exported_parts: Vec<ViewExportedPart>,
    pub semantic_targets: Vec<ViewSemanticTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layout_bounds: Vec<ViewLayoutBoundsResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scroll_regions: Vec<ViewScrollRegionResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<ViewSurfaceResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_blocks: Vec<ViewTextBlockResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_buttons: Vec<ViewActionButtonResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_groups: Vec<ViewFocusGroupResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_navigation: Vec<ViewFocusNavigationResource>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ViewProgramInstruction {
    OpenElement {
        element: ViewElementKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<ViewStyleApplicationTarget>,
        part: Option<ViewPartLocalName>,
        key: Option<u64>,
        source: Option<SourceRangeRef>,
    },
    CloseElement,
    EmitText {
        text_source: String,
        text_block: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<ViewStyleApplicationTarget>,
        part: Option<ViewPartLocalName>,
        source: Option<SourceRangeRef>,
    },
    EmitImage {
        image: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<ViewStyleApplicationTarget>,
        part: Option<ViewPartLocalName>,
        source: Option<SourceRangeRef>,
    },
    EmitCustom {
        element: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<ViewStyleApplicationTarget>,
        part: Option<ViewPartLocalName>,
        source: Option<SourceRangeRef>,
    },
    CallView {
        view: ViewDefinitionRef,
        arguments: Vec<ViewCallArgumentBindingRef>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<ViewStyleApplicationTarget>,
        part: Option<ViewPartLocalName>,
        key: Option<u64>,
        source: Option<SourceRangeRef>,
    },
    Branch {
        condition_program: ViewValueProgramId,
        then_span: u32,
        else_span: Option<u32>,
        source: Option<SourceRangeRef>,
    },
    RepeatKeyed {
        source_program: ViewValueProgramId,
        key_program: ViewValueProgramId,
        body_span: u32,
        source: Option<SourceRangeRef>,
    },
    Await {
        source_program: ViewValueProgramId,
        pending_branch: Option<ViewAwaitBranchSpan>,
        ready_branch: Option<ViewAwaitBranchSpan>,
        error_branch: Option<ViewAwaitBranchSpan>,
        denied_branch: Option<ViewAwaitBranchSpan>,
        source: Option<SourceRangeRef>,
    },
    BindLocal {
        binding: String,
        value_program: ViewValueProgramId,
        source: Option<SourceRangeRef>,
    },
    /// Applies a resolved `#[fx] fn -> Fx` graph to the current retained node.
    ApplyFx {
        /// Package-qualified identity of the original Fx declaration.
        fx: FxId,
        arguments: Vec<ViewFxArgumentBindingRef>,
        key_program: Option<ViewValueProgramId>,
        application_ordinal: u32,
        source: Option<SourceRangeRef>,
    },
    BindHandler {
        event: EventKind,
        handler: ViewHandlerProgramId,
        source: Option<SourceRangeRef>,
    },
    AttachSemantic {
        target: String,
        label_text_source: Option<String>,
        source: Option<SourceRangeRef>,
    },
}

impl ViewProgramInstruction {
    /// Ordered Style applications attached to a node-producing instruction.
    pub fn styles(&self) -> &[ViewStyleApplicationTarget] {
        match self {
            Self::OpenElement { styles, .. }
            | Self::EmitText { styles, .. }
            | Self::EmitImage { styles, .. }
            | Self::EmitCustom { styles, .. }
            | Self::CallView { styles, .. } => styles,
            Self::CloseElement
            | Self::Branch { .. }
            | Self::RepeatKeyed { .. }
            | Self::Await { .. }
            | Self::BindLocal { .. }
            | Self::ApplyFx { .. }
            | Self::BindHandler { .. }
            | Self::AttachSemantic { .. } => &[],
        }
    }

    pub(crate) fn styles_mut(&mut self) -> Option<&mut Vec<ViewStyleApplicationTarget>> {
        match self {
            Self::OpenElement { styles, .. }
            | Self::EmitText { styles, .. }
            | Self::EmitImage { styles, .. }
            | Self::EmitCustom { styles, .. }
            | Self::CallView { styles, .. } => Some(styles),
            Self::CloseElement
            | Self::Branch { .. }
            | Self::RepeatKeyed { .. }
            | Self::Await { .. }
            | Self::BindLocal { .. }
            | Self::ApplyFx { .. }
            | Self::BindHandler { .. }
            | Self::AttachSemantic { .. } => None,
        }
    }

    /// Authored part attached to a node-producing instruction, if present.
    pub fn part(&self) -> Option<&ViewPartLocalName> {
        match self {
            Self::OpenElement { part, .. }
            | Self::EmitText { part, .. }
            | Self::EmitImage { part, .. }
            | Self::EmitCustom { part, .. }
            | Self::CallView { part, .. } => part.as_ref(),
            Self::CloseElement
            | Self::Branch { .. }
            | Self::RepeatKeyed { .. }
            | Self::Await { .. }
            | Self::BindLocal { .. }
            | Self::ApplyFx { .. }
            | Self::BindHandler { .. }
            | Self::AttachSemantic { .. } => None,
        }
    }

    pub(crate) fn source(&self) -> Option<&SourceRangeRef> {
        match self {
            Self::OpenElement { source, .. }
            | Self::EmitText { source, .. }
            | Self::EmitImage { source, .. }
            | Self::EmitCustom { source, .. }
            | Self::CallView { source, .. }
            | Self::Branch { source, .. }
            | Self::RepeatKeyed { source, .. }
            | Self::Await { source, .. }
            | Self::BindLocal { source, .. }
            | Self::ApplyFx { source, .. }
            | Self::BindHandler { source, .. }
            | Self::AttachSemantic { source, .. } => source.as_ref(),
            Self::CloseElement => None,
        }
    }

    pub(crate) fn source_mut(&mut self) -> Option<&mut SourceRangeRef> {
        match self {
            Self::OpenElement { source, .. }
            | Self::EmitText { source, .. }
            | Self::EmitImage { source, .. }
            | Self::EmitCustom { source, .. }
            | Self::CallView { source, .. }
            | Self::Branch { source, .. }
            | Self::RepeatKeyed { source, .. }
            | Self::Await { source, .. }
            | Self::BindLocal { source, .. }
            | Self::ApplyFx { source, .. }
            | Self::BindHandler { source, .. }
            | Self::AttachSemantic { source, .. } => source.as_mut(),
            Self::CloseElement => None,
        }
    }
}

/// Typed reactive argument retained for one View-side Fx application.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewFxArgumentBindingRef {
    pub parameter: String,
    pub value_program: ViewValueProgramId,
}

/// Typed positional or named argument retained for a nested View call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewCallArgumentBindingRef {
    pub ordinal: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value_program: ViewValueProgramId,
}

/// One independently mountable Arcweft View definition in the program.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDefinitionResource {
    pub public_id: ViewDefinitionRef,
    pub body: ViewInstructionSpan,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<ViewStyleApplicationTarget>,
    pub parameters: Vec<ViewParameterResource>,
    pub state_schema_hash: u64,
}

impl ViewDefinitionResource {
    /// Whether this definition owns a parameter that accepts typed dialogue state.
    #[must_use]
    pub fn accepts_dialogue_input(&self) -> bool {
        self.parameters
            .iter()
            .any(|parameter| parameter.role == ViewParameterRole::Dialogue)
    }
}

impl Default for ViewProgramResource {
    fn default() -> Self {
        Self {
            program_id: ViewProgramId::try_new("view.empty.program")
                .expect("the built-in empty View program identity is valid"),
            source_refs: Vec::new(),
            definitions: Vec::new(),
            value_programs: Vec::new(),
            value_inputs: Vec::new(),
            instructions: Vec::new(),
            handlers: Vec::new(),
            exported_parts: Vec::new(),
            semantic_targets: Vec::new(),
            layout_bounds: Vec::new(),
            scroll_regions: Vec::new(),
            surfaces: Vec::new(),
            text_blocks: Vec::new(),
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
            adapter_requirements: Vec::new(),
        }
    }
}

/// One ordered View parameter and its optional executable scalar default.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewParameterResource {
    pub ordinal: u16,
    pub name: String,
    pub role: ViewParameterRole,
    /// Exact checked semantic type shared with the handler/AWBC ABI.
    pub semantic_type: ViewHandlerValueTypeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<FxRuntimeType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_slot: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_program: Option<ViewValueProgramId>,
}

/// Closed runtime role of one authored View parameter.
///
/// Nominal source-language types are resolved by semantic analysis before this
/// boundary. Runtime consumers use the role instead of matching source type
/// spellings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewParameterRole {
    Value,
    Dialogue,
}

/// One typed external value projected into the common View value-program schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewValueInputResource {
    pub namespace: ViewValueInputNamespace,
    pub slot: u16,
    pub value_type: FxRuntimeType,
    pub source: ViewValueInputSource,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewValueInputNamespace {
    Parameter,
    State,
}

/// Closed static source inventory for a View value-program input slot.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewValueInputSource {
    DefinitionParameter { view: String, name: String },
    Projection { path: Vec<String> },
    LifetimeProjection { scope: String, path: Vec<String> },
    Local { view: String, name: String },
    RepeatOrdinal { view: String, binding: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewAwaitBranchSpan {
    pub start_offset: u32,
    pub body_span: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewInstructionSpan {
    pub start_instruction: u32,
    pub end_instruction: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewHandlerRef {
    pub program: ViewHandlerProgramId,
    pub captures: Vec<ViewHandlerCapture>,
    pub result: ViewHandlerResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewSemanticTarget {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    pub label_text_source: Option<String>,
    pub source: Option<SourceRangeRef>,
}

/// Resolved logical bounds for View program targets authored by the View DSL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewLayoutBoundsResource {
    pub public_id: String,
    pub kind: ViewLayoutBoundsKind,
    pub rect: ViewLogicalRect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_rect: Option<ViewLogicalRect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewLayoutBoundsKind {
    TextControl,
    SemanticTarget,
}

/// Logical-pixel rectangle serialized in milli-pixel units.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewLogicalRect {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Product-authored player-rendered action button metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewActionButtonResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub label_text_source: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub action: ViewActionButtonActionResource,
    pub bounds: ViewRuntimeButtonBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Product-authored player-rendered text metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewTextBlockResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub text_source: String,
    pub surface: ViewTextSurface,
    pub bounds: ViewTextBlockBounds,
    #[serde(
        default = "default_text_block_selection_policy",
        skip_serializing_if = "is_text_selection_disabled"
    )]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Authored rendering surface for a View text block.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextSurface {
    Text,
    RichText,
}

/// Product-authored player-rendered surface metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSurfaceResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    #[serde(default = "default_surface_element")]
    pub element: ViewElementKind,
    pub bounds: ViewRuntimeSurfaceBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Resolved style binding for one authored View text target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextStyleBinding {
    pub public_id: String,
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub style: ViewRuntimeControlVisualStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextBlockBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Runtime-facing surface emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeSurface {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub element: ViewElementKind,
    pub bounds: ViewRuntimeSurfaceBounds,
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub style: ViewRuntimeControlVisualStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeSurfaceBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewActionButtonActionResource {
    Noop,
    ActionInvoke {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<ViewActionPayloadResource>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewActionPayloadResource {
    LiteralString {
        value: String,
    },
    TextControlProjection {
        input: String,
        field: ViewActionTextControlPayloadField,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewActionTextControlPayloadField {
    Text,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeButtonBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Authored scroll viewport metadata for Arcweft-owned player scrolling.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewScrollRegionResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    pub bounds: ViewLogicalRect,
    pub content_width_milli: u32,
    pub content_height_milli: u32,
    pub axis: ViewScrollAxis,
    #[serde(default, skip_serializing_if = "ViewScrollOverflowPolicy::is_default")]
    pub overflow: ViewScrollOverflowPolicy,
    #[serde(
        default,
        skip_serializing_if = "ViewScrollIndicatorsPolicy::is_default"
    )]
    pub indicators: ViewScrollIndicatorsPolicy,
    #[serde(
        default,
        skip_serializing_if = "ViewScrollOverscrollPolicy::is_default"
    )]
    pub overscroll: ViewScrollOverscrollPolicy,
    #[serde(default, skip_serializing_if = "ViewFocusAutoScrollPolicy::is_default")]
    pub auto_scroll_focus: ViewFocusAutoScrollPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing scroll viewport emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeScrollRegion {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    pub bounds: ViewRuntimeScrollRegionBounds,
    pub content_width_milli: u32,
    pub content_height_milli: u32,
    pub axis: ViewScrollAxis,
    #[serde(default, skip_serializing_if = "ViewScrollOverflowPolicy::is_default")]
    pub overflow: ViewScrollOverflowPolicy,
    #[serde(
        default,
        skip_serializing_if = "ViewScrollIndicatorsPolicy::is_default"
    )]
    pub indicators: ViewScrollIndicatorsPolicy,
    #[serde(
        default,
        skip_serializing_if = "ViewScrollOverscrollPolicy::is_default"
    )]
    pub overscroll: ViewScrollOverscrollPolicy,
    #[serde(default, skip_serializing_if = "ViewFocusAutoScrollPolicy::is_default")]
    pub auto_scroll_focus: ViewFocusAutoScrollPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeScrollRegionBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewScrollAxis {
    Vertical,
    Horizontal,
}

impl ViewScrollAxis {
    /// Corresponding primary axis used by retained-list virtualization.
    pub const fn virtual_axis(self) -> ViewVirtualAxis {
        match self {
            Self::Vertical => ViewVirtualAxis::Vertical,
            Self::Horizontal => ViewVirtualAxis::Horizontal,
        }
    }

    #[must_use]
    pub fn from_author_symbol(value: &str) -> Option<Self> {
        match normalized_author_symbol(value).as_str() {
            "vertical" | "y" | "block" => Some(Self::Vertical),
            "horizontal" | "x" | "inline" => Some(Self::Horizontal),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_unsupported_dual_axis_symbol(value: &str) -> bool {
        matches!(
            normalized_author_symbol(value).as_str(),
            "both" | "xy" | "yx" | "all" | "2d" | "both-axes"
        )
    }

    #[must_use]
    pub const fn scrolls_x(self) -> bool {
        matches!(self, Self::Horizontal)
    }

    #[must_use]
    pub const fn scrolls_y(self) -> bool {
        matches!(self, Self::Vertical)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewScrollOverflowPolicy {
    #[default]
    Auto,
    Scroll,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewScrollIndicatorsPolicy {
    #[default]
    Auto,
    Visible,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewScrollOverscrollPolicy {
    #[default]
    Clamp,
    Contain,
    Elastic,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusAutoScrollPolicy {
    #[default]
    Nearest,
    Start,
    End,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeActionButton {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub label: String,
    pub enabled: bool,
    pub bounds: ViewRuntimeButtonBounds,
    pub action: ViewRuntimeActionButtonAction,
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub style: ViewRuntimeControlVisualStyle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewRuntimeActionButtonAction {
    Noop,
    ActionInvoke {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<ViewActionPayloadResource>,
    },
    /// Mount/frame-scoped typed handler route sealed by the View runtime.
    ViewHandler {
        event: EventKind,
        route: arcweft_view::ViewHandlerRouteId,
    },
}

/// Authored focus group metadata for Arcweft-owned player navigation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewFocusGroupResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub policy: ViewFocusGroupPolicy,
    #[serde(default)]
    pub initial: ViewFocusInitialPolicy,
    #[serde(default)]
    pub wrap: ViewFocusWrapPolicy,
    #[serde(default)]
    pub disabled_skip: ViewFocusSkipPolicy,
    #[serde(default)]
    pub hidden_skip: ViewFocusSkipPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing focus group emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeFocusGroup {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub policy: ViewFocusGroupPolicy,
    pub initial: ViewFocusInitialPolicy,
    pub wrap: ViewFocusWrapPolicy,
    pub disabled_skip: ViewFocusSkipPolicy,
    pub hidden_skip: ViewFocusSkipPolicy,
}

/// Focus target and directional edges authored by the View DSL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewFocusNavigationResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<ViewFocusNavigationEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing focus navigation emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeFocusNavigation {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<ViewRuntimeFocusNavigationEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewFocusNavigationEdge {
    pub direction: ViewFocusDirection,
    pub target: ViewFocusTargetResolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeFocusNavigationEdge {
    pub direction: ViewFocusDirection,
    pub target: ViewFocusTargetResolution,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Previous,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusTargetResolution {
    Explicit { target: String },
    Auto,
    None,
    GroupBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusGroupPolicy {
    #[default]
    Normal,
    Trap,
    Modal,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusInitialPolicy {
    #[default]
    Auto,
    First,
    Last,
    Explicit {
        target: String,
    },
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusWrapPolicy {
    #[default]
    Wrap,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewFocusSkipPolicy {
    #[default]
    Skip,
    Stop,
}

/// Product View text-source section decoded from `ViewText`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewTextResource {
    pub sources: Vec<ViewTextSourceRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub localized: Vec<ViewLocalizedTextResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rich_text_documents: Vec<ViewRichTextDocumentResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub display_frames: Vec<ViewDisplayFrameResource>,
    pub source_ranges: Vec<SourceRangeRef>,
    pub reveal_policies: Vec<ViewTextRevealPolicyBinding>,
    pub cursor_policies: Vec<ViewTextCursorPolicyBinding>,
    pub redactions: Vec<ViewSecureRedactionMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextSourceRecord {
    pub public_id: String,
    pub kind: ViewTextSourceKind,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextSourceKind {
    Literal {
        value: String,
    },
    Projection {
        path: Vec<String>,
    },
    Local {
        name: String,
    },
    Localized {
        key: String,
        locale: Option<String>,
    },
    RichTextDocument {
        document: String,
    },
    DisplayFrame {
        frame: String,
    },
    /// A typed projection from the dialogue input bound to this View mount.
    ///
    /// The input remains a resolved `LineDisplayFrame` at runtime. It is not
    /// coerced through `RuntimeValue` or reconstructed from a string payload.
    Dialogue {
        parameter: String,
        projection: DialogueTextProjection,
    },
}

/// Field of the nominal dialogue input consumed by an authored View text node.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueTextProjection {
    CharacterDisplayName,
    Content,
}

/// One locale-exact `RichText` document available to View text resolution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewLocalizedTextResource {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    pub document: RichTextDocument,
}

/// One reusable static `RichText` document available to View text resolution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewRichTextDocumentResource {
    pub public_id: String,
    pub document: RichTextDocument,
}

/// One reusable resolved display frame and selected input-gated stage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewDisplayFrameResource {
    pub public_id: String,
    pub frame: LineDisplayFrame,
    pub stage_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextRevealPolicyBinding {
    pub text_source: String,
    pub policy: ViewTextRevealPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextRevealPolicy {
    #[default]
    Immediate,
    Typewriter,
    ManualAdvance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextCursorPolicyBinding {
    pub text_source: String,
    pub policy: ViewTextCursorPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextCursorPolicy {
    Hidden,
    #[default]
    Inherit,
    Visible,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewSecureRedactionMetadata {
    pub text_source: String,
    pub classification: ViewObserveClassification,
    pub replacement: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewObserveClassification {
    #[default]
    Public,
    AgentMasked,
    Secret,
}

/// Product theme/environment section decoded from `ViewTheme`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewThemeResource {
    pub palette_overrides: Vec<SystemColorOverride>,
    pub environment: PresentationEnvironmentOverrides,
    pub dark_mode_visual_golden_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemColorOverride {
    pub color: SystemColor,
    pub light: Option<PresentationColor>,
    pub dark: Option<PresentationColor>,
    pub source: Option<SourceRangeRef>,
}

impl ViewThemeResource {
    /// Checked optional environment values supplied by this theme.
    pub const fn environment_overrides(&self) -> PresentationEnvironmentOverrides {
        self.environment
    }

    /// Applies typed light/dark overrides to the engine palette inventory.
    pub fn system_palette_set(&self) -> SystemPaletteSet {
        self.palette_overrides.iter().fold(
            SystemPaletteSet::ENGINE_DEFAULT,
            |mut palettes, entry| {
                if let Some(color) = entry.light {
                    set_system_color(&mut palettes.light, entry.color, color);
                }
                if let Some(color) = entry.dark {
                    set_system_color(&mut palettes.dark, entry.color, color);
                }
                palettes
            },
        )
    }
}

fn set_system_color(palette: &mut SystemPalette, role: SystemColor, color: PresentationColor) {
    match role {
        SystemColor::Canvas => palette.canvas = color,
        SystemColor::CanvasText => palette.canvas_text = color,
        SystemColor::Surface => palette.surface = color,
        SystemColor::SurfaceText => palette.surface_text = color,
        SystemColor::RaisedSurface => palette.raised_surface = color,
        SystemColor::MutedText => palette.muted_text = color,
        SystemColor::Border => palette.border = color,
        SystemColor::Accent => palette.accent = color,
        SystemColor::AccentText => palette.accent_text = color,
        SystemColor::FocusRing => palette.focus_ring = color,
        SystemColor::Selection => palette.selection = color,
        SystemColor::SelectionText => palette.selection_text = color,
        SystemColor::Danger => palette.danger = color,
        SystemColor::Warning => palette.warning = color,
        SystemColor::Success => palette.success = color,
    }
}

impl ViewInstructionSpan {
    pub const fn new(start_instruction: u32, end_instruction: u32) -> Self {
        Self {
            start_instruction,
            end_instruction,
        }
    }
}

impl ViewTextResource {
    pub fn literal_text(&self, public_id: &str) -> Option<&str> {
        self.sources
            .iter()
            .find(|source| source.public_id == public_id)
            .and_then(|source| match &source.kind {
                ViewTextSourceKind::Literal { value } => Some(value.as_str()),
                ViewTextSourceKind::Projection { .. }
                | ViewTextSourceKind::Local { .. }
                | ViewTextSourceKind::Localized { .. }
                | ViewTextSourceKind::RichTextDocument { .. }
                | ViewTextSourceKind::DisplayFrame { .. }
                | ViewTextSourceKind::Dialogue { .. } => None,
            })
    }

    /// Resolves an exact localization key/locale pair without implicit locale fallback.
    pub fn localized_document(&self, key: &str, locale: Option<&str>) -> Option<&RichTextDocument> {
        self.localized
            .iter()
            .find(|entry| entry.key == key && entry.locale.as_deref() == locale)
            .map(|entry| &entry.document)
    }

    /// Resolves one reusable `RichText` document by stable public identity.
    pub fn rich_text_document(&self, public_id: &str) -> Option<&RichTextDocument> {
        self.rich_text_documents
            .iter()
            .find(|entry| entry.public_id == public_id)
            .map(|entry| &entry.document)
    }

    /// Resolves one reusable display frame and its selected stage.
    pub fn display_frame(&self, public_id: &str) -> Option<&ViewDisplayFrameResource> {
        self.display_frames
            .iter()
            .find(|entry| entry.public_id == public_id)
    }
}

impl ViewProgramResource {
    pub fn handler_ref(&self, program: ViewHandlerProgramId) -> Option<&ViewHandlerRef> {
        self.handlers
            .iter()
            .find(|handler| handler.program == program)
    }

    pub fn runtime_action_buttons(
        &self,
        text: Option<&ViewTextResource>,
    ) -> Vec<ViewRuntimeActionButton> {
        self.action_buttons
            .iter()
            .map(|button| ViewRuntimeActionButton {
                public_id: button.public_id.clone(),
                target: button.public_id.clone(),
                view: button.view.clone(),
                containing_scroll_region: button.containing_scroll_region.clone(),
                label: text
                    .and_then(|resource| resource.literal_text(&button.label_text_source))
                    .unwrap_or(&button.public_id)
                    .to_owned(),
                enabled: button.enabled,
                bounds: button.bounds,
                action: match &button.action {
                    ViewActionButtonActionResource::Noop => ViewRuntimeActionButtonAction::Noop,
                    ViewActionButtonActionResource::ActionInvoke { action, payload } => {
                        ViewRuntimeActionButtonAction::ActionInvoke {
                            action: action.clone(),
                            payload: payload.clone(),
                        }
                    }
                },
                style: ViewRuntimeControlVisualStyle::default(),
            })
            .collect()
    }

    pub fn runtime_surfaces(&self) -> Vec<ViewRuntimeSurface> {
        self.surfaces
            .iter()
            .map(ViewSurfaceResource::runtime_surface)
            .collect()
    }

    pub fn runtime_focus_groups(&self) -> Vec<ViewRuntimeFocusGroup> {
        self.focus_groups
            .iter()
            .map(|group| ViewRuntimeFocusGroup {
                public_id: group.public_id.clone(),
                view: group.view.clone(),
                parent: group.parent.clone(),
                policy: group.policy,
                initial: group.initial.clone(),
                wrap: group.wrap,
                disabled_skip: group.disabled_skip,
                hidden_skip: group.hidden_skip,
            })
            .collect()
    }

    pub fn runtime_focus_navigation(&self) -> Vec<ViewRuntimeFocusNavigation> {
        self.focus_navigation
            .iter()
            .map(|target| ViewRuntimeFocusNavigation {
                public_id: target.public_id.clone(),
                view: target.view.clone(),
                group: target.group.clone(),
                edges: target
                    .edges
                    .iter()
                    .map(|edge| ViewRuntimeFocusNavigationEdge {
                        direction: edge.direction,
                        target: edge.target.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn runtime_scroll_regions(&self) -> Vec<ViewRuntimeScrollRegion> {
        self.scroll_regions
            .iter()
            .map(ViewScrollRegionResource::runtime_scroll_region)
            .collect()
    }

    pub fn text_control_bounds_for(&self, public_id: &str) -> Option<ViewRuntimeTextControlBounds> {
        self.layout_bounds
            .iter()
            .find(|bounds| bounds.is_text_control_for(public_id))
            .map(ViewLayoutBoundsResource::runtime_text_control_bounds)
    }

    pub fn semantic_target_bounds_for(
        &self,
        public_id: &str,
    ) -> Option<ViewRuntimeTextControlBounds> {
        self.layout_bounds
            .iter()
            .find(|bounds| bounds.is_semantic_target_for(public_id))
            .map(ViewLayoutBoundsResource::runtime_text_control_bounds)
    }
}

impl ViewLayoutBoundsResource {
    pub fn text_control(public_id: impl Into<String>, rect: ViewLogicalRect) -> Self {
        Self::new(public_id, ViewLayoutBoundsKind::TextControl, rect)
    }

    pub fn semantic_target(public_id: impl Into<String>, rect: ViewLogicalRect) -> Self {
        Self::new(public_id, ViewLayoutBoundsKind::SemanticTarget, rect)
    }

    pub fn new(
        public_id: impl Into<String>,
        kind: ViewLayoutBoundsKind,
        rect: ViewLogicalRect,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            kind,
            rect,
            hit_rect: None,
            source: None,
        }
    }

    #[must_use]
    pub const fn with_hit_rect(mut self, hit_rect: ViewLogicalRect) -> Self {
        self.hit_rect = Some(hit_rect);
        self
    }

    pub fn is_text_control_for(&self, public_id: &str) -> bool {
        self.kind == ViewLayoutBoundsKind::TextControl && self.public_id == public_id
    }

    pub fn is_semantic_target_for(&self, public_id: &str) -> bool {
        self.kind == ViewLayoutBoundsKind::SemanticTarget && self.public_id == public_id
    }

    pub fn identity_key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.public_id)
    }

    pub const fn is_valid(&self) -> bool {
        self.rect.is_valid()
            && match self.hit_rect {
                Some(hit_rect) => hit_rect.is_valid(),
                None => true,
            }
    }

    pub fn runtime_text_control_bounds(&self) -> ViewRuntimeTextControlBounds {
        self.hit_rect
            .unwrap_or(self.rect)
            .runtime_text_control_bounds()
    }
}

impl ViewTextBlockResource {
    pub fn new(
        public_id: impl Into<String>,
        view: Option<String>,
        containing_scroll_region: Option<String>,
        text_source: impl Into<String>,
        bounds: ViewTextBlockBounds,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            view,
            containing_scroll_region,
            text_source: text_source.into(),
            surface: ViewTextSurface::Text,
            bounds,
            selection_policy: ViewTextSelectionPolicy::Disabled,
            source: None,
        }
    }

    #[must_use]
    pub const fn with_surface(mut self, surface: ViewTextSurface) -> Self {
        self.surface = surface;
        self
    }

    pub const fn is_valid(&self) -> bool {
        self.bounds.is_valid()
    }
}

const fn default_text_block_selection_policy() -> ViewTextSelectionPolicy {
    ViewTextSelectionPolicy::Disabled
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_text_selection_disabled(policy: &ViewTextSelectionPolicy) -> bool {
    matches!(policy, ViewTextSelectionPolicy::Disabled)
}

impl ViewSurfaceResource {
    pub fn new(
        public_id: impl Into<String>,
        view: Option<String>,
        containing_scroll_region: Option<String>,
        element: ViewElementKind,
        bounds: ViewRuntimeSurfaceBounds,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            view,
            containing_scroll_region,
            element,
            bounds,
            source: None,
        }
    }

    pub fn runtime_surface(&self) -> ViewRuntimeSurface {
        ViewRuntimeSurface {
            public_id: self.public_id.clone(),
            target: self.public_id.clone(),
            view: self.view.clone(),
            containing_scroll_region: self.containing_scroll_region.clone(),
            element: self.element,
            bounds: self.bounds,
            style: ViewRuntimeControlVisualStyle::default(),
        }
    }

    pub const fn is_valid(&self) -> bool {
        self.bounds.is_valid()
    }
}

impl ViewScrollRegionResource {
    pub fn new(
        public_id: impl Into<String>,
        view: Option<String>,
        bounds: ViewLogicalRect,
        content_width_milli: u32,
        content_height_milli: u32,
        axis: ViewScrollAxis,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            view,
            bounds,
            content_width_milli,
            content_height_milli,
            axis,
            overflow: ViewScrollOverflowPolicy::default(),
            indicators: ViewScrollIndicatorsPolicy::default(),
            overscroll: ViewScrollOverscrollPolicy::default(),
            auto_scroll_focus: ViewFocusAutoScrollPolicy::default(),
            source: None,
        }
    }

    #[must_use]
    pub const fn with_overflow(mut self, overflow: ViewScrollOverflowPolicy) -> Self {
        self.overflow = overflow;
        self
    }

    #[must_use]
    pub const fn with_indicators(mut self, indicators: ViewScrollIndicatorsPolicy) -> Self {
        self.indicators = indicators;
        self
    }

    #[must_use]
    pub const fn with_overscroll(mut self, overscroll: ViewScrollOverscrollPolicy) -> Self {
        self.overscroll = overscroll;
        self
    }

    #[must_use]
    pub const fn with_auto_scroll_focus(mut self, policy: ViewFocusAutoScrollPolicy) -> Self {
        self.auto_scroll_focus = policy;
        self
    }

    pub const fn is_valid(&self) -> bool {
        self.bounds.is_valid() && self.content_width_milli > 0 && self.content_height_milli > 0
    }

    pub fn runtime_scroll_region(&self) -> ViewRuntimeScrollRegion {
        ViewRuntimeScrollRegion {
            public_id: self.public_id.clone(),
            target: self.public_id.clone(),
            view: self.view.clone(),
            bounds: self.bounds.runtime_scroll_region_bounds(),
            content_width_milli: self.content_width_milli,
            content_height_milli: self.content_height_milli,
            axis: self.axis,
            overflow: self.overflow,
            indicators: self.indicators,
            overscroll: self.overscroll,
            auto_scroll_focus: self.auto_scroll_focus,
        }
    }
}

impl ViewScrollOverflowPolicy {
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Auto)
    }

    pub const fn scroll_enabled(self) -> bool {
        matches!(self, Self::Auto | Self::Scroll)
    }
}

impl ViewScrollIndicatorsPolicy {
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Auto)
    }

    #[must_use]
    pub fn from_author_symbol(value: &str) -> Option<Self> {
        match normalized_author_symbol(value).as_str() {
            "auto" => Some(Self::Auto),
            "visible" | "show" | "shown" | "always" => Some(Self::Visible),
            "hidden" | "hide" | "none" | "never" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl ViewScrollOverscrollPolicy {
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Clamp)
    }

    #[must_use]
    pub fn from_author_symbol(value: &str) -> Option<Self> {
        match normalized_author_symbol(value).as_str() {
            "clamp" | "none" => Some(Self::Clamp),
            "contain" | "contained" => Some(Self::Contain),
            "elastic" | "bounce" => Some(Self::Elastic),
            _ => None,
        }
    }
}

impl ViewFocusAutoScrollPolicy {
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Nearest)
    }

    #[must_use]
    pub fn from_author_symbol(value: &str) -> Option<Self> {
        match normalized_author_symbol(value).as_str() {
            "nearest" | "auto" => Some(Self::Nearest),
            "start" | "leading" => Some(Self::Start),
            "end" | "trailing" => Some(Self::End),
            "disabled" | "disable" | "none" | "off" | "false" => Some(Self::Disabled),
            _ => None,
        }
    }
}

fn normalized_author_symbol(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches('.')
        .replace('_', "-")
        .to_ascii_lowercase()
}

impl ViewLayoutBoundsKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextControl => "text_control",
            Self::SemanticTarget => "semantic_target",
        }
    }
}

impl ViewLogicalRect {
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }

    pub const fn from_px(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self::new(
            x.saturating_mul(1_000),
            y.saturating_mul(1_000),
            width.saturating_mul(1_000),
            height.saturating_mul(1_000),
        )
    }

    pub const fn is_valid(self) -> bool {
        self.width_milli > 0 && self.height_milli > 0
    }

    pub const fn runtime_text_control_bounds(self) -> ViewRuntimeTextControlBounds {
        ViewRuntimeTextControlBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }

    pub const fn runtime_button_bounds(self) -> ViewRuntimeButtonBounds {
        ViewRuntimeButtonBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }

    pub const fn runtime_scroll_region_bounds(self) -> ViewRuntimeScrollRegionBounds {
        ViewRuntimeScrollRegionBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }

    pub const fn runtime_surface_bounds(self) -> ViewRuntimeSurfaceBounds {
        ViewRuntimeSurfaceBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }
}

impl ViewFocusDirection {
    pub const fn is_spatial(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }

    pub const fn linear_delta(self) -> Option<isize> {
        match self {
            Self::Next => Some(1),
            Self::Previous => Some(-1),
            Self::Up | Self::Down | Self::Left | Self::Right => None,
        }
    }
}

impl ViewFocusTargetResolution {
    pub fn explicit_target(&self) -> Option<&str> {
        match self {
            Self::Explicit { target } => Some(target.as_str()),
            Self::Auto | Self::None | Self::GroupBoundary => None,
        }
    }
}

impl ViewFocusInitialPolicy {
    pub fn explicit_target(&self) -> Option<&str> {
        match self {
            Self::Explicit { target } => Some(target.as_str()),
            Self::Auto | Self::First | Self::Last | Self::None => None,
        }
    }
}

impl ViewFocusWrapPolicy {
    pub const fn allows_wrap(self) -> bool {
        matches!(self, Self::Wrap)
    }
}

impl ViewRuntimeButtonBounds {
    const DEFAULT_STACK_X_MILLI: i32 = 48_000;
    const DEFAULT_STACK_Y_MILLI: i32 = 112_000;
    const DEFAULT_STACK_GAP_MILLI: i32 = 16_000;
    const DEFAULT_SLOT_PITCH_MILLI: i32 = 56_000;
    const DEFAULT_WIDTH_MILLI: u32 = 180_000;
    const DEFAULT_HEIGHT_MILLI: u32 = 44_000;

    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }

    pub fn default_slot(index: usize) -> Self {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        Self::new(
            Self::DEFAULT_STACK_X_MILLI,
            Self::DEFAULT_STACK_Y_MILLI
                .saturating_add(index.saturating_mul(Self::DEFAULT_SLOT_PITCH_MILLI)),
            Self::DEFAULT_WIDTH_MILLI,
            Self::DEFAULT_HEIGHT_MILLI,
        )
    }

    pub fn default_submit_slot(
        input_bounds: ViewRuntimeTextControlBounds,
        input_kind: ViewInputKind,
        ordinal_for_input: usize,
    ) -> Self {
        let ordinal = i32::try_from(ordinal_for_input).unwrap_or(i32::MAX);
        let pitch = u32_to_i32_saturating(Self::DEFAULT_WIDTH_MILLI)
            .saturating_add(Self::DEFAULT_STACK_GAP_MILLI);
        let cross_axis_offset = ordinal.saturating_mul(pitch);
        if input_kind.is_multiline() {
            return Self::new(
                input_bounds.x_milli.saturating_add(cross_axis_offset),
                input_bounds
                    .y_milli
                    .saturating_add(u32_to_i32_saturating(input_bounds.height_milli))
                    .saturating_add(Self::DEFAULT_STACK_GAP_MILLI),
                Self::DEFAULT_WIDTH_MILLI,
                Self::DEFAULT_HEIGHT_MILLI,
            );
        }
        let centered_y = input_bounds.y_milli.saturating_add(
            u32_to_i32_saturating(input_bounds.height_milli)
                .saturating_sub(u32_to_i32_saturating(Self::DEFAULT_HEIGHT_MILLI))
                / 2,
        );
        Self::new(
            input_bounds
                .x_milli
                .saturating_add(u32_to_i32_saturating(input_bounds.width_milli))
                .saturating_add(Self::DEFAULT_STACK_GAP_MILLI)
                .saturating_add(cross_axis_offset),
            centered_y,
            Self::DEFAULT_WIDTH_MILLI,
            Self::DEFAULT_HEIGHT_MILLI,
        )
    }
}

impl ViewTextBlockBounds {
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }

    pub const fn from_px(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self::new(
            x.saturating_mul(1_000),
            y.saturating_mul(1_000),
            width.saturating_mul(1_000),
            height.saturating_mul(1_000),
        )
    }

    pub const fn is_valid(self) -> bool {
        self.width_milli > 0 && self.height_milli > 0
    }
}

impl ViewRuntimeSurfaceBounds {
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }

    pub const fn from_px(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self::new(
            x.saturating_mul(1_000),
            y.saturating_mul(1_000),
            width.saturating_mul(1_000),
            height.saturating_mul(1_000),
        )
    }

    pub const fn is_valid(self) -> bool {
        self.width_milli > 0 && self.height_milli > 0
    }
}

impl ViewRuntimeScrollRegionBounds {
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }
}

const fn default_true() -> bool {
    true
}

const fn default_surface_element() -> ViewElementKind {
    ViewElementKind::Panel
}

fn u32_to_i32_saturating(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_text_control_carries_authored_change_and_submit_handlers() {
        let submit = ViewHandlerProgramId::from_checked_digest([1; 32]);
        let change = ViewHandlerProgramId::from_checked_digest([2; 32]);
        let result = ViewHandlerResult::new(
            arcweft_view::ViewHandlerResultRole::DialogueAction,
            arcweft_view::ViewHandlerValueTypeId::from_semantic_digest([3; 32]),
        );
        let mut program = ViewProgramResource::default();
        program.handlers = vec![
            ViewHandlerRef {
                program: submit,
                captures: Vec::new(),
                result,
            },
            ViewHandlerRef {
                program: change,
                captures: Vec::new(),
                result,
            },
        ];
        let input = ViewInputOptions {
            public_id: "field.name".to_owned(),
            view: None,
            containing_scroll_region: None,
            kind: ViewInputKind::TextField,
            value_text_source: "text.name".to_owned(),
            placeholder_text_source: None,
            purpose: ViewInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: false,
            selection_policy: ViewTextSelectionPolicy::Enabled,
            shortcut_policy: ViewTextShortcutPolicy::Enabled,
            tab_policy: ViewTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: ViewTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: ViewSecureInputPolicy::Plain,
            composition_on_blur: CompositionOnBlurPolicy::Commit,
            submit_handler: Some(submit),
            change_handler: Some(change),
            adapter_requirements: Vec::new(),
        };

        let control = input.runtime_text_control(0, None, Some(&program));

        assert_eq!(control.handlers.change.unwrap().program, change);
        assert_eq!(control.handlers.submit.unwrap().program, submit);
    }
}
