//! Scoped text contexts shared by all elements of one admitted observation.

use crate::rich_text::{
    AgentGlyphOrientation, AgentGlyphVerticalForm, AgentHitRegion, AgentHitRegionKind,
    AgentRichTextElementKind, AgentRichTextElementRef,
};
use arcweft_text_layout::TextLayout;
use arcweft_text_model::{
    DialoguePreparedTextRole, PreparedDisplayStageContext, PreparedTextDocumentContext,
    PreparedTextOwnerKind, ProjectedTextRangeDomain, RichTextNodeIndex, RichTextRange,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use thiserror::Error;

#[cfg(test)]
mod tests;

pub const AGENT_TEXT_CONTEXT_CATALOG_VERSION: u8 = 1;
pub const MAX_AGENT_TEXT_CONTEXTS: usize = 4096;
pub const MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_AGENT_TEXT_CONTEXT_ELEMENTS: usize = 262_144;
/// Complete compact canonical JSON envelope, including escaped strings and
/// metadata. Pretty-printing and other serde formats are not this wire codec.
pub const MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES: usize = 64 * 1024 * 1024;

/// Frame/sample integrity; matching this identity does not admit prepared data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTextContextScope {
    pub session_id: String,
    pub frame_id: String,
    pub tick: usize,
    pub capture_time_millis: Option<u32>,
}

/// References are issued only by the immutable catalog factory.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTextContextReference {
    scope: Arc<AgentTextContextScope>,
    owner: u32,
}

impl AgentTextContextReference {
    #[must_use]
    pub fn scope(&self) -> &AgentTextContextScope {
        &self.scope
    }
    #[must_use]
    pub const fn owner(&self) -> u32 {
        self.owner
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTextContextOwner {
    pub root_id: String,
    pub parent_id: Option<String>,
    pub semantic_id: String,
    pub prepared_text: u32,
    pub kind: PreparedTextOwnerKind,
}

/// Stage rows retain the actual admitted context. Ordinary documents retain
/// their text and shaped layout once, including Ruby text and source metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentTextContextContent {
    DisplayStage {
        context: Arc<PreparedDisplayStageContext>,
        layout: Arc<TextLayout>,
    },
    PreparedDocument {
        context: PreparedTextDocumentContext,
        text: String,
        layout: Arc<TextLayout>,
    },
}

impl AgentTextContextContent {
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::DisplayStage { context, .. } => &context.frame().text,
            Self::PreparedDocument { text, .. } => text,
        }
    }
    #[must_use]
    pub fn ranges(&self) -> ProjectedTextRangeDomain {
        match self {
            Self::DisplayStage { context, .. } => context.ranges(),
            Self::PreparedDocument { context, .. } => context.ranges(),
        }
    }
    #[must_use]
    pub fn display_stage(&self) -> Option<&Arc<PreparedDisplayStageContext>> {
        match self {
            Self::DisplayStage { context, .. } => Some(context),
            Self::PreparedDocument { .. } => None,
        }
    }

    #[must_use]
    pub fn prepared_text(&self) -> &str {
        match self {
            Self::DisplayStage { context, .. } => context.prepared_text(),
            Self::PreparedDocument { text, .. } => text,
        }
    }

    #[must_use]
    pub fn layout(&self) -> &Arc<TextLayout> {
        match self {
            Self::DisplayStage { layout, .. } | Self::PreparedDocument { layout, .. } => layout,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentTextContextEntry {
    pub owner_ordinal: u32,
    pub owner: AgentTextContextOwner,
    pub content: AgentTextContextContent,
}

/// Borrowed body-glyph coverage issued by the owning checked context.
/// Runs/pages retain source containment; Ruby borrows its actual layout-owned
/// allocated body intervals, and a glyph/cluster retains its exact shaped ordinal.
#[derive(Debug)]
pub struct AgentTextBodyCapture<'context> {
    layout: &'context TextLayout,
    selector: BodyCaptureSelector<'context>,
}

#[derive(Clone, Copy, Debug)]
enum BodyCaptureSelector<'context> {
    Glyph(usize),
    Cluster(usize),
    Line(usize),
    SourceRange(RichTextRange),
    Ruby(&'context [arcweft_text_layout::TextLayoutBodyAllocation]),
}

impl AgentTextBodyCapture<'_> {
    #[must_use]
    pub fn contains_glyph(&self, glyph_index: u32) -> bool {
        let Ok(index) = usize::try_from(glyph_index) else {
            return false;
        };
        let Some(glyph) = self.layout.glyphs.get(index) else {
            return false;
        };
        match self.selector {
            BodyCaptureSelector::Glyph(selected) => index == selected,
            BodyCaptureSelector::Cluster(selected) => {
                usize::try_from(glyph.cluster_index).ok() == Some(selected)
            }
            BodyCaptureSelector::Line(selected) => {
                usize::try_from(glyph.line_index).ok() == Some(selected)
            }
            BodyCaptureSelector::SourceRange(range) => {
                range.start <= glyph.source_range.start && glyph.source_range.end <= range.end
            }
            BodyCaptureSelector::Ruby(allocations) => {
                let selected = allocations
                    .partition_point(|allocation| allocation.glyph_range.end <= glyph_index);
                allocations
                    .get(selected)
                    .is_some_and(|allocation| allocation.glyph_range.contains(&glyph_index))
            }
        }
    }

    /// A cluster attachment represents its actual body members, including a
    /// cluster with several raster glyphs. Equal byte ranges do not alias it.
    #[must_use]
    pub fn contains_cluster(&self, cluster_index: usize) -> bool {
        match self.selector {
            BodyCaptureSelector::Glyph(index) => {
                self.layout.glyphs.get(index).is_some_and(|glyph| {
                    usize::try_from(glyph.cluster_index).ok() == Some(cluster_index)
                })
            }
            BodyCaptureSelector::Cluster(selected) => selected == cluster_index,
            BodyCaptureSelector::Line(line) => u32::try_from(cluster_index)
                .ok()
                .and_then(|index| self.layout.cluster_glyphs(index))
                .is_some_and(|glyphs| {
                    glyphs
                        .iter()
                        .any(|glyph| usize::try_from(glyph.line_index).ok() == Some(line))
                }),
            BodyCaptureSelector::SourceRange(range) => u32::try_from(cluster_index)
                .ok()
                .and_then(|index| self.layout.cluster_glyphs(index))
                .is_some_and(|glyphs| {
                    glyphs.iter().any(|glyph| {
                        range.start <= glyph.source_range.start
                            && glyph.source_range.end <= range.end
                    })
                }),
            BodyCaptureSelector::Ruby(_) => {
                u32::try_from(cluster_index).ok().is_some_and(|cluster| {
                    let first = self
                        .layout
                        .glyphs
                        .partition_point(|glyph| glyph.cluster_index < cluster);
                    self.layout
                        .glyphs
                        .get(first)
                        .is_some_and(|glyph| glyph.cluster_index == cluster)
                        && u32::try_from(first).is_ok_and(|index| self.contains_glyph(index))
                })
            }
        }
    }
}

