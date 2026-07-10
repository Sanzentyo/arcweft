use arcweft_render_text::{
    RichTextControl, RichTextControlMarker, RichTextEffectPhase, RichTextParam, RichTextStyle,
    RichTextTextRun,
};
use num_traits::ToPrimitive;

const DEFAULT_TYPEWRITER_CPS: f64 = 28.0;
const MIN_TYPEWRITER_CPS: f64 = 1.0;
const MAX_TYPEWRITER_CPS: f64 = 240.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DialogueRevealWindow {
    pub(super) display_start: usize,
    pub(super) visible_end: usize,
    pub(super) complete: bool,
}

/// Independent semantic and visual reveal choices for one dialogue stage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DialogueRevealPolicy {
    /// Completes the current stage, including authored waits and clears.
    pub(super) complete_stage: bool,
    /// Removes per-character delay while preserving authored control timing.
    pub(super) instant_characters: bool,
}

pub(super) fn evaluate_dialogue_reveal(
    text: &str,
    runs: &[RichTextTextRun],
    controls: &[RichTextControlMarker],
    reveal_start: usize,
    policy: DialogueRevealPolicy,
    visual_time_millis: u64,
) -> DialogueRevealWindow {
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
        return DialogueRevealWindow {
            display_start,
            visible_end: text.len(),
            complete: true,
        };
    }

    let mut remaining_micros = visual_time_millis.saturating_mul(1_000);
    let mut display_start = 0;
    let mut control_index = 0;
    if let Some(pending) = apply_controls_through(
        &controls,
        &mut control_index,
        reveal_start,
        &mut remaining_micros,
        &mut display_start,
    ) {
        return DialogueRevealWindow {
            display_start,
            visible_end: pending,
            complete: false,
        };
    }

    let tail = text.get(reveal_start..).unwrap_or_default();
    let mut visible_end = reveal_start;
    for (relative, ch) in tail.char_indices() {
        let char_start = reveal_start.saturating_add(relative);
        if let Some(pending) = apply_controls_through(
            &controls,
            &mut control_index,
            char_start,
            &mut remaining_micros,
            &mut display_start,
        ) {
            return DialogueRevealWindow {
                display_start,
                visible_end: pending,
                complete: false,
            };
        }
        let char_micros = if policy.instant_characters {
            0
        } else {
            micros_per_character(cps_at(runs, char_start))
        };
        if remaining_micros < char_micros {
            return DialogueRevealWindow {
                display_start,
                visible_end,
                complete: false,
            };
        }
        remaining_micros -= char_micros;
        visible_end = char_start.saturating_add(ch.len_utf8());
    }

    if let Some(pending) = apply_controls_through(
        &controls,
        &mut control_index,
        text.len(),
        &mut remaining_micros,
        &mut display_start,
    ) {
        return DialogueRevealWindow {
            display_start,
            visible_end: pending,
            complete: false,
        };
    }
    DialogueRevealWindow {
        display_start,
        visible_end: text.len(),
        complete: true,
    }
}

fn apply_controls_through(
    controls: &[&RichTextControlMarker],
    control_index: &mut usize,
    offset: usize,
    remaining_micros: &mut u64,
    display_start: &mut usize,
) -> Option<usize> {
    while let Some(marker) = controls.get(*control_index)
        && marker.text_offset <= offset
    {
        match marker.control {
            RichTextControl::TimedWait { duration_millis } => {
                let wait_micros = duration_millis.saturating_mul(1_000);
                if *remaining_micros < wait_micros {
                    return Some(marker.text_offset);
                }
                *remaining_micros -= wait_micros;
            }
            RichTextControl::Clear => *display_start = marker.text_offset,
            RichTextControl::Page
            | RichTextControl::LineWait
            | RichTextControl::HardBreak
            | RichTextControl::Reset
            | RichTextControl::Mark { .. }
            | RichTextControl::Raw { .. }
            | RichTextControl::Unknown { .. } => {}
        }
        *control_index = (*control_index).saturating_add(1);
    }
    None
}

fn cps_at(runs: &[RichTextTextRun], offset: usize) -> f64 {
    let Some(run) = runs
        .iter()
        .find(|run| run.range.start <= offset && offset < run.range.end)
    else {
        return DEFAULT_TYPEWRITER_CPS;
    };
    run.styles
        .iter()
        .rev()
        .find_map(|style| match style {
            RichTextStyle::Speed { value } => parse_speed(value),
            _ => None,
        })
        .or_else(|| {
            run.presentation
                .effects
                .iter()
                .find(|effect| {
                    effect.id == "typewriter" && effect.phase == RichTextEffectPhase::GlyphMask
                })
                .and_then(|effect| {
                    effect
                        .params
                        .get("cps")
                        .or_else(|| effect.params.get("speed"))
                        .and_then(param_speed)
                })
        })
        .unwrap_or(DEFAULT_TYPEWRITER_CPS)
        .clamp(MIN_TYPEWRITER_CPS, MAX_TYPEWRITER_CPS)
}

fn parse_speed(value: &str) -> Option<f64> {
    let value = value
        .trim()
        .strip_prefix("cps=")
        .or_else(|| value.trim().strip_prefix("speed="))
        .unwrap_or(value.trim());
    match value {
        "slow" => Some(14.0),
        "normal" => Some(DEFAULT_TYPEWRITER_CPS),
        "fast" => Some(56.0),
        _ => value.parse::<f64>().ok().filter(|value| value.is_finite()),
    }
}

