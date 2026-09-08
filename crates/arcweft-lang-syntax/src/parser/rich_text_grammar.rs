//! Private `RichText` descendants emitted inside the shared dialogue grammar.
//!
//! This module consumes the document lexer's existing cursor and the neutral
//! argument scan owned by `text::dialogue_action`. It never invokes the public
//! dialogue parser, reparses a source substring, or wraps detached AST nodes.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::expression::{completed_slot, emit_expression_node};
use super::lexer::typed_entity_reference_source;
use super::shadow_recovery::{emit_close_delimiter, emit_open_delimiter};
use crate::ast::common::TextRange;
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    SyntaxDialogueActionArgumentParts, SyntaxDialogueActionArgumentProjection,
    SyntaxDialogueActionArgumentSourcePart, SyntaxDialogueActionValue, SyntaxDialogueContent,
    SyntaxDialogueContentIssue, SyntaxDialogueContentProjection,
    SyntaxDialogueContentRecoveryBoundary, SyntaxDialogueControl, SyntaxDialogueMarkName,
    SyntaxDialogueMarkNameIssue, SyntaxDialogueNodeProjection, SyntaxDialogueNodeSourcePart,
    SyntaxDialoguePointActionIdentity, SyntaxDialoguePointActionPayload,
    SyntaxDialoguePointActionProjection, SyntaxDialoguePointActionSourcePart, SyntaxExpressionSlot,
    SyntaxLineBreakKind, SyntaxRawLiteralBody, SyntaxRichTextHostEvent, SyntaxRichTextIssue,
};
use crate::grammar::event::{ExpectedToken, PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::SyntaxName;
use crate::text::{
    DialogueTextDiagnosticCode, MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL,
    MAX_DIALOGUE_ACTION_HEAD_BYTES, MAX_DIALOGUE_POINT_ACTIONS, ScannedContentApplicationCallee,
    ScannedDialogueActionArgument, ScannedDialogueActionArgumentParts,
    ScannedDialogueActionArgumentValue, ScannedDialogueActionArguments, ScannedDialogueSurface,
    ScannedDialogueSurfaceKind, find_dialogue_bracket_boundary, is_dialogue_action_whitespace,
    scan_dialogue_action_argument_value_if_valid, scan_dialogue_action_arguments,
    scan_dialogue_surface, trim_dialogue_action_whitespace, utf8_boundary_at_or_before,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct EmittedDialogueContent {
    projection: SyntaxDialogueContentProjection,
    components: Vec<PendingExpressionComponent>,
    has_real_atom: bool,
}

impl EmittedDialogueContent {
    pub(super) fn into_parts(
        self,
    ) -> (
        SyntaxDialogueContentProjection,
        Vec<PendingExpressionComponent>,
        bool,
    ) {
        (self.projection, self.components, self.has_real_atom)
    }
}

pub(super) fn emit_dialogue_content(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    missing_boundary: SyntaxDialogueContentRecoveryBoundary,
) -> EmittedDialogueContent {
    let content_end = parser
        .offset_at_token_boundary(end)
        .expect("dialogue content end is a lexer boundary");
    parser.start(SyntaxKind::DialogueContent, SyntaxRole::Content);
    let mut state = DialogueContentState::default();
    emit_dialogue_content_nodes(parser, end, content_end, &mut state);

    parser.finish();

    let projection = if state.saw_nontrivia {
        SyntaxDialogueContentProjection::Present(SyntaxDialogueContent::new(state.nodes))
    } else {
        SyntaxDialogueContentProjection::Missing {
            boundary: missing_boundary,
        }
    };
    EmittedDialogueContent {
        projection,
        components: state.components,
        has_real_atom: state.has_real_atom,
    }
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "these independent parser-transaction flags preserve distinct limit and recovery states"
)]
#[derive(Default)]
struct DialogueContentState {
    nodes: Vec<SyntaxDialogueNodeProjection>,
    components: Vec<PendingExpressionComponent>,
    point_action_count: usize,
    argument_count: usize,
    action_limit_exhausted: bool,
    argument_limit_exhausted: bool,
    has_real_atom: bool,
    saw_nontrivia: bool,
}

fn emit_dialogue_content_nodes(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    content_end: usize,
    state: &mut DialogueContentState,
) {
    while parser.cursor() < end {
        emit_dialogue_content_node(parser, end, content_end, state);
    }
}

fn emit_dialogue_content_node(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    content_end: usize,
    state: &mut DialogueContentState,
) {
    if emit_dialogue_trivia(parser, state) {
        return;
    }
    let start = parser.current_offset();
    state.saw_nontrivia = true;
    if emit_action_after_content_limit(
        parser,
        start,
        content_end,
        state.point_action_count,
        &mut state.action_limit_exhausted,
        &mut state.nodes,
        &mut state.components,
    ) {
        state.has_real_atom = true;
        return;
    }
    if emit_typed_dialogue_surface(
        parser,
        start,
        content_end,
        DialogueContentProjectionState {
            point_action_count: &mut state.point_action_count,
            argument_count: &mut state.argument_count,
            action_limit_exhausted: &mut state.action_limit_exhausted,
            argument_limit_exhausted: &mut state.argument_limit_exhausted,
            nodes: &mut state.nodes,
            components: &mut state.components,
        },
    ) || emit_overlong_action(
        parser,
        start,
        content_end,
        &mut state.nodes,
        &mut state.components,
    ) {
        state.has_real_atom = true;
        return;
    }
    emit_authored_or_plain_dialogue_content(parser, end, content_end, start, state);
}

fn emit_dialogue_trivia(
    parser: &mut DocumentParser<'_, '_>,
    state: &mut DialogueContentState,
) -> bool {
    if !parser
        .current_kind()
        .is_some_and(super::cursor::is_trivia_kind)
    {
        return false;
    }
    if parser.current_kind() == Some(SyntaxKind::NewlineToken) {
        let range = parser
            .current()
            .expect("dialogue newline remains inside the content interval")
            .range();
        emit_line_break_node(
            parser,
            &mut state.nodes,
            &mut state.components,
            range,
            SyntaxLineBreakKind::Line,
        );
        state.has_real_atom = true;
        state.saw_nontrivia = true;
    } else {
        let _ = parser
            .bump()
            .expect("dialogue trivia remains inside the content interval");
    }
    true
}

fn emit_authored_or_plain_dialogue_content(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    content_end: usize,
    start: usize,
    state: &mut DialogueContentState,
) {
    let Some(surface) = DialogueActionSurface::scan(parser, content_end) else {
        state.has_real_atom |= emit_plain_dialogue_content(
            parser,
            start,
            end,
            &mut state.nodes,
            &mut state.components,
        );
        return;
    };
    if parser.token_boundary_index(surface.end).is_none() {
        let range = parser
            .current()
            .expect("unpartitionable RichText surface retains one token")
            .range();
        emit_error_node(
            parser,
            range,
            SyntaxDialogueContentIssue::UnclassifiedToken,
            &mut state.nodes,
            &mut state.components,
        );
        return;
    }
    emit_point_action_or_error(
        parser,
        start,
        &surface,
        &mut state.argument_count,
        &mut state.argument_limit_exhausted,
        &mut state.nodes,
        &mut state.components,
    );
    state.point_action_count = state
        .point_action_count
        .checked_add(1)
        .expect("dialogue point-action count remains grammar-bounded");
    state.has_real_atom = true;
}

fn emit_plain_dialogue_content(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    end: usize,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    let plain_end = next_dialogue_surface_start(parser, end);
    let range = SourceRange::new(start, plain_end);
    let source = parser
        .source()
        .get(range.as_range())
        .expect("plain dialogue text remains inside its source");
    if is_real_dialogue_text(source) {
        emit_text_node(parser, range, source.into(), nodes, components);
        true
    } else {
        emit_error_node(
            parser,
            range,
            SyntaxDialogueContentIssue::UnclassifiedToken,
            nodes,
            components,
        );
        false
    }
}

fn emit_point_action_or_error(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    surface: &DialogueActionSurface<'_>,
    argument_count: &mut usize,
    argument_limit_exhausted: &mut bool,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    let whole = SourceRange::new(start, surface.end);
    let open = &surface.open;

    let scanned =
        (!open.attrs.is_empty() && !matches!(open.source_name, "call" | "at")).then(|| {
            let remaining = MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL
                .checked_sub(*argument_count)
                .expect("dialogue point-action argument count remains bounded");
            scan_dialogue_action_arguments(open.attrs, open.attrs_range.start(), remaining)
        });
    if let Some(scanned) = scanned.as_ref()
        && let Some(diagnostic) = scanned.diagnostics().iter().find(|diagnostic| {
            matches!(
                diagnostic.code(),
                DialogueTextDiagnosticCode::DialogueActionArgumentTooMany
                    | DialogueTextDiagnosticCode::DialogueActionArgumentKeyTooLong
                    | DialogueTextDiagnosticCode::DialogueActionArgumentValueTooLong
                    | DialogueTextDiagnosticCode::DialogueActionArgumentLimit
            )
        })
    {
        if diagnostic.code() != DialogueTextDiagnosticCode::DialogueActionArgumentLimit
            || !core::mem::replace(argument_limit_exhausted, true)
        {
            parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                diagnostic.code().as_str(),
                source_range(*diagnostic.range()),
                diagnostic.message(),
            )));
        }
        emit_error_node(
            parser,
            whole,
            SyntaxDialogueContentIssue::InvalidPointAction,
            nodes,
            components,
        );
        return;
    }

    let (identity, arguments, payload, extra_components) =
        point_action_identity_and_payload(parser, open, ordinal, scanned.as_ref());
    let is_marker = matches!(&identity, SyntaxDialoguePointActionIdentity::Mark(_));
    if matches!(identity, SyntaxDialoguePointActionIdentity::Invalid(_)) {
        emit_error_node(
            parser,
            whole,
            SyntaxDialogueContentIssue::InvalidPointAction,
            nodes,
            components,
        );
        return;
    }
    *argument_count = argument_count
        .checked_add(arguments.len())
        .expect("dialogue point-action argument count remains bounded");
    emit_point_action_syntax(parser, surface, open, ordinal, scanned.as_ref(), payload);
    nodes.push(SyntaxDialogueNodeProjection::PointAction(
        SyntaxDialoguePointActionProjection::new(identity, arguments, payload),
    ));
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::PointAction,
            },
            whole,
        ),
    ]);
    components.extend(extra_components);
    components.extend(point_action_components(
        ordinal, whole, surface, open, payload,
    ));
    if let Some(scanned) = scanned.as_ref()
        && !is_marker
    {
        components.extend(point_action_argument_components(ordinal, scanned.entries()));
    }
}

