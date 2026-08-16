use crate::{
    LineDisplayFrame, RichTextControl, RichTextControlMarker, RichTextDisplayMap,
    RichTextHostEventMarker, RichTextRange, RichTextRubyAnnotation, RichTextTextRun,
    RichTextTextSource,
};
use std::collections::BTreeMap;
use thiserror::Error;

/// Structural failure in a resolved line display frame.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LineDisplayFrameValidationError {
    #[error(
        "{kind} {index} has invalid UTF-8 offset {offset} for dialogue text of {text_len} bytes"
    )]
    InvalidOffset {
        kind: &'static str,
        index: usize,
        offset: usize,
        text_len: usize,
    },
    #[error(
        "{kind} {index} has invalid UTF-8 range {start}..{end} for dialogue text of {text_len} bytes"
    )]
    InvalidRange {
        kind: &'static str,
        index: usize,
        start: usize,
        end: usize,
        text_len: usize,
    },
    #[error("{kind} {index} must cover at least one byte")]
    EmptyRange { kind: &'static str, index: usize },
    #[error(
        "text run {index} starts at {actual_start}, but contiguous display text requires {expected_start}"
    )]
    TextRunDiscontinuity {
        index: usize,
        expected_start: usize,
        actual_start: usize,
    },
    #[error("text runs cover {covered_end} of {text_len} dialogue text bytes")]
    IncompleteTextCoverage { covered_end: usize, text_len: usize },
    #[error(
        "{kind} {index} at authored node {node_index} uses anchor {actual}, but another entry at that node uses {expected}"
    )]
    NodeAnchorMismatch {
        kind: &'static str,
        index: usize,
        node_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "{kind} {index} at authored node {node_index} regresses to offset {anchor} before the previous node end {previous_end}"
    )]
    AuthoredOrderRegression {
        kind: &'static str,
        index: usize,
        node_index: usize,
        anchor: usize,
        previous_end: usize,
    },
    #[error("control {index} `{control}` requires a visible text range")]
    MissingControlRange { index: usize, control: &'static str },
    #[error("zero-width control {index} `{control}` must not have a visible text range")]
    UnexpectedControlRange { index: usize, control: &'static str },
    #[error(
        "control {index} visible range starts at {range_start}, not its execution offset {text_offset}"
    )]
    ControlRangeStartMismatch {
        index: usize,
        text_offset: usize,
        range_start: usize,
    },
    #[error("control {index} visible text does not match its typed control payload")]
    ControlTextMismatch { index: usize },
    #[error("control {index} has no matching typed text run")]
    ControlTextRunMissing { index: usize },
    #[error("ruby annotation {index} has no containing RubyBase text run at its authored node")]
    RubyBaseRunMissing { index: usize },
    #[error("dialogue frame has {marker_count} host-event markers for {event_count} host events")]
    HostEventCountMismatch {
        marker_count: usize,
        event_count: usize,
    },
    #[error(
        "host-event marker {marker_index} points to event {actual}, but canonical index is {expected}"
    )]
    HostEventIndexMismatch {
        marker_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("host-event marker {marker_index} payload differs from host event {event_index}")]
    HostEventPayloadMismatch {
        marker_index: usize,
        event_index: usize,
    },
    #[error(
        "display stage {index} has invalid range {start}..{end} with reveal start {reveal_start}"
    )]
    InvalidStage {
        index: usize,
        start: usize,
        reveal_start: usize,
        end: usize,
    },
    #[error("dialogue frame must derive at least one display stage")]
    MissingStage,
}

/// The authored boundary that finishes one input-gated dialogue stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineDisplayStageEnd {
    /// `[l]`: wait, then reveal more text on the same logical page.
    LineWait,
    /// `[p]`: wait, then start a new logical page if more content follows.
    PageWait,
    /// End of the line's resolved content.
    LineEnd,
}

