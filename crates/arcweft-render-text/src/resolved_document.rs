//! Canonical post-resolution text and style boundary.

use crate::{
    DialogueHostEvent, LineDisplayFrame, LineDisplayFrameValidationError, LineDisplayStage,
    RichTextColor, RichTextControl, RichTextDocument, RichTextFontFamily, RichTextInlineDirection,
    RichTextNode, RichTextPresentation, RichTextRange, RichTextStyle, RichTextWritingMode,
    presentation_from_styles,
};
use arcweft_dialogue::rich_text::canonical_tag_name;
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
pub struct LanguageTag(String);

impl LanguageTag {
    /// Validates and stores an ASCII language tag.
    pub fn new(value: impl Into<String>) -> Result<Self, TextResolveError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 63
            && value.split('-').all(|part| {
                !part.is_empty()
                    && part.len() <= 8
                    && part.bytes().all(|b| b.is_ascii_alphanumeric())
            });
        if !valid {
            return Err(TextResolveError::InvalidLanguageTag { value });
        }
        Ok(Self(value))
    }

    /// Returns the normalized source spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
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
        angle: crate::RichTextAngle,
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

impl Default for TextColor {
    fn default() -> Self {
        Self::rgba(245, 245, 245, 255)
    }
}

impl From<&RichTextColor> for TextColor {
    fn from(value: &RichTextColor) -> Self {
        match value {
            RichTextColor::Rgb { red, green, blue } => Self::rgba(*red, *green, *blue, 255),
            RichTextColor::Named { name } => match name.as_str() {
                "red" => Self::rgba(240, 110, 110, 255),
                "green" => Self::rgba(120, 220, 150, 255),
                "blue" => Self::rgba(130, 180, 255, 255),
                "yellow" => Self::rgba(240, 220, 120, 255),
                "muted" | "quiet" => Self::rgba(170, 170, 170, 255),
                _ => Self::default(),
            },
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
            RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } => {
                self.slant = TextSlant::Italic;
            }
            RichTextStyle::Oblique { angle, .. } => {
                self.slant = TextSlant::Oblique { angle: *angle };
            }
            RichTextStyle::Strong { .. } => self.weight = TextWeight::Bold,
            RichTextStyle::Color { value } => self.color = TextColor::from(value),
            RichTextStyle::Font { family } => {
                self.font_families = vec![TextFontFamily::from(family)];
            }
            RichTextStyle::Size {
                points: Some(points),
                ..
            } => {
                self.font_size_milli = u32::from(*points) * 1_000;
                self.line_height_milli = u32::from(*points) * 1_350;
            }
            RichTextStyle::Layout { layout } => {
                self.writing_mode = layout.writing_mode;
                self.direction = layout.direction;
            }
            RichTextStyle::Size { points: None, .. }
            | RichTextStyle::Speed { .. }
            | RichTextStyle::Transform { .. }
            | RichTextStyle::Effect { .. }
            | RichTextStyle::Shader { .. }
            | RichTextStyle::Object { .. }
            | RichTextStyle::Presentation { .. }
            | RichTextStyle::Unknown { .. } => {}
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

    fn resolve_style<'a>(
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
    base_range: RichTextRange,
    source_base_range: RichTextRange,
    text: String,
    style: ResolvedTextStyle,
    presentation: RichTextPresentation,
}

impl ResolvedTextRuby {
    /// Creates a non-empty annotation with equal-length base mappings.
    pub fn new(
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
            base_range,
            source_base_range,
            text,
            style,
            presentation,
        })
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
}