fn point_action_components(
    ordinal: u32,
    whole: SourceRange,
    surface: &DialogueActionSurface<'_>,
    open: &OpenActionSurface<'_>,
    payload: SyntaxDialoguePointActionPayload,
) -> Vec<PendingExpressionComponent> {
    let close_start = surface
        .end
        .checked_sub(']'.len_utf8())
        .expect("point action close remains inside its surface");
    let mut components = vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::Whole,
            },
            whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::OpenDelimiter,
            },
            SourceRange::new(surface.start, surface.start + '['.len_utf8()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::Name,
            },
            source_range(open.name_range),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::CloseDelimiter,
            },
            SourceRange::new(close_start, surface.end),
        ),
    ];
    if !matches!(payload, SyntaxDialoguePointActionPayload::None) {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::DialoguePointAction {
                ordinal,
                part: SyntaxDialoguePointActionSourcePart::Payload,
            },
            source_range(open.attrs_range),
        ));
    }
    if matches!(payload, SyntaxDialoguePointActionPayload::TimedCue(_))
        && let Some(duration) = open
            .timed_cue
            .as_ref()
            .and_then(|timed| timed.duration.as_ref())
    {
        components.extend([
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialoguePointActionArgument {
                    action: ordinal,
                    argument: 0,
                    part: SyntaxDialogueActionArgumentSourcePart::Whole,
                },
                source_range(duration.token_range()),
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialoguePointActionArgument {
                    action: ordinal,
                    argument: 0,
                    part: SyntaxDialogueActionArgumentSourcePart::Value,
                },
                source_range(duration.content_range()),
            ),
        ]);
    }
    components
}

fn point_action_identity_and_payload(
    parser: &DocumentParser<'_, '_>,
    open: &OpenActionSurface<'_>,
    ordinal: u32,
    scanned: Option<&ScannedDialogueActionArguments>,
) -> (
    SyntaxDialoguePointActionIdentity,
    Vec<SyntaxDialogueActionArgumentProjection>,
    SyntaxDialoguePointActionPayload,
    Vec<PendingExpressionComponent>,
) {
    if open.source_name == "mark" {
        let (identity, components) =
            marker_identity(parser, scanned, source_range(open.name_range), ordinal);
        return (
            identity,
            Vec::new(),
            SyntaxDialoguePointActionPayload::None,
            components,
        );
    }
    if open.source_name == "call" {
        return (
            SyntaxDialoguePointActionIdentity::Host(SyntaxRichTextHostEvent::Call),
            Vec::new(),
            SyntaxDialoguePointActionPayload::Call(SyntaxExpressionSlot::Authored),
            Vec::new(),
        );
    }
    if open.source_name == "at" {
        let arguments = open
            .timed_cue
            .as_ref()
            .and_then(|timed| timed.duration.as_ref())
            .map(|duration| {
                vec![SyntaxDialogueActionArgumentProjection::Positional {
                    value: SyntaxDialogueActionValue::new(duration.decoded()),
                }]
            })
            .unwrap_or_default();
        return (
            SyntaxDialoguePointActionIdentity::Host(SyntaxRichTextHostEvent::TimedCue),
            arguments,
            SyntaxDialoguePointActionPayload::TimedCue(SyntaxExpressionSlot::Authored),
            Vec::new(),
        );
    }
    if let Some(control) = SyntaxDialogueControl::from_source_name(open.source_name) {
        let arguments = scanned
            .map(|scanned| {
                scanned
                    .entries()
                    .iter()
                    .map(|argument| syntax_argument(parser.source(), argument))
                    .collect()
            })
            .unwrap_or_default();
        return (
            SyntaxDialoguePointActionIdentity::Control(control),
            arguments,
            SyntaxDialoguePointActionPayload::None,
            Vec::new(),
        );
    }
    let Some(host) = host_event(open.source_name) else {
        return (
            SyntaxDialoguePointActionIdentity::Invalid(SyntaxRichTextIssue::InvalidPayload),
            Vec::new(),
            SyntaxDialoguePointActionPayload::None,
            Vec::new(),
        );
    };
    let arguments = scanned
        .map(|scanned| {
            scanned
                .entries()
                .iter()
                .map(|argument| syntax_argument(parser.source(), argument))
                .collect()
        })
        .unwrap_or_default();
    (
        SyntaxDialoguePointActionIdentity::Host(host),
        arguments,
        SyntaxDialoguePointActionPayload::None,
        Vec::new(),
    )
}

