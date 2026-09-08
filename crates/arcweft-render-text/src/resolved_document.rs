//! Canonical post-resolution text and style boundary.

use arcweft_core::locale::LocaleId;
use arcweft_presentation::fx::FxColor;
use arcweft_text_model::{
    LineDisplayFrame, LineDisplayFrameValidationError, LineDisplayStage, RichTextAngle,
    RichTextColor, RichTextControl, RichTextDocument, RichTextFontFamily, RichTextInlineDirection,
    RichTextNode, RichTextNodeIndex, RichTextNodeRange, RichTextPresentation, RichTextRange,
    RichTextRubyPosition, RichTextStyle, RichTextTextRunRange, RichTextWritingMode,
    presentation_from_styles,
};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Write};
use thiserror::Error;

/// Deterministic revision of an owning resolved-text source record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct TextDocumentRevision(u64);

impl TextDocumentRevision {
    /// Builds a revision from a source-owner revision value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the source-owner revision value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn for_source(source: &impl fmt::Debug) -> Self {
        let mut writer = RevisionWriter(0xcbf2_9ce4_8422_2325);
        write!(&mut writer, "{source:?}").expect("revision writer is infallible");
        Self(writer.0)
    }
}

struct RevisionWriter(u64);

impl Write for RevisionWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = value.as_bytes().iter().fold(self.0, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        Ok(())
    }
}

/// Validated BCP-47-style language identifier used during shaping.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct LanguageTag(LocaleId);

impl LanguageTag {
    /// Validates and stores an ASCII language tag.
    pub fn new(value: impl Into<String>) -> Result<Self, TextResolveError> {
        let value = value.into();
        LocaleId::try_new(value)
            .map(Self)
            .map_err(|error| TextResolveError::InvalidLanguageTag {
                value: error.into_value(),
            })
    }

    /// Returns the normalized source spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub const fn locale_id(&self) -> &LocaleId {
        &self.0
    }
}

/// One family in the ordered project-font fallback stack.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum TextFontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named(String),
}

impl From<&RichTextFontFamily> for TextFontFamily {
    fn from(value: &RichTextFontFamily) -> Self {
        match value {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }
}

/// Closed font-weight set used by the shared text shaper.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextWeight {
    Thin,
    ExtraLight,
    Light,
    #[default]
    Normal,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

/// Closed font-slant set used by the shared text shaper.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextSlant {
    #[default]
    Upright,
    Italic,
    Oblique {
        angle: RichTextAngle,
    },
}

/// Resolved RGBA color.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct TextColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl TextColor {
    /// Creates a color from unpremultiplied channels.
    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns unpremultiplied RGBA channels.
    #[must_use]
    pub const fn channels(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }
}

impl From<FxColor> for TextColor {
    fn from(value: FxColor) -> Self {
        fn channel(value: arcweft_presentation::fx::Opacity) -> u8 {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "Opacity is validated in [0, 1] before deterministic u8 quantization"
            )]
            {
                (value.value().get() * 255.0).round() as u8
            }
        }
        Self::rgba(
            channel(value.red()),
            channel(value.green()),
            channel(value.blue()),
            channel(value.alpha()),
        )
    }
}

impl Default for TextColor {
    fn default() -> Self {
        Self::rgba(245, 245, 245, 255)
    }
}

impl From<&RichTextColor> for TextColor {
    fn from(value: &RichTextColor) -> Self {
        match value {
            RichTextColor::Rgba8 { value } => Self::rgba(value[0], value[1], value[2], value[3]),
            // Resource-backed colors are resolved by the resource/style owner,
            // not by guessing authoring names inside the text renderer.
            RichTextColor::Resource { .. } => Self::default(),
        }
    }
}

/// Fully resolved, renderer-independent text style.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResolvedTextStyle {
    font_families: Vec<TextFontFamily>,
    font_size_milli: u32,
    line_height_milli: u32,
    weight: TextWeight,
    slant: TextSlant,
    color: TextColor,
    letter_spacing_milli: i32,
    word_spacing_milli: i32,
    writing_mode: RichTextWritingMode,
    direction: RichTextInlineDirection,
    language: Option<LanguageTag>,
}

impl ResolvedTextStyle {
    /// Creates a validated closed style value.
    pub fn new(
        font_families: Vec<TextFontFamily>,
        font_size_milli: u32,
        line_height_milli: u32,
    ) -> Result<Self, TextResolveError> {
        let style = Self {
            font_families,
            font_size_milli,
            line_height_milli,
            ..Self::default()
        };
        style.validate()?;
        Ok(style)
    }

