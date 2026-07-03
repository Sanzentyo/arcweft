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
    pub action_buttons: Vec<UiActionButtonResource>,
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
            Self::Surface | Self::Row | Self::Column | Self::Stack | Self::Button => None,
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
    pub source: Option<SourceRangeRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiActionButtonActionResource {
    TextInputSubmit {
        input: String,
        ime_policy: UiTextSubmitImePolicy,
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRuntimeActionButtonAction {
    TextInputSubmit {
        input_target: String,
        ime_policy: UiTextSubmitImePolicy,
    },
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
        let mut next_y_milli = UiRuntimeTextControlBounds::DEFAULT_STACK_Y_MILLI;
        self.options
            .iter()
            .map(|option| {
                let bounds = UiRuntimeTextControlBounds::stacked_slot(next_y_milli, option.kind);
                next_y_milli =
                    UiRuntimeTextControlBounds::next_stacked_slot_y(next_y_milli, option.kind);
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

    const fn default_height_milli(self) -> u32 {
        match self {
            Self::TextField | Self::SecureField => 48_000,
            Self::TextArea => 136_000,
        }
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
                    UiActionButtonActionResource::TextInputSubmit { input, ime_policy } => {
                        UiRuntimeActionButtonAction::TextInputSubmit {
                            input_target: input.clone(),
                            ime_policy: *ime_policy,
                        }
                    }
                },
            })
            .collect()
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
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
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