fn host_event(source: &str) -> Option<SyntaxRichTextHostEvent> {
    Some(match source {
        "voice" => SyntaxRichTextHostEvent::Voice,
        "face" => SyntaxRichTextHostEvent::Face,
        "pose" => SyntaxRichTextHostEvent::Pose,
        "show" => SyntaxRichTextHostEvent::Show,
        "hide" => SyntaxRichTextHostEvent::Hide,
        "move" => SyntaxRichTextHostEvent::Move,
        "scale" => SyntaxRichTextHostEvent::Scale,
        "rotate" => SyntaxRichTextHostEvent::Rotate,
        "anim" => SyntaxRichTextHostEvent::Animation,
        "shake" => SyntaxRichTextHostEvent::StageShake,
        "signal" => SyntaxRichTextHostEvent::Signal,
        _ => return None,
    })
}

fn emit_point_action_syntax(
    parser: &mut DocumentParser<'_, '_>,
    surface: &DialogueActionSurface<'_>,
    open: &OpenActionSurface<'_>,
    ordinal: u32,
    scanned: Option<&ScannedDialogueActionArguments>,
    payload: SyntaxDialoguePointActionPayload,
) {
    let kind = match payload {
        SyntaxDialoguePointActionPayload::None if open.source_name == "mark" => {
            SyntaxKind::DialogueMark
        }
        SyntaxDialoguePointActionPayload::Call(_)
        | SyntaxDialoguePointActionPayload::TimedCue(_)
        | SyntaxDialoguePointActionPayload::None => SyntaxKind::DialogueControl,
    };
    parser.start(kind, SyntaxRole::DialogueNode(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    bump_to_range_start(parser, open.name_range);
    emit_range_node(
        parser,
        SyntaxKind::DialoguePointActionName,
        SyntaxRole::Name,
        open.name_range,
    );
    match payload {
        SyntaxDialoguePointActionPayload::Call(_) => {
            let range = open.attrs_range;
            let _ = emit_expression_payload(
                parser,
                range,
                SyntaxKind::DialogueActionDialogueCallPayload,
                SyntaxRole::Operand,
            );
        }
        SyntaxDialoguePointActionPayload::TimedCue(_) => {
            let timed = open
                .timed_cue
                .as_ref()
                .expect("timed-cue actions retain their scanned payload");
            emit_timed_point_payload(parser, open, timed, ordinal);
        }
        SyntaxDialoguePointActionPayload::None => {
            if let Some(scanned) = scanned {
                let mut point_argument_limit_exhausted = false;
                emit_argument_payload(
                    parser,
                    open.attrs_range,
                    scanned,
                    surface.unterminated_quote.is_some(),
                    &mut point_argument_limit_exhausted,
                );
            }
        }
    }
    emit_point_action_close(parser, surface);
}

fn emit_timed_point_payload(
    parser: &mut DocumentParser<'_, '_>,
    open: &OpenActionSurface<'_>,
    timed: &ScannedTimedCuePayload,
    ordinal: u32,
) {
    parser.start(
        SyntaxKind::DialogueActionTimedCuePayload,
        SyntaxRole::Payload,
    );
    if let Some(duration) = timed.duration.as_ref() {
        bump_to_range_start(parser, duration.token_range());
        parser.start(
            SyntaxKind::DialogueActionPositionalArgument,
            SyntaxRole::Argument(0),
        );
        let mut cursor = PartitionedEventCursor::new(parser, duration.token_range().start());
        emit_present_value(&mut cursor, duration);
        cursor.finish_at(duration.token_range().end());
        parser.finish();
    }
    let call_range = timed.call.filter(|_| !timed.malformed);
    let call = match call_range {
        Some(range) => emit_expression_payload(
            parser,
            range,
            SyntaxKind::DialogueActionDialogueCallPayload,
            SyntaxRole::Operand,
        ),
        None => emit_expression_payload(
            parser,
            TextRange::new(open.attrs_range.end(), open.attrs_range.end()),
            SyntaxKind::DialogueActionDialogueCallPayload,
            SyntaxRole::Operand,
        ),
    };
    let _ = (ordinal, call);
    bump_until_offset(parser, open.attrs_range.end());
    parser.finish();
}

fn emit_point_action_close(
    parser: &mut DocumentParser<'_, '_>,
    surface: &DialogueActionSurface<'_>,
) {
    let close = surface
        .end
        .checked_sub(']'.len_utf8())
        .expect("point action closing delimiter follows its opening delimiter");
    bump_until_offset(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.dialogue.point_action.missing_close",
    );
    emit_unterminated_quote_diagnostic(parser, surface.unterminated_quote);
    parser.finish();
}

fn next_dialogue_surface_start(parser: &DocumentParser<'_, '_>, end: usize) -> usize {
    let content_end = parser
        .offset_at_token_boundary(end)
        .expect("dialogue content end is a lexer boundary");
    let next = parser
        .cursor()
        .checked_add(1)
        .expect("dialogue token cursor remains representable");
    (next..end)
        .filter_map(|index| parser.token_at(index))
        .map(|token| token.range().start())
        .find(|start| {
            scan_dialogue_surface(parser.source(), *start, content_end).is_some()
                || parser.source()[*start..].starts_with('[')
        })
        .unwrap_or(content_end)
}

fn is_real_dialogue_text(source: &str) -> bool {
    source.chars().any(|character| {
        character.is_alphabetic()
            || character == '_'
            || (!character.is_ascii() && !character.is_numeric() && !character.is_whitespace())
    })
}

fn emit_text_node(
    parser: &mut DocumentParser<'_, '_>,
    range: SourceRange,
    decoded: Box<str>,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    parser.start(SyntaxKind::DialogueText, SyntaxRole::DialogueNode(ordinal));
    let mut cursor = PartitionedEventCursor::new(parser, range.start());
    cursor.emit_to(range.end());
    cursor.finish_at(range.end());
    parser.finish();
    nodes.push(SyntaxDialogueNodeProjection::Text(decoded));
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Text,
            },
            range,
        ),
    ]);
}

fn emit_error_node(
    parser: &mut DocumentParser<'_, '_>,
    range: SourceRange,
    issue: SyntaxDialogueContentIssue,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    parser.start(SyntaxKind::DialogueError, SyntaxRole::DialogueNode(ordinal));
    let mut cursor = PartitionedEventCursor::new(parser, range.start());
    cursor.emit_to(range.end());
    cursor.finish_at(range.end());
    parser.finish();
    nodes.push(SyntaxDialogueNodeProjection::Error(issue));
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Error,
            },
            range,
        ),
    ]);
}

fn emit_line_break_node(
    parser: &mut DocumentParser<'_, '_>,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
    range: SourceRange,
    kind: SyntaxLineBreakKind,
) {
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    parser.start(
        SyntaxKind::DialogueLineBreak,
        SyntaxRole::DialogueNode(ordinal),
    );
    let _ = parser
        .bump()
        .expect("dialogue line break retains its authored token");
    parser.finish();
    nodes.push(SyntaxDialogueNodeProjection::LineBreak(kind));
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            range,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::LineBreak,
            },
            range,
        ),
    ]);
}