    /// Sets the resolved font weight.
    #[must_use]
    pub const fn with_weight(mut self, weight: TextWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Replaces the ordered project-font family stack.
    pub fn with_font_families(
        mut self,
        font_families: Vec<TextFontFamily>,
    ) -> Result<Self, TextResolveError> {
        self.font_families = font_families;
        self.validate()?;
        Ok(self)
    }

    /// Sets the resolved font slant.
    #[must_use]
    pub const fn with_slant(mut self, slant: TextSlant) -> Self {
        self.slant = slant;
        self
    }

    /// Sets the resolved text color.
    #[must_use]
    pub const fn with_color(mut self, color: TextColor) -> Self {
        self.color = color;
        self
    }

    /// Sets letter and word spacing in milli-pixels.
    #[must_use]
    pub const fn with_spacing(mut self, letter_milli: i32, word_milli: i32) -> Self {
        self.letter_spacing_milli = letter_milli;
        self.word_spacing_milli = word_milli;
        self
    }

    /// Sets the resolved writing mode and inline direction.
    #[must_use]
    pub const fn with_flow(
        mut self,
        writing_mode: RichTextWritingMode,
        direction: RichTextInlineDirection,
    ) -> Self {
        self.writing_mode = writing_mode;
        self.direction = direction;
        self
    }

    /// Sets the shaping language.
    #[must_use]
    pub fn with_language(mut self, language: Option<LanguageTag>) -> Self {
        self.language = language;
        self
    }

    /// Replaces font-size and line-height metrics while preserving every
    /// other resolved style field.
    pub fn with_font_metrics(
        mut self,
        font_size_milli: u32,
        line_height_milli: u32,
    ) -> Result<Self, TextResolveError> {
        self.font_size_milli = font_size_milli;
        self.line_height_milli = line_height_milli;
        self.validate()?;
        Ok(self)
    }

    #[must_use]
    pub fn font_families(&self) -> &[TextFontFamily] {
        &self.font_families
    }

    #[must_use]
    pub const fn font_size_milli(&self) -> u32 {
        self.font_size_milli
    }

    #[must_use]
    pub const fn line_height_milli(&self) -> u32 {
        self.line_height_milli
    }

    #[must_use]
    pub const fn weight(&self) -> TextWeight {
        self.weight
    }

    #[must_use]
    pub const fn slant(&self) -> TextSlant {
        self.slant
    }

    #[must_use]
    pub const fn color(&self) -> TextColor {
        self.color
    }

    #[must_use]
    pub const fn letter_spacing_milli(&self) -> i32 {
        self.letter_spacing_milli
    }

    #[must_use]
    pub const fn word_spacing_milli(&self) -> i32 {
        self.word_spacing_milli
    }

    #[must_use]
    pub const fn writing_mode(&self) -> RichTextWritingMode {
        self.writing_mode
    }

    #[must_use]
    pub const fn direction(&self) -> RichTextInlineDirection {
        self.direction
    }

    #[must_use]
    pub fn language(&self) -> Option<&LanguageTag> {
        self.language.as_ref()
    }

    fn apply(&mut self, rich_style: &RichTextStyle) {
        match rich_style {
            RichTextStyle::Em | RichTextStyle::Italic => {
                self.slant = TextSlant::Italic;
            }
            RichTextStyle::Oblique { angle } => {
                self.slant = TextSlant::Oblique { angle: *angle };
            }
            RichTextStyle::Strong => self.weight = TextWeight::Bold,
            RichTextStyle::Color { value } => self.color = TextColor::from(value),
            RichTextStyle::Font { family } => {
                self.font_families = vec![TextFontFamily::from(family)];
            }
            RichTextStyle::Size { milli_points } => {
                if let Ok(milli_points) = u32::try_from(milli_points.0) {
                    self.font_size_milli = milli_points;
                    self.line_height_milli = milli_points.saturating_mul(1_350) / 1_000;
                }
            }
            RichTextStyle::Layout { layout } => {
                if !matches!(layout.writing_mode, RichTextWritingMode::HorizontalTb)
                    || matches!(layout.ruby_position, RichTextRubyPosition::Auto)
                {
                    self.writing_mode = layout.writing_mode;
                }
                if !matches!(layout.direction, RichTextInlineDirection::Auto) {
                    self.direction = layout.direction;
                }
            }
            RichTextStyle::Speed { .. }
            | RichTextStyle::Transform { .. }
            | RichTextStyle::Fx { .. }
            | RichTextStyle::Object { .. }
            | RichTextStyle::Presentation { .. } => {}
        }
    }

    fn validate(&self) -> Result<(), TextResolveError> {
        if self.font_families.is_empty() {
            return Err(TextResolveError::EmptyFontFamilyStack);
        }
        if let Some(index) = self.font_families.iter().position(
            |family| matches!(family, TextFontFamily::Named(name) if name.trim().is_empty()),
        ) {
            return Err(TextResolveError::EmptyNamedFontFamily { index });
        }
        if self.font_size_milli == 0 {
            return Err(TextResolveError::ZeroFontSize);
        }
        if self.line_height_milli == 0 {
            return Err(TextResolveError::ZeroLineHeight);
        }
        Ok(())
    }
}

impl Default for ResolvedTextStyle {
    fn default() -> Self {
        Self {
            font_families: vec![TextFontFamily::SansSerif],
            font_size_milli: 16_000,
            line_height_milli: 21_600,
            weight: TextWeight::Normal,
            slant: TextSlant::Upright,
            color: TextColor::default(),
            letter_spacing_milli: 0,
            word_spacing_milli: 0,
            writing_mode: RichTextWritingMode::HorizontalTb,
            direction: RichTextInlineDirection::Auto,
            language: None,
        }
    }
}

/// Base resolved style and presentation applied before source-local spans.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TextStyleCascade {
    style: ResolvedTextStyle,
    presentation: RichTextPresentation,
}