impl AgentTextContextEntry {
    /// Resolve body coverage from the exact admitted element and shaped layout.
    /// The result borrows this owner and cannot be detached or independently
    /// manufactured from source ranges.
    pub fn capture_body(
        &self,
        element: &AgentRichTextElementRef,
    ) -> Result<AgentTextBodyCapture<'_>, AgentTextContextError> {
        self.validate_element_shape(element)?;
        let selector = match element.kind {
            AgentRichTextElementKind::TextGlyph => BodyCaptureSelector::Glyph(element.index),
            AgentRichTextElementKind::GlyphCluster => BodyCaptureSelector::Cluster(element.index),
            AgentRichTextElementKind::TextLine => BodyCaptureSelector::Line(element.index),
            AgentRichTextElementKind::Ruby => {
                let range = self
                    .content
                    .ranges()
                    .display_to_source(element.range)
                    .map_err(|_| self.range_error())?;
                let ruby = self
                    .content
                    .layout()
                    .ruby
                    .iter()
                    .find(|ruby| {
                        ruby.owner_node == element.node_index
                            && ruby.base_range == range
                            && element.ruby.as_ref() == Some(&ruby.text)
                    })
                    .ok_or_else(|| self.range_error())?;
                BodyCaptureSelector::Ruby(&ruby.base_allocations)
            }
            AgentRichTextElementKind::TextPage
            | AgentRichTextElementKind::TextRun
            | AgentRichTextElementKind::TextObjectProxy => BodyCaptureSelector::SourceRange(
                self.content
                    .ranges()
                    .display_to_source(element.range)
                    .map_err(|_| self.range_error())?,
            ),
        };
        Ok(AgentTextBodyCapture {
            layout: self.content.layout(),
            selector,
        })
    }
    /// Deterministic descriptor issued from the structured element and its
    /// exact context. Consumers resolve the reference, never parse this string.
    pub fn element_id(
        &self,
        element: &AgentRichTextElementRef,
    ) -> Result<String, AgentTextContextError> {
        self.validate_element_shape(element)?;
        let root = &self.owner.root_id;
        let index = element.index;
        Ok(match element.kind {
            AgentRichTextElementKind::TextPage => format!("{root}.page.{index}"),
            AgentRichTextElementKind::TextLine => format!("{root}.line.{index}"),
            AgentRichTextElementKind::TextRun => format!("{root}.run.{index}"),
            AgentRichTextElementKind::Ruby => format!("{root}.ruby.{index}"),
            AgentRichTextElementKind::TextGlyph => format!(
                "{root}.glyph.{index}.{}.{}",
                element.range.start, element.range.end
            ),
            AgentRichTextElementKind::GlyphCluster => format!(
                "{root}.cluster.{index}.{}.{}",
                element.range.start, element.range.end
            ),
            AgentRichTextElementKind::TextObjectProxy => {
                format!("{root}.proxy.{}.{index}", self.proxy_run_index(element)?)
            }
        })
    }

    pub fn element_parent_id(
        &self,
        element: Option<&AgentRichTextElementRef>,
    ) -> Result<Option<String>, AgentTextContextError> {
        let Some(element) = element else {
            return Ok(self.owner.parent_id.clone());
        };
        self.validate_element_shape(element)?;
        let root = &self.owner.root_id;
        let stage = self.content.display_stage();
        let page = stage.map_or_else(
            || root.clone(),
            |stage| format!("{root}.page.{}", stage.frame().page_index),
        );
        Ok(Some(match element.kind {
            AgentRichTextElementKind::TextPage => root.clone(),
            AgentRichTextElementKind::TextLine => page,
            AgentRichTextElementKind::TextRun | AgentRichTextElementKind::Ruby => {
                if stage.is_some() {
                    self.content
                        .layout()
                        .lines
                        .iter()
                        .position(|line| {
                            self.content
                                .ranges()
                                .source_to_display(line.source_range)
                                .is_ok_and(|range| {
                                    range.start <= element.range.start
                                        && element.range.end <= range.end
                                })
                        })
                        .map_or(page, |line| format!("{root}.line.{line}"))
                } else {
                    root.clone()
                }
            }
            AgentRichTextElementKind::TextObjectProxy => {
                format!("{root}.run.{}", self.proxy_run_index(element)?)
            }
            AgentRichTextElementKind::TextGlyph | AgentRichTextElementKind::GlyphCluster => {
                format!("{root}.run.{}", self.run_index_for_range(element.range)?)
            }
        }))
    }

    fn range_error(&self) -> AgentTextContextError {
        AgentTextContextError::ObjectRange {
            object_id: self.owner.root_id.clone(),
        }
    }

    fn prepared_display_range(&self) -> RichTextRange {
        self.content
            .ranges()
            .prepared_to_display(self.content.ranges().prepared_range())
            .expect("admitted context owns its prepared extent")
    }

    fn clipped_run_range(&self, index: usize) -> Result<RichTextRange, AgentTextContextError> {
        if let Some(stage) = self.content.display_stage() {
            let run = stage
                .frame()
                .display_map
                .text_runs
                .get(index)
                .ok_or_else(|| self.range_error())?;
            let prepared = self.prepared_display_range();
            let clipped = RichTextRange::new(
                run.range.start.max(prepared.start),
                run.range.end.min(prepared.end),
            );
            if clipped.start >= clipped.end {
                return Err(self.range_error());
            }
            Ok(clipped)
        } else {
            let run = self
                .content
                .layout()
                .runs
                .get(index)
                .filter(|run| usize::try_from(run.run_index).ok() == Some(index))
                .ok_or_else(|| self.range_error())?;
            self.content
                .ranges()
                .source_to_display(run.source_range)
                .map_err(|_| self.range_error())
        }
    }

    fn run_index_for_range(&self, range: RichTextRange) -> Result<usize, AgentTextContextError> {
        if let Some(stage) = self.content.display_stage() {
            let runs = &stage.frame().display_map.text_runs;
            let index = runs.partition_point(|run| run.range.end <= range.start);
            runs.get(index)
                .filter(|run| run.range.start <= range.start && range.end <= run.range.end)
                .map(|_| index)
                .ok_or_else(|| self.range_error())
        } else {
            let source = self
                .content
                .ranges()
                .display_to_source(range)
                .map_err(|_| self.range_error())?;
            let runs = &self.content.layout().runs;
            let index = runs.partition_point(|run| run.source_range.end <= source.start);
            runs.get(index)
                .filter(|run| {
                    run.source_range.start <= source.start && source.end <= run.source_range.end
                })
                .map(|_| index)
                .ok_or_else(|| self.range_error())
        }
    }

    fn proxy_run_index(
        &self,
        element: &AgentRichTextElementRef,
    ) -> Result<usize, AgentTextContextError> {
        let index = self.run_index_for_range(element.range)?;
        if self.clipped_run_range(index)? != element.range {
            return Err(self.range_error());
        }
        let presentation = if let Some(stage) = self.content.display_stage() {
            &stage.frame().display_map.text_runs[index].presentation
        } else {
            &self.content.layout().runs[index].presentation
        };
        let proxy = presentation
            .object_proxies
            .get(element.index)
            .ok_or_else(|| self.range_error())?;
        let mut narrowed = presentation.clone();
        narrowed.object_proxies = vec![proxy.clone()];
        if element.presentation.as_ref() != Some(&narrowed) || element.hit_test != proxy.hit_test {
            return Err(self.range_error());
        }
        Ok(index)
    }

    fn validate_element_shape(
        &self,
        element: &AgentRichTextElementRef,
    ) -> Result<(), AgentTextContextError> {
        let domain = self.content.ranges();
        domain
            .validate_display_range(element.range, self.content.text())
            .map_err(|_| self.range_error())?;
        let prepared = self.prepared_display_range();
        if element.range.start < prepared.start || element.range.end > prepared.end {
            return Err(self.range_error());
        }
        let layout = self.content.layout();
        let expected_page = self
            .content
            .display_stage()
            .map_or(0, |stage| stage.frame().page_index);
        if element.page != expected_page {
            return Err(self.range_error());
        }
        let range = match element.kind {
            AgentRichTextElementKind::TextPage => {
                if self.content.display_stage().is_none() || element.index != expected_page {
                    return Err(self.range_error());
                }
                prepared
            }
            AgentRichTextElementKind::TextLine => domain
                .source_to_display(
                    layout
                        .lines
                        .get(element.index)
                        .ok_or_else(|| self.range_error())?
                        .source_range,
                )
                .map_err(|_| self.range_error())?,
            AgentRichTextElementKind::TextRun => self.clipped_run_range(element.index)?,
            AgentRichTextElementKind::TextGlyph => domain
                .source_to_display(
                    layout
                        .glyphs
                        .get(element.index)
                        .ok_or_else(|| self.range_error())?
                        .source_range,
                )
                .map_err(|_| self.range_error())?,
            AgentRichTextElementKind::GlyphCluster => {
                let mut glyphs = layout
                    .cluster_glyphs(u32::try_from(element.index).map_err(|_| self.range_error())?)
                    .ok_or_else(|| self.range_error())?
                    .iter();
                let first = glyphs.next().ok_or_else(|| self.range_error())?;
                let range = glyphs.fold(first.source_range, |range, glyph| {
                    RichTextRange::new(
                        range.start.min(glyph.source_range.start),
                        range.end.max(glyph.source_range.end),
                    )
                });
                domain
                    .source_to_display(range)
                    .map_err(|_| self.range_error())?
            }
            AgentRichTextElementKind::Ruby => {
                let (range, text, node, presentation) = if let Some(stage) =
                    self.content.display_stage()
                {
                    let ruby = stage
                        .frame()
                        .display_map
                        .ruby_annotations
                        .get(element.index)
                        .ok_or_else(|| self.range_error())?;
                    (
                        ruby.base_range,
                        ruby.ruby.as_str(),
                        ruby.owner_node,
                        &ruby.presentation,
                    )
                } else {
                    let ruby = layout
                        .ruby
                        .iter()
                        .find(|ruby| usize::try_from(ruby.ruby_index).ok() == Some(element.index))
                        .ok_or_else(|| self.range_error())?;
                    (
                        domain
                            .source_to_display(ruby.base_range)
                            .map_err(|_| self.range_error())?,
                        ruby.text.as_str(),
                        ruby.owner_node,
                        &ruby.presentation,
                    )
                };
                if element.ruby.as_deref() != Some(text)
                    || element.node_index != node
                    || element.presentation.as_ref() != Some(presentation)
                {
                    return Err(self.range_error());
                }
                range
            }
            AgentRichTextElementKind::TextObjectProxy => {
                self.clipped_run_range(self.proxy_run_index(element)?)?
            }
        };
        if range != element.range {
            return Err(self.range_error());
        }
        if !matches!(element.kind, AgentRichTextElementKind::Ruby) && element.ruby.is_some() {
            return Err(self.range_error());
        }
        if matches!(
            element.kind,
            AgentRichTextElementKind::TextRun
                | AgentRichTextElementKind::TextGlyph
                | AgentRichTextElementKind::GlyphCluster
                | AgentRichTextElementKind::TextObjectProxy
        ) {
            let run_index = if element.kind == AgentRichTextElementKind::TextRun {
                element.index
            } else {
                self.run_index_for_range(element.range)?
            };
            let (node, source, presentation) = if let Some(stage) = self.content.display_stage() {
                let run = &stage.frame().display_map.text_runs[run_index];
                (run.node_index, Some(run.source), &run.presentation)
            } else {
                (
                    RichTextNodeIndex::new(0),
                    None,
                    &layout.runs[run_index].presentation,
                )
            };
            if element.node_index != node
                || element.source != source
                || (element.kind != AgentRichTextElementKind::TextObjectProxy
                    && element.presentation.as_ref() != Some(presentation))
            {
                return Err(self.range_error());
            }
        }
        if matches!(
            element.kind,
            AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
        ) {
            let (node, presentation) = if let Some(stage) = self.content.display_stage() {
                let runs = &stage.frame().display_map.text_runs;
                let node = runs
                    .iter()
                    .find(|run| overlap(run.range, element.range))
                    .map_or(RichTextNodeIndex::new(0), |run| run.node_index);
                let presentation = runs
                    .iter()
                    .filter(|run| overlap(run.range, element.range))
                    .map(|run| run.presentation.clone())
                    .reduce(|mut accumulated, next| {
                        accumulated.merge(next);
                        accumulated
                    });
                (node, presentation)
            } else {
                (RichTextNodeIndex::new(0), None)
            };
            if element.node_index != node
                || element.source.is_some()
                || element.presentation != presentation
            {
                return Err(self.range_error());
            }
        }
        self.validate_element_paint_metadata(element)
    }

    fn validate_element_paint_metadata(
        &self,
        element: &AgentRichTextElementRef,
    ) -> Result<(), AgentTextContextError> {
        let glyph = match element.kind {
            AgentRichTextElementKind::TextGlyph => self.content.layout().glyphs.get(element.index),
            AgentRichTextElementKind::GlyphCluster => self
                .content
                .layout()
                .cluster_glyphs(u32::try_from(element.index).map_err(|_| self.range_error())?)
                .and_then(|glyphs| glyphs.first()),
            _ => None,
        };
        let orientation = glyph.map(|glyph| match glyph.orientation {
            arcweft_text_layout::GlyphOrientation::Upright => AgentGlyphOrientation::Upright,
            arcweft_text_layout::GlyphOrientation::SidewaysCw => AgentGlyphOrientation::SidewaysCw,
            arcweft_text_layout::GlyphOrientation::TextCombineUpright => {
                AgentGlyphOrientation::TextCombineUpright
            }
        });
        let vertical_form = glyph.map(|glyph| match glyph.vertical_form {
            arcweft_text_layout::GlyphVerticalForm::None => AgentGlyphVerticalForm::None,
            arcweft_text_layout::GlyphVerticalForm::UprightAlternate => {
                AgentGlyphVerticalForm::UprightAlternate
            }
            arcweft_text_layout::GlyphVerticalForm::RotatedAlternate => {
                AgentGlyphVerticalForm::RotatedAlternate
            }
        });
        if element.orientation != orientation || element.vertical_form != vertical_form {
            return Err(self.range_error());
        }
        if element.kind != AgentRichTextElementKind::Ruby
            && (element.ruby_base_bbox.is_some() || element.ruby_annotation_bbox.is_some())
        {
            return Err(self.range_error());
        }
        for region in &element.hit_regions {
            self.validate_hit_region(element, region)?;
        }
        Ok(())
    }

    fn validate_hit_region(
        &self,
        element: &AgentRichTextElementRef,
        region: &AgentHitRegion,
    ) -> Result<(), AgentTextContextError> {
        if region.range.start < element.range.start || region.range.end > element.range.end {
            return Err(self.range_error());
        }
        self.content
            .ranges()
            .validate_display_range(region.range, self.content.text())
            .map_err(|_| self.range_error())?;
        if region.kind == AgentHitRegionKind::TextObjectProxy {
            let presentation = if matches!(
                element.kind,
                AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
            ) {
                let index = self.run_index_for_range(region.range)?;
                if let Some(stage) = self.content.display_stage() {
                    &stage.frame().display_map.text_runs[index].presentation
                } else {
                    &self.content.layout().runs[index].presentation
                }
            } else {
                element
                    .presentation
                    .as_ref()
                    .ok_or_else(|| self.range_error())?
            };
            if !presentation.object_proxies.iter().any(|proxy| {
                proxy.hit_test
                    && region.proxy_id.as_ref() == Some(&proxy.id)
                    && region.proxy_type == proxy.type_name
                    && region.proxy_declaration == proxy.declaration
                    && region.proxy_schema == proxy.schema
                    && region.proxy_fields == proxy.fields
                    && region.proxy_role == proxy.role
                    && region.proxy_layer.as_ref()
                        == proxy.layer.as_ref().or(presentation.layer.as_ref())
                    && region.depth == proxy.depth.map(|depth| depth.0)
                    && region.proxy_params.is_empty()
            }) {
                return Err(self.range_error());
            }
            return Ok(());
        }
        let expected = match element.kind {
            AgentRichTextElementKind::TextPage => AgentHitRegionKind::TextPage,
            AgentRichTextElementKind::TextLine => AgentHitRegionKind::TextLine,
            AgentRichTextElementKind::TextRun => AgentHitRegionKind::TextRun,
            AgentRichTextElementKind::TextGlyph => AgentHitRegionKind::TextGlyph,
            AgentRichTextElementKind::GlyphCluster => AgentHitRegionKind::GlyphCluster,
            AgentRichTextElementKind::TextObjectProxy => return Err(self.range_error()),
            AgentRichTextElementKind::Ruby => match region.kind {
                AgentHitRegionKind::RubyObject => AgentHitRegionKind::RubyObject,
                AgentHitRegionKind::RubyBase
                    if element.ruby_base_bbox.as_ref() == Some(&region.bbox) =>
                {
                    AgentHitRegionKind::RubyBase
                }
                AgentHitRegionKind::RubyAnnotation
                    if element.ruby_annotation_bbox.as_ref() == Some(&region.bbox) =>
                {
                    AgentHitRegionKind::RubyAnnotation
                }
                _ => return Err(self.range_error()),
            },
        };
        if region.kind != expected
            || region.range != element.range
            || region.proxy_id.is_some()
            || region.proxy_type.is_some()
            || region.proxy_declaration.is_some()
            || region.proxy_schema.is_some()
            || !region.proxy_fields.is_empty()
            || region.proxy_role.is_some()
            || region.proxy_layer.is_some()
            || region.depth.is_some()
            || !region.proxy_params.is_empty()
        {
            return Err(self.range_error());
        }
        Ok(())
    }
}