/// One input-gated view of a resolved dialogue line.
///
/// A stage contains the whole currently visible page, plus the byte offset at
/// which newly revealed text begins. This distinction lets `[l]` retain its
/// already-visible prefix while `[p]` starts the next stage from an empty page.
#[derive(Clone, Copy, Debug)]
pub struct LineDisplayStage<'a> {
    frame: &'a LineDisplayFrame,
    index: usize,
    page_index: usize,
    text_range: RichTextRange,
    reveal_start: usize,
    node_after: Option<usize>,
    node_end: usize,
    end: LineDisplayStageEnd,
}

impl LineDisplayFrame {
    /// Validates the resolved display map before playback or save restoration.
    ///
    /// The check covers UTF-8 boundaries, authored ordering, typed control and
    /// host-event coherence, and the ranges derived for input-gated stages.
    pub fn validate(&self) -> Result<(), LineDisplayFrameValidationError> {
        let mut covered_end = 0;
        for (index, run) in self.display_map.text_runs.iter().enumerate() {
            validate_range(&self.text, "text run", index, run.range, true)?;
            if run.range.start != covered_end {
                return Err(LineDisplayFrameValidationError::TextRunDiscontinuity {
                    index,
                    expected_start: covered_end,
                    actual_start: run.range.start,
                });
            }
            covered_end = run.range.end;
        }
        if covered_end != self.text.len() {
            return Err(LineDisplayFrameValidationError::IncompleteTextCoverage {
                covered_end,
                text_len: self.text.len(),
            });
        }

        for (index, ruby) in self.display_map.ruby_annotations.iter().enumerate() {
            validate_range(&self.text, "ruby annotation", index, ruby.base_range, true)?;
            if !self.display_map.text_runs.iter().any(|run| {
                run.node_index == ruby.node_index
                    && run.source == RichTextTextSource::RubyBase
                    && contains(run.range, ruby.base_range)
            }) {
                return Err(LineDisplayFrameValidationError::RubyBaseRunMissing { index });
            }
        }

        for (index, marker) in self.display_map.controls.iter().enumerate() {
            validate_offset(&self.text, "control", index, marker.text_offset)?;
            if let Some(range) = marker.range {
                validate_range(&self.text, "control", index, range, true)?;
                if range.start != marker.text_offset {
                    return Err(LineDisplayFrameValidationError::ControlRangeStartMismatch {
                        index,
                        text_offset: marker.text_offset,
                        range_start: range.start,
                    });
                }
            }
            validate_control(&self.text, &self.display_map.text_runs, marker, index)?;
        }

        if self.display_map.host_events.len() != self.host_events.len() {
            return Err(LineDisplayFrameValidationError::HostEventCountMismatch {
                marker_count: self.display_map.host_events.len(),
                event_count: self.host_events.len(),
            });
        }
        for (index, marker) in self.display_map.host_events.iter().enumerate() {
            validate_offset(&self.text, "host event", index, marker.text_offset)?;
            if marker.event_index != index {
                return Err(LineDisplayFrameValidationError::HostEventIndexMismatch {
                    marker_index: index,
                    expected: index,
                    actual: marker.event_index,
                });
            }
            if self.host_events.get(index) != Some(&marker.event) {
                return Err(LineDisplayFrameValidationError::HostEventPayloadMismatch {
                    marker_index: index,
                    event_index: marker.event_index,
                });
            }
        }

        self.validate_authored_order()?;
        let stages = self.stage_descriptors();
        if stages.is_empty() {
            return Err(LineDisplayFrameValidationError::MissingStage);
        }
        for (index, stage) in stages.iter().enumerate() {
            if stage.text_range.start > stage.reveal_start
                || stage.reveal_start > stage.text_range.end
                || stage.text_range.end > self.text.len()
                || !self.text.is_char_boundary(stage.text_range.start)
                || !self.text.is_char_boundary(stage.reveal_start)
                || !self.text.is_char_boundary(stage.text_range.end)
            {
                return Err(LineDisplayFrameValidationError::InvalidStage {
                    index,
                    start: stage.text_range.start,
                    reveal_start: stage.reveal_start,
                    end: stage.text_range.end,
                });
            }
        }
        Ok(())
    }