impl TextStyleCascade {
    /// Creates a cascade from its lowest-priority resolved style.
    #[must_use]
    pub fn new(style: ResolvedTextStyle) -> Self {
        Self {
            style,
            presentation: RichTextPresentation::default(),
        }
    }

    /// Sets presentation metadata inherited by every resolved run.
    #[must_use]
    pub fn with_presentation(mut self, presentation: RichTextPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    #[must_use]
    pub const fn style(&self) -> &ResolvedTextStyle {
        &self.style
    }

    #[must_use]
    pub const fn presentation(&self) -> &RichTextPresentation {
        &self.presentation
    }

    /// Resolves an authored style stack over this cascade's base style.
    ///
    /// Presentation adapters use this for sibling content, such as a
    /// dialogue View speaker label, that must inherit the same frame-level style
    /// as the canonical rich-text document without reimplementing cascade
    /// rules.
    pub fn resolve_style<'a>(
        &self,
        styles: impl IntoIterator<Item = &'a RichTextStyle>,
    ) -> Result<ResolvedTextStyle, TextResolveError> {
        let resolved = styles
            .into_iter()
            .fold(self.style.clone(), |mut style, rich| {
                style.apply(rich);
                style
            });
        resolved.validate()?;
        Ok(resolved)
    }

    fn resolve_presentation(&self, presentation: &RichTextPresentation) -> RichTextPresentation {
        let mut resolved = self.presentation.clone();
        resolved.merge(presentation.clone());
        resolved
    }
}

/// Source category retained after text resolution.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedTextRunSource {
    Plain,
    Localized,
    Dialogue { node_index: usize },
    Editable,
    Generated,
}

/// One styled span in canonical document-local order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTextRun {
    range: RichTextRange,
    source_range: RichTextRange,
    style: ResolvedTextStyle,
    presentation: RichTextPresentation,
    source: ResolvedTextRunSource,
}

impl ResolvedTextRun {
    /// Creates a run whose source and document ranges have equal byte length.
    pub fn new(
        range: RichTextRange,
        source_range: RichTextRange,
        style: ResolvedTextStyle,
        presentation: RichTextPresentation,
        source: ResolvedTextRunSource,
    ) -> Result<Self, TextResolveError> {
        validate_pair("text run", 0, range, source_range)?;
        Ok(Self {
            range,
            source_range,
            style,
            presentation,
            source,
        })
    }

    #[must_use]
    pub const fn range(&self) -> RichTextRange {
        self.range
    }

    #[must_use]
    pub const fn source_range(&self) -> RichTextRange {
        self.source_range
    }

    #[must_use]
    pub const fn style(&self) -> &ResolvedTextStyle {
        &self.style
    }

    #[must_use]
    pub const fn presentation(&self) -> &RichTextPresentation {
        &self.presentation
    }

    #[must_use]
    pub const fn source(&self) -> ResolvedTextRunSource {
        self.source
    }
}

/// Ruby annotation attached to a canonical base range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTextRuby {
    owner_node: RichTextNodeIndex,
    body_nodes: RichTextNodeRange,
    base_runs: RichTextTextRunRange,
    base_range: RichTextRange,
    source_base_range: RichTextRange,
    text: String,
    style: ResolvedTextStyle,
    presentation: RichTextPresentation,
}