fn overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

/// Scoped element coverage issued after validating the root once. A collector
/// can stream candidates without revalidating or copying the root's full
/// multi-glyph cluster on every comparison.
#[derive(Debug)]
pub struct AgentTextElementCapture<'capture> {
    catalog: &'capture AgentTextContextCatalog,
    reference: &'capture AgentTextContextReference,
    body: AgentTextBodyCapture<'capture>,
}

impl AgentTextElementCapture<'_> {
    /// Check an actual final body-cluster attachment against this issued root.
    pub fn contains(
        &self,
        candidate: &crate::object::AgentObservedObject,
    ) -> Result<bool, AgentTextContextError> {
        candidate.validate_text_context(self.catalog)?;
        let Some(element) = &candidate.rich_text_ref else {
            return Ok(false);
        };
        Ok(candidate.text_context.as_ref() == Some(self.reference)
            && element.kind == AgentRichTextElementKind::GlyphCluster
            && self.body.contains_cluster(element.index))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentTextContextCatalog {
    scope: Arc<AgentTextContextScope>,
    entries: Arc<BTreeMap<u32, Arc<AgentTextContextEntry>>>,
}

#[derive(Debug, Error)]
pub enum AgentTextContextError {
    #[error(transparent)]
    ImageFormat(#[from] crate::image::AgentImageFormatError),
    #[error("text context scope has an empty session/frame identity")]
    InvalidScope,
    #[error("unsupported text context catalog version {actual}")]
    Version { actual: u8 },
    #[error("text context reference belongs to another session/frame/sample")]
    ScopeMismatch,
    #[error("text context prepared owner {owner} is missing")]
    Missing { owner: u32 },
    #[error("text context repeats prepared owner ordinal {owner}")]
    Duplicate { owner: u32 },
    #[error("text context repeats root object {root_id}")]
    DuplicateRoot { root_id: String },
    #[error("text context repeats prepared text item {text}")]
    DuplicatePreparedText { text: u32 },
    #[error("text context owner {owner} has a mismatched content family")]
    ContentFamily { owner: u32 },
    #[error("text context owner {owner} has an empty root/semantic identity")]
    InvalidOwner { owner: u32 },
    #[error("text context exceeds {what} limit {limit}")]
    Limit { what: &'static str, limit: usize },
    #[error("text object {object_id} has a missing context")]
    MissingObjectContext { object_id: String },
    #[error("text object {object_id} has a mismatched context owner")]
    ObjectOwner { object_id: String },
    #[error("text object {object_id} has an invalid element range or text")]
    ObjectRange { object_id: String },
    #[error("text context owner {owner} has an invalid prepared document: {reason}")]
    PreparedDocument { owner: u32, reason: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Default)]
struct CatalogBudget {
    text_bytes: usize,
    elements: usize,
}

impl CatalogBudget {
    fn observe(&mut self, entry: &AgentTextContextEntry) -> Result<(), AgentTextContextError> {
        let (extra_text, stage_elements) = match &entry.content {
            AgentTextContextContent::DisplayStage { context, .. } => {
                let frame = context.frame();
                let extra = frame
                    .display_map
                    .ruby_annotations
                    .iter()
                    .try_fold(0usize, |count, ruby| count.checked_add(ruby.ruby.len()))
                    .ok_or(AgentTextContextError::Limit {
                        what: "text bytes",
                        limit: MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES,
                    })?;
                let elements = [
                    frame.display_map.text_runs.len(),
                    frame.display_map.ruby_annotations.len(),
                    frame.display_map.controls.len(),
                    frame.display_map.host_events.len(),
                    frame.style_contributions.len(),
                    frame.inline_failures.len(),
                    frame.unresolved.len(),
                ]
                .into_iter()
                .try_fold(0usize, usize::checked_add)
                .ok_or(AgentTextContextError::Limit {
                    what: "elements",
                    limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS,
                })?;
                (extra, elements)
            }
            AgentTextContextContent::PreparedDocument { .. } => (0, 0),
        };
        let layout = entry.content.layout();
        let layout_text = layout
            .ruby
            .iter()
            .try_fold(0usize, |count, ruby| count.checked_add(ruby.text.len()))
            .ok_or(AgentTextContextError::Limit {
                what: "text bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES,
            })?;
        let elements = [
            stage_elements,
            layout.lines.len(),
            layout.runs.len(),
            layout.glyphs.len(),
            layout.ruby.len(),
            layout.source_map.glyphs().len(),
        ]
        .into_iter()
        .chain(
            layout
                .ruby
                .iter()
                .flat_map(|ruby| [ruby.glyphs.len(), ruby.base_allocations.len()]),
        )
        .try_fold(0usize, usize::checked_add)
        .ok_or(AgentTextContextError::Limit {
            what: "elements",
            limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS,
        })?;
        self.text_bytes = self
            .text_bytes
            .checked_add(entry.content.text().len())
            .and_then(|count| count.checked_add(extra_text))
            .and_then(|count| count.checked_add(layout_text))
            .ok_or(AgentTextContextError::Limit {
                what: "text bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES,
            })?;
        self.elements =
            self.elements
                .checked_add(elements)
                .ok_or(AgentTextContextError::Limit {
                    what: "elements",
                    limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS,
                })?;
        if self.text_bytes > MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES {
            return Err(AgentTextContextError::Limit {
                what: "text bytes",
                limit: MAX_AGENT_TEXT_CONTEXT_TEXT_BYTES,
            });
        }
        if self.elements > MAX_AGENT_TEXT_CONTEXT_ELEMENTS {
            return Err(AgentTextContextError::Limit {
                what: "elements",
                limit: MAX_AGENT_TEXT_CONTEXT_ELEMENTS,
            });
        }
        Ok(())
    }
}

impl AgentTextContextCatalog {
    pub fn try_new(
        scope: AgentTextContextScope,
        entries: impl IntoIterator<Item = AgentTextContextEntry>,
    ) -> Result<Self, AgentTextContextError> {
        if scope.session_id.is_empty() || scope.frame_id.is_empty() {
            return Err(AgentTextContextError::InvalidScope);
        }
        let mut rows = BTreeMap::new();
        let mut roots = BTreeSet::new();
        let mut texts = BTreeSet::new();
        let mut budget = CatalogBudget::default();
        for entry in entries {
            if rows.len() >= MAX_AGENT_TEXT_CONTEXTS
                || usize::try_from(entry.owner_ordinal)
                    .map_or(true, |n| n >= MAX_AGENT_TEXT_CONTEXTS)
            {
                return Err(AgentTextContextError::Limit {
                    what: "context count",
                    limit: MAX_AGENT_TEXT_CONTEXTS,
                });
            }
            budget.observe(&entry)?;
            validate_entry(&entry)?;
            if !texts.insert(entry.owner.prepared_text) {
                return Err(AgentTextContextError::DuplicatePreparedText {
                    text: entry.owner.prepared_text,
                });
            }
            if !roots.insert(entry.owner.root_id.clone()) {
                return Err(AgentTextContextError::DuplicateRoot {
                    root_id: entry.owner.root_id,
                });
            }
            let owner = entry.owner_ordinal;
            if rows.insert(owner, Arc::new(entry)).is_some() {
                return Err(AgentTextContextError::Duplicate { owner });
            }
        }
        let catalog = Self {
            scope: Arc::new(scope),
            entries: Arc::new(rows),
        };
        validate_text_context_envelope(|writer| {
            serde_json::to_writer(writer, &AgentTextContextCatalogWire(&catalog))
        })?;
        Ok(catalog)
    }

    pub fn empty(scope: AgentTextContextScope) -> Result<Self, AgentTextContextError> {
        Self::try_new(scope, std::iter::empty())
    }
    #[must_use]
    pub fn scope(&self) -> &AgentTextContextScope {
        &self.scope
    }
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &AgentTextContextEntry> {
        self.entries.values().map(Arc::as_ref)
    }
    pub fn reference(
        &self,
        owner: u32,
    ) -> Result<AgentTextContextReference, AgentTextContextError> {
        if !self.entries.contains_key(&owner) {
            return Err(AgentTextContextError::Missing { owner });
        }
        Ok(AgentTextContextReference {
            scope: Arc::clone(&self.scope),
            owner,
        })
    }
    pub fn resolve(
        &self,
        reference: &AgentTextContextReference,
    ) -> Result<&AgentTextContextEntry, AgentTextContextError> {
        if reference.scope.as_ref() != self.scope.as_ref() {
            return Err(AgentTextContextError::ScopeMismatch);
        }
        self.entries
            .get(&reference.owner)
            .map(Arc::as_ref)
            .ok_or(AgentTextContextError::Missing {
                owner: reference.owner,
            })
    }
    pub fn selected<'a>(
        &self,
        references: impl IntoIterator<Item = &'a AgentTextContextReference>,
    ) -> Result<Self, AgentTextContextError> {
        let mut entries = BTreeMap::new();
        for reference in references {
            self.resolve(reference)?;
            let entry =
                self.entries
                    .get(&reference.owner)
                    .ok_or(AgentTextContextError::Missing {
                        owner: reference.owner,
                    })?;
            entries.insert(reference.owner, Arc::clone(entry));
        }
        Ok(Self {
            scope: Arc::clone(&self.scope),
            entries: Arc::new(entries),
        })
    }
    pub fn decode_json(bytes: &[u8]) -> Result<Self, AgentTextContextError> {
        decode_text_context_envelope(bytes)
    }

    /// Issue exact body-cluster contributors for a checked text element.
    /// Equal source ranges and ordinals in another owner cannot alias it.
    pub fn element_capture<'capture>(
        &'capture self,
        root: &'capture crate::object::AgentObservedObject,
    ) -> Result<AgentTextElementCapture<'capture>, AgentTextContextError> {
        root.validate_text_context(self)?;
        let reference = root.text_context.as_ref().ok_or_else(|| {
            AgentTextContextError::MissingObjectContext {
                object_id: root.id.clone(),
            }
        })?;
        let element =
            root.rich_text_ref
                .as_ref()
                .ok_or_else(|| AgentTextContextError::ObjectOwner {
                    object_id: root.id.clone(),
                })?;
        Ok(AgentTextElementCapture {
            catalog: self,
            reference,
            body: self.resolve(reference)?.capture_body(element)?,
        })
    }

    /// Admit one complete object list under this exact context catalog.
    /// Different issued IDs may share source text or one context; the same
    /// object identity may occur only once in an Objects/Observation list.
    pub(crate) fn validate_objects(
        &self,
        objects: &[crate::object::AgentObservedObject],
    ) -> Result<(), AgentTextContextError> {
        for object in objects {
            object.validate_text_context(self)?;
        }
        let mut ids = BTreeSet::new();
        for object in objects {
            if !ids.insert(object.id.as_str()) {
                return Err(AgentTextContextError::ObjectOwner {
                    object_id: object.id.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn validate_element(
        &self,
        object_id: &str,
        reference: &AgentTextContextReference,
        element: Option<&crate::rich_text::AgentRichTextElementRef>,
        text: Option<&str>,
    ) -> Result<(), AgentTextContextError> {
        let entry = self.resolve(reference)?;
        let Some(element) = element else {
            if object_id != entry.owner.root_id {
                return Err(AgentTextContextError::ObjectOwner {
                    object_id: object_id.to_owned(),
                });
            }
            if text != Some(entry.content.text()) {
                return Err(AgentTextContextError::ObjectRange {
                    object_id: object_id.to_owned(),
                });
            }
            return Ok(());
        };
        if object_id != entry.element_id(element)? {
            return Err(AgentTextContextError::ObjectOwner {
                object_id: object_id.to_owned(),
            });
        }
        let ranges = entry.content.ranges();
        ranges
            .validate_display_range(element.range, entry.content.text())
            .map_err(|_| AgentTextContextError::ObjectRange {
                object_id: object_id.to_owned(),
            })?;
        if element.kind == crate::rich_text::AgentRichTextElementKind::Ruby {
            let annotation = match &entry.content {
                AgentTextContextContent::DisplayStage { context, .. } => context
                    .frame()
                    .display_map
                    .ruby_annotations
                    .get(element.index)
                    .map(|ruby| (ruby.base_range, ruby.ruby.as_str())),
                AgentTextContextContent::PreparedDocument { layout, .. } => layout
                    .ruby
                    .iter()
                    .find(|ruby| usize::try_from(ruby.ruby_index).ok() == Some(element.index))
                    .and_then(|ruby| {
                        ranges
                            .source_to_display(ruby.base_range)
                            .ok()
                            .map(|range| (range, ruby.text.as_str()))
                    }),
            };
            if annotation != element.ruby.as_deref().map(|ruby| (element.range, ruby))
                || text
                    != element
                        .ruby
                        .as_deref()
                        .and_then(|ruby| {
                            entry
                                .content
                                .text()
                                .get(element.range.start..element.range.end)
                                .map(|base| format!("{base} ({ruby})"))
                        })
                        .as_deref()
            {
                return Err(AgentTextContextError::ObjectRange {
                    object_id: object_id.to_owned(),
                });
            }
        } else if text
            != entry
                .content
                .text()
                .get(element.range.start..element.range.end)
        {
            return Err(AgentTextContextError::ObjectRange {
                object_id: object_id.to_owned(),
            });
        }
        for region in &element.hit_regions {
            ranges
                .validate_display_range(region.range, entry.content.text())
                .map_err(|_| AgentTextContextError::ObjectRange {
                    object_id: object_id.to_owned(),
                })?;
        }
        Ok(())
    }
}

/// Counts bytes emitted by the owning compact JSON grammar without buffering
/// them. A failed write remains failed, including after an encoder error.
#[derive(Default)]
pub(crate) struct TextContextWireBudget {
    written: usize,
    exceeded: bool,
}

impl std::io::Write for TextContextWireBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match self.written.checked_add(bytes.len()) {
            Some(written) if !self.exceeded && written <= MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES => {
                self.written = written;
                Ok(bytes.len())
            }
            _ => {
                self.exceeded = true;
                Err(std::io::ErrorKind::InvalidData.into())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.exceeded {
            Err(std::io::ErrorKind::InvalidData.into())
        } else {
            Ok(())
        }
    }
}

/// Preflight the same borrowed envelope that the owner serializes. Callers use
/// their Wire grammar directly, so checking never serializes the owner again.
pub(crate) fn validate_text_context_envelope(
    encode: impl FnOnce(&mut TextContextWireBudget) -> Result<(), serde_json::Error>,
) -> Result<(), AgentTextContextError> {
    let mut budget = TextContextWireBudget::default();
    let encoded = encode(&mut budget);
    if budget.exceeded {
        return Err(AgentTextContextError::Limit {
            what: "wire bytes",
            limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        });
    }
    encoded.map_err(AgentTextContextError::from)
}

pub(crate) fn decode_text_context_envelope<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, AgentTextContextError> {
    if bytes.len() > MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES {
        return Err(AgentTextContextError::Limit {
            what: "wire bytes",
            limit: MAX_AGENT_TEXT_CONTEXT_WIRE_BYTES,
        });
    }
    Ok(serde_json::from_slice(bytes)?)
}

fn validate_entry(entry: &AgentTextContextEntry) -> Result<(), AgentTextContextError> {
    if entry.owner.root_id.is_empty() || entry.owner.semantic_id.is_empty() {
        return Err(AgentTextContextError::InvalidOwner {
            owner: entry.owner_ordinal,
        });
    }
    if matches!(
        entry.owner.kind,
        PreparedTextOwnerKind::DialogueView {
            role: DialoguePreparedTextRole::Content,
            ..
        }
    ) && !matches!(entry.content, AgentTextContextContent::DisplayStage { .. })
    {
        return Err(AgentTextContextError::ContentFamily {
            owner: entry.owner_ordinal,
        });
    }
    if let AgentTextContextContent::PreparedDocument { context, text, .. } = &entry.content {
        context
            .validate_text(text)
            .map_err(|error| AgentTextContextError::PreparedDocument {
                owner: entry.owner_ordinal,
                reason: error.to_string(),
            })?;
    }
    entry
        .content
        .layout()
        .validate_text_context(entry.content.ranges(), entry.content.prepared_text())
        .map_err(|error| AgentTextContextError::PreparedDocument {
            owner: entry.owner_ordinal,
            reason: error.to_string(),
        })?;
    Ok(())
}

/// Borrow exactly the final catalogue grammar for admission and serialization.
/// The factory checks this shape once; selected catalogues only remove rows.
struct AgentTextContextCatalogWire<'a>(&'a AgentTextContextCatalog);

impl Serialize for AgentTextContextCatalog {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        AgentTextContextCatalogWire(self).serialize(serializer)
    }
}

impl Serialize for AgentTextContextCatalogWire<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeSeq, SerializeStruct};
        struct Entries<'a>(&'a AgentTextContextCatalog);
        impl Serialize for Entries<'_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut seq = serializer.serialize_seq(Some(self.0.entries.len()))?;
                for entry in self.0.entries.values() {
                    seq.serialize_element(entry.as_ref())?;
                }
                seq.end()
            }
        }
        let mut wire = serializer.serialize_struct("AgentTextContextCatalog", 3)?;
        wire.serialize_field("version", &AGENT_TEXT_CONTEXT_CATALOG_VERSION)?;
        wire.serialize_field("scope", self.0.scope.as_ref())?;
        wire.serialize_field("entries", &Entries(self.0))?;
        wire.end()
    }
}

