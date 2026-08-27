//! Deterministic semantic and visual dialogue reveal evaluation.

use crate::{RichTextControl, RichTextControlMarker, RichTextStyle, RichTextTextRun};
use arcweft_core::step::RuntimeDialogueContentEventKind;

const DEFAULT_TYPEWRITER_MILLI_CPS: i32 = 28_000;
const MIN_TYPEWRITER_MILLI_CPS: i32 = 1_000;
const MAX_TYPEWRITER_MILLI_CPS: i32 = 240_000;
const NANOSECONDS_PER_MILLISECOND: u128 = 1_000_000;
const MILLI_CPS_NANOSECOND_NUMERATOR: u128 = 1_000_000_000_000;

/// Independent semantic and visual reveal choices for one dialogue stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DialogueRevealPolicy {
    pub complete_stage: bool,
    pub instant_characters: bool,
}

/// Stage-local logical reveal time retained at nanosecond precision.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueRevealElapsed(u64);

impl DialogueRevealElapsed {
    #[must_use]
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    #[must_use]
    pub const fn as_nanos(self) -> u64 {
        self.0
    }
}

impl From<arcweft_core::time::LogicalDuration> for DialogueRevealElapsed {
    fn from(value: arcweft_core::time::LogicalDuration) -> Self {
        Self::from_nanos(value.as_nanos())
    }
}

/// Exact reveal result shared by the runtime driver and renderers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueRevealEvaluation {
    display_start: usize,
    visible_end: usize,
    complete: bool,
    reached_content_events: Box<[RuntimeDialogueContentEventKind]>,
}

impl DialogueRevealEvaluation {
    #[must_use]
    pub const fn display_start(&self) -> usize {
        self.display_start
    }