impl ResolvedTextRuby {
    /// Creates a non-empty annotation with exact source-tree and run
    /// ownership intervals.
    pub fn new(
        owner_node: RichTextNodeIndex,
        body_nodes: RichTextNodeRange,
        base_runs: RichTextTextRunRange,
        base_range: RichTextRange,
        source_base_range: RichTextRange,
        text: impl Into<String>,
        style: ResolvedTextStyle,
        presentation: RichTextPresentation,
    ) -> Result<Self, TextResolveError> {
        validate_pair("ruby", 0, base_range, source_base_range)?;
        let text = text.into();
        if text.is_empty() {
            return Err(TextResolveError::EmptyRubyText { index: 0 });
        }
        Ok(Self {
            owner_node,
            body_nodes,
            base_runs,
            base_range,
            source_base_range,
            text,
            style,
            presentation,
        })
    }

    /// Creates an annotation while retaining its exact source-tree and
    /// resolved-run ownership intervals.
    #[must_use]
    pub const fn owner_node(&self) -> RichTextNodeIndex {
        self.owner_node
    }

    #[must_use]
    pub const fn body_nodes(&self) -> RichTextNodeRange {
        self.body_nodes
    }

    #[must_use]
    pub const fn base_runs(&self) -> RichTextTextRunRange {
        self.base_runs
    }

    #[must_use]
    pub const fn base_range(&self) -> RichTextRange {
        self.base_range
    }

    #[must_use]
    pub const fn source_base_range(&self) -> RichTextRange {
        self.source_base_range
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn style(&self) -> &ResolvedTextStyle {
        &self.style
    }

    #[must_use]
    pub const fn presentation(&self) -> &RichTextPresentation {
        &self.presentation
    }
}

/// Canonical borrowed post-resolution text document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTextDocument<'a> {
    text: &'a str,
    source_origin: usize,
    runs: Vec<ResolvedTextRun>,
    ruby: Vec<ResolvedTextRuby>,
    revision: TextDocumentRevision,
}

impl<'a> ResolvedTextDocument<'a> {
    /// Creates and validates one complete canonical text projection.
    pub fn new(
        text: &'a str,
        source_origin: usize,
        runs: Vec<ResolvedTextRun>,
        ruby: Vec<ResolvedTextRuby>,
        revision: TextDocumentRevision,
    ) -> Result<Self, TextResolveError> {
        source_origin
            .checked_add(text.len())
            .ok_or(TextResolveError::SourceOriginOverflow {
                source_origin,
                text_len: text.len(),
            })?;
        validate_runs(text, source_origin, &runs)?;
        validate_ruby(text, source_origin, &runs, &ruby)?;
        Ok(Self {
            text,
            source_origin,
            runs,
            ruby,
            revision,
        })
    }