impl<'de> Deserialize<'de> for AgentTextContextCatalog {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        fn bounded_entries<'de, D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Vec<AgentTextContextEntry>, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Vec<AgentTextContextEntry>;
                fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str("bounded text context rows")
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut seq: A,
                ) -> Result<Self::Value, A::Error> {
                    if seq.size_hint().is_some_and(|n| n > MAX_AGENT_TEXT_CONTEXTS) {
                        return Err(serde::de::Error::custom(
                            "text context count limit exceeded",
                        ));
                    }
                    let mut entries = Vec::new();
                    let mut budget = CatalogBudget::default();
                    while entries.len() < MAX_AGENT_TEXT_CONTEXTS {
                        let Some(entry) = seq.next_element::<AgentTextContextEntry>()? else {
                            return Ok(entries);
                        };
                        budget.observe(&entry).map_err(serde::de::Error::custom)?;
                        entries.push(entry);
                    }
                    if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                        return Err(serde::de::Error::custom(
                            "text context count limit exceeded",
                        ));
                    }
                    Ok(entries)
                }
            }
            deserializer.deserialize_seq(Visitor)
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            version: u8,
            scope: AgentTextContextScope,
            #[serde(deserialize_with = "bounded_entries")]
            entries: Vec<AgentTextContextEntry>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.version != AGENT_TEXT_CONTEXT_CATALOG_VERSION {
            return Err(serde::de::Error::custom(AgentTextContextError::Version {
                actual: wire.version,
            }));
        }
        Self::try_new(wire.scope, wire.entries).map_err(serde::de::Error::custom)
    }
}