fn emit_scanned_surface(
    parser: &mut DocumentParser<'_, '_>,
    surface: &ScannedDialogueSurface,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let whole = SourceRange::new(surface.range().start(), surface.range().end());
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    match surface.kind() {
        ScannedDialogueSurfaceKind::Escape { escaped, value, .. } => {
            emit_scanned_escape(parser, *escaped, *value, ordinal, whole, nodes, components);
        }
        ScannedDialogueSurfaceKind::Ruby(ruby) => {
            emit_scanned_ruby(parser, ruby, ordinal, whole, nodes, components);
        }
        ScannedDialogueSurfaceKind::Interpolation {
            open,
            payload,
            close,
            ..
        } => {
            emit_scanned_interpolation(
                parser,
                ScannedInterpolationSource {
                    open: *open,
                    payload: *payload,
                    close: *close,
                    whole,
                },
                ordinal,
                nodes,
                components,
            );
        }
        ScannedDialogueSurfaceKind::ContentApplication {
            hash,
            head,
            callee,
            body,
        } => {
            let source = ScannedContentApplicationSource {
                hash: *hash,
                head: *head,
                callee: callee.clone(),
                body: *body,
                whole,
            };
            emit_scanned_content_application(parser, &source, ordinal, nodes, components);
        }
    }
}

fn emit_scanned_escape(
    parser: &mut DocumentParser<'_, '_>,
    escaped: TextRange,
    value: char,
    ordinal: u32,
    whole: SourceRange,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    emit_dialogue_range_owner(
        parser,
        SyntaxKind::DialogueEscape,
        SyntaxRole::DialogueNode(ordinal),
        whole,
    );
    nodes.push(SyntaxDialogueNodeProjection::Escape(value));
    components.extend(dialogue_node_components(
        ordinal,
        whole,
        SyntaxDialogueNodeSourcePart::Escape,
        SourceRange::new(escaped.start(), escaped.end()),
    ));
}

fn emit_scanned_ruby(
    parser: &mut DocumentParser<'_, '_>,
    ruby: &crate::text::ScannedDialogueRuby,
    ordinal: u32,
    whole: SourceRange,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    emit_dialogue_range_owner(
        parser,
        SyntaxKind::DialogueRuby,
        SyntaxRole::DialogueNode(ordinal),
        whole,
    );
    nodes.push(SyntaxDialogueNodeProjection::Ruby {
        base: ruby.base().value().into(),
        ruby: ruby.ruby().value().into(),
    });
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Ruby,
            },
            whole,
        ),
    ]);
}

#[derive(Clone, Copy)]
struct ScannedInterpolationSource {
    open: TextRange,
    payload: TextRange,
    close: TextRange,
    whole: SourceRange,
}

#[derive(Clone)]
struct ScannedContentApplicationSource {
    hash: TextRange,
    head: TextRange,
    callee: ScannedContentApplicationCallee,
    body: Option<crate::text::ScannedContentApplicationBody>,
    whole: SourceRange,
}

#[allow(
    clippy::too_many_lines,
    reason = "the content-application owner emits one complete hash target/body projection and its exact component inventory"
)]
fn emit_scanned_content_application(
    parser: &mut DocumentParser<'_, '_>,
    source: &ScannedContentApplicationSource,
    ordinal: u32,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let owner = parser.start_projected_owner(
        SyntaxKind::AttachedContentApplicationExpression,
        SyntaxRole::DialogueNode(ordinal),
    );
    debug_assert!(parser.at("#"));
    parser.bump();

    let head_end = parser
        .token_boundary_index(source.head.end())
        .expect("content escape head ends at a lexer boundary");
    let target = emit_expression_node(parser, head_end, SyntaxRole::Target);

    let (content, mut body_components) = if let Some(body) = source.body {
        let open = body.open();
        debug_assert_eq!(parser.current_offset(), open.start());
        emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
        parser.start(SyntaxKind::PostfixBracketPayload, SyntaxRole::Payload);
        let is_raw_literal = is_raw_content_target(parser, target, &source.callee);
        let (content, nested_components) = if is_raw_literal {
            let range = source_range(body.content());
            parser.start(SyntaxKind::DialogueContent, SyntaxRole::Content);
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.emit_to(range.end());
            cursor.finish_at(range.end());
            parser.finish();
            (
                SyntaxDialogueContentProjection::RawLiteral(SyntaxRawLiteralBody::new(
                    parser
                        .source()
                        .get(range.as_range())
                        .expect("raw literal body remains inside source"),
                    range,
                )),
                Vec::new(),
            )
        } else {
            let content_end = parser
                .token_boundary_index(body.content().end())
                .expect("content escape body ends at a lexer boundary");
            let missing_boundary = match body.close() {
                Some(close) => SyntaxDialogueContentRecoveryBoundary::CloseBracket {
                    range: source_range(close),
                },
                None => SyntaxDialogueContentRecoveryBoundary::MissingBracketClose {
                    insertion: body.content().end(),
                },
            };
            let emitted = emit_dialogue_content(parser, content_end, missing_boundary);
            let (content, nested_components, _) = emitted.into_parts();
            (content, nested_components)
        };
        parser.finish();
        let close = body.close().map_or_else(
            || {
                let at = SourceRange::new(body.content().end(), body.content().end());
                emit_missing_content_application_close(parser, at.start());
                at
            },
            source_range,
        );
        if body.close().is_some() {
            emit_close_delimiter(
                parser,
                SyntaxKind::CloseBracketNode,
                "]",
                "syntax.expression.missing_postfix_bracket_close",
            );
        }
        let mut components = vec![
            PendingExpressionComponent::new(
                ExpressionComponentRole::OpenBracket,
                source_range(open),
            ),
            PendingExpressionComponent::new(ExpressionComponentRole::CloseBracket, close),
            PendingExpressionComponent::new(
                ExpressionComponentRole::Content,
                source_range(body.content()),
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::ContentBody,
                source_range(body.content()),
            ),
        ];
        components.extend(nested_components);
        (content, components)
    } else {
        (
            SyntaxDialogueContentProjection::Missing {
                boundary: SyntaxDialogueContentRecoveryBoundary::Inline {
                    insertion: source.head.end(),
                },
            },
            Vec::new(),
        )
    };

    // This slot belongs to the already-started hash wrapper.  The target's
    // own slot is intentionally not published as the dialogue node slot:
    // attachment must select the wrapper expression, not its inner call.
    nodes.push(SyntaxDialogueNodeProjection::ContentApplication(
        SyntaxExpressionSlot::Authored,
    ));
    let mut outer = vec![
        PendingExpressionComponent::new(ExpressionComponentRole::Hash, source_range(source.hash)),
        PendingExpressionComponent::new(ExpressionComponentRole::Target, source_range(source.head)),
    ];
    outer.append(&mut body_components);
    parser.set_expression_projection(
        owner,
        crate::expressions::PendingExpressionProjection::new(
            ExpressionProjection::AttachedContentApplication(
                crate::expressions::SyntaxAttachedContentApplicationProjection::new(
                    crate::expressions::SyntaxAttachedContentApplicationForm::Hash,
                    content,
                    false,
                ),
            ),
            outer,
        ),
    );
    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            source.whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Hash,
            },
            source_range(source.hash),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Expression,
            },
            source_range(source.head),
        ),
    ]);
    parser.finish();
}

