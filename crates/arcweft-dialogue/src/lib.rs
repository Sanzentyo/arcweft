use arcweft_id::PublicId;
use core::time::Duration;

mod character_dialogue;
pub mod character_presentation;
mod inline_failure;
mod presentation_profile;
mod presentation_revision;
pub mod rich_text;

pub use character_dialogue::{
    CharacterDialogue, CharacterDialogueCleanupValue, CharacterDialogueConfig,
    CharacterDialogueContentApplication, CharacterDialogueContractIdentity,
    CharacterDialogueCustomFieldId, CharacterDialogueCustomValue, CharacterDialogueFocusValue,
    CharacterDialogueHookValue, CharacterDialogueLimits, CharacterDialoguePatch,
    CharacterDialoguePortraitValue, CharacterDialogueRichTextValue,
    CharacterDialogueRuntimeCustomFieldCatalog, CharacterDialogueRuntimeCustomFieldDescriptor,
    CharacterDialogueRuntimeSchema, CharacterDialogueStageValue, CharacterDialogueStyleValue,
    CharacterDialogueTypedValue, CharacterDialogueValue, CharacterDialogueValueError,
    CharacterDialogueVoice, CharacterDialogueVoiceId, DialogueLocaleId,
    PRODUCTION_CHARACTER_DIALOGUE_LIMITS, PatchField, RuntimeFieldPath, StructuredPatch,
};
pub use inline_failure::{
    FallbackStylePolicy, InlineFailurePolicy, InlineFallback, InlineTextFailure,
};
pub use presentation_profile::DialoguePresentationProfile;
pub use presentation_revision::DialogueProfileRevision;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContent {
    parts: Vec<DialogueContentPart>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueContentPart {
    Text(String),
    Ruby { base: String, ruby: String },
    Tag(DialogueTag),
    Interpolation(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueTag {
    Page,
    Line,
    Break,
    Wait(Duration),
    Custom { name: String, args: Vec<TagArg> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TagArg {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlan {
    cues: Vec<TimelineCue>,
    steps: Vec<LinePlanStep>,
    cancel_rules: Vec<CancelRule>,
    cancel_scopes: Vec<CancelScope>,
    output: Option<OutPayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineCue {
    pub anchor: TimelineAnchor,
    pub cue: CueAction,
    pub cancel_on_drop: CancelOnDrop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CueAction {
    Face { target: String, face: String },
    Call(PlanCall),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Cue<'a> {
    Face { target: &'a str, face: &'a str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOnDrop {
    Cancel,
    Finish,
    Detach,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelRule {
    pub trigger: CancelTrigger,
    pub action: CancelAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    SkipLine,
    BackToTitle,
    Named(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancelAction {
    Continue,
    Goto(PublicId),
    Out(OutPayload),
    Cancelled(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutPayload {
    Unit,
    Expr(PlanExpr),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinePlanStep {
    Let {
        name: String,
        expr: PlanExpr,
    },
    At {
        anchor: TimelineAnchor,
        step: Box<LinePlanStep>,
    },
    Call(PlanCall),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanCall {
    pub receiver: String,
    pub method: String,
    pub args: Vec<PlanArg>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanArg {
    pub name: Option<String>,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanExpr {
    Name(String),
    Call(PlanCall),
    Tuple(Vec<PlanExpr>),
    Discard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineAnchor {
    Start,
    FromStart(Duration),
    AfterPrevious(Duration),
    EndMinus(Duration),
    Marker(String),
    Phoneme(String),
    CharIndex(u32),
    WordIndex(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelScope {
    pub trigger: CancelTrigger,
    pub exit: LineExit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CancelTrigger {
    Input(String),
    Signal(PublicId),
    Timeout(Duration),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineExit {
    Completed,
    Continue,
    Cancelled(String),
    Goto(PublicId),
    Out(OutPayload),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlanBuilder {
    cues: Vec<TimelineCue>,
    steps: Vec<LinePlanStep>,
    cancel_rules: Vec<CancelRule>,
    output: Option<OutPayload>,
}

impl DialogueContent {
    pub fn new(parts: impl Into<Vec<DialogueContentPart>>) -> Self {
        Self {
            parts: parts.into(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new([DialogueContentPart::Text(text.into())])
    }

    pub fn parse_lossy(source: &str) -> Self {
        let mut parts = Vec::new();
        let mut cursor = 0;
        let mut text_start = 0;

        while cursor < source.len() {
            let Some(ch) = source[cursor..].chars().next() else {
                break;
            };
            if let Some((end, base, ruby)) = parse_lossy_ruby(source, cursor) {
                push_text(&mut parts, &source[text_start..cursor]);
                parts.push(DialogueContentPart::Ruby { base, ruby });
                cursor = end;
                text_start = cursor;
                continue;
            }
            cursor += ch.len_utf8();
        }

        push_text(&mut parts, &source[text_start..]);
        Self::new(parts)
    }

    pub fn parts(&self) -> &[DialogueContentPart] {
        &self.parts
    }
}

fn parse_lossy_ruby(source: &str, start: usize) -> Option<(usize, String, String)> {
    parse_natural_ruby_lossy(source, start)
        .or_else(|| parse_ascii_explicit_ruby_lossy(source, start))
        .or_else(|| parse_ascii_compact_ruby_lossy(source, start))
        .or_else(|| parse_bracket_ruby_lossy(source, start))
}

fn parse_natural_ruby_lossy(source: &str, start: usize) -> Option<(usize, String, String)> {
    let after_marker = source.get(start..)?.strip_prefix('｜')?;
    let open = after_marker.find('《')?;
    let base = &after_marker[..open];
    let ruby_start = open + '《'.len_utf8();
    let ruby_tail = after_marker.get(ruby_start..)?;
    let close = ruby_tail.find('》')?;
    let ruby = &ruby_tail[..close];
    (!base.is_empty() && !ruby.is_empty()).then(|| {
        (
            start + '｜'.len_utf8() + ruby_start + close + '》'.len_utf8(),
            base.to_owned(),
            ruby.to_owned(),
        )
    })
}

fn parse_ascii_explicit_ruby_lossy(source: &str, start: usize) -> Option<(usize, String, String)> {
    let after_marker = source.get(start..)?.strip_prefix("|[")?;
    let base_end = after_marker.find("](")?;
    let base = &after_marker[..base_end];
    let ruby_start = base_end + "](".len();
    let ruby_tail = after_marker.get(ruby_start..)?;
    let ruby_end = ruby_tail.find(')')?;
    let ruby = &ruby_tail[..ruby_end];
    (!base.is_empty() && !ruby.is_empty()).then(|| {
        (
            start + "|[".len() + ruby_start + ruby_end + ')'.len_utf8(),
            base.to_owned(),
            ruby.to_owned(),
        )
    })
}

fn parse_ascii_compact_ruby_lossy(source: &str, start: usize) -> Option<(usize, String, String)> {
    let after_marker = source.get(start..)?.strip_prefix('|')?;
    if after_marker.starts_with('[') {
        return None;
    }
    let open = after_marker.find('{')?;
    let base = &after_marker[..open];
    if base.is_empty()
        || base
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '[' | ']' | '{' | '}' | '#' | '|'))
    {
        return None;
    }
    let ruby_start = open + '{'.len_utf8();
    let ruby_tail = after_marker.get(ruby_start..)?;
    let close = ruby_tail.find('}')?;
    let ruby = &ruby_tail[..close];
    (!ruby.is_empty()).then(|| {
        (
            start + '|'.len_utf8() + ruby_start + close + '}'.len_utf8(),
            base.to_owned(),
            ruby.to_owned(),
        )
    })
}

fn parse_bracket_ruby_lossy(source: &str, start: usize) -> Option<(usize, String, String)> {
    let after_open = source.get(start..)?.strip_prefix('[')?;
    let close = after_open.find(']')?;
    let inside = after_open[..close].trim();
    let mut parts = inside.splitn(2, char::is_whitespace);
    let tag = parts.next().unwrap_or_default();
    let attrs = parts.next().unwrap_or_default().trim();
    if !matches!(tag, "ruby" | "rb") {
        return None;
    }
    let ruby = parse_ruby_rt_lossy(attrs)?;
    let body_start = start + '['.len_utf8() + close + ']'.len_utf8();
    let close_tag = format!("[/{tag}]");
    let tail = source.get(body_start..)?;
    let body_end = tail.find(&close_tag)?;
    let base = tail[..body_end].trim();
    (!base.is_empty()).then(|| {
        (
            body_start + body_end + close_tag.len(),
            base.to_owned(),
            ruby,
        )
    })
}

fn parse_ruby_rt_lossy(attrs: &str) -> Option<String> {
    let value = attrs.trim().strip_prefix("rt")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    if let Some(quoted) = value.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some(quoted[..end].to_owned());
    }
    let end = value.find(char::is_whitespace).unwrap_or(value.len());
    (end > 0).then(|| value[..end].to_owned())
}

fn push_text(parts: &mut Vec<DialogueContentPart>, text: &str) {
    if !text.is_empty() {
        parse_control_tags(text, parts);
    }
}

fn parse_control_tags(text: &str, parts: &mut Vec<DialogueContentPart>) {
    let mut rest = text;
    loop {
        let Some(index) = rest.find('[') else {
            if !rest.is_empty() {
                parts.push(DialogueContentPart::Text(rest.to_owned()));
            }
            return;
        };

        let (before, after_open) = rest.split_at(index);
        if !before.is_empty() {
            parts.push(DialogueContentPart::Text(before.to_owned()));
        }

        let after_open = &after_open[1..];
        let Some(close) = after_open.find(']') else {
            parts.push(DialogueContentPart::Text(format!("[{after_open}")));
            return;
        };

        let tag = &after_open[..close];
        match tag {
            "p" | "page" => parts.push(DialogueContentPart::Tag(DialogueTag::Page)),
            "l" | "wait" => parts.push(DialogueContentPart::Tag(DialogueTag::Line)),
            "r" | "br" | "nl" => parts.push(DialogueContentPart::Tag(DialogueTag::Break)),
            value if value.starts_with("w ") => {
                if let Some(duration) = parse_wait_duration(value.trim_start_matches("w ").trim()) {
                    parts.push(DialogueContentPart::Tag(DialogueTag::Wait(duration)));
                } else {
                    parts.push(DialogueContentPart::Text(format!("[{tag}]")));
                }
            }
            _ => parts.push(DialogueContentPart::Text(format!("[{tag}]"))),
        }
        rest = &after_open[close + 1..];
    }
}

fn parse_wait_duration(value: &str) -> Option<Duration> {
    value
        .strip_suffix("ms")
        .and_then(|ms| ms.parse::<u64>().ok())
        .map(Duration::from_millis)
}

impl LinePlan {
    pub fn new(steps: impl Into<Vec<LinePlanStep>>) -> Self {
        Self {
            cues: Vec::new(),
            steps: steps.into(),
            cancel_rules: Vec::new(),
            cancel_scopes: Vec::new(),
            output: None,
        }
    }

    #[must_use]
    pub fn with_out(mut self, value: PlanExpr) -> Self {
        self.output = Some(OutPayload::Expr(value));
        self
    }

    #[must_use]
    pub fn with_cancel_scope(mut self, scope: CancelScope) -> Self {
        self.cancel_scopes.push(scope);
        self
    }

    pub fn steps(&self) -> &[LinePlanStep] {
        &self.steps
    }

    pub fn cancel_scopes(&self) -> &[CancelScope] {
        &self.cancel_scopes
    }

    pub fn cues(&self) -> &[TimelineCue] {
        &self.cues
    }

    pub fn cancel_rules(&self) -> &[CancelRule] {
        &self.cancel_rules
    }

    pub const fn output(&self) -> Option<&OutPayload> {
        self.output.as_ref()
    }
}

impl Default for LinePlan {
    fn default() -> Self {
        Self::new([])
    }
}

impl LinePlanBuilder {
    pub const fn new() -> Self {
        Self {
            cues: Vec::new(),
            steps: Vec::new(),
            cancel_rules: Vec::new(),
            output: None,
        }
    }

    #[must_use]
    pub fn at(mut self, offset: Duration, cue: Cue<'_>) -> Self {
        self.cues.push(TimelineCue {
            anchor: TimelineAnchor::FromStart(offset),
            cue: cue.into(),
            cancel_on_drop: CancelOnDrop::Cancel,
        });
        self
    }

    #[must_use]
    pub fn cue(mut self, cue: TimelineCue) -> Self {
        self.cues.push(cue);
        self
    }

    #[must_use]
    pub fn step(mut self, step: LinePlanStep) -> Self {
        self.steps.push(step);
        self
    }

    #[must_use]
    pub fn cancel_on(mut self, input: InputEventKind, action: CancelAction) -> Self {
        self.cancel_rules.push(CancelRule {
            trigger: CancelTrigger::Input(input.into_name()),
            action,
        });
        self
    }

    #[must_use]
    pub fn out_payload(mut self, payload: OutPayload) -> Self {
        self.output = Some(payload);
        self
    }

    pub fn build(self) -> LinePlan {
        LinePlan {
            cues: self.cues,
            steps: self.steps,
            cancel_rules: self.cancel_rules,
            cancel_scopes: Vec::new(),
            output: self.output,
        }
    }
}

impl Default for LinePlanBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Cue<'_>> for CueAction {
    fn from(value: Cue<'_>) -> Self {
        match value {
            Cue::Face { target, face } => Self::Face {
                target: target.to_owned(),
                face: face.to_owned(),
            },
        }
    }
}

impl InputEventKind {
    fn into_name(self) -> String {
        match self {
            Self::SkipLine => "SkipLine".to_owned(),
            Self::BackToTitle => "BackToTitle".to_owned(),
            Self::Named(name) => name,
        }
    }
}

#[cfg(test)]
mod tests;