    #[must_use]
    pub const fn visible_end(&self) -> usize {
        self.visible_end
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    #[must_use]
    pub const fn reached_content_events(&self) -> &[RuntimeDialogueContentEventKind] {
        &self.reached_content_events
    }
}

/// Evaluates one stage from the same logical time and completion policy for
/// presentation event routing and visual projection.
#[must_use]
pub fn evaluate_dialogue_reveal(
    text: &str,
    runs: &[RichTextTextRun],
    controls: &[RichTextControlMarker],
    reveal_start: usize,
    policy: DialogueRevealPolicy,
    elapsed: DialogueRevealElapsed,
) -> DialogueRevealEvaluation {
    let reveal_start = reveal_start.min(text.len());
    let mut controls = controls
        .iter()
        .filter(|marker| {
            marker.text_offset <= text.len() && text.is_char_boundary(marker.text_offset)
        })
        .collect::<Vec<_>>();
    controls.sort_by_key(|marker| (marker.text_offset, marker.node_index));
    if policy.complete_stage {
        let display_start = controls
            .iter()
            .rev()
            .find_map(|marker| {
                matches!(marker.control, RichTextControl::Clear).then_some(marker.text_offset)
            })
            .unwrap_or(0);
        return DialogueRevealEvaluation {
            display_start,
            visible_end: text.len(),
            complete: true,
            reached_content_events: controls
                .into_iter()
                .filter_map(|marker| content_event(&marker.control))
                .collect(),
        };
    }

    let mut remaining_nanos = u128::from(elapsed.as_nanos());
    let mut display_start = 0;
    let mut control_index = 0;
    let mut reached = Vec::new();
    if let Some(pending) = apply_controls_through(
        &controls,
        &mut control_index,
        reveal_start,
        &mut remaining_nanos,
        &mut display_start,
        &mut reached,
    ) {
        return evaluation(display_start, pending, false, reached);
    }

    let tail = text.get(reveal_start..).unwrap_or_default();
    let mut visible_end = reveal_start;
    for (relative, ch) in tail.char_indices() {
        let char_start = reveal_start.saturating_add(relative);
        if let Some(pending) = apply_controls_through(
            &controls,
            &mut control_index,
            char_start,
            &mut remaining_nanos,
            &mut display_start,
            &mut reached,
        ) {
            return evaluation(display_start, pending, false, reached);
        }
        let char_nanos = if policy.instant_characters {
            0
        } else {
            nanos_per_character(milli_cps_at(runs, char_start))
        };
        if remaining_nanos < char_nanos {
            return evaluation(display_start, visible_end, false, reached);
        }
        remaining_nanos -= char_nanos;
        visible_end = char_start.saturating_add(ch.len_utf8());
    }

    if let Some(pending) = apply_controls_through(
        &controls,
        &mut control_index,
        text.len(),
        &mut remaining_nanos,
        &mut display_start,
        &mut reached,
    ) {
        return evaluation(display_start, pending, false, reached);
    }
    evaluation(display_start, text.len(), true, reached)
}

fn evaluation(
    display_start: usize,
    visible_end: usize,
    complete: bool,
    reached_content_events: Vec<RuntimeDialogueContentEventKind>,
) -> DialogueRevealEvaluation {
    DialogueRevealEvaluation {
        display_start,
        visible_end,
        complete,
        reached_content_events: reached_content_events.into_boxed_slice(),
    }
}

fn apply_controls_through(
    controls: &[&RichTextControlMarker],
    control_index: &mut usize,
    offset: usize,
    remaining_nanos: &mut u128,
    display_start: &mut usize,
    reached: &mut Vec<RuntimeDialogueContentEventKind>,
) -> Option<usize> {
    while let Some(marker) = controls.get(*control_index)
        && marker.text_offset <= offset
    {
        match &marker.control {
            RichTextControl::TimedWait { duration_millis } => {
                let wait_nanos = u128::from(*duration_millis) * NANOSECONDS_PER_MILLISECOND;
                if *remaining_nanos < wait_nanos {
                    return Some(marker.text_offset);
                }
                *remaining_nanos -= wait_nanos;
            }
            RichTextControl::Clear => *display_start = marker.text_offset,
            control => {
                if let Some(event) = content_event(control) {
                    reached.push(event);
                }
            }
        }
        *control_index = (*control_index).saturating_add(1);
    }
    None
}

fn content_event(control: &RichTextControl) -> Option<RuntimeDialogueContentEventKind> {
    match control {
        RichTextControl::Mark { mark, .. } => Some(RuntimeDialogueContentEventKind::Mark(*mark)),
        RichTextControl::Effect { site } => Some(RuntimeDialogueContentEventKind::Effect(*site)),
        _ => None,
    }
}

fn milli_cps_at(runs: &[RichTextTextRun], offset: usize) -> i32 {
    let Some(run) = runs
        .iter()
        .find(|run| run.range.start <= offset && offset < run.range.end)
    else {
        return DEFAULT_TYPEWRITER_MILLI_CPS;
    };
    run.styles
        .iter()
        .rev()
        .find_map(|style| match style {
            RichTextStyle::Speed { milli_cps } => Some(milli_cps.0),
            _ => None,
        })
        .unwrap_or(DEFAULT_TYPEWRITER_MILLI_CPS)
        .clamp(MIN_TYPEWRITER_MILLI_CPS, MAX_TYPEWRITER_MILLI_CPS)
}

fn nanos_per_character(milli_cps: i32) -> u128 {
    let denominator = u128::from(milli_cps.unsigned_abs());
    MILLI_CPS_NANOSECOND_NUMERATOR.div_ceil(denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Milli, RichTextPresentation, RichTextRange, RichTextTextSource};
    use arcweft_core::runtime_id::{RuntimeDialogueEffectSiteId, RuntimeDialogueMarkId};

    fn run(text: &str, milli_cps: i32) -> RichTextTextRun {
        RichTextTextRun {
            range: RichTextRange::new(0, text.len()),
            source: RichTextTextSource::Text,
            node_index: 0,
            styles: vec![RichTextStyle::Speed {
                milli_cps: Milli(milli_cps),
            }],
            presentation: RichTextPresentation::default(),
        }
    }

    fn control(
        node_index: usize,
        text_offset: usize,
        control: RichTextControl,
    ) -> RichTextControlMarker {
        RichTextControlMarker {
            node_index,
            text_offset,
            control,
            range: None,
        }
    }

    #[test]
    fn character_boundary_uses_exact_nanoseconds_without_millisecond_truncation() {
        let text = "A";
        let runs = vec![run(text, 28_000)];
        let policy = DialogueRevealPolicy::default();
        let boundary = 35_714_286;

        assert_eq!(
            evaluate_dialogue_reveal(
                text,
                &runs,
                &[],
                0,
                policy,
                DialogueRevealElapsed::from_nanos(boundary - 1),
            )
            .visible_end(),
            0,
        );
        let exact = evaluate_dialogue_reveal(
            text,
            &runs,
            &[],
            0,
            policy,
            DialogueRevealElapsed::from_nanos(boundary),
        );
        assert_eq!(exact.visible_end(), 1);
        assert!(exact.is_complete());
    }

    #[test]
    fn typed_content_events_are_reported_only_when_their_offset_is_reached() {
        let text = "A";
        let runs = vec![run(text, 28_000)];
        let mark = RuntimeDialogueMarkId::from_zero_based(0).expect("mark");
        let effect = RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect");
        let controls = vec![
            control(
                1,
                1,
                RichTextControl::Mark {
                    mark,
                    diagnostic_name: "beat".to_owned(),
                },
            ),
            control(2, 1, RichTextControl::Effect { site: effect }),
        ];

        let pending = evaluate_dialogue_reveal(
            text,
            &runs,
            &controls,
            0,
            DialogueRevealPolicy::default(),
            DialogueRevealElapsed::from_nanos(35_714_285),
        );
        assert!(pending.reached_content_events().is_empty());

        let reached = evaluate_dialogue_reveal(
            text,
            &runs,
            &controls,
            0,
            DialogueRevealPolicy::default(),
            DialogueRevealElapsed::from_nanos(35_714_286),
        );
        assert_eq!(
            reached.reached_content_events(),
            [
                RuntimeDialogueContentEventKind::Mark(mark),
                RuntimeDialogueContentEventKind::Effect(effect),
            ],
        );
    }

    #[test]
    fn explicit_completion_reaches_events_after_an_unfinished_wait() {
        let effect = RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect");
        let controls = vec![
            control(
                1,
                0,
                RichTextControl::TimedWait {
                    duration_millis: u64::MAX,
                },
            ),
            control(2, 0, RichTextControl::Effect { site: effect }),
        ];
        let reveal = evaluate_dialogue_reveal(
            "",
            &[],
            &controls,
            0,
            DialogueRevealPolicy {
                complete_stage: true,
                instant_characters: false,
            },
            DialogueRevealElapsed::default(),
        );
        assert_eq!(
            reveal.reached_content_events(),
            [RuntimeDialogueContentEventKind::Effect(effect)],
        );
        assert!(reveal.is_complete());
    }
}