fn is_raw_content_target(
    parser: &DocumentParser<'_, '_>,
    target: super::expression::CompletedNode,
    callee: &ScannedContentApplicationCallee,
) -> bool {
    let ScannedContentApplicationCallee::ImplicitRoot(name) = callee else {
        return false;
    };
    let raw = crate::name::SyntaxName::try_new("raw")
        .expect("the built-in raw content callable has a valid syntax name");
    if name != &raw {
        return false;
    }
    let Some(range) = parser.completed_range(target.start_event) else {
        return false;
    };
    let Some(projection) = parser.expression_projection_for_range(range) else {
        return false;
    };
    let crate::expressions::ExpressionProjection::Call(
        crate::expressions::SyntaxCallProjection::Parenthesized(call),
    ) = projection.projection()
    else {
        return false;
    };
    !projection.has_recovery()
        && call.arguments().is_empty()
        && call.terminator() == crate::expressions::SyntaxCallArgumentListTerminator::Closed
}

fn emit_missing_content_application_close(parser: &mut DocumentParser<'_, '_>, at: usize) {
    parser.start(SyntaxKind::CloseBracketNode, SyntaxRole::CloseDelimiter);
    parser.push(SyntaxEvent::MissingToken {
        expected: ExpectedToken::try_with_spelling(SyntaxKind::PunctuationToken, "]")
            .expect("real grammar punctuation token"),
        at,
    });
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        "syntax.expression.missing_postfix_bracket_close",
        SourceRange::new(at, at),
        "missing closing `]`",
    )));
    parser.finish();
}

fn emit_scanned_interpolation(
    parser: &mut DocumentParser<'_, '_>,
    source: ScannedInterpolationSource,
    ordinal: u32,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    parser.start(
        SyntaxKind::DialogueInterpolation,
        SyntaxRole::DialogueNode(ordinal),
    );
    emit_range_node(
        parser,
        SyntaxKind::OpenBracketNode,
        SyntaxRole::OpenDelimiter,
        source.open,
    );
    let expression_end = parser
        .token_boundary_index(source.payload.end())
        .expect("interpolation payload ends at a lexer boundary");
    let expression = emit_expression_node(parser, expression_end, SyntaxRole::Operand);
    let slot = completed_slot(parser, expression);
    bump_until_offset(parser, source.close.start());
    emit_range_node(
        parser,
        SyntaxKind::CloseBracketNode,
        SyntaxRole::CloseDelimiter,
        source.close,
    );
    parser.finish();
    nodes.push(SyntaxDialogueNodeProjection::Interpolation(slot));
    components.extend(dialogue_node_components(
        ordinal,
        source.whole,
        SyntaxDialogueNodeSourcePart::Interpolation,
        SourceRange::new(source.payload.start(), source.payload.end()),
    ));
}

fn emit_dialogue_range_owner(
    parser: &mut DocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    range: SourceRange,
) {
    parser.start(kind, role);
    let mut cursor = PartitionedEventCursor::new(parser, range.start());
    cursor.emit_to(range.end());
    cursor.finish_at(range.end());
    parser.finish();
}

fn dialogue_node_components(
    ordinal: u32,
    whole: SourceRange,
    part: SyntaxDialogueNodeSourcePart,
    part_range: SourceRange,
) -> [PendingExpressionComponent; 2] {
    [
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode { ordinal, part },
            part_range,
        ),
    ]
}

#[derive(Clone, Copy)]
enum DialogueContentLimit {
    PointActions,
    Arguments,
}

fn emit_action_after_content_limit(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    content_action_count: usize,
    action_limit_exhausted: &mut bool,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    if !parser.at("[")
        || (!*action_limit_exhausted && content_action_count < MAX_DIALOGUE_POINT_ACTIONS)
    {
        return false;
    }
    if let Some(boundary) = find_dialogue_bracket_boundary(parser.source(), start, content_end) {
        if !core::mem::replace(action_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                DialogueContentLimit::PointActions,
                SourceRange::new(start, boundary.end()),
            );
        }
        let range = SourceRange::new(start, boundary.end());
        let text = parser.source()[range.as_range()].into();
        emit_text_node(parser, range, text, nodes, components);
    } else {
        let _ = parser.bump();
    }
    true
}

struct DialogueContentProjectionState<'a> {
    point_action_count: &'a mut usize,
    argument_count: &'a mut usize,
    action_limit_exhausted: &'a mut bool,
    argument_limit_exhausted: &'a mut bool,
    nodes: &'a mut Vec<SyntaxDialogueNodeProjection>,
    components: &'a mut Vec<PendingExpressionComponent>,
}

fn emit_typed_dialogue_surface(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    state: DialogueContentProjectionState<'_>,
) -> bool {
    let DialogueContentProjectionState {
        point_action_count,
        argument_count,
        action_limit_exhausted,
        argument_limit_exhausted,
        nodes,
        components,
    } = state;
    let Some(surface) = scan_dialogue_surface(parser.source(), start, content_end) else {
        return false;
    };
    let action_overflow =
        surface.point_actions() > MAX_DIALOGUE_POINT_ACTIONS.saturating_sub(*point_action_count);
    let argument_overflow = surface.action_arguments()
        > MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL.saturating_sub(*argument_count);
    if action_overflow {
        if !core::mem::replace(action_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                DialogueContentLimit::PointActions,
                SourceRange::new(start, surface.end()),
            );
        }
        let range = SourceRange::new(start, surface.end());
        let text = parser.source()[range.as_range()].into();
        emit_text_node(parser, range, text, nodes, components);
    } else if argument_overflow {
        if !core::mem::replace(argument_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                DialogueContentLimit::Arguments,
                SourceRange::new(start, surface.end()),
            );
        }
        let range = SourceRange::new(start, surface.end());
        let text = parser.source()[range.as_range()].into();
        emit_text_node(parser, range, text, nodes, components);
    } else {
        *point_action_count += surface.point_actions();
        *argument_count += surface.action_arguments();
        emit_scanned_surface(parser, &surface, nodes, components);
    }
    true
}

fn emit_overlong_action(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    if !parser.at("[") {
        return false;
    }
    let Some(boundary) = find_dialogue_bracket_boundary(parser.source(), start, content_end) else {
        return false;
    };
    let body_start = start
        .checked_add('['.len_utf8())
        .expect("point-action body starts after its opening delimiter");
    let inside = &parser.source()[body_start..boundary.close()];
    if inside.len() <= MAX_DIALOGUE_ACTION_HEAD_BYTES {
        return false;
    }
    let limit = utf8_boundary_at_or_before(inside, MAX_DIALOGUE_ACTION_HEAD_BYTES);
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        DialogueTextDiagnosticCode::DialogueActionHeadTooLong.as_str(),
        SourceRange::new(
            body_start
                .checked_add(limit)
                .expect("point-action limit offset remains representable"),
            boundary.close(),
        ),
        format!("dialogue point-action head exceeds {MAX_DIALOGUE_ACTION_HEAD_BYTES} bytes"),
    )));
    let range = SourceRange::new(start, boundary.end());
    let text = parser.source()[range.as_range()].into();
    emit_text_node(parser, range, text, nodes, components);
    true
}

