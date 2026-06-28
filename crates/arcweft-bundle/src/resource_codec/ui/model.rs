use crate::BundleVirtualFileRef;
use crate::container::BundleDigest;
use crate::resource_codec::types::{CrossSectionRef, DigestRef, SourceRangeRef};
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

pub type CompactUiProgramResource = UiProgramResource;
pub type CompactUiStyleResource = UiStyleResource;
pub type CompactUiTextResource = UiTextResource;
pub type CompactUiInputResource = UiInputResource;
pub type CompactUiThemeResource = UiThemeResource;
