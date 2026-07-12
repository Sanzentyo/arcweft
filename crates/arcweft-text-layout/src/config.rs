//! Host configuration and structured layout failures.

use crate::{LayoutPoint, LayoutSize};
use arcweft_render_text::{RichTextRange, RichTextWritingMode};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use thiserror::Error;

/// Text layout failed before geometry could be produced.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TextLayoutError<E = std::convert::Infallible>
where
    E: StdError + 'static,
{
    /// Host constraints contain non-finite or negative geometry.
    #[error("text layout request contains invalid geometry")]
    InvalidRequestGeometry,
    /// A display-map range did not align with the resolved frame text.
    #[error("display range {range:?} is not valid for the resolved text")]
    InvalidRange {
        /// Invalid byte range.
        range: RichTextRange,
    },
    /// The selected shaping backend rejected one resolved run.
    #[error("text shaper rejected run {run_index}: {source}")]
    Shape {
        run_index: usize,
        #[source]
        source: E,
    },
    /// The selected shaping backend rejected one ruby annotation.
    #[error("text shaper rejected ruby annotation {ruby_index}: {source}")]
    ShapeRuby {
        ruby_index: usize,
        #[source]
        source: E,
    },
    /// Derived ruby font metrics violated the resolved-style contract.
    #[error("ruby annotation {ruby_index} has invalid resolved font metrics")]
    InvalidRubyStyle { ruby_index: usize },
    /// A shaper returned a source range outside its requested run.
    #[error("shaped glyph {glyph_index} in run {run_index} has invalid source range {range:?}")]
    InvalidShapedRange {
        run_index: usize,
        glyph_index: usize,
        range: RichTextRange,
    },
    /// Shaping or placement returned a non-finite or negative geometry value.
    #[error("shaped glyph {glyph_index} in run {run_index} has invalid geometry")]
    InvalidShapedGeometry {
        run_index: usize,
        glyph_index: usize,
    },
    /// A shaper returned a ruby-text range outside its annotation.
    #[error("shaped glyph {glyph_index} in ruby {ruby_index} has invalid text range {range:?}")]
    InvalidRubyShapedRange {
        ruby_index: usize,
        glyph_index: usize,
        range: RichTextRange,
    },
    /// Shaping returned non-finite or negative ruby geometry.
    #[error("shaped glyph {glyph_index} in ruby {ruby_index} has invalid geometry")]
    InvalidRubyShapedGeometry {
        ruby_index: usize,
        glyph_index: usize,
    },
}

/// Final document layout request. Font metrics come from each resolved run.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextLayoutRequest {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
    /// Horizontal line-wrapping policy. Vertical writing always follows its
    /// column constraints.
    pub horizontal_wrap: HorizontalWrap,
    pub default_writing_mode: RichTextWritingMode,
    pub jlreq_strictness: JlreqStrictness,
}

impl Default for TextLayoutRequest {
    fn default() -> Self {
        Self {
            origin: LayoutPoint::new(24.0, 24.0),
            size: LayoutSize::new(720.0, 360.0),
            horizontal_wrap: HorizontalWrap::Wrap,
            default_writing_mode: RichTextWritingMode::HorizontalTb,
            jlreq_strictness: JlreqStrictness::Normal,
        }
    }
}

/// Horizontal line wrapping selected by the resolved text container.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalWrap {
    /// Wrap shaped clusters at the container inline boundary.
    #[default]
    Wrap,
    /// Keep each hard source line on one visual line so an editable control can scroll it.
    NoWrap,
}

/// Static layout configuration supplied by the host textbox.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextLayoutConfig {
    /// Textbox-local origin.
    pub origin: LayoutPoint,
    /// Available layout size.
    pub size: LayoutSize,
    /// Base body font size.
    pub font_size: f32,
    /// Inline advance for body text.
    pub line_advance: f32,
    /// Ruby annotation font size.
    pub ruby_font_size: f32,
    /// Default writing mode when a run has no layout presentation.
    pub writing_mode: RichTextWritingMode,
    /// JLREQ punctuation pair strictness used by vertical column planning.
    pub jlreq_strictness: JlreqStrictness,
    /// Effect time used by layout-phase rich-text effects.
    pub effect_time_seconds: f32,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            origin: LayoutPoint::new(24.0, 24.0),
            size: LayoutSize::new(720.0, 360.0),
            font_size: 30.0,
            line_advance: 42.0,
            ruby_font_size: 14.0,
            writing_mode: RichTextWritingMode::HorizontalTb,
            jlreq_strictness: JlreqStrictness::Normal,
            effect_time_seconds: 0.0,
        }
    }
}

/// Strictness preset for JLREQ punctuation pair planning.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JlreqStrictness {
    /// Prefer looser breaks, while still keeping non-separable repeat marks.
    Loose,
    /// Balanced default for narrative text.
    #[default]
    Normal,
    /// Prefer stricter Japanese composition around weak punctuation pairs.
    Strict,
}
