use super::{
    CrossSectionRef, Deserialize, Serialize, ViewElementKind, ViewElementTextInputKind,
    ViewHandlerRef, ViewProgramResource, ViewRuntimeControlVisualStyle, ViewTextResource, fmt,
};
use arcweft_view::ViewHandlerProgramId;

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
    pub submit_handler: Option<ViewHandlerProgramId>,
    pub change_handler: Option<ViewHandlerProgramId>,
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
    #[serde(
        default,
        skip_serializing_if = "ViewRuntimeControlVisualStyle::is_default"
    )]
    pub style: ViewRuntimeControlVisualStyle,
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
    pub program: ViewHandlerProgramId,
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
            style: ViewRuntimeControlVisualStyle::default(),
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
                .map(|handler| ViewRuntimeTextControlHandler::from_program(program, handler)),
            submit: input
                .submit_handler
                .map(|handler| ViewRuntimeTextControlHandler::from_program(program, handler)),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.change.is_none() && self.submit.is_none()
    }
}

impl ViewRuntimeTextControlHandler {
    pub fn from_program(
        program: Option<&ViewProgramResource>,
        handler: ViewHandlerProgramId,
    ) -> Self {
        let specification = program
            .and_then(|program| program.handler_ref(handler))
            .expect("validated View input handler retains its exact program specification");
        Self::from_handler_ref(specification)
    }

    pub fn from_handler_ref(handler: &ViewHandlerRef) -> Self {
        Self {
            program: handler.program,
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

fn clamp_text_byte_offset(value: &str, offset: u32) -> u32 {
    let mut index = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index = index.saturating_sub(1);
    }
    u32::try_from(index).unwrap_or(u32::MAX)
}