/// Structured rejection raised while constructing canonical text.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextResolveError {
    #[error(transparent)]
    InvalidDisplayFrame(#[from] LineDisplayFrameValidationError),
    #[error("display stage belongs to a different line display frame")]
    StageOwnerMismatch,
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
    RubyBaseRunMissing { index: usize },
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

impl LineDisplayFrame {
    /// Borrows one stage slice and rebases its clipped metadata.
    pub fn resolve_stage_document<'a>(
        &'a self,
        stage: LineDisplayStage<'a>,
        cascade: &TextStyleCascade,
    ) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
        if !std::ptr::eq(self, stage.frame()) {
            return Err(TextResolveError::StageOwnerMismatch);
        }
        self.validate()?;
        let source_extent = stage.text_range();
        let text = stage.text();
        let runs = self
            .display_map
            .text_runs
            .iter()
            .filter_map(|run| intersect(run.range, source_extent).map(|range| (run, range)))
            .map(|(run, source_range)| {
                let style = cascade.resolve_style(run.styles.iter())?;
                ResolvedTextRun::new(
                    rebase(source_range, source_extent.start),
                    source_range,
                    style,
                    cascade.resolve_presentation(&run.presentation),
                    ResolvedTextRunSource::Dialogue {
                        node_index: run.node_index,
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ruby = self
            .display_map
            .ruby_annotations
            .iter()
            .filter(|ruby| contains(source_extent, ruby.base_range))
            .map(|ruby| {
                let style = cascade.resolve_style(ruby.styles.iter())?;
                ResolvedTextRuby::new(
                    rebase(ruby.base_range, source_extent.start),
                    ruby.base_range,
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
            TextDocumentRevision::for_source(self),
        )
    }
}

impl RichTextDocument {
    /// Resolves a static rich-text document into the canonical borrowed model.
    pub fn resolve_document<'a>(
        &'a self,
        cascade: &TextStyleCascade,
    ) -> Result<ResolvedTextDocument<'a>, TextResolveError> {
        let mut offset = 0;
        let mut active_styles = Vec::new();
        let mut runs = Vec::new();
        let mut ruby = Vec::new();
        let document_text = self.resolved_text();
        for (node_index, node) in self.nodes.iter().enumerate() {
            match node {
                RichTextNode::Text { text }
                | RichTextNode::Control {
                    control: RichTextControl::Raw { text },
                } => {
                    push_node_text(
                        text,
                        node_index,
                        document_text,
                        &active_styles,
                        cascade,
                        &mut offset,
                        &mut runs,
                    )?;
                }
                RichTextNode::Ruby { base, ruby: text } => {
                    let range = push_node_text(
                        base,
                        node_index,
                        document_text,
                        &active_styles,
                        cascade,
                        &mut offset,
                        &mut runs,
                    )?;
                    if !base.is_empty() {
                        let presentation = presentation_from_styles(active_styles.iter());
                        let style = cascade.resolve_style(active_styles.iter())?;
                        ruby.push(ResolvedTextRuby::new(
                            range,
                            range,
                            text.clone(),
                            style,
                            cascade.resolve_presentation(&presentation),
                        )?);
                    }
                }
                RichTextNode::StyleStart { style } => active_styles.push(style.clone()),
                RichTextNode::StyleEnd { name } => remove_style(&mut active_styles, name),
                RichTextNode::Control {
                    control: RichTextControl::HardBreak,
                } => {
                    push_node_text(
                        "\n",
                        node_index,
                        document_text,
                        &active_styles,
                        cascade,
                        &mut offset,
                        &mut runs,
                    )?;
                }
                RichTextNode::Interpolation { .. }
                | RichTextNode::HostEvent {
                    event: DialogueHostEvent::Conditional { .. },
                } => return Err(TextResolveError::DynamicNode { node_index }),
                RichTextNode::Control { .. } | RichTextNode::HostEvent { .. } => {}
            }
        }
        ResolvedTextDocument::new(
            self.resolved_text(),
            0,
            runs,
            ruby,
            TextDocumentRevision::for_source(self),
        )
    }
}

fn push_node_text(
    text: &str,
    node_index: usize,
    document_text: &str,
    styles: &[RichTextStyle],
    cascade: &TextStyleCascade,
    offset: &mut usize,
    runs: &mut Vec<ResolvedTextRun>,
) -> Result<RichTextRange, TextResolveError> {
    let range = RichTextRange::new(*offset, *offset + text.len());
    if document_text.get(range.start..range.end) != Some(text) {
        return Err(TextResolveError::SourceTextMismatch {
            node_index,
            start: range.start,
            end: range.end,
        });
    }
    *offset = range.end;
    if !text.is_empty() {
        let presentation = presentation_from_styles(styles.iter());
        let style = cascade.resolve_style(styles.iter())?;
        runs.push(ResolvedTextRun::new(
            range,
            range,
            style,
            cascade.resolve_presentation(&presentation),
            ResolvedTextRunSource::Generated,
        )?);
    }
    Ok(range)
}

fn remove_style(active_styles: &mut Vec<RichTextStyle>, name: &str) {
    if name == "/" {
        active_styles.pop();
        return;
    }
    let name = canonical_tag_name(name);
    if let Some(index) = active_styles
        .iter()
        .rposition(|style| style.tag_name() == name)
    {
        active_styles.remove(index);
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
        if !runs
            .iter()
            .any(|run| contains(run.range, annotation.base_range))
        {
            return Err(TextResolveError::RubyBaseRunMissing { index });
        }
    }
    Ok(())
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
