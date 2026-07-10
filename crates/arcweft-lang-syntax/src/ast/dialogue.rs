use super::common::{TextRange, Visibility};
use super::ids::{EntityRef, EntityRefSyntax, IdRef};
use super::items::Attribute;
use super::line_plan::LinePlan;
use crate::expr::Expr;
use thiserror::Error;

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
    Expr(DialogueExpr),
    Ruby { base: String, ruby: String },
    Escape(char),
}

/// Expression interpolation embedded in dialogue text mode.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueExpr {
    expr: Expr,
    source: String,
    range: TextRange,
}

/// Bracket tag such as `[p]`, `[wait ...]`, or `[ruby rt="..."]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueTag {
    name: String,
    attrs: String,
}

/// Positive duration accepted by the `[w ...]` dialogue control.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueWaitDuration {
    millis: u64,
}

/// Positive dialogue reveal rate in thousandths of a character per second.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialogueRevealSpeed {
    milli_cps: u32,
}

/// Invalid surface duration attached to a `[w ...]` control.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DialogueWaitDurationError {
    #[error("dialogue wait requires a duration such as `500ms` or `0.5s`")]
    Missing,
    #[error("dialogue wait duration `{value}` must use `ms` or `s`")]
    UnsupportedUnit { value: String },
    #[error("dialogue wait duration `{value}` is not a non-negative decimal")]
    InvalidNumber { value: String },
    #[error("dialogue wait duration `{value}` has precision below one millisecond")]
    SubMillisecondPrecision { value: String },
    #[error("dialogue wait duration must be greater than zero")]
    Zero,
    #[error("dialogue wait duration `{value}` exceeds the supported millisecond range")]
    Overflow { value: String },
}

/// Invalid value attached to a `[speed ...]` dialogue modifier.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DialogueRevealSpeedError {
    #[error("dialogue speed requires `slow`, `normal`, `fast`, or a characters-per-second value")]
    Missing,
    #[error("dialogue speed `{value}` is not a supported name or positive decimal")]
    InvalidNumber { value: String },
    #[error(
        "dialogue speed `{value}` has precision below one thousandth of a character per second"
    )]
    ExcessPrecision { value: String },
    #[error("dialogue speed `{value}` must be between 1 and 240 characters per second")]
    OutOfRange { value: String },
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
    style_raw: Option<String>,
    style_range: Option<TextRange>,
    rich_text: Option<Expr>,
    rich_text_raw: Option<String>,
    rich_text_range: Option<TextRange>,
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
    pub(crate) style_raw: Option<String>,
    pub(crate) style_range: Option<TextRange>,
    pub(crate) rich_text: Option<Expr>,
    pub(crate) rich_text_raw: Option<String>,
    pub(crate) rich_text_range: Option<TextRange>,
    pub(crate) args: Vec<LineArg>,
}

/// Non-reserved line option preserved as a named argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineArg {
    name: String,
    value: Expr,
    raw_value: String,
    value_range: TextRange,
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

impl DialogueExpr {
    pub(crate) fn new(expr: Expr, source: String, range: TextRange) -> Self {
        Self {
            expr,
            source,
            range,
        }
    }

    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub const fn range(&self) -> TextRange {
        self.range
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

    /// Parses the positional or `time=` duration of a `[w ...]` tag.
    pub fn wait_duration(&self) -> Result<DialogueWaitDuration, DialogueWaitDurationError> {
        let attrs = self.attrs.trim();
        if attrs.is_empty() {
            return Err(DialogueWaitDurationError::Missing);
        }
        let value = attrs.strip_prefix("time=").unwrap_or(attrs).trim();
        if value.is_empty() || value.split_whitespace().count() != 1 {
            return Err(DialogueWaitDurationError::InvalidNumber {
                value: value.to_owned(),
            });
        }
        let millis = if let Some(number) = value.strip_suffix("ms") {
            parse_wait_integer(number, value)?
        } else if let Some(number) = value.strip_suffix('s') {
            parse_wait_seconds(number, value)?
        } else {
            return Err(DialogueWaitDurationError::UnsupportedUnit {
                value: value.to_owned(),
            });
        };
        if millis == 0 {
            return Err(DialogueWaitDurationError::Zero);
        }
        Ok(DialogueWaitDuration { millis })
    }

    /// Parses the named or numeric rate of a `[speed ...]` tag.
    pub fn reveal_speed(&self) -> Result<DialogueRevealSpeed, DialogueRevealSpeedError> {
        let attrs = self.attrs.trim();
        if attrs.is_empty() {
            return Err(DialogueRevealSpeedError::Missing);
        }
        let value = attrs
            .strip_prefix("cps=")
            .or_else(|| attrs.strip_prefix("speed="))
            .unwrap_or(attrs)
            .trim();
        if value.is_empty() || value.split_whitespace().count() != 1 {
            return Err(DialogueRevealSpeedError::InvalidNumber {
                value: value.to_owned(),
            });
        }
        let milli_cps = match value {
            "slow" => 14_000,
            "normal" => 28_000,
            "fast" => 56_000,
            _ => parse_reveal_speed_milli(value)?,
        };
        if !(1_000..=240_000).contains(&milli_cps) {
            return Err(DialogueRevealSpeedError::OutOfRange {
                value: value.to_owned(),
            });
        }
        Ok(DialogueRevealSpeed { milli_cps })
    }
}

impl DialogueWaitDuration {
    #[must_use]
    pub const fn millis(self) -> u64 {
        self.millis
    }
}

impl DialogueRevealSpeed {
    #[must_use]
    pub const fn milli_cps(self) -> u32 {
        self.milli_cps
    }