fn emit_content_limit_diagnostic(
    parser: &mut DocumentParser<'_, '_>,
    limit: DialogueContentLimit,
    range: SourceRange,
) {
    let (code, message) = match limit {
        DialogueContentLimit::PointActions => (
            DialogueTextDiagnosticCode::DialoguePointActionLimit.as_str(),
            format!("dialogue content has more than {MAX_DIALOGUE_POINT_ACTIONS} point actions"),
        ),
        DialogueContentLimit::Arguments => (
            DialogueTextDiagnosticCode::DialogueActionArgumentLimit.as_str(),
            format!(
                "dialogue content has more than {MAX_DIALOGUE_ACTION_ARGUMENTS_TOTAL} RichText arguments"
            ),
        ),
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code, range, message,
    )));
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DialogueActionSurface<'source> {
    start: usize,
    end: usize,
    unterminated_quote: Option<TextRange>,
    open: OpenActionSurface<'source>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenActionSurface<'source> {
    source_name: &'source str,
    name_range: TextRange,
    attrs: &'source str,
    attrs_range: TextRange,
    timed_cue: Option<ScannedTimedCuePayload>,
}

impl<'source> DialogueActionSurface<'source> {
    fn scan(parser: &DocumentParser<'source, '_>, content_end: usize) -> Option<Self> {
        parser.at("[").then_some(())?;
        let open = parser.current_offset();
        let boundary = find_dialogue_bracket_boundary(parser.source(), open, content_end)?;
        let close = boundary.close();
        let end = boundary.end();
        let unterminated_quote = boundary
            .unterminated_quote_start()
            .map(|start| TextRange::new(start, end));
        let body_start = open.checked_add('['.len_utf8())?;
        let inside_source = parser.source().get(body_start..close)?;
        let inside = trim_dialogue_action_whitespace(inside_source);
        if inside.is_empty() {
            return None;
        }
        let inside_start = body_start.checked_add(subslice_offset(inside_source, inside))?;

        let (source_name, attrs) = split_action_head(inside);
        let name_start = inside_start.checked_add(subslice_offset(inside, source_name))?;
        if source_name.is_empty() {
            return None;
        }
        let attrs_start = inside_start.checked_add(subslice_offset(inside, attrs))?;
        Some(Self {
            start: open,
            end,
            unterminated_quote,
            open: OpenActionSurface {
                source_name,
                name_range: TextRange::new(name_start, name_start.checked_add(source_name.len())?),
                attrs,
                attrs_range: TextRange::new(attrs_start, attrs_start.checked_add(attrs.len())?),
                timed_cue: (source_name == "at").then(|| {
                    scan_timed_cue_payload(
                        parser,
                        TextRange::new(
                            attrs_start,
                            attrs_start
                                .checked_add(attrs.len())
                                .expect("RichText argument range remains representable"),
                        ),
                    )
                }),
            },
        })
    }
}

/// The one parser-selected decomposition of an inline timed-cue payload.
///
/// The classification is performed over the already lexed token topology. In
/// particular, a `call=` token pair nested inside the call expression is not a
/// second timed-cue field. The attached CST then owns the selected duration and
/// call payload without requiring a source-string split or a second expression
/// parse.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ScannedTimedCuePayload {
    duration: Option<ScannedDialogueActionArgumentValue>,
    call: Option<TextRange>,
    malformed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TimedCueDelimiterDepth {
    angle: usize,
    paren: usize,
    bracket: usize,
    brace: usize,
}

impl TimedCueDelimiterDepth {
    const fn is_top_level(self) -> bool {
        self.angle == 0 && self.paren == 0 && self.bracket == 0 && self.brace == 0
    }

    fn bump(&mut self, spelling: &str) {
        match spelling {
            "<" => self.angle = self.angle.saturating_add(1),
            ">" => self.angle = self.angle.saturating_sub(1),
            "(" => self.paren = self.paren.saturating_add(1),
            ")" => self.paren = self.paren.saturating_sub(1),
            "[" => self.bracket = self.bracket.saturating_add(1),
            "]" => self.bracket = self.bracket.saturating_sub(1),
            "{" => self.brace = self.brace.saturating_add(1),
            "}" => self.brace = self.brace.saturating_sub(1),
            _ => {}
        }
    }
}

fn scan_timed_cue_payload(
    parser: &DocumentParser<'_, '_>,
    range: TextRange,
) -> ScannedTimedCuePayload {
    let start = parser
        .token_boundary_index(range.start())
        .expect("timed-cue payload starts at a lexer boundary");
    let end = parser
        .token_boundary_index(range.end())
        .expect("timed-cue payload ends at a lexer boundary");
    let mut significant = Vec::new();
    let mut depth = TimedCueDelimiterDepth::default();
    for index in start..end {
        let token = parser
            .token_at(index)
            .expect("timed-cue token interval remains inside the document lexer");
        if is_timed_cue_trivia(token.kind()) {
            continue;
        }
        significant.push((index, depth));
        depth.bump(parser.text_of(token));
    }

    let call_heads = significant
        .windows(2)
        .enumerate()
        .filter_map(|(position, window)| {
            let (key_index, key_depth) = window[0];
            let (equals_index, _) = window[1];
            (key_depth.is_top_level()
                && parser.text_of(parser.token_at(key_index)?) == "call"
                && parser.text_of(parser.token_at(equals_index)?) == "=")
                .then_some((position, key_index, equals_index))
        })
        .collect::<Vec<_>>();
    let Some((call_position, _key_index, equals_index)) = call_heads.first().copied() else {
        let duration = (significant.len() == 1)
            .then(|| timed_cue_duration(parser, significant[0].0))
            .flatten();
        let malformed = significant.len() > 1 || duration.is_none();
        return ScannedTimedCuePayload {
            duration,
            call: None,
            malformed,
        };
    };

    let duration = (call_position == 1)
        .then(|| timed_cue_duration(parser, significant[0].0))
        .flatten();
    let malformed = call_position != 1 || call_heads.len() != 1 || duration.is_none();
    let call_start = significant.get(call_position + 2).map_or_else(
        || token_end(parser, equals_index),
        |(index, _)| token_start(parser, *index),
    );
    let call = (!malformed)
        .then(|| (call_start < range.end()).then_some(TextRange::new(call_start, range.end())))
        .flatten();
    ScannedTimedCuePayload {
        duration,
        call,
        malformed: malformed || call.is_none(),
    }
}

fn timed_cue_duration(
    parser: &DocumentParser<'_, '_>,
    index: usize,
) -> Option<ScannedDialogueActionArgumentValue> {
    let token = parser
        .token_at(index)
        .expect("timed-cue duration remains a lexer token");
    scan_dialogue_action_argument_value_if_valid(parser.text_of(token), token.range().start())
}

fn token_start(parser: &DocumentParser<'_, '_>, index: usize) -> usize {
    parser
        .token_at(index)
        .expect("timed-cue token remains in the lexer interval")
        .range()
        .start()
}

fn token_end(parser: &DocumentParser<'_, '_>, index: usize) -> usize {
    parser
        .token_at(index)
        .expect("timed-cue token remains in the lexer interval")
        .range()
        .end()
}

fn is_timed_cue_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::WhitespaceToken
            | SyntaxKind::NewlineToken
            | SyntaxKind::CommentToken
            | SyntaxKind::DocCommentToken
    )
}