    /// Number of user-input-gated display stages in this line.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.stage_descriptors().len()
    }

    /// Number of logical pages represented by the input-gated stages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.stage_descriptors()
            .iter()
            .map(|stage| stage.page_index)
            .max()
            .map_or(0, |last| last.saturating_add(1))
    }

    /// Returns one display stage by zero-based authored order.
    #[must_use]
    pub fn stage(&self, index: usize) -> Option<LineDisplayStage<'_>> {
        self.stage_descriptors()
            .into_iter()
            .nth(index)
            .map(|descriptor| descriptor.bind(self, index))
    }

    /// Returns every display stage in authored order.
    #[must_use]
    pub fn stages(&self) -> Vec<LineDisplayStage<'_>> {
        self.stage_descriptors()
            .into_iter()
            .enumerate()
            .map(|(index, descriptor)| descriptor.bind(self, index))
            .collect()
    }

    fn stage_descriptors(&self) -> Vec<StageDescriptor> {
        let mut gates = self
            .display_map
            .controls
            .iter()
            .enumerate()
            .filter(|(_, marker)| {
                matches!(
                    marker.control,
                    RichTextControl::Page | RichTextControl::LineWait
                ) && self.valid_control_offset(marker)
            })
            .collect::<Vec<_>>();
        gates.sort_by_key(|(order, marker)| (marker.node_index, *order));

        let last_mapped_node = self
            .display_map
            .text_runs
            .iter()
            .map(|run| run.node_index)
            .chain(
                self.display_map
                    .ruby_annotations
                    .iter()
                    .map(|ruby| ruby.node_index),
            )
            .chain(
                self.display_map
                    .controls
                    .iter()
                    .map(|marker| marker.node_index),
            )
            .chain(
                self.display_map
                    .host_events
                    .iter()
                    .map(|marker| marker.node_index),
            )
            .max();

        let mut stages = Vec::with_capacity(gates.len().saturating_add(1));
        let mut retained_start = 0;
        let mut reveal_start = 0;
        let mut page_index = 0;
        let mut node_after = None;

        for (_, gate) in gates {
            let end = match gate.control {
                RichTextControl::Page => LineDisplayStageEnd::PageWait,
                RichTextControl::LineWait => LineDisplayStageEnd::LineWait,
                _ => unreachable!("gates contain only page and line waits"),
            };
            stages.push(StageDescriptor {
                page_index,
                text_range: RichTextRange::new(retained_start, gate.text_offset),
                reveal_start,
                node_after,
                node_end: gate.node_index,
                end,
            });
            let last_reached_clear = self
                .display_map
                .controls
                .iter()
                .enumerate()
                .filter(|(_, marker)| {
                    matches!(marker.control, RichTextControl::Clear)
                        && self.valid_control_offset(marker)
                        && node_after.is_none_or(|after| marker.node_index > after)
                        && marker.node_index <= gate.node_index
                })
                .max_by_key(|(order, marker)| (marker.node_index, *order))
                .map(|(_, marker)| marker.text_offset);
            node_after = Some(gate.node_index);
            reveal_start = gate.text_offset;
            if matches!(gate.control, RichTextControl::Page) {
                retained_start = gate.text_offset;
                page_index = page_index.saturating_add(1);
            } else if let Some(clear_offset) = last_reached_clear {
                // `[l]` retains the display produced by the completed stage.
                // Text removed by a reached `[clear]` must not reappear when
                // the next stage starts revealing on the same logical page.
                retained_start = retained_start.max(clear_offset);
            }
        }

        let needs_tail = stages.is_empty()
            || node_after
                .zip(last_mapped_node)
                .is_some_and(|(gate, last)| gate < last);
        if needs_tail {
            stages.push(StageDescriptor {
                page_index,
                text_range: RichTextRange::new(retained_start, self.text.len()),
                reveal_start,
                node_after,
                node_end: last_mapped_node.unwrap_or(0),
                end: LineDisplayStageEnd::LineEnd,
            });
        }
        stages
    }

    fn valid_control_offset(&self, marker: &RichTextControlMarker) -> bool {
        marker.text_offset <= self.text.len() && self.text.is_char_boundary(marker.text_offset)
    }

    fn validate_authored_order(&self) -> Result<(), LineDisplayFrameValidationError> {
        let mut entries = BTreeMap::<usize, Vec<MappedExtent>>::new();
        for (index, run) in self.display_map.text_runs.iter().enumerate() {
            entries
                .entry(run.node_index)
                .or_default()
                .push(MappedExtent {
                    kind: "text run",
                    index,
                    anchor: run.range.start,
                    end: run.range.end,
                });
        }
        for (index, ruby) in self.display_map.ruby_annotations.iter().enumerate() {
            entries
                .entry(ruby.node_index)
                .or_default()
                .push(MappedExtent {
                    kind: "ruby annotation",
                    index,
                    anchor: ruby.base_range.start,
                    end: ruby.base_range.end,
                });
        }
        for (index, marker) in self.display_map.controls.iter().enumerate() {
            entries
                .entry(marker.node_index)
                .or_default()
                .push(MappedExtent {
                    kind: "control",
                    index,
                    anchor: marker.text_offset,
                    end: marker.range.map_or(marker.text_offset, |range| range.end),
                });
        }
        for (index, marker) in self.display_map.host_events.iter().enumerate() {
            entries
                .entry(marker.node_index)
                .or_default()
                .push(MappedExtent {
                    kind: "host event",
                    index,
                    anchor: marker.text_offset,
                    end: marker.text_offset,
                });
        }

        let mut previous_end = 0;
        for (node_index, node_entries) in entries {
            let expected = node_entries[0].anchor;
            for entry in &node_entries {
                if entry.anchor != expected {
                    return Err(LineDisplayFrameValidationError::NodeAnchorMismatch {
                        kind: entry.kind,
                        index: entry.index,
                        node_index,
                        expected,
                        actual: entry.anchor,
                    });
                }
                if entry.anchor < previous_end {
                    return Err(LineDisplayFrameValidationError::AuthoredOrderRegression {
                        kind: entry.kind,
                        index: entry.index,
                        node_index,
                        anchor: entry.anchor,
                        previous_end,
                    });
                }
            }
            previous_end = node_entries
                .iter()
                .map(|entry| entry.end)
                .max()
                .unwrap_or(previous_end);
        }
        Ok(())
    }
}