    /// Canonical decimal consumed by the render-text style boundary.
    #[must_use]
    pub fn canonical_cps(self) -> String {
        let whole = self.milli_cps / 1_000;
        let fraction = self.milli_cps % 1_000;
        if fraction == 0 {
            return whole.to_string();
        }
        let fraction = format!("{fraction:03}");
        format!("{whole}.{}", fraction.trim_end_matches('0'))
    }
}

fn parse_reveal_speed_milli(value: &str) -> Result<u32, DialogueRevealSpeedError> {
    let (whole, fraction) = match value.split_once('.') {
        Some((_, "")) => {
            return Err(DialogueRevealSpeedError::InvalidNumber {
                value: value.to_owned(),
            });
        }
        Some(parts) => parts,
        None => (value, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(DialogueRevealSpeedError::InvalidNumber {
            value: value.to_owned(),
        });
    }
    if fraction.len() > 3 {
        return Err(DialogueRevealSpeedError::ExcessPrecision {
            value: value.to_owned(),
        });
    }
    let whole = whole
        .parse::<u32>()
        .ok()
        .and_then(|whole| whole.checked_mul(1_000))
        .ok_or_else(|| DialogueRevealSpeedError::OutOfRange {
            value: value.to_owned(),
        })?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        let scale = match fraction.len() {
            1 => 100,
            2 => 10,
            _ => 1,
        };
        fraction
            .parse::<u32>()
            .map_err(|_| DialogueRevealSpeedError::InvalidNumber {
                value: value.to_owned(),
            })?
            * scale
    };
    whole
        .checked_add(fraction)
        .ok_or_else(|| DialogueRevealSpeedError::OutOfRange {
            value: value.to_owned(),
        })
}

fn parse_wait_integer(number: &str, source: &str) -> Result<u64, DialogueWaitDurationError> {
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DialogueWaitDurationError::InvalidNumber {
            value: source.to_owned(),
        });
    }
    number
        .parse::<u64>()
        .map_err(|_| DialogueWaitDurationError::Overflow {
            value: source.to_owned(),
        })
}

fn parse_wait_seconds(number: &str, source: &str) -> Result<u64, DialogueWaitDurationError> {
    let (seconds, fraction) = match number.split_once('.') {
        Some((_, "")) => {
            return Err(DialogueWaitDurationError::InvalidNumber {
                value: source.to_owned(),
            });
        }
        Some((seconds, fraction)) => (seconds, fraction),
        None => (number, ""),
    };
    if seconds.is_empty()
        || !seconds.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(DialogueWaitDurationError::InvalidNumber {
            value: source.to_owned(),
        });
    }
    if fraction.len() > 3 {
        return Err(DialogueWaitDurationError::SubMillisecondPrecision {
            value: source.to_owned(),
        });
    }
    let whole = seconds
        .parse::<u64>()
        .map_err(|_| DialogueWaitDurationError::Overflow {
            value: source.to_owned(),
        })?
        .checked_mul(1_000)
        .ok_or_else(|| DialogueWaitDurationError::Overflow {
            value: source.to_owned(),
        })?;
    let fractional = if fraction.is_empty() {
        0
    } else {
        let scale = match fraction.len() {
            1 => 100,
            2 => 10,
            _ => 1,
        };
        fraction
            .parse::<u64>()
            .map_err(|_| DialogueWaitDurationError::InvalidNumber {
                value: source.to_owned(),
            })?
            * scale
    };
    whole
        .checked_add(fractional)
        .ok_or_else(|| DialogueWaitDurationError::Overflow {
            value: source.to_owned(),
        })
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
            style_raw: init.style_raw,
            style_range: init.style_range,
            rich_text: init.rich_text,
            rich_text_raw: init.rich_text_raw,
            rich_text_range: init.rich_text_range,
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

    pub fn style_raw(&self) -> Option<&str> {
        self.style_raw.as_deref()
    }

    pub const fn style_range(&self) -> Option<TextRange> {
        self.style_range
    }

    pub const fn rich_text(&self) -> Option<&Expr> {
        self.rich_text.as_ref()
    }

    pub fn rich_text_raw(&self) -> Option<&str> {
        self.rich_text_raw.as_deref()
    }

    pub const fn rich_text_range(&self) -> Option<TextRange> {
        self.rich_text_range
    }

    pub fn args(&self) -> &[LineArg] {
        &self.args
    }
}

impl LineArg {
    pub(crate) const fn new(
        name: String,
        value: Expr,
        raw_value: String,
        value_range: TextRange,
    ) -> Self {
        Self {
            name,
            value,
            raw_value,
            value_range,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }

    pub fn raw_value(&self) -> &str {
        &self.raw_value
    }

    pub const fn value_range(&self) -> &TextRange {
        &self.value_range
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
