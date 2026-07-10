use super::runtime_control_style::ViewRuntimeControlStyle;
use crate::BundleVirtualFileRef;
use crate::container::BundleDigest;
use crate::resource_codec::types::{CrossSectionRef, DigestRef, SourceRangeRef};
pub use arcweft_view::program::ViewElementKind;
use arcweft_view::program::ViewElementTextInputKind;
use core::fmt;
use serde::{Deserialize, Serialize};

/// Product View program section decoded from `ViewProgram`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewProgramResource {
    pub program_id: String,
    pub root_view: String,
    pub instructions: Vec<ViewProgramInstruction>,
    pub child_spans: Vec<ViewChildSpan>,
    pub handlers: Vec<ViewHandlerRef>,
    pub state_schema_hashes: Vec<ViewStateSchemaHashRef>,
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
#[serde(rename_all = "snake_case")]
pub enum ViewProgramInstruction {
    OpenElement {
        element: ViewElementKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        style: Option<String>,
        part: Option<String>,
        key: Option<u64>,
        source: Option<SourceRangeRef>,
    },
    CloseElement,
    EmitText {
        text_source: String,
        style: Option<String>,
        part: Option<String>,
        source: Option<SourceRangeRef>,
    },
    EmitImage {
        image: String,
        style: Option<String>,
        part: Option<String>,
        source: Option<SourceRangeRef>,
    },
    EmitCustom {
        element: String,
        style: Option<String>,
        part: Option<String>,
        source: Option<SourceRangeRef>,
    },
    CallView {
        view: String,
        child_span: u32,
        props_schema: Option<DigestRef>,
        style: Option<String>,
        part: Option<String>,
        key: Option<u64>,
        source: Option<SourceRangeRef>,
    },
    Branch {
        condition_schema: DigestRef,
        then_span: u32,
        else_span: Option<u32>,
        source: Option<SourceRangeRef>,
    },
    RepeatKeyed {
        source_schema: DigestRef,
        key_schema: DigestRef,
        body_span: u32,
        source: Option<SourceRangeRef>,
    },
    Await {
        source_schema: DigestRef,
        pending_branch: Option<ViewAwaitBranchSpan>,
        ready_branch: Option<ViewAwaitBranchSpan>,
        error_branch: Option<ViewAwaitBranchSpan>,
        denied_branch: Option<ViewAwaitBranchSpan>,
        source: Option<SourceRangeRef>,
    },
    BindLocal {
        pattern_schema: DigestRef,
        value_schema: DigestRef,
        source: Option<SourceRangeRef>,
    },
    ApplyStyle {
        style: ViewStyleApplyRef,
        source: Option<SourceRangeRef>,
    },
    BindHandler {
        event: String,
        handler: String,
        source: Option<SourceRangeRef>,
    },
    AttachSemantic {
        target: String,
        label_text_source: Option<String>,
        source: Option<SourceRangeRef>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewAwaitBranchSpan {
    pub pattern_schema: DigestRef,
    pub body_span: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleApplyRef {
    Named(String),
    InlineArcweft { patch_id: u32 },
    InlineCss { patch_id: u32 },
}

impl ViewStyleApplyRef {
    pub fn runtime_style_part(&self) -> String {
        match self {
            Self::Named(style) => style.clone(),
            Self::InlineArcweft { patch_id } | Self::InlineCss { patch_id } => {
                Self::inline_patch_part(*patch_id)
            }
        }
    }

    pub fn inline_patch_part(patch_id: u32) -> String {
        format!("style.inline.patch.{patch_id}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewChildSpan {
    pub start_instruction: u32,
    pub end_instruction: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewHandlerRef {
    pub handler_id: String,
    pub event: String,
    pub awbc_function_index: u32,
    pub handler_abi: BundleDigest,
    pub function_binding: Option<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStateSchemaHashRef {
    pub public_id: Option<String>,
    pub hash: BundleDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewExportedPart {
    pub part_id: String,
    pub public_name: String,
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
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Product-authored player-rendered text metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextBlockResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub text_source: String,
    pub bounds: ViewRuntimeTextBlockBounds,
    #[serde(
        default = "default_text_block_selection_policy",
        skip_serializing_if = "is_text_selection_disabled"
    )]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Product-authored player-rendered surface metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing text block emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextBlock {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub text: String,
    pub bounds: ViewRuntimeTextBlockBounds,
    #[serde(default, skip_serializing_if = "is_text_selection_disabled")]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default, skip_serializing_if = "ViewRuntimeControlStyle::is_default")]
    pub style: ViewRuntimeControlStyle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextBlockBounds {
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
    #[serde(default, skip_serializing_if = "ViewRuntimeControlStyle::is_default")]
    pub style: ViewRuntimeControlStyle,
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
    #[serde(default, skip_serializing_if = "ViewRuntimeControlStyle::is_default")]
    pub style: ViewRuntimeControlStyle,
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

/// Product style section decoded from `ViewStyle`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleResource {
    pub style_program_id: String,
    pub arcweft_sources: Vec<StyleSourceIdentity>,
    pub css_sources: Vec<StyleSourceIdentity>,
    pub tokens: Vec<ViewStyleToken>,
    pub rules: Vec<ViewStyleRule>,
    pub part_rules: Vec<ViewPartStyleRule>,
    pub environment_predicates: Vec<ViewEnvironmentPredicate>,
    pub source_map_refs: Vec<SourceRangeRef>,
    pub external_css_descriptors: Vec<ExternalCssDescriptorRef>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StyleSourceIdentity {
    pub public_id: String,
    pub syntax: StyleSyntax,
    pub identity: StyleSourceRef,
    pub content_digest: Option<BundleDigest>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleSyntax {
    #[default]
    Arcweft,
    Css,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleSourceRef {
    Inline { source_digest: BundleDigest },
    File { path: String },
    EmbeddedFile { file: BundleVirtualFileRef },
    Section { reference: CrossSectionRef },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleToken {
    pub public_id: String,
    pub value: ViewStyleValue,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleRule {
    pub selector: ViewStyleSelector,
    pub declarations: Vec<ViewStyleDeclaration>,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewPartStyleRule {
    pub part: String,
    pub selector: ViewStyleSelector,
    pub declarations: Vec<ViewStyleDeclaration>,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleSelector {
    pub parts: Vec<ViewStyleSelectorPart>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleSelectorPart {
    Element(ViewElementKind),
    Part(String),
    State(ViewElementState),
    Interaction(ViewInteractionState),
    Environment(ViewEnvironmentPredicate),
    Descendant,
    Child,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewElementState {
    FocusVisible,
    ReadOnly,
    Invalid,
    Composing,
    PlaceholderShown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewInteractionState {
    Hover,
    Active,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewStyleDeclaration {
    pub property: String,
    pub value: ViewStyleValue,
    pub op: StyleAssignOp,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleAssignOp {
    #[default]
    Replace,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewStyleValue {
    Token(String),
    SystemColor(SystemColor),
    Rgba(RgbaColor),
    Milli(i32),
    Text(String),
    List(Vec<ViewStyleValue>),
    Resource(String),
    Digest(BundleDigest),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemColor {
    Canvas,
    CanvasText,
    Panel,
    PanelText,
    RaisedPanel,
    MutedText,
    Border,
    Accent,
    AccentText,
    FocusRing,
    Selection,
    SelectionText,
    Danger,
    Warning,
    Success,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewEnvironmentPredicate {
    ColorScheme(ColorSchemeDefault),
    Contrast(ContrastPreference),
    ReduceMotion(bool),
    TextScaleAtLeastMilli(u32),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSchemeDefault {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastPreference {
    #[default]
    Standard,
    More,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExternalCssDescriptorRef {
    pub public_id: String,
    pub identity: ExternalCssIdentity,
    pub source_map: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCssIdentity {
    File { path: String },
    EmbeddedFile { file: BundleVirtualFileRef },
    Section { reference: CrossSectionRef },
}

/// Product View text-source section decoded from `ViewText`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewTextResource {
    pub sources: Vec<ViewTextSourceRecord>,
    pub display_frame_refs: Vec<CrossSectionRef>,
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
    Literal { value: String },
    Localized { key: String, locale: Option<String> },
    RichTextDocument { document: CrossSectionRef },
    DisplayFrame { frame: CrossSectionRef },
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

/// Product text-input metadata section decoded from `ViewInput`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewInputResource {
    pub options: Vec<ViewInputOptions>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewInputOptions {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub kind: ViewInputKind,
    pub value_text_source: String,
    pub placeholder_text_source: Option<String>,
    pub purpose: ViewInputPurpose,
    pub autocorrect: TextAssistPolicy,
    pub spellcheck: TextAssistPolicy,
    pub capitalization: TextCapitalization,
    pub enter_key: EnterKeyHint,
    pub multiline: bool,
    #[serde(default)]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default)]
    pub shortcut_policy: ViewTextShortcutPolicy,
    #[serde(default)]
    pub tab_policy: ViewTextTabPolicy,
    #[serde(default)]
    pub vertical_navigation_policy: ViewTextVerticalNavigationPolicy,
    pub secure_policy: ViewSecureInputPolicy,
    pub composition_on_blur: CompositionOnBlurPolicy,
    pub submit_handler: Option<String>,
    pub change_handler: Option<String>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewInputKind {
    #[default]
    TextField,
    TextArea,
    SecureField,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewInputPurpose {
    #[default]
    Text,
    Search,
    Name,
    Email,
    Url,
    Telephone,
    Number,
    Decimal,
    Password,
    Pin,
    Terminal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAssistPolicy {
    #[default]
    PlatformDefault,
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextSelectionPolicy {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextShortcutPolicy {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextTabPolicy {
    #[default]
    FocusNavigation,
    InsertTab,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewTextVerticalNavigationPolicy {
    #[default]
    LogicalLine,
    VisualLine,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextCapitalization {
    #[default]
    None,
    Sentences,
    Words,
    Characters,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnterKeyHint {
    #[default]
    Default,
    Enter,
    Done,
    Go,
    Next,
    Search,
    Send,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewSecureInputPolicy {
    #[default]
    Plain,
    Sensitive,
    Password,
    OneTimeCode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionOnBlurPolicy {
    #[default]
    Commit,
    Cancel,
    PreserveUntilAdapterDecision,
}

/// Runtime-facing text-control emission produced from typed product View resources.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControl {
    pub public_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub containing_scroll_region: Option<String>,
    pub session: u64,
    pub value: String,
    pub selection: ViewRuntimeTextSelection,
    pub options: ViewRuntimeTextControlOptions,
    pub kind: ViewInputKind,
    pub bounds: ViewRuntimeTextControlBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeTextControlHandlers::is_empty"
    )]
    pub handlers: ViewRuntimeTextControlHandlers,
    #[serde(default, skip_serializing_if = "ViewRuntimeControlStyle::is_default")]
    pub style: ViewRuntimeControlStyle,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControlHandlers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<ViewRuntimeTextControlHandler>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit: Option<ViewRuntimeTextControlHandler>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControlHandler {
    pub handler_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<ViewRuntimeTextControlHandlerRuntime>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControlHandlerRuntime {
    pub awbc_function_index: u32,
    pub handler_abi: BundleDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_binding: Option<CrossSectionRef>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextSelection {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControlBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewRuntimeTextControlOptions {
    pub purpose: ViewInputPurpose,
    pub autocorrect: TextAssistPolicy,
    pub spellcheck: TextAssistPolicy,
    pub capitalization: TextCapitalization,
    pub enter_key: EnterKeyHint,
    pub multiline: bool,
    #[serde(default)]
    pub selection_policy: ViewTextSelectionPolicy,
    #[serde(default)]
    pub shortcut_policy: ViewTextShortcutPolicy,
    #[serde(default)]
    pub tab_policy: ViewTextTabPolicy,
    #[serde(default)]
    pub vertical_navigation_policy: ViewTextVerticalNavigationPolicy,
    pub secure_policy: ViewSecureInputPolicy,
    pub composition_on_blur: CompositionOnBlurPolicy,
}

/// Product theme/environment section decoded from `ViewTheme`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewThemeResource {
    pub palette_overrides: Vec<SystemColorOverride>,
    pub defaults: ViewThemeEnvironmentDefaults,
    pub dark_mode_visual_golden_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemColorOverride {
    pub color: SystemColor,
    pub light: Option<RgbaColor>,
    pub dark: Option<RgbaColor>,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ViewThemeEnvironmentDefaults {
    pub color_scheme: ColorSchemeDefault,
    pub contrast: ContrastPreference,
    pub reduce_motion: bool,
    pub text_scale_milli: u32,
}

impl ViewChildSpan {
    pub const fn new(start_instruction: u32, end_instruction: u32) -> Self {
        Self {
            start_instruction,
            end_instruction,
        }
    }
}

impl RgbaColor {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, 255)
    }
}

impl ViewStyleSelector {
    pub fn max_depth(&self) -> usize {
        self.parts.iter().fold(0_usize, |depth, part| match part {
            ViewStyleSelectorPart::Descendant | ViewStyleSelectorPart::Child => depth + 1,
            _ => depth.max(1),
        })
    }
}

impl ViewTextResource {
    pub fn literal_text(&self, public_id: &str) -> Option<&str> {
        self.sources
            .iter()
            .find(|source| source.public_id == public_id)
            .and_then(|source| match &source.kind {
                ViewTextSourceKind::Literal { value } => Some(value.as_str()),
                ViewTextSourceKind::Localized { .. }
                | ViewTextSourceKind::RichTextDocument { .. }
                | ViewTextSourceKind::DisplayFrame { .. } => None,
            })
    }
}

impl ViewInputResource {
    pub fn runtime_text_controls(
        &self,
        text: Option<&ViewTextResource>,
        program: Option<&ViewProgramResource>,
    ) -> Vec<ViewRuntimeTextControl> {
        let fallback_bounds = ViewRuntimeTextControlBounds::default_stacked_slots(
            self.options.iter().map(|option| option.kind),
        );
        self.options
            .iter()
            .zip(fallback_bounds)
            .map(|(option, fallback)| {
                let bounds = program
                    .and_then(|program| program.text_control_bounds_for(&option.public_id))
                    .unwrap_or(fallback);
                option.runtime_text_control_with_bounds(bounds, text, program)
            })
            .collect()
    }
}

impl ViewInputOptions {
    pub fn runtime_text_control(
        &self,
        index: usize,
        text: Option<&ViewTextResource>,
        program: Option<&ViewProgramResource>,
    ) -> ViewRuntimeTextControl {
        self.runtime_text_control_with_bounds(
            ViewRuntimeTextControlBounds::default_slot(index, self.kind),
            text,
            program,
        )
    }

    fn runtime_text_control_with_bounds(
        &self,
        bounds: ViewRuntimeTextControlBounds,
        text: Option<&ViewTextResource>,
        program: Option<&ViewProgramResource>,
    ) -> ViewRuntimeTextControl {
        let value = text
            .and_then(|resource| resource.literal_text(&self.value_text_source))
            .unwrap_or_default()
            .to_owned();
        let label = runtime_label_source(program, self)
            .and_then(|source| text.and_then(|resource| resource.literal_text(source)))
            .map(ToOwned::to_owned);
        ViewRuntimeTextControl {
            public_id: self.public_id.clone(),
            target: self.public_id.clone(),
            view: self.view.clone(),
            containing_scroll_region: self.containing_scroll_region.clone(),
            session: self.runtime_text_session(),
            selection: ViewRuntimeTextSelection::collapsed_at_end(&value),
            options: ViewRuntimeTextControlOptions::from_input(self),
            kind: self.kind,
            bounds,
            value,
            label,
            handlers: ViewRuntimeTextControlHandlers::from_input(self, program),
            style: ViewRuntimeControlStyle::default(),
        }
    }

    pub fn runtime_text_session(&self) -> u64 {
        stable_text_session(&self.public_id)
    }
}

impl ViewInputKind {
    pub const fn from_element(element: ViewElementKind) -> Option<Self> {
        match element.text_input_kind() {
            Some(ViewElementTextInputKind::TextField) => Some(Self::TextField),
            Some(ViewElementTextInputKind::TextArea) => Some(Self::TextArea),
            Some(ViewElementTextInputKind::SecureField) => Some(Self::SecureField),
            None => None,
        }
    }

    pub const fn runtime_control_element(self) -> ViewElementKind {
        match self {
            Self::TextField => ViewElementKind::TextField,
            Self::TextArea => ViewElementKind::TextArea,
            Self::SecureField => ViewElementKind::SecureField,
        }
    }

    pub const fn is_secure(self) -> bool {
        matches!(self, Self::SecureField)
    }

    pub const fn is_multiline(self) -> bool {
        matches!(self, Self::TextArea)
    }

    pub const fn default_text_control_height_milli(self) -> u32 {
        match self {
            Self::TextField | Self::SecureField => 48_000,
            Self::TextArea => 136_000,
        }
    }

    const fn default_height_milli(self) -> u32 {
        self.default_text_control_height_milli()
    }
}

impl ViewSecureInputPolicy {
    pub const fn is_secure(self) -> bool {
        !matches!(self, Self::Plain)
    }
}

impl fmt::Debug for ViewRuntimeTextControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ViewRuntimeTextControl")
            .field("public_id", &self.public_id)
            .field("target", &self.target)
            .field("view", &self.view)
            .field("containing_scroll_region", &self.containing_scroll_region)
            .field("session", &self.session)
            .field("value", &self.diagnostic_value())
            .field("selection", &self.selection)
            .field("options", &self.options)
            .field("kind", &self.kind)
            .field("bounds", &self.bounds)
            .field("label", &self.label)
            .field("handlers", &self.handlers)
            .field("style", &self.style)
            .finish()
    }
}

impl ViewProgramResource {
    pub fn handler_ref(&self, handler_id: &str) -> Option<&ViewHandlerRef> {
        self.handlers
            .iter()
            .find(|handler| handler.handler_id == handler_id)
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
                style: ViewRuntimeControlStyle::default(),
            })
            .collect()
    }

    pub fn runtime_text_blocks(
        &self,
        text: Option<&ViewTextResource>,
    ) -> Vec<ViewRuntimeTextBlock> {
        self.text_blocks
            .iter()
            .map(|block| ViewRuntimeTextBlock {
                public_id: block.public_id.clone(),
                target: block.public_id.clone(),
                view: block.view.clone(),
                containing_scroll_region: block.containing_scroll_region.clone(),
                text: text
                    .and_then(|resource| resource.literal_text(&block.text_source))
                    .unwrap_or_default()
                    .to_owned(),
                bounds: block.bounds,
                selection_policy: block.selection_policy,
                style: ViewRuntimeControlStyle::default(),
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
        bounds: ViewRuntimeTextBlockBounds,
    ) -> Self {
        Self {
            public_id: public_id.into(),
            view,
            containing_scroll_region,
            text_source: text_source.into(),
            bounds,
            selection_policy: ViewTextSelectionPolicy::Disabled,
            style: None,
            source: None,
        }
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
            style: None,
            source: None,
        }
    }

    #[must_use]
    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style = Some(style.into());
        self
    }

    pub fn runtime_surface(&self) -> ViewRuntimeSurface {
        ViewRuntimeSurface {
            public_id: self.public_id.clone(),
            target: self.public_id.clone(),
            view: self.view.clone(),
            containing_scroll_region: self.containing_scroll_region.clone(),
            element: self.element,
            bounds: self.bounds,
            style: ViewRuntimeControlStyle::default(),
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

impl ViewRuntimeTextControl {
    pub const fn is_secure(&self) -> bool {
        self.options.secure_policy.is_secure() || self.kind.is_secure()
    }

    #[must_use]
    pub fn redacted_for_observation(&self) -> Self {
        if self.is_secure() {
            Self {
                value: String::new(),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }

    fn diagnostic_value(&self) -> String {
        if self.is_secure() {
            "<redacted>".to_owned()
        } else {
            self.value.clone()
        }
    }
}

impl ViewRuntimeTextControlHandlers {
    pub fn from_input(input: &ViewInputOptions, program: Option<&ViewProgramResource>) -> Self {
        Self {
            change: input
                .change_handler
                .as_deref()
                .map(|handler| ViewRuntimeTextControlHandler::from_program(program, handler)),
            submit: input
                .submit_handler
                .as_deref()
                .map(|handler| ViewRuntimeTextControlHandler::from_program(program, handler)),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.change.is_none() && self.submit.is_none()
    }
}

impl ViewRuntimeTextControlHandler {
    pub fn unresolved(handler_id: impl Into<String>) -> Self {
        Self {
            handler_id: handler_id.into(),
            runtime: None,
        }
    }

    pub fn from_program(program: Option<&ViewProgramResource>, handler_id: &str) -> Self {
        program
            .and_then(|program| program.handler_ref(handler_id))
            .map_or_else(|| Self::unresolved(handler_id), Self::from_handler_ref)
    }

    pub fn from_handler_ref(handler: &ViewHandlerRef) -> Self {
        Self {
            handler_id: handler.handler_id.clone(),
            runtime: Some(ViewRuntimeTextControlHandlerRuntime {
                awbc_function_index: handler.awbc_function_index,
                handler_abi: handler.handler_abi,
                function_binding: handler.function_binding,
            }),
        }
    }
}

impl ViewRuntimeTextSelection {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn collapsed_at_end(value: &str) -> Self {
        let end = u32::try_from(value.len()).unwrap_or(u32::MAX);
        Self::new(end, end)
    }

    #[must_use]
    pub fn clamped_to_text(self, value: &str) -> Self {
        Self::new(
            clamp_text_byte_offset(value, self.start),
            clamp_text_byte_offset(value, self.end),
        )
    }
}

impl ViewRuntimeTextControlBounds {
    const DEFAULT_STACK_X_MILLI: i32 = 48_000;
    const DEFAULT_STACK_Y_MILLI: i32 = 48_000;
    const DEFAULT_STACK_WIDTH_MILLI: u32 = 420_000;
    const DEFAULT_STACK_GAP_MILLI: i32 = 16_000;

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

    pub fn default_stacked_slots(kinds: impl IntoIterator<Item = ViewInputKind>) -> Vec<Self> {
        let mut next_y_milli = Self::DEFAULT_STACK_Y_MILLI;
        kinds
            .into_iter()
            .map(|kind| {
                let bounds = Self::stacked_slot(next_y_milli, kind);
                next_y_milli = Self::next_stacked_slot_y(next_y_milli, kind);
                bounds
            })
            .collect()
    }

    fn default_slot(index: usize, kind: ViewInputKind) -> Self {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        Self::stacked_slot(
            Self::DEFAULT_STACK_Y_MILLI
                .saturating_add(index.saturating_mul(Self::default_slot_pitch_milli())),
            kind,
        )
    }

    const fn stacked_slot(y_milli: i32, kind: ViewInputKind) -> Self {
        Self::new(
            Self::DEFAULT_STACK_X_MILLI,
            y_milli,
            Self::DEFAULT_STACK_WIDTH_MILLI,
            kind.default_height_milli(),
        )
    }

    fn next_stacked_slot_y(y_milli: i32, kind: ViewInputKind) -> i32 {
        y_milli
            .saturating_add(i32::try_from(kind.default_height_milli()).unwrap_or(i32::MAX))
            .saturating_add(Self::DEFAULT_STACK_GAP_MILLI)
    }

    const fn default_slot_pitch_milli() -> i32 {
        64_000
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

impl ViewRuntimeTextBlockBounds {
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

impl ViewRuntimeTextControlOptions {
    pub const fn from_input(input: &ViewInputOptions) -> Self {
        Self {
            purpose: input.purpose,
            autocorrect: input.autocorrect,
            spellcheck: input.spellcheck,
            capitalization: input.capitalization,
            enter_key: input.enter_key,
            multiline: input.multiline || input.kind.is_multiline(),
            selection_policy: input.selection_policy,
            shortcut_policy: input.shortcut_policy,
            tab_policy: input.tab_policy,
            vertical_navigation_policy: input.vertical_navigation_policy,
            secure_policy: input.secure_policy,
            composition_on_blur: input.composition_on_blur,
        }
    }
}

const fn default_true() -> bool {
    true
}

const fn default_surface_element() -> ViewElementKind {
    ViewElementKind::Panel
}

fn runtime_label_source<'a>(
    program: Option<&'a ViewProgramResource>,
    input: &'a ViewInputOptions,
) -> Option<&'a str> {
    program
        .and_then(|program| {
            program.semantic_targets.iter().find_map(|target| {
                (target.target == input.public_id || target.public_id == input.public_id)
                    .then_some(target.label_text_source.as_deref())
                    .flatten()
            })
        })
        .or(input.placeholder_text_source.as_deref())
}

fn stable_text_session(public_id: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let hash = public_id.as_bytes().iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    });
    if hash == 0 { 1 } else { hash }
}

pub type CompactViewProgramResource = ViewProgramResource;
pub type CompactViewStyleResource = ViewStyleResource;
pub type CompactViewTextResource = ViewTextResource;
pub type CompactViewInputResource = ViewInputResource;
pub type CompactViewThemeResource = ViewThemeResource;

fn clamp_text_byte_offset(value: &str, offset: u32) -> u32 {
    let mut index = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index = index.saturating_sub(1);
    }
    u32::try_from(index).unwrap_or(u32::MAX)
}

fn u32_to_i32_saturating(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_text_control_carries_authored_change_and_submit_handlers() {
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
            submit_handler: Some("handler.name.submit".to_owned()),
            change_handler: Some("handler.name.change".to_owned()),
            adapter_requirements: Vec::new(),
        };

        let control = input.runtime_text_control(0, None, None);

        assert_eq!(
            control.handlers.change.unwrap().handler_id,
            "handler.name.change"
        );
        assert_eq!(
            control.handlers.submit.unwrap().handler_id,
            "handler.name.submit"
        );
    }
}
