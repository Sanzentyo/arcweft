use arcweft_id::{PublicId, TextKey};
use arcweft_source::SourceAnchor;
use core::time::Duration;

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
    options: DialogueOptions,
    content: DialogueContent,
    plan: LinePlan,
    source: SourceAnchor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueOptions {
    pub id: Option<PublicId>,
    pub text_key: Option<TextKey>,
    pub voice: Option<VoiceRef>,
    pub face: Option<PublicId>,
    pub text_box: Option<TextBoxRef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerPreset {
    speaker: SpeakerRef,
    options: DialogueOptions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoiceRef {
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
    steps: Vec<LinePlanStep>,
    cancel_scopes: Vec<CancelScope>,
    returns: Option<PlanExpr>,
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
    Return(PlanExpr),
}

impl SpeakerRef {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }

    #[must_use]
    pub fn preset(&self, options: DialogueOptions) -> SpeakerPreset {
        SpeakerPreset {
            speaker: self.clone(),
            options,
        }
    }
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
        options: DialogueOptions,
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
        options: DialogueOptions,
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

    pub const fn options(&self) -> &DialogueOptions {
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

impl DialogueOptions {
    pub const fn empty() -> Self {
        Self {
            id: None,
            text_key: None,
            voice: None,
            face: None,
            text_box: None,
        }
    }

    #[must_use]
    pub fn with_voice(mut self, voice: VoiceRef) -> Self {
        self.voice = Some(voice);
        self
    }

    #[must_use]
    pub fn with_face(mut self, face: PublicId) -> Self {
        self.face = Some(face);
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
            face: override_options.face.or(self.face),
            text_box: override_options.text_box.or(self.text_box),
        }
    }
}

impl Default for DialogueOptions {
    fn default() -> Self {
        Self::empty()
    }
}

impl SpeakerPreset {
    pub const fn speaker(&self) -> &SpeakerRef {
        &self.speaker
    }

    pub const fn options(&self) -> &DialogueOptions {
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

    pub fn parts(&self) -> &[DialogueContentPart] {
        &self.parts
    }
}

impl LinePlan {
    pub fn new(steps: impl Into<Vec<LinePlanStep>>) -> Self {
        Self {
            steps: steps.into(),
            cancel_scopes: Vec::new(),
            returns: None,
        }
    }

    #[must_use]
    pub fn with_return(mut self, value: PlanExpr) -> Self {
        self.returns = Some(value);
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

    pub const fn returns(&self) -> Option<&PlanExpr> {
        self.returns.as_ref()
    }
}

impl Default for LinePlan {
    fn default() -> Self {
        Self::new([])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DialogueContent, DialogueContentPart, DialogueLine, DialogueOptions, DialogueTag, LinePlan,
        LinePlanStep, PlanCall, PlanExpr, SpeakerRef, TextBoxRef, TimelineAnchor, VoiceRef,
    };
    use arcweft_id::PublicId;
    use arcweft_source::SourceAnchor;
    use core::time::Duration;

    #[test]
    fn models_speaker_preset_and_line_plan_return() {
        let alice =
            SpeakerRef::new(PublicId::try_new("character.alice").expect("valid speaker id"));
        let textbox = TextBoxRef::new(PublicId::try_new("textbox.side").expect("valid textbox id"));
        let smile = PublicId::try_new("expression.smile").expect("valid expression id");
        let worried = "worried".to_owned();

        let alice2 = alice.preset(
            DialogueOptions::empty()
                .with_face(smile.clone())
                .with_voice(VoiceRef::Auto)
                .with_text_box(textbox),
        );

        let content = DialogueContent::new([
            DialogueContentPart::Text("今日は少しだけ、".to_owned()),
            DialogueContentPart::Ruby {
                base: "変な夢".to_owned(),
                ruby: "へんなゆめ".to_owned(),
            },
            DialogueContentPart::Text("を見たんだ。".to_owned()),
            DialogueContentPart::Tag(DialogueTag::Page),
        ]);

        let actor = LinePlanStep::Let {
            name: "actor".to_owned(),
            expr: PlanExpr::Call(PlanCall {
                receiver: "alice2".to_owned(),
                method: "stage_handle".to_owned(),
                args: Vec::new(),
            }),
        };
        let face0 = LinePlanStep::Let {
            name: "face0".to_owned(),
            expr: PlanExpr::Call(PlanCall {
                receiver: "actor".to_owned(),
                method: "face".to_owned(),
                args: Vec::new(),
            }),
        };
        let face1 = LinePlanStep::Let {
            name: "face1".to_owned(),
            expr: PlanExpr::Name(worried),
        };
        let timed_face = LinePlanStep::At {
            anchor: TimelineAnchor::FromStart(Duration::from_millis(420)),
            step: Box::new(face1),
        };
        let voice = LinePlanStep::Let {
            name: "voice".to_owned(),
            expr: PlanExpr::Call(PlanCall {
                receiver: "line".to_owned(),
                method: "voice_handle".to_owned(),
                args: Vec::new(),
            }),
        };

        let plan =
            LinePlan::new([actor, face0, timed_face, voice]).with_return(PlanExpr::Tuple(vec![
                PlanExpr::Name("actor".to_owned()),
                PlanExpr::Tuple(vec![
                    PlanExpr::Name("face0".to_owned()),
                    PlanExpr::Name("face1".to_owned()),
                    PlanExpr::Name("voice".to_owned()),
                ]),
            ]));

        let line = DialogueLine::from_preset(
            &alice2,
            DialogueOptions::empty(),
            content,
            plan,
            SourceAnchor::generated(),
        );

        assert_eq!(line.speaker().id().as_str(), "character.alice");
        assert_eq!(line.options().face.as_ref(), Some(&smile));
        assert!(matches!(line.options().voice, Some(VoiceRef::Auto)));
        assert_eq!(line.plan().steps().len(), 4);
        assert!(line.plan().returns().is_some());
    }
}
