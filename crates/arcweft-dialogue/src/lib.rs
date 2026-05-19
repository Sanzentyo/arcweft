use arcweft_id::{PublicId, TextKey};
use arcweft_source::SourceAnchor;
use core::time::Duration;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerRef {
    id: PublicId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextBoxRef {
    id: PublicId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueLine {
    speaker: SpeakerRef,
    options: SayOptions,
    content: DialogueContent,
    plan: LinePlan,
    source: SourceAnchor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SayOptions {
    pub id: Option<PublicId>,
    pub text_key: Option<TextKey>,
    pub voice: Option<VoicePolicy>,
    pub look: Option<PublicId>,
    pub text_box: Option<TextBoxRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerPreset {
    speaker: SpeakerRef,
    options: SayOptions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoicePolicy {
    Auto,
    Id(PublicId),
}

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
pub struct DialogueLineBuilder {
    preset: SpeakerPreset,
    options: SayOptions,
    content: Option<DialogueContent>,
    plan: LinePlanBuilder,
    source: SourceAnchor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlanBuilder {
    cues: Vec<TimelineCue>,
    steps: Vec<LinePlanStep>,
    cancel_rules: Vec<CancelRule>,
    output: Option<OutPayload>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{kind}")]
pub struct DialogueBuildError {
    kind: DialogueBuildErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DialogueBuildErrorKind {
    #[error("dialogue line content is required")]
    MissingContent,
}

impl SpeakerRef {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    #[must_use]
    pub fn preset(&self, options: SayOptions) -> SpeakerPreset {
        SpeakerPreset {
            speaker: self.clone(),
            options,
        }
    }
}

pub fn character(name: &str) -> SpeakerRef {
    SpeakerRef::new(domain_id("character", name))
}

pub fn textbox(name: &str) -> TextBoxRef {
    TextBoxRef::new(domain_id("textbox", name))
}

/// Creates a dialogue line id from a full public id such as `say.opening.001`.
///
/// # Panics
///
/// Panics when `name` is not a valid `PublicId`.
pub fn line_id(name: &str) -> PublicId {
    PublicId::try_new(name).expect("line id helper requires a valid public id")
}

fn look_id(name: &str) -> PublicId {
    domain_id("look", name)
}

fn domain_id(domain: &str, name: &str) -> PublicId {
    PublicId::try_new(format!("{domain}.{name}")).expect("domain helper requires a valid public id")
}

impl TextBoxRef {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

impl DialogueLine {
    pub fn new(
        speaker: SpeakerRef,
        options: SayOptions,
        content: DialogueContent,
        plan: LinePlan,
        source: SourceAnchor,
    ) -> Self {
        Self {
            speaker,
            options,
            content,
            plan,
            source,
        }
    }

    pub fn from_preset(
        preset: &SpeakerPreset,
        options: SayOptions,
        content: DialogueContent,
        plan: LinePlan,
        source: SourceAnchor,
    ) -> Self {
        Self::new(
            preset.speaker.clone(),
            preset.options.clone().merged_with(options),
            content,
            plan,
            source,
        )
    }

    pub const fn speaker(&self) -> &SpeakerRef {
        &self.speaker
    }

    pub const fn options(&self) -> &SayOptions {
        &self.options
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> &LinePlan {
        &self.plan
    }

    pub const fn source(&self) -> &SourceAnchor {
        &self.source
    }
}

impl SayOptions {
    pub const fn empty() -> Self {
        Self {
            id: None,
            text_key: None,
            voice: None,
            look: None,
            text_box: None,
        }
    }

    #[must_use]
    pub fn with_voice(mut self, voice: VoicePolicy) -> Self {
        self.voice = Some(voice);
        self
    }

    #[must_use]
    pub fn with_look(mut self, look: PublicId) -> Self {
        self.look = Some(look);
        self
    }

    #[must_use]
    pub fn with_text_box(mut self, text_box: TextBoxRef) -> Self {
        self.text_box = Some(text_box);
        self
    }

    fn merged_with(self, override_options: Self) -> Self {
        Self {
            id: override_options.id.or(self.id),
            text_key: override_options.text_key.or(self.text_key),
            voice: override_options.voice.or(self.voice),
            look: override_options.look.or(self.look),
            text_box: override_options.text_box.or(self.text_box),
        }
    }
}

impl Default for SayOptions {
    fn default() -> Self {
        Self::empty()
    }
}

impl SpeakerPreset {
    pub fn new(speaker: SpeakerRef) -> Self {
        Self {
            speaker,
            options: SayOptions::empty(),
        }
    }

    #[must_use]
    pub fn voice(mut self, voice: VoicePolicy) -> Self {
        self.options.voice = Some(voice);
        self
    }

    #[must_use]
    pub fn look(mut self, look: &str) -> Self {
        self.options.look = Some(look_id(look));
        self
    }

    #[must_use]
    pub fn window(mut self, text_box: TextBoxRef) -> Self {
        self.options.text_box = Some(text_box);
        self
    }

    pub fn say(&self) -> DialogueLineBuilder {
        DialogueLineBuilder::new(self.clone())
    }

    pub const fn speaker(&self) -> &SpeakerRef {
        &self.speaker
    }

    pub const fn options(&self) -> &SayOptions {
        &self.options
    }
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
        let mut rest = source;

        while let Some(start) = rest.find('｜') {
            let (before, after_marker) = rest.split_at(start);
            push_text(&mut parts, before);

            let after_marker = &after_marker['｜'.len_utf8()..];
            let Some(open) = after_marker.find('《') else {
                push_text(&mut parts, "｜");
                rest = after_marker;
                continue;
            };
            let Some(close_relative) = after_marker[open + '《'.len_utf8()..].find('》') else {
                push_text(&mut parts, "｜");
                rest = after_marker;
                continue;
            };

            let base = &after_marker[..open];
            let ruby_start = open + '《'.len_utf8();
            let ruby_end = ruby_start + close_relative;
            let ruby = &after_marker[ruby_start..ruby_end];
            parts.push(DialogueContentPart::Ruby {
                base: base.to_owned(),
                ruby: ruby.to_owned(),
            });
            rest = &after_marker[ruby_end + '》'.len_utf8()..];
        }

        parse_control_tags(rest, &mut parts);
        Self::new(parts)
    }

    pub fn parts(&self) -> &[DialogueContentPart] {
        &self.parts
    }
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
            "p" => parts.push(DialogueContentPart::Tag(DialogueTag::Page)),
            "l" => parts.push(DialogueContentPart::Tag(DialogueTag::Line)),
            "r" | "br" => parts.push(DialogueContentPart::Tag(DialogueTag::Break)),
            _ => parts.push(DialogueContentPart::Text(format!("[{tag}]"))),
        }
        rest = &after_open[close + 1..];
    }
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

impl DialogueLineBuilder {
    fn new(preset: SpeakerPreset) -> Self {
        Self {
            preset,
            options: SayOptions::empty(),
            content: None,
            plan: LinePlanBuilder::new(),
            source: SourceAnchor::generated(),
        }
    }

    #[must_use]
    pub fn id(mut self, id: PublicId) -> Self {
        self.options.id = Some(id);
        self
    }

    #[must_use]
    pub fn voice(mut self, voice: VoicePolicy) -> Self {
        self.options.voice = Some(voice);
        self
    }

    #[must_use]
    pub fn look(mut self, look: &str) -> Self {
        self.options.look = Some(look_id(look));
        self
    }

    #[must_use]
    pub fn window(mut self, text_box: TextBoxRef) -> Self {
        self.options.text_box = Some(text_box);
        self
    }

    #[must_use]
    pub fn content(mut self, content: DialogueContent) -> Self {
        self.content = Some(content);
        self
    }

    #[must_use]
    pub fn at(mut self, offset: Duration, cue: Cue<'_>) -> Self {
        self.plan = self.plan.at(offset, cue);
        self
    }

    #[must_use]
    pub fn cancel_on(mut self, input: InputEventKind, action: CancelAction) -> Self {
        self.plan = self.plan.cancel_on(input, action);
        self
    }

    #[must_use]
    pub fn out_payload(mut self, payload: OutPayload) -> Self {
        self.plan = self.plan.out_payload(payload);
        self
    }

    pub fn build(self) -> Result<DialogueLine, DialogueBuildError> {
        let content = self
            .content
            .ok_or_else(|| DialogueBuildError::new(DialogueBuildErrorKind::MissingContent))?;

        Ok(DialogueLine::from_preset(
            &self.preset,
            self.options,
            content,
            self.plan.build(),
            self.source,
        ))
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

impl DialogueBuildError {
    const fn new(kind: DialogueBuildErrorKind) -> Self {
        Self { kind }
    }

    pub const fn kind(&self) -> DialogueBuildErrorKind {
        self.kind
    }
}

#[cfg(test)]
mod tests;