impl<'a> LineDisplayStage<'a> {
    /// Full resolved line that owns this stage.
    #[must_use]
    pub const fn frame(self) -> &'a LineDisplayFrame {
        self.frame
    }

    /// Zero-based stage index within the resolved line.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    /// Zero-based logical page index. `[l]` does not change this value.
    #[must_use]
    pub const fn page_index(self) -> usize {
        self.page_index
    }

    /// Full-line UTF-8 byte range shown on the current logical page.
    #[must_use]
    pub const fn text_range(self) -> RichTextRange {
        self.text_range
    }

    /// Page-local UTF-8 byte offset where this stage starts revealing.
    #[must_use]
    pub fn reveal_start(self) -> usize {
        self.reveal_start.saturating_sub(self.text_range.start)
    }

    /// Authored boundary that finishes this stage.
    #[must_use]
    pub const fn end(self) -> LineDisplayStageEnd {
        self.end
    }

    /// Resolved text visible on this stage's logical page.
    ///
    /// # Panics
    ///
    /// Panics only if the owning frame is internally inconsistent: stage
    /// ranges are constructed from validated UTF-8 display-map offsets.
    #[must_use]
    pub fn text(self) -> &'a str {
        self.frame
            .text
            .get(self.text_range.start..self.text_range.end)
            .expect("stage ranges are validated UTF-8 boundaries")
    }

    /// Page-local text runs, including the retained prefix of an `[l]` stage.
    #[must_use]
    pub fn text_runs(self) -> Vec<RichTextTextRun> {
        self.frame
            .display_map
            .text_runs
            .iter()
            .filter_map(|run| {
                intersect(run.range, self.text_range).map(|range| {
                    let mut run = run.clone();
                    run.range = self.rebase(range);
                    run
                })
            })
            .collect()
    }

    /// Page-local ruby annotations whose bases are visible in this stage.
    #[must_use]
    pub fn ruby_annotations(self) -> Vec<RichTextRubyAnnotation> {
        self.frame
            .display_map
            .ruby_annotations
            .iter()
            .filter(|ruby| contains(self.text_range, ruby.base_range))
            .cloned()
            .map(|mut ruby| {
                ruby.base_range = self.rebase(ruby.base_range);
                ruby
            })
            .collect()
    }

    /// Controls reached while revealing this stage, in authored order.
    #[must_use]
    pub fn controls(self) -> Vec<RichTextControlMarker> {
        self.frame
            .display_map
            .controls
            .iter()
            .filter(|marker| self.contains_node(marker.node_index))
            .filter(|marker| {
                marker.text_offset >= self.text_range.start
                    && marker.text_offset <= self.text_range.end
            })
            .cloned()
            .map(|mut marker| {
                marker.text_offset = marker.text_offset.saturating_sub(self.text_range.start);
                marker.range = marker
                    .range
                    .and_then(|range| intersect(range, self.text_range))
                    .map(|range| self.rebase(range));
                marker
            })
            .collect()
    }

    /// Host events reached while revealing this stage, in authored order.
    #[must_use]
    pub fn host_events(self) -> Vec<RichTextHostEventMarker> {
        self.frame
            .display_map
            .host_events
            .iter()
            .filter(|marker| self.contains_node(marker.node_index))
            .filter(|marker| {
                marker.text_offset >= self.text_range.start
                    && marker.text_offset <= self.text_range.end
            })
            .cloned()
            .map(|mut marker| {
                marker.text_offset = marker.text_offset.saturating_sub(self.text_range.start);
                marker
            })
            .collect()
    }

    /// Materializes the current stage as a standalone resolved display frame.
    ///
    /// The projection intentionally uses the resolved display map as its source
    /// of truth. Authored nodes outside the active stage are not copied, so
    /// observation and capture cannot expose future page content.
    #[must_use]
    pub fn to_frame(self) -> LineDisplayFrame {
        let mut host_event_markers = self.host_events();
        let host_events = host_event_markers
            .iter()
            .map(|marker| marker.event.clone())
            .collect::<Vec<_>>();
        for (event_index, marker) in host_event_markers.iter_mut().enumerate() {
            marker.event_index = event_index;
        }
        LineDisplayFrame {
            line: self.frame.line.clone(),
            character: self.frame.character.clone(),
            text_key: self.frame.text_key.clone(),
            effective: self.frame.effective.clone(),
            text: self.text().to_owned(),
            base_styles: self.frame.base_styles.clone(),
            style_contributions: self.frame.style_contributions.clone(),
            nodes: Vec::new(),
            display_map: RichTextDisplayMap {
                text_runs: self.text_runs(),
                ruby_annotations: self.ruby_annotations(),
                controls: self.controls(),
                host_events: host_event_markers,
            },
            host_events,
            inline_failures: self.frame.inline_failures.clone(),
            unresolved: self.frame.unresolved.clone(),
        }
    }

    fn contains_node(self, node_index: usize) -> bool {
        self.node_after.is_none_or(|after| node_index > after) && node_index <= self.node_end
    }

    const fn rebase(self, range: RichTextRange) -> RichTextRange {
        RichTextRange::new(
            range.start - self.text_range.start,
            range.end - self.text_range.start,
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct StageDescriptor {
    page_index: usize,
    text_range: RichTextRange,
    reveal_start: usize,
    node_after: Option<usize>,
    node_end: usize,
    end: LineDisplayStageEnd,
}

#[derive(Clone, Copy, Debug)]
struct MappedExtent {
    kind: &'static str,
    index: usize,
    anchor: usize,
    end: usize,
}

impl StageDescriptor {
    const fn bind(self, frame: &LineDisplayFrame, index: usize) -> LineDisplayStage<'_> {
        LineDisplayStage {
            frame,
            index,
            page_index: self.page_index,
            text_range: self.text_range,
            reveal_start: self.reveal_start,
            node_after: self.node_after,
            node_end: self.node_end,
            end: self.end,
        }
    }
}

fn intersect(left: RichTextRange, right: RichTextRange) -> Option<RichTextRange> {
    let start = left.start.max(right.start);
    let end = left.end.min(right.end);
    (start < end).then(|| RichTextRange::new(start, end))
}

const fn contains(outer: RichTextRange, inner: RichTextRange) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn validate_offset(
    text: &str,
    kind: &'static str,
    index: usize,
    offset: usize,
) -> Result<(), LineDisplayFrameValidationError> {
    if offset <= text.len() && text.is_char_boundary(offset) {
        return Ok(());
    }
    Err(LineDisplayFrameValidationError::InvalidOffset {
        kind,
        index,
        offset,
        text_len: text.len(),
    })
}

fn validate_range(
    text: &str,
    kind: &'static str,
    index: usize,
    range: RichTextRange,
    non_empty: bool,
) -> Result<(), LineDisplayFrameValidationError> {
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return Err(LineDisplayFrameValidationError::InvalidRange {
            kind,
            index,
            start: range.start,
            end: range.end,
            text_len: text.len(),
        });
    }
    if non_empty && range.start == range.end {
        return Err(LineDisplayFrameValidationError::EmptyRange { kind, index });
    }
    Ok(())
}

