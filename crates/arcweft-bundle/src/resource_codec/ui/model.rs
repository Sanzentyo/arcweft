use super::runtime_control_style::UiRuntimeControlStyle;
use crate::BundleVirtualFileRef;
use crate::container::BundleDigest;
use crate::resource_codec::types::{CrossSectionRef, DigestRef, SourceRangeRef};
use core::fmt;
use serde::{Deserialize, Serialize};

/// Product UI program section decoded from `UiProgram`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiProgramResource {
    pub program_id: String,
    pub root_component: String,
    pub instructions: Vec<UiProgramInstruction>,
    pub child_spans: Vec<UiChildSpan>,
    pub handlers: Vec<UiHandlerRef>,
    pub state_schema_hashes: Vec<UiStateSchemaHashRef>,
    pub exported_parts: Vec<UiExportedPart>,
    pub semantic_targets: Vec<UiSemanticTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layout_bounds: Vec<UiLayoutBoundsResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_buttons: Vec<UiActionButtonResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_groups: Vec<UiFocusGroupResource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_navigation: Vec<UiFocusNavigationResource>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiProgramInstruction {
    OpenElement {
        element: UiElementKind,
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
    CallComponent {
        component: String,
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
    ApplyStyle {
        style: UiStyleApplyRef,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiElementKind {
    Surface,
    Box,
    Scroll,
    Row,
    Column,
    Stack,
    Button,
    TextField,
    TextArea,
    SecureField,
}

impl UiElementKind {
    pub const fn text_input_kind(self) -> Option<UiInputKind> {
        match self {
            Self::TextField => Some(UiInputKind::TextField),
            Self::TextArea => Some(UiInputKind::TextArea),
            Self::SecureField => Some(UiInputKind::SecureField),
            Self::Surface
            | Self::Box
            | Self::Scroll
            | Self::Row
            | Self::Column
            | Self::Stack
            | Self::Button => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiStyleApplyRef {
    Named(String),
    InlineArcweft { patch_id: u32 },
    InlineCss { patch_id: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiChildSpan {
    pub start_instruction: u32,
    pub end_instruction: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiHandlerRef {
    pub handler_id: String,
    pub event: String,
    pub awbc_function_index: u32,
    pub handler_abi: BundleDigest,
    pub function_binding: Option<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiStateSchemaHashRef {
    pub public_id: Option<String>,
    pub hash: BundleDigest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiExportedPart {
    pub part_id: String,
    pub public_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiSemanticTarget {
    pub public_id: String,
    pub target: String,
    pub label_text_source: Option<String>,
    pub source: Option<SourceRangeRef>,
}

/// Resolved logical bounds for UI program targets authored by the View DSL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiLayoutBoundsResource {
    pub public_id: String,
    pub kind: UiLayoutBoundsKind,
    pub rect: UiLogicalRect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_rect: Option<UiLogicalRect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLayoutBoundsKind {
    TextControl,
    SemanticTarget,
}

/// Logical-pixel rectangle serialized in milli-pixel units.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiLogicalRect {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Product-authored player-rendered action button metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiActionButtonResource {
    pub public_id: String,
    pub label_text_source: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub action: UiActionButtonActionResource,
    pub bounds: UiRuntimeButtonBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiActionButtonActionResource {
    Noop,
    TextInputSubmit {
        input: String,
        ime_policy: UiTextSubmitImePolicy,
    },
    ActionInvoke {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextSubmitImePolicy {
    #[default]
    Commit,
    Cancel,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeButtonBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeActionButton {
    pub public_id: String,
    pub target: String,
    pub label: String,
    pub enabled: bool,
    pub bounds: UiRuntimeButtonBounds,
    pub action: UiRuntimeActionButtonAction,
    #[serde(default, skip_serializing_if = "UiRuntimeControlStyle::is_default")]
    pub style: UiRuntimeControlStyle,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRuntimeActionButtonAction {
    Noop,
    TextInputSubmit {
        input_target: String,
        ime_policy: UiTextSubmitImePolicy,
    },
    ActionInvoke {
        action: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<String>,
    },
}

/// Authored focus group metadata for Arcweft-owned player navigation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiFocusGroupResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub policy: UiFocusGroupPolicy,
    #[serde(default)]
    pub initial: UiFocusInitialPolicy,
    #[serde(default)]
    pub wrap: UiFocusWrapPolicy,
    #[serde(default)]
    pub disabled_skip: UiFocusSkipPolicy,
    #[serde(default)]
    pub hidden_skip: UiFocusSkipPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing focus group emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeFocusGroup {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub policy: UiFocusGroupPolicy,
    pub initial: UiFocusInitialPolicy,
    pub wrap: UiFocusWrapPolicy,
    pub disabled_skip: UiFocusSkipPolicy,
    pub hidden_skip: UiFocusSkipPolicy,
}

/// Focus target and directional edges authored by the View DSL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiFocusNavigationResource {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<UiFocusNavigationEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

/// Runtime-facing focus navigation emitted in display snapshots.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeFocusNavigation {
    pub public_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<UiRuntimeFocusNavigationEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiFocusNavigationEdge {
    pub direction: UiFocusDirection,
    pub target: UiFocusTargetResolution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeFocusNavigationEdge {
    pub direction: UiFocusDirection,
    pub target: UiFocusTargetResolution,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Previous,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusTargetResolution {
    Explicit { target: String },
    Auto,
    None,
    GroupBoundary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusGroupPolicy {
    #[default]
    Normal,
    Trap,
    Modal,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusInitialPolicy {
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
pub enum UiFocusWrapPolicy {
    #[default]
    Wrap,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusSkipPolicy {
    #[default]
    Skip,
    Stop,
}

/// Product style section decoded from `UiStyle`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiStyleResource {
    pub style_program_id: String,
    pub arcweft_sources: Vec<StyleSourceIdentity>,
    pub css_sources: Vec<StyleSourceIdentity>,
    pub tokens: Vec<UiStyleToken>,
    pub rules: Vec<UiStyleRule>,
    pub part_rules: Vec<UiPartStyleRule>,
    pub environment_predicates: Vec<UiEnvironmentPredicate>,
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
pub struct UiStyleToken {
    pub public_id: String,
    pub value: UiStyleValue,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiStyleRule {
    pub selector: UiStyleSelector,
    pub declarations: Vec<UiStyleDeclaration>,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiPartStyleRule {
    pub part: String,
    pub selector: UiStyleSelector,
    pub declarations: Vec<UiStyleDeclaration>,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiStyleSelector {
    pub parts: Vec<UiStyleSelectorPart>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiStyleSelectorPart {
    Element(UiElementKind),
    Part(String),
    State(UiElementState),
    Interaction(UiInteractionState),
    Environment(UiEnvironmentPredicate),
    Descendant,
    Child,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiElementState {
    FocusVisible,
    ReadOnly,
    Invalid,
    Composing,
    PlaceholderShown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInteractionState {
    Hover,
    Active,
    Disabled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiStyleDeclaration {
    pub property: String,
    pub value: UiStyleValue,
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
pub enum UiStyleValue {
    Token(String),
    SystemColor(SystemColor),
    Rgba(RgbaColor),
    Milli(i32),
    Text(String),
    List(Vec<UiStyleValue>),
    Resource(String),
    Digest(BundleDigest),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemColor {
    Canvas,
    CanvasText,
    Surface,
    SurfaceText,
    RaisedSurface,
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
pub enum UiEnvironmentPredicate {
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

/// Product UI text-source section decoded from `UiText`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiTextResource {
    pub sources: Vec<UiTextSourceRecord>,
    pub display_frame_refs: Vec<CrossSectionRef>,
    pub source_ranges: Vec<SourceRangeRef>,
    pub reveal_policies: Vec<UiTextRevealPolicyBinding>,
    pub cursor_policies: Vec<UiTextCursorPolicyBinding>,
    pub redactions: Vec<UiSecureRedactionMetadata>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiTextSourceRecord {
    pub public_id: String,
    pub kind: UiTextSourceKind,
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextSourceKind {
    Literal { value: String },
    Localized { key: String, locale: Option<String> },
    RichTextDocument { document: CrossSectionRef },
    DisplayFrame { frame: CrossSectionRef },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiTextRevealPolicyBinding {
    pub text_source: String,
    pub policy: UiTextRevealPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextRevealPolicy {
    #[default]
    Immediate,
    Typewriter,
    ManualAdvance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiTextCursorPolicyBinding {
    pub text_source: String,
    pub policy: UiTextCursorPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextCursorPolicy {
    Hidden,
    #[default]
    Inherit,
    Visible,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiSecureRedactionMetadata {
    pub text_source: String,
    pub classification: UiObserveClassification,
    pub replacement: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiObserveClassification {
    #[default]
    Public,
    AgentMasked,
    Secret,
}

/// Product text-input metadata section decoded from `UiInput`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiInputResource {
    pub options: Vec<UiInputOptions>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiInputOptions {
    pub public_id: String,
    pub kind: UiInputKind,
    pub value_text_source: String,
    pub placeholder_text_source: Option<String>,
    pub purpose: UiInputPurpose,
    pub autocorrect: TextAssistPolicy,
    pub spellcheck: TextAssistPolicy,
    pub capitalization: TextCapitalization,
    pub enter_key: EnterKeyHint,
    pub multiline: bool,
    #[serde(default)]
    pub selection_policy: UiTextSelectionPolicy,
    #[serde(default)]
    pub shortcut_policy: UiTextShortcutPolicy,
    #[serde(default)]
    pub tab_policy: UiTextTabPolicy,
    #[serde(default)]
    pub vertical_navigation_policy: UiTextVerticalNavigationPolicy,
    pub secure_policy: UiSecureInputPolicy,
    pub composition_on_blur: CompositionOnBlurPolicy,
    pub submit_handler: Option<String>,
    pub change_handler: Option<String>,
    pub adapter_requirements: Vec<CrossSectionRef>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInputKind {
    #[default]
    TextField,
    TextArea,
    SecureField,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiInputPurpose {
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
pub enum UiTextSelectionPolicy {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextShortcutPolicy {
    #[default]
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextTabPolicy {
    #[default]
    FocusNavigation,
    InsertTab,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTextVerticalNavigationPolicy {
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
pub enum UiSecureInputPolicy {
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

/// Runtime-facing text-control emission produced from typed product UI resources.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControl {
    pub public_id: String,
    pub target: String,
    pub session: u64,
    pub value: String,
    pub selection: UiRuntimeTextSelection,
    pub options: UiRuntimeTextControlOptions,
    pub kind: UiInputKind,
    pub bounds: UiRuntimeTextControlBounds,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "UiRuntimeTextControlHandlers::is_empty"
    )]
    pub handlers: UiRuntimeTextControlHandlers,
    #[serde(default, skip_serializing_if = "UiRuntimeControlStyle::is_default")]
    pub style: UiRuntimeControlStyle,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControlHandlers {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<UiRuntimeTextControlHandler>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit: Option<UiRuntimeTextControlHandler>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControlHandler {
    pub handler_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<UiRuntimeTextControlHandlerRuntime>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControlHandlerRuntime {
    pub awbc_function_index: u32,
    pub handler_abi: BundleDigest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_binding: Option<CrossSectionRef>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextSelection {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControlBounds {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiRuntimeTextControlOptions {
    pub purpose: UiInputPurpose,
    pub autocorrect: TextAssistPolicy,
    pub spellcheck: TextAssistPolicy,
    pub capitalization: TextCapitalization,
    pub enter_key: EnterKeyHint,
    pub multiline: bool,
    #[serde(default)]
    pub selection_policy: UiTextSelectionPolicy,
    #[serde(default)]
    pub shortcut_policy: UiTextShortcutPolicy,
    #[serde(default)]
    pub tab_policy: UiTextTabPolicy,
    #[serde(default)]
    pub vertical_navigation_policy: UiTextVerticalNavigationPolicy,
    pub secure_policy: UiSecureInputPolicy,
    pub composition_on_blur: CompositionOnBlurPolicy,
}

/// Product theme/environment section decoded from `UiTheme`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UiThemeResource {
    pub palette_overrides: Vec<SystemColorOverride>,
    pub defaults: UiThemeEnvironmentDefaults,
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
pub struct UiThemeEnvironmentDefaults {
    pub color_scheme: ColorSchemeDefault,
    pub contrast: ContrastPreference,
    pub reduce_motion: bool,
    pub text_scale_milli: u32,
}

impl UiChildSpan {
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

impl UiStyleSelector {
    pub fn max_depth(&self) -> usize {
        self.parts.iter().fold(0_usize, |depth, part| match part {
            UiStyleSelectorPart::Descendant | UiStyleSelectorPart::Child => depth + 1,
            _ => depth.max(1),
        })
    }
}

impl UiTextResource {
    pub fn literal_text(&self, public_id: &str) -> Option<&str> {
        self.sources
            .iter()
            .find(|source| source.public_id == public_id)
            .and_then(|source| match &source.kind {
                UiTextSourceKind::Literal { value } => Some(value.as_str()),
                UiTextSourceKind::Localized { .. }
                | UiTextSourceKind::RichTextDocument { .. }
                | UiTextSourceKind::DisplayFrame { .. } => None,
            })
    }
}

impl UiInputResource {
    pub fn runtime_text_controls(
        &self,
        text: Option<&UiTextResource>,
        program: Option<&UiProgramResource>,
    ) -> Vec<UiRuntimeTextControl> {
        let fallback_bounds = UiRuntimeTextControlBounds::default_stacked_slots(
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

impl UiInputOptions {
    pub fn runtime_text_control(
        &self,
        index: usize,
        text: Option<&UiTextResource>,
        program: Option<&UiProgramResource>,
    ) -> UiRuntimeTextControl {
        self.runtime_text_control_with_bounds(
            UiRuntimeTextControlBounds::default_slot(index, self.kind),
            text,
            program,
        )
    }

    fn runtime_text_control_with_bounds(
        &self,
        bounds: UiRuntimeTextControlBounds,
        text: Option<&UiTextResource>,
        program: Option<&UiProgramResource>,
    ) -> UiRuntimeTextControl {
        let value = text
            .and_then(|resource| resource.literal_text(&self.value_text_source))
            .unwrap_or_default()
            .to_owned();
        let label = runtime_label_source(program, self)
            .and_then(|source| text.and_then(|resource| resource.literal_text(source)))
            .map(ToOwned::to_owned);
        UiRuntimeTextControl {
            public_id: self.public_id.clone(),
            target: self.public_id.clone(),
            session: self.runtime_text_session(),
            selection: UiRuntimeTextSelection::collapsed_at_end(&value),
            options: UiRuntimeTextControlOptions::from_input(self),
            kind: self.kind,
            bounds,
            value,
            label,
            handlers: UiRuntimeTextControlHandlers::from_input(self, program),
            style: UiRuntimeControlStyle::default(),
        }
    }

    pub fn runtime_text_session(&self) -> u64 {
        stable_text_session(&self.public_id)
    }
}

impl UiInputKind {
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

impl UiSecureInputPolicy {
    pub const fn is_secure(self) -> bool {
        !matches!(self, Self::Plain)
    }
}

impl fmt::Debug for UiRuntimeTextControl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UiRuntimeTextControl")
            .field("public_id", &self.public_id)
            .field("target", &self.target)
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

impl UiProgramResource {
    pub fn handler_ref(&self, handler_id: &str) -> Option<&UiHandlerRef> {
        self.handlers
            .iter()
            .find(|handler| handler.handler_id == handler_id)
    }

    pub fn runtime_action_buttons(
        &self,
        text: Option<&UiTextResource>,
    ) -> Vec<UiRuntimeActionButton> {
        self.action_buttons
            .iter()
            .map(|button| UiRuntimeActionButton {
                public_id: button.public_id.clone(),
                target: button.public_id.clone(),
                label: text
                    .and_then(|resource| resource.literal_text(&button.label_text_source))
                    .unwrap_or(&button.public_id)
                    .to_owned(),
                enabled: button.enabled,
                bounds: button.bounds,
                action: match &button.action {
                    UiActionButtonActionResource::Noop => UiRuntimeActionButtonAction::Noop,
                    UiActionButtonActionResource::TextInputSubmit { input, ime_policy } => {
                        UiRuntimeActionButtonAction::TextInputSubmit {
                            input_target: input.clone(),
                            ime_policy: *ime_policy,
                        }
                    }
                    UiActionButtonActionResource::ActionInvoke { action, payload } => {
                        UiRuntimeActionButtonAction::ActionInvoke {
                            action: action.clone(),
                            payload: payload.clone(),
                        }
                    }
                },
                style: UiRuntimeControlStyle::default(),
            })
            .collect()
    }

    pub fn runtime_focus_groups(&self) -> Vec<UiRuntimeFocusGroup> {
        self.focus_groups
            .iter()
            .map(|group| UiRuntimeFocusGroup {
                public_id: group.public_id.clone(),
                parent: group.parent.clone(),
                policy: group.policy,
                initial: group.initial.clone(),
                wrap: group.wrap,
                disabled_skip: group.disabled_skip,
                hidden_skip: group.hidden_skip,
            })
            .collect()
    }

    pub fn runtime_focus_navigation(&self) -> Vec<UiRuntimeFocusNavigation> {
        self.focus_navigation
            .iter()
            .map(|target| UiRuntimeFocusNavigation {
                public_id: target.public_id.clone(),
                group: target.group.clone(),
                edges: target
                    .edges
                    .iter()
                    .map(|edge| UiRuntimeFocusNavigationEdge {
                        direction: edge.direction,
                        target: edge.target.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn text_control_bounds_for(&self, public_id: &str) -> Option<UiRuntimeTextControlBounds> {
        self.layout_bounds
            .iter()
            .find(|bounds| bounds.is_text_control_for(public_id))
            .map(UiLayoutBoundsResource::runtime_text_control_bounds)
    }

    pub fn semantic_target_bounds_for(
        &self,
        public_id: &str,
    ) -> Option<UiRuntimeTextControlBounds> {
        self.layout_bounds
            .iter()
            .find(|bounds| bounds.is_semantic_target_for(public_id))
            .map(UiLayoutBoundsResource::runtime_text_control_bounds)
    }
}

impl UiLayoutBoundsResource {
    pub fn text_control(public_id: impl Into<String>, rect: UiLogicalRect) -> Self {
        Self::new(public_id, UiLayoutBoundsKind::TextControl, rect)
    }

    pub fn semantic_target(public_id: impl Into<String>, rect: UiLogicalRect) -> Self {
        Self::new(public_id, UiLayoutBoundsKind::SemanticTarget, rect)
    }

    pub fn new(
        public_id: impl Into<String>,
        kind: UiLayoutBoundsKind,
        rect: UiLogicalRect,
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
    pub const fn with_hit_rect(mut self, hit_rect: UiLogicalRect) -> Self {
        self.hit_rect = Some(hit_rect);
        self
    }

    pub fn is_text_control_for(&self, public_id: &str) -> bool {
        self.kind == UiLayoutBoundsKind::TextControl && self.public_id == public_id
    }

    pub fn is_semantic_target_for(&self, public_id: &str) -> bool {
        self.kind == UiLayoutBoundsKind::SemanticTarget && self.public_id == public_id
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

    pub fn runtime_text_control_bounds(&self) -> UiRuntimeTextControlBounds {
        self.hit_rect
            .unwrap_or(self.rect)
            .runtime_text_control_bounds()
    }
}

impl UiLayoutBoundsKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextControl => "text_control",
            Self::SemanticTarget => "semantic_target",
        }
    }
}

impl UiLogicalRect {
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

    pub const fn runtime_text_control_bounds(self) -> UiRuntimeTextControlBounds {
        UiRuntimeTextControlBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }

    pub const fn runtime_button_bounds(self) -> UiRuntimeButtonBounds {
        UiRuntimeButtonBounds::new(
            self.x_milli,
            self.y_milli,
            self.width_milli,
            self.height_milli,
        )
    }
}

impl UiFocusDirection {
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

impl UiFocusTargetResolution {
    pub fn explicit_target(&self) -> Option<&str> {
        match self {
            Self::Explicit { target } => Some(target.as_str()),
            Self::Auto | Self::None | Self::GroupBoundary => None,
        }
    }
}

impl UiFocusInitialPolicy {
    pub fn explicit_target(&self) -> Option<&str> {
        match self {
            Self::Explicit { target } => Some(target.as_str()),
            Self::Auto | Self::First | Self::Last | Self::None => None,
        }
    }
}

impl UiFocusWrapPolicy {
    pub const fn allows_wrap(self) -> bool {
        matches!(self, Self::Wrap)
    }
}

impl UiRuntimeTextControl {
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

impl UiRuntimeTextControlHandlers {
    pub fn from_input(input: &UiInputOptions, program: Option<&UiProgramResource>) -> Self {
        Self {
            change: input
                .change_handler
                .as_deref()
                .map(|handler| UiRuntimeTextControlHandler::from_program(program, handler)),
            submit: input
                .submit_handler
                .as_deref()
                .map(|handler| UiRuntimeTextControlHandler::from_program(program, handler)),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.change.is_none() && self.submit.is_none()
    }
}

impl UiRuntimeTextControlHandler {
    pub fn unresolved(handler_id: impl Into<String>) -> Self {
        Self {
            handler_id: handler_id.into(),
            runtime: None,
        }
    }

    pub fn from_program(program: Option<&UiProgramResource>, handler_id: &str) -> Self {
        program
            .and_then(|program| program.handler_ref(handler_id))
            .map_or_else(|| Self::unresolved(handler_id), Self::from_handler_ref)
    }

    pub fn from_handler_ref(handler: &UiHandlerRef) -> Self {
        Self {
            handler_id: handler.handler_id.clone(),
            runtime: Some(UiRuntimeTextControlHandlerRuntime {
                awbc_function_index: handler.awbc_function_index,
                handler_abi: handler.handler_abi,
                function_binding: handler.function_binding,
            }),
        }
    }
}

impl UiRuntimeTextSelection {
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

impl UiRuntimeTextControlBounds {
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

    pub fn default_stacked_slots(kinds: impl IntoIterator<Item = UiInputKind>) -> Vec<Self> {
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

    fn default_slot(index: usize, kind: UiInputKind) -> Self {
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        Self::stacked_slot(
            Self::DEFAULT_STACK_Y_MILLI
                .saturating_add(index.saturating_mul(Self::default_slot_pitch_milli())),
            kind,
        )
    }

    const fn stacked_slot(y_milli: i32, kind: UiInputKind) -> Self {
        Self::new(
            Self::DEFAULT_STACK_X_MILLI,
            y_milli,
            Self::DEFAULT_STACK_WIDTH_MILLI,
            kind.default_height_milli(),
        )
    }

    fn next_stacked_slot_y(y_milli: i32, kind: UiInputKind) -> i32 {
        y_milli
            .saturating_add(i32::try_from(kind.default_height_milli()).unwrap_or(i32::MAX))
            .saturating_add(Self::DEFAULT_STACK_GAP_MILLI)
    }

    const fn default_slot_pitch_milli() -> i32 {
        64_000
    }
}

impl UiRuntimeButtonBounds {
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
        input_bounds: UiRuntimeTextControlBounds,
        input_kind: UiInputKind,
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

impl UiRuntimeTextControlOptions {
    pub const fn from_input(input: &UiInputOptions) -> Self {
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

fn runtime_label_source<'a>(
    program: Option<&'a UiProgramResource>,
    input: &'a UiInputOptions,
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

pub type CompactUiProgramResource = UiProgramResource;
pub type CompactUiStyleResource = UiStyleResource;
pub type CompactUiTextResource = UiTextResource;
pub type CompactUiInputResource = UiInputResource;
pub type CompactUiThemeResource = UiThemeResource;

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
        let input = UiInputOptions {
            public_id: "field.name".to_owned(),
            kind: UiInputKind::TextField,
            value_text_source: "text.name".to_owned(),
            placeholder_text_source: None,
            purpose: UiInputPurpose::Text,
            autocorrect: TextAssistPolicy::PlatformDefault,
            spellcheck: TextAssistPolicy::PlatformDefault,
            capitalization: TextCapitalization::None,
            enter_key: EnterKeyHint::Default,
            multiline: false,
            selection_policy: UiTextSelectionPolicy::Enabled,
            shortcut_policy: UiTextShortcutPolicy::Enabled,
            tab_policy: UiTextTabPolicy::FocusNavigation,
            vertical_navigation_policy: UiTextVerticalNavigationPolicy::LogicalLine,
            secure_policy: UiSecureInputPolicy::Plain,
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