fn marker_identity(
    parser: &DocumentParser<'_, '_>,
    arguments: Option<&ScannedDialogueActionArguments>,
    fallback_range: SourceRange,
    action: u32,
) -> (
    SyntaxDialoguePointActionIdentity,
    Vec<PendingExpressionComponent>,
) {
    let Some(arguments) = arguments else {
        return (
            SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                SyntaxDialogueMarkNameIssue::MissingSuffix,
                fallback_range,
            )),
            Vec::new(),
        );
    };
    if arguments.entries().len() != 1 {
        let issue = if arguments.entries().is_empty() {
            SyntaxDialogueMarkNameIssue::MissingSuffix
        } else {
            SyntaxDialogueMarkNameIssue::MultipleArguments
        };
        return (
            SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                issue,
                arguments
                    .entries()
                    .first()
                    .map_or(fallback_range, |argument| source_range(argument.range())),
            )),
            Vec::new(),
        );
    }
    let argument = &arguments.entries()[0];
    match argument {
        ScannedDialogueActionArgument::Positional { value, .. } => {
            let range = source_range(value.token_range());
            if value.opening_quote_range().is_some() {
                return (
                    SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                        SyntaxDialogueMarkNameIssue::Quoted,
                        range,
                    )),
                    Vec::new(),
                );
            }
            let spelling = parser.source().get(range.as_range()).unwrap_or("");
            if !spelling.starts_with('@') {
                return (
                    SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                        SyntaxDialogueMarkNameIssue::MissingReference,
                        range,
                    )),
                    Vec::new(),
                );
            }
            let projection = typed_entity_reference_source(range, spelling);
            let identity =
                SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::from_reference(
                    projection.syntax().clone(),
                    range,
                    projection.components().to_vec(),
                ));
            let components = projection
                .components()
                .iter()
                .map(|component| {
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::DialoguePointAction {
                            ordinal: action,
                            part: SyntaxDialoguePointActionSourcePart::Marker(component.part()),
                        },
                        component.range(),
                    )
                })
                .collect();
            (identity, components)
        }
        ScannedDialogueActionArgument::Named { range, .. } => (
            SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                SyntaxDialogueMarkNameIssue::Attributed,
                source_range(*range),
            )),
            Vec::new(),
        ),
        ScannedDialogueActionArgument::Invalid { range, .. } => (
            SyntaxDialoguePointActionIdentity::Mark(SyntaxDialogueMarkName::recovered(
                SyntaxDialogueMarkNameIssue::Malformed,
                source_range(*range),
            )),
            Vec::new(),
        ),
    }
}

fn syntax_argument(
    source: &str,
    argument: &ScannedDialogueActionArgument,
) -> SyntaxDialogueActionArgumentProjection {
    match argument {
        ScannedDialogueActionArgument::Positional { value, .. } => {
            SyntaxDialogueActionArgumentProjection::Positional {
                value: SyntaxDialogueActionValue::new(value.decoded()),
            }
        }
        ScannedDialogueActionArgument::Named {
            name_range, value, ..
        } => {
            let name = SyntaxName::try_new(
                source
                    .get(name_range.as_range())
                    .expect("RichText argument name remains inside source"),
            );
            SyntaxDialogueActionArgumentProjection::Named {
                name,
                value: SyntaxDialogueActionValue::new(value.decoded()),
            }
        }
        ScannedDialogueActionArgument::Invalid { issue, parts, .. } => {
            SyntaxDialogueActionArgumentProjection::Invalid {
                issue: *issue,
                authored_parts: syntax_argument_parts(*parts),
            }
        }
    }
}

const fn syntax_argument_parts(
    parts: ScannedDialogueActionArgumentParts,
) -> SyntaxDialogueActionArgumentParts {
    SyntaxDialogueActionArgumentParts::new(
        parts.name().is_some(),
        parts.equals().is_some(),
        parts.value().is_some(),
    )
}

fn point_action_argument_components(
    action: u32,
    arguments: &[ScannedDialogueActionArgument],
) -> Vec<PendingExpressionComponent> {
    arguments
        .iter()
        .enumerate()
        .flat_map(|(ordinal, _)| {
            let argument = u16::try_from(ordinal).expect("dialogue action argument limit fits u16");
            let mut parts = vec![PendingExpressionComponent::new(
                ExpressionComponentRole::DialoguePointActionArgument {
                    action,
                    argument,
                    part: SyntaxDialogueActionArgumentSourcePart::Whole,
                },
                source_range(arguments[ordinal].range()),
            )];
            match &arguments[ordinal] {
                ScannedDialogueActionArgument::Positional { value, .. } => {
                    parts.push(PendingExpressionComponent::new(
                        ExpressionComponentRole::DialoguePointActionArgument {
                            action,
                            argument,
                            part: SyntaxDialogueActionArgumentSourcePart::Value,
                        },
                        source_range(value.content_range()),
                    ));
                }
                ScannedDialogueActionArgument::Named {
                    name_range,
                    equals_range,
                    value,
                    ..
                } => {
                    parts.extend([
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::DialoguePointActionArgument {
                                action,
                                argument,
                                part: SyntaxDialogueActionArgumentSourcePart::Name,
                            },
                            source_range(*name_range),
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::DialoguePointActionArgument {
                                action,
                                argument,
                                part: SyntaxDialogueActionArgumentSourcePart::Equals,
                            },
                            source_range(*equals_range),
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::DialoguePointActionArgument {
                                action,
                                argument,
                                part: SyntaxDialogueActionArgumentSourcePart::Value,
                            },
                            source_range(value.content_range()),
                        ),
                    ]);
                }
                ScannedDialogueActionArgument::Invalid {
                    parts: authored, ..
                } => {
                    for (part, range) in [
                        (
                            SyntaxDialogueActionArgumentSourcePart::Name,
                            authored.name(),
                        ),
                        (
                            SyntaxDialogueActionArgumentSourcePart::Equals,
                            authored.equals(),
                        ),
                        (
                            SyntaxDialogueActionArgumentSourcePart::Value,
                            authored.value(),
                        ),
                    ] {
                        if let Some(range) = range {
                            parts.push(PendingExpressionComponent::new(
                                ExpressionComponentRole::DialoguePointActionArgument {
                                    action,
                                    argument,
                                    part,
                                },
                                source_range(range),
                            ));
                        }
                    }
                }
            }
            parts
        })
        .collect()
}

const fn source_range(range: TextRange) -> SourceRange {
    SourceRange::new(range.start(), range.end())
}

fn emit_expression_payload(
    parser: &mut DocumentParser<'_, '_>,
    range: TextRange,
    kind: SyntaxKind,
    expression_role: SyntaxRole,
) -> SyntaxExpressionSlot {
    bump_to_range_start(parser, range);
    parser.start(kind, SyntaxRole::Payload);
    let end = parser
        .token_boundary_index(range.end())
        .expect("dedicated RichText payload ends at a lexer boundary");
    let expression = emit_expression_node(parser, end, expression_role);
    let slot = completed_slot(parser, expression);
    parser.finish();
    slot
}

fn emit_argument_payload(
    parser: &mut DocumentParser<'_, '_>,
    range: TextRange,
    scanned: &ScannedDialogueActionArguments,
    action_reports_unterminated_quote: bool,
    argument_limit_exhausted: &mut bool,
) {
    parser.start(
        SyntaxKind::DialogueActionArgumentPayload,
        SyntaxRole::Payload,
    );
    parser.start(
        SyntaxKind::DialogueActionArgumentList,
        SyntaxRole::Element(0),
    );
    for (ordinal, argument) in scanned.entries().iter().enumerate() {
        bump_to_range_start(parser, argument.range());
        emit_argument(
            parser,
            argument,
            u16::try_from(ordinal).expect("dialogue action argument limit fits u16"),
        );
    }
    bump_until_offset(parser, range.end());
    parser.finish();
    for diagnostic in scanned.diagnostics() {
        if action_reports_unterminated_quote
            && diagnostic.code()
                == DialogueTextDiagnosticCode::DialogueActionArgumentUnterminatedQuote
        {
            continue;
        }
        if diagnostic.code() == DialogueTextDiagnosticCode::DialogueActionArgumentLimit
            && core::mem::replace(argument_limit_exhausted, true)
        {
            continue;
        }
        let range = diagnostic.range();
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic.code().as_str(),
            SourceRange::new(range.start(), range.end()),
            diagnostic.message(),
        )));
    }
    parser.finish();
}