fn validate_control(
    text: &str,
    runs: &[RichTextTextRun],
    marker: &RichTextControlMarker,
    index: usize,
) -> Result<(), LineDisplayFrameValidationError> {
    let (control, expected_text, source) = match &marker.control {
        RichTextControl::HardBreak => (
            "hard_break",
            Some("\n"),
            Some(RichTextTextSource::ControlHardBreak),
        ),
        RichTextControl::Raw { text } => (
            "raw",
            Some(text.as_str()),
            Some(RichTextTextSource::ControlRaw),
        ),
        RichTextControl::Page => ("page", None, None),
        RichTextControl::LineWait => ("line_wait", None, None),
        RichTextControl::TimedWait { .. } => ("timed_wait", None, None),
        RichTextControl::Clear => ("clear", None, None),
        RichTextControl::Reset => ("reset", None, None),
        RichTextControl::Mark { .. } => ("mark", None, None),
    };
    match (expected_text, source, marker.range) {
        (Some(expected_text), Some(source), Some(range)) => {
            if text.get(range.start..range.end) != Some(expected_text) {
                return Err(LineDisplayFrameValidationError::ControlTextMismatch { index });
            }
            if !runs.iter().any(|run| {
                run.node_index == marker.node_index && run.range == range && run.source == source
            }) {
                return Err(LineDisplayFrameValidationError::ControlTextRunMissing { index });
            }
        }
        (Some(_), Some(_), None) => {
            return Err(LineDisplayFrameValidationError::MissingControlRange { index, control });
        }
        (None, None, Some(_)) => {
            return Err(LineDisplayFrameValidationError::UnexpectedControlRange { index, control });
        }
        (None, None, None) => {}
        _ => unreachable!("typed control validation cases are exhaustive"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CharacterDialoguePresentationConfig, DialogueHostEvent, DialoguePresentationCharacter,
        ResolvedRichTextNode, RichTextNode, RichTextPresentation,
    };
    use arcweft_character::id::CharacterId;
    use arcweft_core::{entry::RuntimeValueDigest, plan::RuntimeLineId};
    use arcweft_dialogue::InlineFailurePolicy;
    use arcweft_id::TextKey;
    use arcweft_view::ViewId;

    fn frame(nodes: &[RichTextNode]) -> LineDisplayFrame {
        let mut text = String::new();
        let mut display_map = RichTextDisplayMap::default();
        let mut resolved_nodes = Vec::with_capacity(nodes.len());
        for (node_index, node) in nodes.iter().enumerate() {
            match node {
                RichTextNode::Text { text: value } => {
                    let start = text.len();
                    text.push_str(value);
                    display_map.text_runs.push(RichTextTextRun {
                        range: RichTextRange::new(start, text.len()),
                        source: RichTextTextSource::Text,
                        node_index,
                        styles: Vec::new(),
                        presentation: RichTextPresentation::default(),
                    });
                    resolved_nodes.push(ResolvedRichTextNode::Text {
                        text: value.clone(),
                    });
                }
                RichTextNode::Control { control } => {
                    display_map.controls.push(RichTextControlMarker {
                        node_index,
                        text_offset: text.len(),
                        control: control.clone(),
                        range: None,
                    });
                    resolved_nodes.push(ResolvedRichTextNode::Control {
                        control: control.clone(),
                    });
                }
                _ => panic!("playback fixture only accepts text and zero-width controls"),
            }
        }
        LineDisplayFrame {
            line: RuntimeLineId::canonical("playback.test").expect("canonical line"),
            character: DialoguePresentationCharacter {
                id: CharacterId::try_new("character.alice").expect("character identity"),
                display_name: "Alice".to_owned(),
            },
            text_key: TextKey::try_new("text.playback.test").expect("text key"),
            effective: CharacterDialoguePresentationConfig {
                view: ViewId::try_new("view.playback.test").expect("View identity"),
                voice: None,
                look: None,
                stage: None,
                portrait: None,
                focus: None,
                cleanup: None,
                source_locale: None,
                hooks: Vec::new(),
                inline_failure: InlineFailurePolicy::FailLine,
                custom: BTreeMap::new(),
                config_digest: RuntimeValueDigest::ZERO,
            },
            text,
            base_styles: Vec::new(),
            style_contributions: Vec::new(),
            nodes: resolved_nodes,
            display_map,
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    #[test]
    fn page_and_line_waits_build_distinct_stage_shapes() {
        let frame = frame(&[
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "B".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::LineWait,
            },
            RichTextNode::Text {
                text: "C".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
        ]);

        let stages = frame.stages();
        assert_eq!(stages.len(), 3);
        assert_eq!(frame.page_count(), 2);
        assert_eq!(stages[0].text(), "A");
        assert_eq!(stages[0].page_index(), 0);
        assert_eq!(stages[0].reveal_start(), 0);
        assert_eq!(stages[0].end(), LineDisplayStageEnd::PageWait);
        assert_eq!(stages[1].text(), "B");
        assert_eq!(stages[1].page_index(), 1);
        assert_eq!(stages[1].reveal_start(), 0);
        assert_eq!(stages[1].end(), LineDisplayStageEnd::LineWait);
        assert_eq!(stages[2].text(), "BC");
        assert_eq!(stages[2].page_index(), 1);
        assert_eq!(stages[2].reveal_start(), 1);
        assert_eq!(stages[2].end(), LineDisplayStageEnd::PageWait);
    }

    #[test]
    fn trailing_page_wait_does_not_create_an_empty_tail() {
        let frame = frame(&[
            RichTextNode::Text {
                text: "終端".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
        ]);

        assert_eq!(frame.stage_count(), 1);
        assert_eq!(frame.page_count(), 1);
        assert_eq!(frame.stage(0).expect("stage").text(), "終端");
    }

    #[test]
    fn leading_and_consecutive_page_waits_preserve_authored_waits() {
        let frame = frame(&[
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "A".to_owned(),
            },
        ]);

        let stages = frame.stages();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0].text(), "");
        assert_eq!(stages[1].text(), "");
        assert_eq!(stages[2].text(), "A");
        assert_eq!(stages[2].page_index(), 2);
    }

    #[test]
    fn control_offsets_and_stage_runs_are_utf8_exact() {
        let frame = frame(&[
            RichTextNode::Text {
                text: "夢".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "続".to_owned(),
            },
        ]);

        assert_eq!(frame.display_map.controls[0].text_offset, "夢".len());
        let second = frame.stage(1).expect("second stage");
        assert_eq!(second.text(), "続");
        assert_eq!(
            second.text_runs()[0].range,
            RichTextRange::new(0, "続".len())
        );
    }

    #[test]
    fn line_wait_retains_the_display_origin_of_a_reached_clear() {
        let frame = frame(&[
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Clear,
            },
            RichTextNode::Text {
                text: "B".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::LineWait,
            },
            RichTextNode::Text {
                text: "C".to_owned(),
            },
        ]);

        let stages = frame.stages();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].text(), "AB");
        assert_eq!(stages[0].controls()[0].control, RichTextControl::Clear);
        assert_eq!(stages[1].text(), "BC");
        assert_eq!(stages[1].reveal_start(), 1);
        assert!(stages[1].controls().is_empty());
        assert_eq!(stages[1].page_index(), 0);
        frame.validate().expect("resolved frame is valid");
    }

    #[test]
    fn validation_rejects_a_control_offset_inside_a_utf8_codepoint() {
        let mut frame = frame(&[
            RichTextNode::Text {
                text: "夢".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
        ]);
        frame.display_map.controls[0].text_offset = 1;

        assert!(matches!(
            frame.validate(),
            Err(LineDisplayFrameValidationError::InvalidOffset {
                kind: "control",
                index: 0,
                offset: 1,
                ..
            })
        ));
    }

    #[test]
    fn validation_rejects_authored_gate_offsets_that_regress() {
        let mut frame = frame(&[
            RichTextNode::Text {
                text: "A".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::Page,
            },
            RichTextNode::Text {
                text: "B".to_owned(),
            },
            RichTextNode::Control {
                control: RichTextControl::LineWait,
            },
        ]);
        frame.display_map.controls[1].text_offset = 0;

        assert!(matches!(
            frame.validate(),
            Err(LineDisplayFrameValidationError::AuthoredOrderRegression {
                kind: "control",
                index: 1,
                anchor: 0,
                ..
            })
        ));
    }

    #[test]
    fn validation_rejects_a_host_event_payload_mismatch() {
        let mut frame = frame(&[RichTextNode::Text {
            text: "A".to_owned(),
        }]);
        frame.host_events.push(DialogueHostEvent::Voice {
            source: crate::DialogueVoiceSource::Identity {
                id: "voice-a".to_owned(),
            },
        });
        frame.display_map.host_events.push(RichTextHostEventMarker {
            node_index: 1,
            text_offset: 1,
            event_index: 0,
            event: DialogueHostEvent::Voice {
                source: crate::DialogueVoiceSource::Identity {
                    id: "voice-b".to_owned(),
                },
            },
        });

        assert!(matches!(
            frame.validate(),
            Err(LineDisplayFrameValidationError::HostEventPayloadMismatch {
                marker_index: 0,
                event_index: 0,
            })
        ));
    }

    #[test]
    fn frame_without_mapped_content_still_has_one_stage() {
        let frame = frame(&[]);

        assert_eq!(frame.stage_count(), 1);
        assert_eq!(frame.stage(0).expect("empty stage").text(), "");
    }
}