    #[must_use]
    pub const fn text(&self) -> &'a str {
        self.text
    }

    #[must_use]
    pub const fn source_origin(&self) -> usize {
        self.source_origin
    }

    #[must_use]
    pub fn source_range(&self) -> RichTextRange {
        RichTextRange::new(self.source_origin, self.source_origin + self.text.len())
    }

    #[must_use]
    pub fn runs(&self) -> &[ResolvedTextRun] {
        &self.runs
    }

    #[must_use]
    pub fn ruby(&self) -> &[ResolvedTextRuby] {
        &self.ruby
    }

    #[must_use]
    pub const fn revision(&self) -> TextDocumentRevision {
        self.revision
    }

    /// Borrows a validated document-local subrange and rebases its metadata.
    ///
    /// This is used by display controls such as `[clear]`: the remaining text
    /// starts again at the dialogue View origin without cloning the source string or
    /// rebuilding its style cascade.
    pub fn project(
        &self,
        range: RichTextRange,
    ) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
        if range.start > range.end
            || range.end > self.text.len()
            || !self.text.is_char_boundary(range.start)
            || !self.text.is_char_boundary(range.end)
        {
            return Err(TextResolveError::InvalidUtf8Range {
                kind: "text projection",
                index: 0,
                start: range.start,
                end: range.end,
                text_len: self.text.len(),
            });
        }
        let text =
            self.text
                .get(range.start..range.end)
                .ok_or(TextResolveError::InvalidUtf8Range {
                    kind: "text projection",
                    index: 0,
                    start: range.start,
                    end: range.end,
                    text_len: self.text.len(),
                })?;
        let mut run_index_map = vec![None; self.runs.len()];
        let runs = self
            .runs
            .iter()
            .enumerate()
            .filter_map(|(run_index, run)| {
                intersect(run.range, range).map(|clipped| (run_index, run, clipped))
            })
            .map(|(run_index, run, clipped)| {
                let source_start = run.source_range.start + clipped.start - run.range.start;
                let source_range = RichTextRange::new(
                    source_start,
                    source_start + clipped.end.saturating_sub(clipped.start),
                );
                let projected_index = u32::try_from(run_index_map.iter().flatten().count())
                    .map_err(|_| TextResolveError::RunIndexOverflow { index: run_index })?;
                run_index_map[run_index] = Some(projected_index);
                ResolvedTextRun::new(
                    rebase(clipped, range.start),
                    source_range,
                    run.style.clone(),
                    run.presentation.clone(),
                    run.source,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ruby = self
            .ruby
            .iter()
            .filter(|annotation| contains(range, annotation.base_range))
            .map(|annotation| {
                let old_run_range = annotation.base_runs.as_usize_range();
                let projected_run_start = old_run_range
                    .clone()
                    .find_map(|index| run_index_map.get(index).copied().flatten())
                    .ok_or(TextResolveError::RubyRunMissing { index: 0 })?;
                let projected_run_end = old_run_range
                    .rev()
                    .find_map(|index| run_index_map.get(index).copied().flatten())
                    .and_then(|index| index.checked_add(1))
                    .ok_or(TextResolveError::RubyRunMissing { index: 0 })?;
                ResolvedTextRuby::new(
                    annotation.owner_node,
                    annotation.body_nodes,
                    RichTextTextRunRange::new(projected_run_start, projected_run_end),
                    rebase(annotation.base_range, range.start),
                    annotation.source_base_range,
                    annotation.text.clone(),
                    annotation.style.clone(),
                    annotation.presentation.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        ResolvedTextDocument::new(
            text,
            self.source_origin + range.start,
            runs,
            ruby,
            self.revision,
        )
    }
}

/// Structured rejection raised while constructing canonical text.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextResolveError {
    #[error(transparent)]
    InvalidDisplayFrame(#[from] LineDisplayFrameValidationError),
    #[error("display stage belongs to a different line display frame")]
    StageOwnerMismatch,
    #[error("display frame has no stage {index}")]
    InvalidDisplayStage { index: usize },
    #[error("{kind} {index} has an empty range")]
    EmptyRange { kind: &'static str, index: usize },
    #[error("{kind} {index} has descending range {start}..{end}")]
    DescendingRange {
        kind: &'static str,
        index: usize,
        start: usize,
        end: usize,
    },
    #[error("{kind} {index} maps {range_len} bytes to {source_len} source bytes")]
    RangeLengthMismatch {
        kind: &'static str,
        index: usize,
        range_len: usize,
        source_len: usize,
    },
    #[error("{kind} {index} has invalid UTF-8 range {start}..{end} for {text_len} bytes")]
    InvalidUtf8Range {
        kind: &'static str,
        index: usize,
        start: usize,
        end: usize,
        text_len: usize,
    },
    #[error("text run {index} starts at {actual_start}, expected {expected_start}")]
    RunDiscontinuity {
        index: usize,
        expected_start: usize,
        actual_start: usize,
    },
    #[error("text runs cover {covered_end} of {text_len} bytes")]
    IncompleteRunCoverage { covered_end: usize, text_len: usize },
    #[error("resolved text run index {index} exceeds u32::MAX")]
    RunIndexOverflow { index: usize },
    #[error(
        "{kind} {index} source range is {actual_start}..{actual_end}, expected {expected_start}..{expected_end}"
    )]
    SourceRangeMismatch {
        kind: &'static str,
        index: usize,
        expected_start: usize,
        expected_end: usize,
        actual_start: usize,
        actual_end: usize,
    },
    #[error("ruby annotation {index} has empty text")]
    EmptyRubyText { index: usize },
    #[error("ruby annotation {index} is not contained in a resolved text run")]
    RubyRunMissing { index: usize },
    #[error("source origin {source_origin} plus text length {text_len} overflows usize")]
    SourceOriginOverflow {
        source_origin: usize,
        text_len: usize,
    },
    #[error("rich-text node {node_index} requires runtime resolution before canonicalization")]
    DynamicNode { node_index: usize },
    #[error("rich-text node {node_index} no longer matches cached source text at {start}..{end}")]
    SourceTextMismatch {
        node_index: usize,
        start: usize,
        end: usize,
    },
    #[error("control {node_index} `{control}` is not allowed inside Ruby")]
    RubyControlForbidden {
        node_index: usize,
        control: &'static str,
    },
    #[error("font family stack must not be empty")]
    EmptyFontFamilyStack,
    #[error("named font family {index} must not be empty")]
    EmptyNamedFontFamily { index: usize },
    #[error("font size must be greater than zero")]
    ZeroFontSize,
    #[error("line height must be greater than zero")]
    ZeroLineHeight,
    #[error("invalid language tag `{value}`")]
    InvalidLanguageTag { value: String },
}

/// Borrows one stage slice and rebases its clipped metadata.
pub fn resolve_stage_document<'a>(
    frame: &'a LineDisplayFrame,
    stage: LineDisplayStage<'a>,
    cascade: &TextStyleCascade,
) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
    if !std::ptr::eq(frame, stage.frame()) {
        return Err(TextResolveError::StageOwnerMismatch);
    }
    frame.validate()?;
    let source_extent = stage.text_range();
    let text = stage.text();
    let projection = stage.projection();
    let runs = projection
        .display_map
        .text_runs
        .iter()
        .map(|run| {
            let source_range = RichTextRange::new(
                source_extent.start + run.range.start,
                source_extent.start + run.range.end,
            );
            let style = cascade.resolve_style(run.styles.iter())?;
            ResolvedTextRun::new(
                run.range,
                source_range,
                style,
                cascade.resolve_presentation(&run.presentation),
                ResolvedTextRunSource::Dialogue {
                    node_index: run.node_index.as_usize(),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ruby = projection
        .display_map
        .ruby_annotations
        .iter()
        .map(|ruby| {
            let style = cascade.resolve_style(ruby.styles.iter())?;
            let source_base_range = RichTextRange::new(
                source_extent.start + ruby.base_range.start,
                source_extent.start + ruby.base_range.end,
            );
            ResolvedTextRuby::new(
                ruby.owner_node,
                ruby.body_nodes,
                ruby.base_runs,
                ruby.base_range,
                source_base_range,
                ruby.ruby.clone(),
                style,
                cascade.resolve_presentation(&ruby.presentation),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    ResolvedTextDocument::new(
        text,
        source_extent.start,
        runs,
        ruby,
        TextDocumentRevision::for_source(frame),
    )
}

/// Resolves a static rich-text document into the canonical borrowed model.
pub fn resolve_document<'a>(
    document: &'a RichTextDocument,
    cascade: &TextStyleCascade,
) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
    resolve_document_with_source(document, cascade, ResolvedTextRunSource::Generated)
}

/// Resolves a static document while retaining its shared source category.
pub fn resolve_document_with_source<'a>(
    document: &'a RichTextDocument,
    cascade: &TextStyleCascade,
    source: ResolvedTextRunSource,
) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
    let mut ruby = Vec::new();
    let mut resolver = StaticDocumentResolver::new(document.resolved_text(), cascade, source);
    let mut node_index = 0;
    resolver.resolve_nodes(&document.nodes, &[], &mut node_index, false, &mut ruby)?;
    ruby.sort_by_key(ResolvedTextRuby::owner_node);
    ResolvedTextDocument::new(
        document.resolved_text(),
        0,
        resolver.runs,
        ruby,
        TextDocumentRevision::for_source(document),
    )
}

struct StaticDocumentResolver<'a> {
    document_text: &'a str,
    cascade: &'a TextStyleCascade,
    source: ResolvedTextRunSource,
    offset: usize,
    runs: Vec<ResolvedTextRun>,
}

impl<'a> StaticDocumentResolver<'a> {
    fn new(
        document_text: &'a str,
        cascade: &'a TextStyleCascade,
        source: ResolvedTextRunSource,
    ) -> Self {
        Self {
            document_text,
            cascade,
            source,
            offset: 0,
            runs: Vec::new(),
        }
    }

    fn push_text(
        &mut self,
        text: &str,
        node_index: usize,
        styles: &[RichTextStyle],
        source: ResolvedTextRunSource,
    ) -> Result<RichTextRange, TextResolveError> {
        let range = RichTextRange::new(self.offset, self.offset + text.len());
        if self.document_text.get(range.start..range.end) != Some(text) {
            return Err(TextResolveError::SourceTextMismatch {
                node_index,
                start: range.start,
                end: range.end,
            });
        }
        self.offset = range.end;
        if !text.is_empty() {
            let presentation = presentation_from_styles(styles.iter());
            let style = self.cascade.resolve_style(styles.iter())?;
            self.runs.push(ResolvedTextRun::new(
                range,
                range,
                style,
                self.cascade.resolve_presentation(&presentation),
                source,
            )?);
        }
        Ok(range)
    }

    fn resolve_nodes(
        &mut self,
        nodes: &[RichTextNode],
        styles: &[RichTextStyle],
        node_index: &mut usize,
        in_ruby: bool,
        ruby: &mut Vec<ResolvedTextRuby>,
    ) -> Result<(), TextResolveError> {
        for node in nodes {
            let current_index = *node_index;
            *node_index =
                (*node_index)
                    .checked_add(1)
                    .ok_or(TextResolveError::SourceOriginOverflow {
                        source_origin: current_index,
                        text_len: 1,
                    })?;
            match node {
                RichTextNode::Text { text } => {
                    self.push_text(text, current_index, styles, self.source)?;
                }
                RichTextNode::Raw { text } => {
                    self.push_text(text, current_index, styles, self.source)?;
                }
                RichTextNode::Scope { style, body } => {
                    let mut nested_styles = styles.to_vec();
                    nested_styles.push(style.as_ref().clone());
                    self.resolve_nodes(body, &nested_styles, node_index, in_ruby, ruby)?;
                }
                RichTextNode::Ruby { body, ruby: text } => {
                    let start = self.offset;
                    let body_start = *node_index;
                    let run_start = self.runs.len();
                    self.resolve_nodes(body, styles, node_index, true, ruby)?;
                    let range = RichTextRange::new(start, self.offset);
                    if range.start != range.end {
                        let presentation = presentation_from_styles(styles.iter());
                        let style = self.cascade.resolve_style(styles.iter())?;
                        let owner_node =
                            RichTextNodeIndex::try_from_index(current_index).map_err(|_| {
                                TextResolveError::RunIndexOverflow {
                                    index: current_index,
                                }
                            })?;
                        let body_start =
                            RichTextNodeIndex::try_from_index(body_start).map_err(|_| {
                                TextResolveError::RunIndexOverflow { index: body_start }
                            })?;
                        let body_end =
                            RichTextNodeIndex::try_from_index(*node_index).map_err(|_| {
                                TextResolveError::RunIndexOverflow { index: *node_index }
                            })?;
                        let run_start = u32::try_from(run_start)
                            .map_err(|_| TextResolveError::RunIndexOverflow { index: run_start })?;
                        let run_end = u32::try_from(self.runs.len()).map_err(|_| {
                            TextResolveError::RunIndexOverflow {
                                index: self.runs.len(),
                            }
                        })?;
                        ruby.push(ResolvedTextRuby::new(
                            owner_node,
                            RichTextNodeRange::new(body_start, body_end),
                            RichTextTextRunRange::new(run_start, run_end),
                            range,
                            range,
                            text.clone(),
                            style,
                            self.cascade.resolve_presentation(&presentation),
                        )?);
                    }
                }
                RichTextNode::Control {
                    control: RichTextControl::HardBreak,
                } => {
                    self.push_text("\n", current_index, styles, self.source)?;
                }
                RichTextNode::Control { control }
                    if in_ruby
                        && matches!(
                            control,
                            RichTextControl::Page
                                | RichTextControl::LineWait
                                | RichTextControl::Clear
                        ) =>
                {
                    return Err(TextResolveError::RubyControlForbidden {
                        node_index: current_index,
                        control: match control {
                            RichTextControl::Page => "page",
                            RichTextControl::LineWait => "line_wait",
                            RichTextControl::Clear => "clear",
                            _ => unreachable!("guard restricts ruby controls"),
                        },
                    });
                }
                RichTextNode::Interpolation { .. } | RichTextNode::ContentInsert { .. } => {
                    return Err(TextResolveError::DynamicNode {
                        node_index: current_index,
                    });
                }
                RichTextNode::Control { .. } | RichTextNode::HostEvent { .. } => {}
            }
        }
        Ok(())
    }
}

fn validate_pair(
    kind: &'static str,
    index: usize,
    range: RichTextRange,
    source_range: RichTextRange,
) -> Result<(), TextResolveError> {
    if range.start > range.end {
        return Err(TextResolveError::DescendingRange {
            kind,
            index,
            start: range.start,
            end: range.end,
        });
    }
    if source_range.start > source_range.end {
        return Err(TextResolveError::DescendingRange {
            kind,
            index,
            start: source_range.start,
            end: source_range.end,
        });
    }
    if range.start == range.end {
        return Err(TextResolveError::EmptyRange { kind, index });
    }
    let range_len = range.end - range.start;
    let source_len = source_range.end - source_range.start;
    if range_len != source_len {
        return Err(TextResolveError::RangeLengthMismatch {
            kind,
            index,
            range_len,
            source_len,
        });
    }
    Ok(())
}

fn validate_runs(
    text: &str,
    source_origin: usize,
    runs: &[ResolvedTextRun],
) -> Result<(), TextResolveError> {
    let mut covered_end = 0;
    for (index, run) in runs.iter().enumerate() {
        validate_text_range(text, "text run", index, run.range)?;
        if run.range.start != covered_end {
            return Err(TextResolveError::RunDiscontinuity {
                index,
                expected_start: covered_end,
                actual_start: run.range.start,
            });
        }
        validate_source_mapping(
            "text run",
            index,
            source_origin,
            run.range,
            run.source_range,
        )?;
        covered_end = run.range.end;
    }
    if covered_end != text.len() {
        return Err(TextResolveError::IncompleteRunCoverage {
            covered_end,
            text_len: text.len(),
        });
    }
    Ok(())
}

fn validate_ruby(
    text: &str,
    source_origin: usize,
    runs: &[ResolvedTextRun],
    ruby: &[ResolvedTextRuby],
) -> Result<(), TextResolveError> {
    for (index, annotation) in ruby.iter().enumerate() {
        validate_text_range(text, "ruby", index, annotation.base_range)?;
        validate_source_mapping(
            "ruby",
            index,
            source_origin,
            annotation.base_range,
            annotation.source_base_range,
        )?;
        if annotation.text.is_empty() {
            return Err(TextResolveError::EmptyRubyText { index });
        }
        if annotation.body_nodes.start.get() != annotation.owner_node.get().saturating_add(1)
            || annotation.body_nodes.start > annotation.body_nodes.end
        {
            return Err(TextResolveError::RubyRunMissing { index });
        }
        let run_range = annotation.base_runs().as_usize_range();
        if run_range.start >= run_range.end || run_range.end > runs.len() {
            return Err(TextResolveError::RubyRunMissing { index });
        }
        let mut covered_end = annotation.base_range().start;
        for run in &runs[run_range] {
            if run.range().start != covered_end {
                return Err(TextResolveError::RubyRunMissing { index });
            }
            covered_end = run.range().end;
        }
        if covered_end != annotation.base_range().end {
            return Err(TextResolveError::RubyRunMissing { index });
        }
        if let Some(previous) = ruby.get(index.wrapping_sub(1))
            && annotation.owner_node() <= previous.owner_node()
        {
            return Err(TextResolveError::RubyRunMissing { index });
        }
    }
    for first in 0..ruby.len() {
        for second in first.saturating_add(1)..ruby.len() {
            let left = &ruby[first];
            let right = &ruby[second];
            if ranges_cross(
                &left.body_nodes.start.get(),
                &left.body_nodes.end.get(),
                &right.body_nodes.start.get(),
                &right.body_nodes.end.get(),
            ) || ranges_cross(
                &left.base_range.start,
                &left.base_range.end,
                &right.base_range.start,
                &right.base_range.end,
            ) || ranges_cross(
                &u64::from(left.base_runs.start),
                &u64::from(left.base_runs.end),
                &u64::from(right.base_runs.start),
                &u64::from(right.base_runs.end),
            ) {
                return Err(TextResolveError::RubyRunMissing { index: second });
            }
        }
    }
    Ok(())
}

fn ranges_cross<T: Ord>(left_start: &T, left_end: &T, right_start: &T, right_end: &T) -> bool {
    (left_start < right_start && right_start < left_end && left_end < right_end)
        || (right_start < left_start && left_start < right_end && right_end < left_end)
}

fn validate_text_range(
    text: &str,
    kind: &'static str,
    index: usize,
    range: RichTextRange,
) -> Result<(), TextResolveError> {
    if range.start >= range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(TextResolveError::InvalidUtf8Range {
            kind,
            index,
            start: range.start,
            end: range.end,
            text_len: text.len(),
        });
    }
    Ok(())
}

fn validate_source_mapping(
    kind: &'static str,
    index: usize,
    source_origin: usize,
    range: RichTextRange,
    source_range: RichTextRange,
) -> Result<(), TextResolveError> {
    let expected = RichTextRange::new(source_origin + range.start, source_origin + range.end);
    if source_range != expected {
        return Err(TextResolveError::SourceRangeMismatch {
            kind,
            index,
            expected_start: expected.start,
            expected_end: expected.end,
            actual_start: source_range.start,
            actual_end: source_range.end,
        });
    }
    Ok(())
}

fn intersect(left: RichTextRange, right: RichTextRange) -> Option<RichTextRange> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then(|| RichTextRange::new(start, end))
}

const fn contains(outer: RichTextRange, inner: RichTextRange) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

const fn rebase(range: RichTextRange, origin: usize) -> RichTextRange {
    RichTextRange::new(range.start - origin, range.end - origin)
}
