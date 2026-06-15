use super::common::{TextRange, Visibility};
use super::ids::{EntityRef, EntityRefSyntax, IdRef};
use super::items::Attribute;
use super::line_plan::LinePlan;
use crate::expr::Expr;

/// Parsed dialogue content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueContent {
    raw: String,
    tokens: Vec<DialogueToken>,
    range: TextRange,
}

/// Token emitted inside dialogue text mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueToken {
    Text(String),
    Raw(String),
    Tag(DialogueTag),
    InferredTag(DialogueTag),
    Mark(LineMark),
    EndTag(String),
    InferredEndTag,
    Expr(Expr),
    Ruby { base: String, ruby: String },
    Escape(char),
}

/// Bracket tag such as `[p]`, `[wait ...]`, or `[ruby rt="..."]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTag {
    name: String,
    attrs: String,
}

/// Zero-width marker emitted by `[mark .name]` inside dialogue text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineMark {
    name: String,
}

/// `alice(args): ...` speaker-line sugar for a character dialogue call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerLine {
    speaker: String,
    options: LineOptions,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// Canonical `alice.say(args)[...]` content call, plus `alice[...]` shorthand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentCall {
    callee: String,
    options: LineOptions,
    content: DialogueContent,
    plan: Option<LinePlan>,
    range: TextRange,
}

/// Structured dialogue line options parsed from the raw call argument list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LineOptions {
    id: Option<IdRef>,
    text_key: Option<IdRef>,
    voice: Option<Expr>,
    look: Option<Expr>,
    stage: Option<Expr>,
    portrait: Option<Expr>,
    focus: Option<Expr>,
    cleanup: Option<Expr>,
    window: Option<EntityRefSyntax>,
    source_locale: Option<String>,
    hooks: Vec<Expr>,
    style: Option<Expr>,
    rich_text: Option<Expr>,
    args: Vec<LineArg>,
}

/// Internal initializer for structured dialogue line options.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LineOptionsInit {
    pub(crate) id: Option<IdRef>,
    pub(crate) text_key: Option<IdRef>,
    pub(crate) voice: Option<Expr>,
    pub(crate) look: Option<Expr>,
    pub(crate) stage: Option<Expr>,
    pub(crate) portrait: Option<Expr>,
    pub(crate) focus: Option<Expr>,
    pub(crate) cleanup: Option<Expr>,
    pub(crate) window: Option<EntityRefSyntax>,
    pub(crate) source_locale: Option<String>,
    pub(crate) hooks: Vec<Expr>,
    pub(crate) style: Option<Expr>,
    pub(crate) rich_text: Option<Expr>,
    pub(crate) args: Vec<LineArg>,
}

/// Non-reserved line option preserved as a named argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineArg {
    name: String,
    value: Expr,
}

/// Global dialogue default declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueDefaultsItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    assignments: Vec<DialogueDefaultAssignment>,
    range: TextRange,
}

/// One assignment inside a `dialogue defaults` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueDefaultAssignment {
    path: DialogueDefaultPath,
    op: DialogueDefaultAssignOp,
    value: Expr,
    raw_value: String,
    range: TextRange,
    path_range: TextRange,
    value_range: TextRange,
}

/// Dot-separated path of a structured dialogue defaults assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueDefaultPath {
    segments: Vec<String>,
}

/// Assignment operator used by a dialogue defaults assignment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialogueDefaultAssignOp {
    Replace,
    Append,
}