fn param_speed(param: &RichTextParam) -> Option<f64> {
    match param {
        RichTextParam::Int { value } => value.to_f64(),
        RichTextParam::Milli { value } => value.0.to_f64().map(|value| value / 1_000.0),
        RichTextParam::Text { value } | RichTextParam::Raw { value } => parse_speed(value),
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    }
}

fn micros_per_character(cps: f64) -> u64 {
    let micros = (1_000_000.0 / cps).ceil();
    micros.to_u64().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_render_text::{RichTextPresentation, RichTextRange, RichTextTextSource};

    const TIMED_REVEAL: DialogueRevealPolicy = DialogueRevealPolicy {
        complete_stage: false,
        instant_characters: false,
    };
    const COMPLETE_REVEAL: DialogueRevealPolicy = DialogueRevealPolicy {
        complete_stage: true,
        instant_characters: false,
    };
    const INSTANT_CHARACTERS: DialogueRevealPolicy = DialogueRevealPolicy {
        complete_stage: false,
        instant_characters: true,
    };

    fn run(text: &str, styles: Vec<RichTextStyle>) -> RichTextTextRun {
        RichTextTextRun {
            range: RichTextRange::new(0, text.len()),
            source: RichTextTextSource::Text,
            node_index: 0,
            styles,
            presentation: RichTextPresentation::default(),
        }
    }

    fn control(offset: usize, control: RichTextControl) -> RichTextControlMarker {
        RichTextControlMarker {
            node_index: 1,
            text_offset: offset,
            control,
            range: None,
        }
    }

    #[test]
    fn timed_wait_begins_only_after_reveal_reaches_its_offset() {
        let text = "AB";
        let runs = vec![run(
            text,
            vec![RichTextStyle::Speed {
                value: "10".to_owned(),
            }],
        )];
        let controls = vec![control(
            1,
            RichTextControl::TimedWait {
                duration_millis: 500,
            },
        )];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, 100),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: 1,
                complete: false,
            }
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, 599).visible_end,
            1
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, 700),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: 2,
                complete: true,
            }
        );
    }

    #[test]
    fn instant_character_reveal_still_honors_timed_waits() {
        let text = "AB";
        let runs = vec![run(text, Vec::new())];
        let controls = vec![control(
            1,
            RichTextControl::TimedWait {
                duration_millis: 500,
            },
        )];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, INSTANT_CHARACTERS, 0),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: 1,
                complete: false,
            }
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, INSTANT_CHARACTERS, 499),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: 1,
                complete: false,
            }
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, INSTANT_CHARACTERS, 500),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: text.len(),
                complete: true,
            }
        );
    }

    #[test]
    fn clear_changes_the_display_origin_when_reached() {
        let text = "AB";
        let runs = vec![run(
            text,
            vec![RichTextStyle::Speed {
                value: "10".to_owned(),
            }],
        )];
        let controls = vec![control(1, RichTextControl::Clear)];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, 100),
            DialogueRevealWindow {
                display_start: 1,
                visible_end: 1,
                complete: false,
            }
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, 200),
            DialogueRevealWindow {
                display_start: 1,
                visible_end: 2,
                complete: true,
            }
        );
    }

    #[test]
    fn speed_style_changes_the_rate_for_following_runs() {
        let text = "ABCD";
        let runs = vec![
            RichTextTextRun {
                range: RichTextRange::new(0, 1),
                source: RichTextTextSource::Text,
                node_index: 0,
                styles: vec![RichTextStyle::Speed {
                    value: "5".to_owned(),
                }],
                presentation: RichTextPresentation::default(),
            },
            RichTextTextRun {
                range: RichTextRange::new(1, text.len()),
                source: RichTextTextSource::Text,
                node_index: 1,
                styles: vec![RichTextStyle::Speed {
                    value: "20".to_owned(),
                }],
                presentation: RichTextPresentation::default(),
            },
        ];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &[], 0, TIMED_REVEAL, 199).visible_end,
            0
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &[], 0, TIMED_REVEAL, 200).visible_end,
            1
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &[], 0, TIMED_REVEAL, 250).visible_end,
            2
        );
        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &[], 0, TIMED_REVEAL, 350),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: text.len(),
                complete: true,
            }
        );
    }

    #[test]
    fn explicit_completion_finishes_waits_and_applies_the_last_clear() {
        let text = "AB";
        let runs = vec![run(text, Vec::new())];
        let controls = vec![
            control(
                1,
                RichTextControl::TimedWait {
                    duration_millis: u64::MAX,
                },
            ),
            control(1, RichTextControl::Clear),
        ];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, COMPLETE_REVEAL, 0),
            DialogueRevealWindow {
                display_start: 1,
                visible_end: text.len(),
                complete: true,
            }
        );
    }

    #[test]
    fn maximum_visual_time_is_not_a_completion_sentinel() {
        let text = "AB";
        let runs = vec![run(text, Vec::new())];
        let controls = vec![control(
            1,
            RichTextControl::TimedWait {
                duration_millis: u64::MAX,
            },
        )];

        assert_eq!(
            evaluate_dialogue_reveal(text, &runs, &controls, 0, TIMED_REVEAL, u64::MAX),
            DialogueRevealWindow {
                display_start: 0,
                visible_end: 1,
                complete: false,
            }
        );
    }
}