fn emit_argument(
    parser: &mut DocumentParser<'_, '_>,
    argument: &ScannedDialogueActionArgument,
    ordinal: u16,
) {
    match argument {
        ScannedDialogueActionArgument::Positional { value, range } => {
            parser.start(
                SyntaxKind::DialogueActionPositionalArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            emit_present_value(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedDialogueActionArgument::Named {
            name_range,
            equals_range,
            value,
            range,
        } => {
            parser.start(
                SyntaxKind::DialogueActionNamedArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.start(SyntaxKind::DialogueActionArgumentKey, SyntaxRole::Key);
            cursor.emit_to(name_range.end());
            cursor.finish();
            cursor.start(SyntaxKind::DialogueActionArgumentEquals, SyntaxRole::Equals);
            cursor.emit_to_as(equals_range.end(), SyntaxKind::PunctuationToken);
            cursor.finish();
            emit_present_value(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedDialogueActionArgument::Invalid {
            range, issue_range, ..
        } => {
            parser.start(
                SyntaxKind::DialogueActionInvalidArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.emit_to(issue_range.start());
            cursor.start(
                SyntaxKind::DialogueActionInvalidArgumentIssue,
                SyntaxRole::Issue,
            );
            cursor.emit_to(issue_range.end());
            cursor.finish();
            cursor.emit_to(range.end());
            cursor.finish_at(range.end());
            parser.finish();
        }
    }
}

fn emit_present_value(
    cursor: &mut PartitionedEventCursor<'_, '_, '_>,
    value: &ScannedDialogueActionArgumentValue,
) {
    cursor.start(SyntaxKind::DialogueActionArgumentValue, SyntaxRole::Value);
    cursor.start(SyntaxKind::DialogueActionArgumentToken, SyntaxRole::Token);
    if let Some(opening) = value.opening_quote_range() {
        cursor.start(
            SyntaxKind::DialogueActionArgumentQuote,
            SyntaxRole::OpeningQuote,
        );
        cursor.emit_to_as(opening.end(), SyntaxKind::PunctuationToken);
        cursor.finish();
    }
    cursor.start(
        SyntaxKind::DialogueActionArgumentContent,
        SyntaxRole::Content,
    );
    cursor.emit_to(value.content_range().end());
    cursor.finish();
    if let Some(closing) = value.closing_quote_range() {
        cursor.start(
            SyntaxKind::DialogueActionArgumentQuote,
            SyntaxRole::ClosingQuote,
        );
        cursor.emit_to_as(closing.end(), SyntaxKind::PunctuationToken);
        cursor.finish();
    }
    cursor.finish_at(value.token_range().end());
    cursor.finish();
    cursor.finish();
}

struct PartitionedEventCursor<'parser, 'source, 'events> {
    parser: &'parser mut DocumentParser<'source, 'events>,
    offset: usize,
}

impl<'parser, 'source, 'events> PartitionedEventCursor<'parser, 'source, 'events> {
    fn new(parser: &'parser mut DocumentParser<'source, 'events>, offset: usize) -> Self {
        assert_eq!(
            parser.current().map(|token| token.range().start()),
            Some(offset),
            "partitioned RichText range begins at the current lexer boundary"
        );
        Self { parser, offset }
    }

    fn start(&mut self, kind: SyntaxKind, role: SyntaxRole) {
        self.parser.start(kind, role);
    }

    fn finish(&mut self) {
        self.parser.finish();
    }

    fn emit_to(&mut self, end: usize) {
        self.emit_to_with_kind(end, None);
    }

    fn emit_to_as(&mut self, end: usize, split_kind: SyntaxKind) {
        self.emit_to_with_kind(end, Some(split_kind));
    }

    fn emit_to_with_kind(&mut self, end: usize, split_kind: Option<SyntaxKind>) {
        assert!(self.offset <= end, "RichText ranges remain ordered");
        while self.offset < end {
            let token = self
                .parser
                .current()
                .expect("RichText range stays inside the lexed dialogue payload");
            assert!(
                token.range().start() <= self.offset && self.offset < token.range().end(),
                "partition cursor remains inside the current lexer token"
            );
            let segment_end = end.min(token.range().end());
            let whole = self.offset == token.range().start() && segment_end == token.range().end();
            let kind = if whole {
                token.kind()
            } else {
                split_kind.unwrap_or(SyntaxKind::TextToken)
            };
            self.parser.push(SyntaxEvent::token(
                kind,
                SourceRange::new(self.offset, segment_end),
            ));
            self.offset = segment_end;
            if self.offset == token.range().end() {
                let consumed = self
                    .parser
                    .take_for_partition()
                    .expect("partitioned token remains current");
                assert_eq!(consumed, token);
            }
        }
    }

    fn finish_at(&self, expected: usize) {
        assert_eq!(
            self.offset, expected,
            "RichText node retains its exact range"
        );
    }
}

fn emit_range_node(
    parser: &mut DocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    range: TextRange,
) {
    parser.start(kind, role);
    bump_until_offset(parser, range.end());
    parser.finish();
}

fn bump_to_range_start(parser: &mut DocumentParser<'_, '_>, range: TextRange) {
    bump_until_offset(parser, range.start());
}

fn bump_until_offset(parser: &mut DocumentParser<'_, '_>, end: usize) {
    while parser.current_offset() < end {
        let token = parser
            .current()
            .expect("RichText range stays inside the dialogue payload");
        assert!(
            token.range().end() <= end,
            "non-value RichText range ends at a lexer boundary"
        );
        let _ = parser.bump();
    }
    assert_eq!(parser.current_offset(), end);
}

fn emit_unterminated_quote_diagnostic(
    parser: &mut DocumentParser<'_, '_>,
    range: Option<TextRange>,
) {
    let Some(range) = range else {
        return;
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        DialogueTextDiagnosticCode::DialogueActionArgumentUnterminatedQuote.as_str(),
        SourceRange::new(range.start(), range.end()),
        "unterminated quote in dialogue action arguments",
    )));
}

fn split_action_head(source: &str) -> (&str, &str) {
    source
        .char_indices()
        .find_map(|(index, character)| is_dialogue_action_whitespace(character).then_some(index))
        .map_or((source, &source[source.len()..]), |index| {
            (
                &source[..index],
                trim_dialogue_action_whitespace(&source[index..]),
            )
        })
}

fn subslice_offset(source: &str, subslice: &str) -> usize {
    let source_start = source.as_ptr() as usize;
    let source_end = source_start
        .checked_add(source.len())
        .expect("source address range does not overflow");
    let subslice_start = subslice.as_ptr() as usize;
    let subslice_end = subslice_start
        .checked_add(subslice.len())
        .expect("subslice address range does not overflow");
    assert!(
        source_start <= subslice_start && subslice_end <= source_end,
        "RichText range source must be an authored subslice"
    );
    subslice_start - source_start
}