impl DialogueContent {
    pub(crate) const fn new(raw: String, tokens: Vec<DialogueToken>, range: TextRange) -> Self {
        Self { raw, tokens, range }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn tokens(&self) -> &[DialogueToken] {
        &self.tokens
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl DialogueTag {
    pub(crate) const fn new(name: String, attrs: String) -> Self {
        Self { name, attrs }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attrs(&self) -> &str {
        &self.attrs
    }
}

impl LineMark {
    pub(crate) fn new(name: String) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl SpeakerLine {
    pub(crate) const fn new(
        speaker: String,
        options: LineOptions,
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            speaker,
            options,
            content,
            plan,
            range,
        }
    }

    pub fn speaker(&self) -> &str {
        &self.speaker
    }

    pub const fn options(&self) -> &LineOptions {
        &self.options
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ContentCall {
    pub(crate) const fn new(
        callee: String,
        options: LineOptions,
        content: DialogueContent,
        plan: Option<LinePlan>,
        range: TextRange,
    ) -> Self {
        Self {
            callee,
            options,
            content,
            plan,
            range,
        }
    }

    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub const fn options(&self) -> &LineOptions {
        &self.options
    }

    pub const fn content(&self) -> &DialogueContent {
        &self.content
    }

    pub const fn plan(&self) -> Option<&LinePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl LineOptions {
    pub(crate) fn new(init: LineOptionsInit) -> Self {
        Self {
            id: init.id,
            text_key: init.text_key,
            voice: init.voice,
            look: init.look,
            stage: init.stage,
            portrait: init.portrait,
            focus: init.focus,
            cleanup: init.cleanup,
            window: init.window,
            source_locale: init.source_locale,
            hooks: init.hooks,
            style: init.style,
            rich_text: init.rich_text,
            args: init.args,
        }
    }

    pub const fn id(&self) -> Option<&IdRef> {
        self.id.as_ref()
    }

    pub const fn text_key(&self) -> Option<&IdRef> {
        self.text_key.as_ref()
    }

    pub const fn voice(&self) -> Option<&Expr> {
        self.voice.as_ref()
    }

    pub const fn look(&self) -> Option<&Expr> {
        self.look.as_ref()
    }

    pub const fn stage(&self) -> Option<&Expr> {
        self.stage.as_ref()
    }

    pub const fn portrait(&self) -> Option<&Expr> {
        self.portrait.as_ref()
    }

    pub const fn focus(&self) -> Option<&Expr> {
        self.focus.as_ref()
    }

    pub const fn cleanup(&self) -> Option<&Expr> {
        self.cleanup.as_ref()
    }

    pub const fn window(&self) -> Option<&EntityRefSyntax> {
        self.window.as_ref()
    }

    pub fn source_locale(&self) -> Option<&str> {
        self.source_locale.as_deref()
    }

    pub fn hooks(&self) -> &[Expr] {
        &self.hooks
    }

    pub const fn style(&self) -> Option<&Expr> {
        self.style.as_ref()
    }

    pub const fn rich_text(&self) -> Option<&Expr> {
        self.rich_text.as_ref()
    }

    pub fn args(&self) -> &[LineArg] {
        &self.args
    }
}

impl LineArg {
    pub(crate) const fn new(name: String, value: Expr) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }
}

impl DialogueDefaultsItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        id: Option<EntityRef>,
        assignments: Vec<DialogueDefaultAssignment>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            id,
            assignments,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn assignments(&self) -> &[DialogueDefaultAssignment] {
        &self.assignments
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl DialogueDefaultAssignment {
    pub(crate) const fn new(
        path: DialogueDefaultPath,
        op: DialogueDefaultAssignOp,
        value: Expr,
        raw_value: String,
        range: TextRange,
        path_range: TextRange,
        value_range: TextRange,
    ) -> Self {
        Self {
            path,
            op,
            value,
            raw_value,
            range,
            path_range,
            value_range,
        }
    }

    pub const fn path(&self) -> &DialogueDefaultPath {
        &self.path
    }

    pub const fn op(&self) -> DialogueDefaultAssignOp {
        self.op
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }

    pub fn raw_value(&self) -> &str {
        &self.raw_value
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }

    pub const fn path_range(&self) -> &TextRange {
        &self.path_range
    }

    pub const fn value_range(&self) -> &TextRange {
        &self.value_range
    }
}

impl DialogueDefaultPath {
    pub fn from_dotted(path: &str) -> Option<Self> {
        let segments: Vec<String> = path
            .split('.')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect();
        (!segments.is_empty()).then_some(Self { segments })
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn dotted(&self) -> String {
        self.segments.join(".")
    }
}
