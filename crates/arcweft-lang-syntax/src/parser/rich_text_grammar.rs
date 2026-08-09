//! Private `RichText` descendants emitted inside the shared dialogue grammar.
//!
//! This module consumes the document lexer's existing cursor and the neutral
//! argument scan owned by `text::rich_text_tag`. It never invokes the public
//! dialogue parser, reparses a source substring, or wraps detached AST nodes.

use arcweft_source::SourceRange;

use super::cursor::DocumentParser;
use super::expression::{completed_slot, emit_expression_node};
use super::shadow_recovery::{emit_close_delimiter, emit_open_delimiter};
use crate::ast::common::TextRange;
use crate::expressions::{
    ExpressionComponentRole, PendingExpressionComponent, SyntaxBuiltinRichTextTag,
    SyntaxDialogueContent, SyntaxDialogueContentIssue, SyntaxDialogueContentProjection,
    SyntaxDialogueContentRecoveryBoundary, SyntaxDialogueNodeProjection,
    SyntaxDialogueNodeSourcePart, SyntaxExpressionSlot, SyntaxLineBreakKind,
    SyntaxRichTextArgumentParts, SyntaxRichTextArgumentProjection,
    SyntaxRichTextArgumentSourcePart, SyntaxRichTextDirectStyle, SyntaxRichTextEndTagProjection,
    SyntaxRichTextIssue, SyntaxRichTextTagIdentity, SyntaxRichTextTagPayloadProjection,
    SyntaxRichTextTagProjection, SyntaxRichTextTagSourcePart, SyntaxRichTextValue,
};
use crate::grammar::event::{PendingSyntaxDiagnostic, SyntaxEvent};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::name::SyntaxName;
use crate::text::{
    DialogueTextDiagnosticCode, MAX_RICH_TEXT_CONTENT_ARGUMENTS, MAX_RICH_TEXT_CONTENT_TAGS,
    MAX_RICH_TEXT_TAG_BODY_BYTES, ScannedDialogueSurface, ScannedDialogueSurfaceKind,
    ScannedInlineStyle, ScannedInlineStyleKind, ScannedTagArgValue, ScannedTagArgument,
    ScannedTagArgumentParts, ScannedTagArguments, find_dialogue_tag_boundary_before,
    is_rich_text_whitespace, scan_dialogue_surface, scan_tag_arguments, trim_rich_text_whitespace,
    utf8_boundary_at_or_before,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpenRichTextSpan {
    tag: u32,
    identity: SyntaxRichTextTagIdentity,
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

    emit_unclosed_rich_text_spans(
        parser,
        content_end,
        state.open_spans,
        &mut state.nodes,
        &mut state.components,
    );
    parser.finish();

    let projection = if state.saw_nontrivia {
        SyntaxDialogueContentProjection::Present(SyntaxDialogueContent::new(
            state.nodes,
            state.tags,
        ))
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
    tags: Vec<SyntaxRichTextTagProjection>,
    components: Vec<PendingExpressionComponent>,
    open_spans: Vec<OpenRichTextSpan>,
    content_tag_count: usize,
    argument_count: usize,
    tag_limit_exhausted: bool,
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
    if emit_tag_after_content_limit(
        parser,
        start,
        content_end,
        state.content_tag_count,
        &mut state.tag_limit_exhausted,
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
        RichTextContentProjectionState {
            content_tag_count: &mut state.content_tag_count,
            argument_count: &mut state.argument_count,
            tag_limit_exhausted: &mut state.tag_limit_exhausted,
            argument_limit_exhausted: &mut state.argument_limit_exhausted,
            nodes: &mut state.nodes,
            tags: &mut state.tags,
            components: &mut state.components,
        },
    ) || emit_overlong_tag(
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
    let Some(surface) = RichTextTagSurface::scan(parser, content_end) else {
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
    emit_authored_rich_text_surface(
        parser,
        start,
        surface,
        RichTextAuthoredSurfaceState {
            argument_count: &mut state.argument_count,
            argument_limit_exhausted: &mut state.argument_limit_exhausted,
            nodes: &mut state.nodes,
            tags: &mut state.tags,
            components: &mut state.components,
            open_spans: &mut state.open_spans,
        },
    );
    state.content_tag_count = state
        .content_tag_count
        .checked_add(1)
        .expect("RichText tag count remains grammar-bounded");
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

struct RichTextAuthoredSurfaceState<'a> {
    argument_count: &'a mut usize,
    argument_limit_exhausted: &'a mut bool,
    nodes: &'a mut Vec<SyntaxDialogueNodeProjection>,
    tags: &'a mut Vec<SyntaxRichTextTagProjection>,
    components: &'a mut Vec<PendingExpressionComponent>,
    open_spans: &'a mut Vec<OpenRichTextSpan>,
}

fn emit_authored_rich_text_surface(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    surface: RichTextTagSurface<'_>,
    mut state: RichTextAuthoredSurfaceState<'_>,
) {
    let tag = u32::try_from(state.tags.len()).expect("RichText tag limit fits u32");
    match surface.body {
        RichTextTagBody::Open(open) => {
            emit_authored_open_rich_text(parser, start, surface, open, tag, state);
        }
        RichTextTagBody::End { name_range } => {
            emit_authored_end_rich_text(parser, start, surface, name_range, tag, &mut state);
        }
    }
}

fn emit_authored_open_rich_text(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    tag: u32,
    state: RichTextAuthoredSurfaceState<'_>,
) {
    let RichTextAuthoredSurfaceState {
        argument_count,
        argument_limit_exhausted,
        nodes,
        tags,
        components,
        open_spans,
    } = state;
    let scanned_arguments = (!open.attrs.is_empty()
        && !matches!(open.source_name, "fx" | "call" | "!" | "if"))
    .then(|| {
        let remaining = MAX_RICH_TEXT_CONTENT_ARGUMENTS
            .checked_sub(*argument_count)
            .expect("retained RichText argument count stays inside its limit");
        scan_tag_arguments(open.attrs, open.attrs_range.start(), remaining)
    });
    if emit_argument_limit_recovery(
        parser,
        start,
        surface,
        scanned_arguments.as_ref(),
        argument_limit_exhausted,
        nodes,
        components,
    ) {
        return;
    }
    let emitted = emit_open_tag(
        parser,
        surface,
        open,
        tag,
        scanned_arguments,
        argument_count,
        argument_limit_exhausted,
    );
    let node = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    let identity = emitted.identity.clone();
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::DialogueNode {
            ordinal: node,
            part: SyntaxDialogueNodeSourcePart::Whole,
        },
        SourceRange::new(start, surface.end),
    ));
    if let Some(part) = dialogue_node_source_part(&emitted.node) {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: node,
                part,
            },
            SourceRange::new(start, surface.end),
        ));
    }
    components.extend(emitted.components);
    nodes.push(emitted.node);
    tags.push(SyntaxRichTextTagProjection::new(
        identity.clone(),
        emitted.arguments,
        emitted.payload,
        None,
    ));
    if identity.opens_span() {
        open_spans.push(OpenRichTextSpan { tag, identity });
    }
}

fn emit_argument_limit_recovery(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    surface: RichTextTagSurface<'_>,
    scanned: Option<&ScannedTagArguments>,
    argument_limit_exhausted: &mut bool,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    let Some(diagnostic) = scanned.and_then(|scanned| {
        scanned.diagnostics().iter().find(|diagnostic| {
            matches!(
                diagnostic.code(),
                DialogueTextDiagnosticCode::RichTextAttributeTooMany
                    | DialogueTextDiagnosticCode::RichTextAttributeKeyTooLong
                    | DialogueTextDiagnosticCode::RichTextAttributeValueTooLong
                    | DialogueTextDiagnosticCode::RichTextContentArgumentLimit
            )
        })
    }) else {
        return false;
    };
    let publish = diagnostic.code() != DialogueTextDiagnosticCode::RichTextContentArgumentLimit
        || !core::mem::replace(argument_limit_exhausted, true);
    if publish {
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            diagnostic.code().as_str(),
            source_range(*diagnostic.range()),
            diagnostic.message(),
        )));
    }
    let range = SourceRange::new(start, surface.end);
    let text = parser.source()[range.as_range()].into();
    emit_text_node(parser, range, text, nodes, components);
    true
}

fn emit_authored_end_rich_text(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    surface: RichTextTagSurface<'_>,
    name_range: TextRange,
    tag: u32,
    state: &mut RichTextAuthoredSurfaceState<'_>,
) {
    let node = u32::try_from(state.nodes.len()).expect("dialogue node limit fits u32");
    let inferred = name_range.start() == name_range.end();
    let (identity, matched) = match_rich_text_end(parser, name_range, state.open_spans);
    emit_end_tag(
        parser,
        surface,
        name_range,
        matched.as_ref().map_or(tag, |span| span.tag),
    );
    let issue = matched
        .is_none()
        .then_some(SyntaxRichTextIssue::InvalidNesting);
    let end = SyntaxRichTextEndTagProjection::new(identity, inferred, issue);
    state.nodes.push(if inferred {
        SyntaxDialogueNodeProjection::InferredEndTag(end)
    } else {
        SyntaxDialogueNodeProjection::AuthoredEndTag(end)
    });
    state.components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::DialogueNode {
            ordinal: node,
            part: SyntaxDialogueNodeSourcePart::Whole,
        },
        SourceRange::new(start, surface.end),
    ));
    if let Some(span) = matched {
        let paired = state
            .tags
            .get_mut(span.tag as usize)
            .expect("open span tag ordinal remains live")
            .pair_with_end_node(node);
        assert!(paired, "one RichText start tag pairs with one end node");
        state.components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: span.tag,
                part: SyntaxRichTextTagSourcePart::EndTag,
            },
            SourceRange::new(start, surface.end),
        ));
    }
}

fn match_rich_text_end(
    parser: &DocumentParser<'_, '_>,
    name_range: TextRange,
    open_spans: &mut Vec<OpenRichTextSpan>,
) -> (Option<SyntaxRichTextTagIdentity>, Option<OpenRichTextSpan>) {
    if name_range.start() == name_range.end() {
        let matched = open_spans.pop();
        return (matched.as_ref().map(|span| span.identity.clone()), matched);
    }
    let authored_name = parser
        .source()
        .get(name_range.as_range())
        .expect("RichText end tag name remains in source");
    let identity = tag_identity(authored_name);
    let matched = open_spans
        .iter()
        .rposition(|span| rich_text_end_matches(authored_name, &span.identity))
        .map(|position| open_spans.remove(position));
    (Some(identity), matched)
}

fn emit_unclosed_rich_text_spans(
    parser: &mut DocumentParser<'_, '_>,
    content_end: usize,
    open_spans: Vec<OpenRichTextSpan>,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    for span in open_spans {
        let node = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
        parser.start(SyntaxKind::DialogueError, SyntaxRole::DialogueNode(node));
        parser.finish();
        nodes.push(SyntaxDialogueNodeProjection::Error(
            SyntaxDialogueContentIssue::UnclosedTag,
        ));
        let at = SourceRange::new(content_end, content_end);
        components.extend([
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialogueNode {
                    ordinal: node,
                    part: SyntaxDialogueNodeSourcePart::Whole,
                },
                at,
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialogueNode {
                    ordinal: node,
                    part: SyntaxDialogueNodeSourcePart::Error,
                },
                at,
            ),
        ]);
        let tag_range = components
            .iter()
            .find(|component| {
                component.role()
                    == ExpressionComponentRole::RichTextTag {
                        tag: span.tag,
                        part: SyntaxRichTextTagSourcePart::Whole,
                    }
            })
            .map_or(at, |component| component.range());
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.rich_text.tag.unclosed",
            tag_range,
            "RichText start tag has no matching end tag",
        )));
    }
}

const fn dialogue_node_source_part(
    node: &SyntaxDialogueNodeProjection,
) -> Option<SyntaxDialogueNodeSourcePart> {
    match node {
        SyntaxDialogueNodeProjection::LineBreak(_) => Some(SyntaxDialogueNodeSourcePart::LineBreak),
        SyntaxDialogueNodeProjection::Text(_)
        | SyntaxDialogueNodeProjection::Raw(_)
        | SyntaxDialogueNodeProjection::Escape(_)
        | SyntaxDialogueNodeProjection::Ruby { .. }
        | SyntaxDialogueNodeProjection::AuthoredStartTag { .. }
        | SyntaxDialogueNodeProjection::InferredStartTag { .. }
        | SyntaxDialogueNodeProjection::AuthoredEndTag(_)
        | SyntaxDialogueNodeProjection::InferredEndTag(_)
        | SyntaxDialogueNodeProjection::Interpolation(_)
        | SyntaxDialogueNodeProjection::Error(_) => None,
    }
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
    tags: &mut Vec<SyntaxRichTextTagProjection>,
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
        ScannedDialogueSurfaceKind::Raw { body, .. } => {
            emit_scanned_raw(parser, body, ordinal, whole, nodes, components);
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
        ScannedDialogueSurfaceKind::InlineStyle(style) => {
            emit_inline_style(parser, surface, style, nodes, tags, components);
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
                part: SyntaxDialogueNodeSourcePart::RubyBase,
            },
            SourceRange::new(ruby.base().range().start(), ruby.base().range().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal,
                part: SyntaxDialogueNodeSourcePart::RubyText,
            },
            SourceRange::new(ruby.ruby().range().start(), ruby.ruby().range().end()),
        ),
    ]);
}

fn emit_scanned_raw(
    parser: &mut DocumentParser<'_, '_>,
    body: &crate::text::ScannedDialogueText,
    ordinal: u32,
    whole: SourceRange,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    emit_dialogue_range_owner(
        parser,
        SyntaxKind::DialogueRaw,
        SyntaxRole::DialogueNode(ordinal),
        whole,
    );
    nodes.push(SyntaxDialogueNodeProjection::Raw(body.value().into()));
    components.extend(dialogue_node_components(
        ordinal,
        whole,
        SyntaxDialogueNodeSourcePart::Raw,
        SourceRange::new(body.range().start(), body.range().end()),
    ));
}

#[derive(Clone, Copy)]
struct ScannedInterpolationSource {
    open: TextRange,
    payload: TextRange,
    close: TextRange,
    whole: SourceRange,
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

fn emit_inline_style(
    parser: &mut DocumentParser<'_, '_>,
    surface: &ScannedDialogueSurface,
    style: &ScannedInlineStyle,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    tags: &mut Vec<SyntaxRichTextTagProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let ordinals = InlineStyleOrdinals::new(nodes.len(), tags.len());
    let emitted = emit_inline_style_surface(parser, surface, style, ordinals);

    tags.push(SyntaxRichTextTagProjection::new(
        emitted.identity.clone(),
        emitted.arguments,
        SyntaxRichTextTagPayloadProjection::Arguments,
        Some(ordinals.end_node),
    ));
    nodes.extend([
        SyntaxDialogueNodeProjection::InferredStartTag { tag: ordinals.tag },
        SyntaxDialogueNodeProjection::Text(style.body().value().into()),
        SyntaxDialogueNodeProjection::InferredEndTag(SyntaxRichTextEndTagProjection::new(
            Some(emitted.identity),
            true,
            None,
        )),
    ]);
    components.extend(inline_style_components(surface, style, ordinals));
    components.extend(emitted.argument_components);
}

#[derive(Clone, Copy)]
struct InlineStyleOrdinals {
    tag: u32,
    start_node: u32,
    text_node: u32,
    end_node: u32,
}

impl InlineStyleOrdinals {
    fn new(node_count: usize, tag_count: usize) -> Self {
        let start_node = u32::try_from(node_count).expect("dialogue node limit fits u32");
        let text_node = start_node
            .checked_add(1)
            .expect("dialogue node ordinal fits u32");
        Self {
            tag: u32::try_from(tag_count).expect("RichText tag limit fits u32"),
            start_node,
            text_node,
            end_node: text_node
                .checked_add(1)
                .expect("dialogue node ordinal fits u32"),
        }
    }
}

struct EmittedInlineStyleSurface {
    identity: SyntaxRichTextTagIdentity,
    arguments: Vec<SyntaxRichTextArgumentProjection>,
    argument_components: Vec<PendingExpressionComponent>,
}

fn emit_inline_style_surface(
    parser: &mut DocumentParser<'_, '_>,
    surface: &ScannedDialogueSurface,
    style: &ScannedInlineStyle,
    ordinals: InlineStyleOrdinals,
) -> EmittedInlineStyleSurface {
    let whole = surface.range();
    let prefix = TextRange::new(whole.start(), style.body().range().start());
    let suffix = TextRange::new(style.body().range().end(), whole.end());
    let identity = inline_style_identity(style.style());
    let mut cursor = PartitionedEventCursor::new(parser, whole.start());
    cursor.start(
        SyntaxKind::RichTextTag,
        SyntaxRole::RichTextTag(ordinals.tag),
    );
    cursor.emit_to(style.name().start());
    cursor.start(SyntaxKind::RichTextTagName, SyntaxRole::Name);
    cursor.emit_to(style.name().end());
    cursor.finish();
    let (arguments, argument_components) =
        emit_inline_style_value(&mut cursor, style, ordinals.tag);
    cursor.emit_to(prefix.end());
    cursor.finish();
    cursor.start(
        SyntaxKind::DialogueText,
        SyntaxRole::DialogueNode(ordinals.text_node),
    );
    cursor.emit_to(style.body().range().end());
    cursor.finish();
    cursor.start(
        SyntaxKind::RichTextEndTag,
        SyntaxRole::RichTextTag(ordinals.tag),
    );
    cursor.emit_to(suffix.end());
    cursor.finish();
    cursor.finish_at(whole.end());
    EmittedInlineStyleSurface {
        identity,
        arguments,
        argument_components,
    }
}

const fn inline_style_identity(style: ScannedInlineStyleKind) -> SyntaxRichTextTagIdentity {
    SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::DirectStyle(match style {
        ScannedInlineStyleKind::Emphasis => SyntaxRichTextDirectStyle::Emphasis,
        ScannedInlineStyleKind::Strong => SyntaxRichTextDirectStyle::Strong,
        ScannedInlineStyleKind::Color => SyntaxRichTextDirectStyle::Color,
    }))
}

fn emit_inline_style_value(
    cursor: &mut PartitionedEventCursor<'_, '_, '_>,
    style: &ScannedInlineStyle,
    tag: u32,
) -> (
    Vec<SyntaxRichTextArgumentProjection>,
    Vec<PendingExpressionComponent>,
) {
    let Some(value) = style.value() else {
        return (Vec::new(), Vec::new());
    };
    cursor.emit_to(value.token_range().start());
    cursor.start(
        SyntaxKind::RichTextPositionalArgument,
        SyntaxRole::Argument(0),
    );
    emit_present_value(cursor, value);
    cursor.finish();
    (
        vec![SyntaxRichTextArgumentProjection::Positional {
            value: SyntaxRichTextValue::new(value.decoded()),
        }],
        vec![
            PendingExpressionComponent::new(
                ExpressionComponentRole::RichTextArgument {
                    tag,
                    argument: 0,
                    part: SyntaxRichTextArgumentSourcePart::Whole,
                },
                SourceRange::new(value.token_range().start(), value.token_range().end()),
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::RichTextArgument {
                    tag,
                    argument: 0,
                    part: SyntaxRichTextArgumentSourcePart::Value,
                },
                SourceRange::new(value.content_range().start(), value.content_range().end()),
            ),
        ],
    )
}

fn inline_style_components(
    surface: &ScannedDialogueSurface,
    style: &ScannedInlineStyle,
    ordinals: InlineStyleOrdinals,
) -> Vec<PendingExpressionComponent> {
    let whole = surface.range();
    let prefix = TextRange::new(whole.start(), style.body().range().start());
    let suffix = TextRange::new(style.body().range().end(), whole.end());
    vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: ordinals.start_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(prefix.start(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: ordinals.text_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(style.body().range().start(), style.body().range().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: ordinals.text_node,
                part: SyntaxDialogueNodeSourcePart::Text,
            },
            SourceRange::new(style.body().range().start(), style.body().range().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: ordinals.end_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(suffix.start(), suffix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::Whole,
            },
            SourceRange::new(prefix.start(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::OpenDelimiter,
            },
            SourceRange::new(
                prefix.start(),
                prefix
                    .start()
                    .checked_add('['.len_utf8())
                    .expect("inline RichText opening delimiter remains representable"),
            ),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::Name,
            },
            SourceRange::new(style.name().start(), style.name().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::Payload,
            },
            SourceRange::new(style.name().end(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::CloseDelimiter,
            },
            SourceRange::new(style.separator().start(), style.separator().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::InferenceInsertion,
            },
            SourceRange::new(style.inferred_end(), style.inferred_end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinals.tag,
                part: SyntaxRichTextTagSourcePart::EndTag,
            },
            SourceRange::new(suffix.start(), suffix.end()),
        ),
    ]
}

#[derive(Clone, Copy)]
enum RichTextContentLimit {
    Tags,
    Arguments,
}

fn emit_tag_after_content_limit(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    content_tag_count: usize,
    tag_limit_exhausted: &mut bool,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    if !parser.at("[") || (!*tag_limit_exhausted && content_tag_count < MAX_RICH_TEXT_CONTENT_TAGS)
    {
        return false;
    }
    if let Some(boundary) = find_dialogue_tag_boundary_before(parser.source(), start, content_end) {
        if !core::mem::replace(tag_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                RichTextContentLimit::Tags,
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

struct RichTextContentProjectionState<'a> {
    content_tag_count: &'a mut usize,
    argument_count: &'a mut usize,
    tag_limit_exhausted: &'a mut bool,
    argument_limit_exhausted: &'a mut bool,
    nodes: &'a mut Vec<SyntaxDialogueNodeProjection>,
    tags: &'a mut Vec<SyntaxRichTextTagProjection>,
    components: &'a mut Vec<PendingExpressionComponent>,
}

fn emit_typed_dialogue_surface(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    state: RichTextContentProjectionState<'_>,
) -> bool {
    let RichTextContentProjectionState {
        content_tag_count,
        argument_count,
        tag_limit_exhausted,
        argument_limit_exhausted,
        nodes,
        tags,
        components,
    } = state;
    let Some(surface) = scan_dialogue_surface(parser.source(), start, content_end) else {
        return false;
    };
    let tag_overflow =
        surface.rich_text_tags() > MAX_RICH_TEXT_CONTENT_TAGS.saturating_sub(*content_tag_count);
    let argument_overflow = surface.rich_text_arguments()
        > MAX_RICH_TEXT_CONTENT_ARGUMENTS.saturating_sub(*argument_count);
    if tag_overflow {
        if !core::mem::replace(tag_limit_exhausted, true) {
            emit_content_limit_diagnostic(
                parser,
                RichTextContentLimit::Tags,
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
                RichTextContentLimit::Arguments,
                SourceRange::new(start, surface.end()),
            );
        }
        let range = SourceRange::new(start, surface.end());
        let text = parser.source()[range.as_range()].into();
        emit_text_node(parser, range, text, nodes, components);
    } else {
        *content_tag_count += surface.rich_text_tags();
        *argument_count += surface.rich_text_arguments();
        emit_scanned_surface(parser, &surface, nodes, tags, components);
    }
    true
}

fn emit_overlong_tag(
    parser: &mut DocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
    if !parser.at("[") {
        return false;
    }
    let Some(boundary) = find_dialogue_tag_boundary_before(parser.source(), start, content_end)
    else {
        return false;
    };
    let body_start = start
        .checked_add('['.len_utf8())
        .expect("RichText tag body starts after its opening delimiter");
    let inside = &parser.source()[body_start..boundary.close()];
    if inside.len() <= MAX_RICH_TEXT_TAG_BODY_BYTES {
        return false;
    }
    let limit = utf8_boundary_at_or_before(inside, MAX_RICH_TEXT_TAG_BODY_BYTES);
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        DialogueTextDiagnosticCode::RichTextTagBodyTooLong.as_str(),
        SourceRange::new(
            body_start
                .checked_add(limit)
                .expect("RichText tag limit offset remains representable"),
            boundary.close(),
        ),
        format!("dialogue RichText tag body exceeds {MAX_RICH_TEXT_TAG_BODY_BYTES} bytes"),
    )));
    let range = SourceRange::new(start, boundary.end());
    let text = parser.source()[range.as_range()].into();
    emit_text_node(parser, range, text, nodes, components);
    true
}

fn emit_content_limit_diagnostic(
    parser: &mut DocumentParser<'_, '_>,
    limit: RichTextContentLimit,
    range: SourceRange,
) {
    let (code, message) = match limit {
        RichTextContentLimit::Tags => (
            DialogueTextDiagnosticCode::RichTextContentTagLimit.as_str(),
            format!("dialogue content has more than {MAX_RICH_TEXT_CONTENT_TAGS} RichText tags"),
        ),
        RichTextContentLimit::Arguments => (
            DialogueTextDiagnosticCode::RichTextContentArgumentLimit.as_str(),
            format!(
                "dialogue content has more than {MAX_RICH_TEXT_CONTENT_ARGUMENTS} RichText arguments"
            ),
        ),
    };
    parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
        code, range, message,
    )));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RichTextTagSurface<'source> {
    start: usize,
    end: usize,
    unterminated_quote: Option<TextRange>,
    body: RichTextTagBody<'source>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RichTextTagBody<'source> {
    Open(OpenTagSurface<'source>),
    End { name_range: TextRange },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenTagSurface<'source> {
    source_name: &'source str,
    name_range: TextRange,
    attrs: &'source str,
    attrs_range: TextRange,
}

impl<'source> RichTextTagSurface<'source> {
    fn scan(parser: &DocumentParser<'source, '_>, content_end: usize) -> Option<Self> {
        parser.at("[").then_some(())?;
        let open = parser.current_offset();
        let boundary = find_dialogue_tag_boundary_before(parser.source(), open, content_end)?;
        let close = boundary.close();
        let end = boundary.end();
        let unterminated_quote = boundary
            .unterminated_quote_start()
            .map(|start| TextRange::new(start, end));
        let body_start = open.checked_add('['.len_utf8())?;
        let inside_source = parser.source().get(body_start..close)?;
        let inside = trim_rich_text_whitespace(inside_source);
        if inside.is_empty() {
            return None;
        }
        let inside_start = body_start.checked_add(subslice_offset(inside_source, inside))?;

        if let Some(name) = inside.strip_prefix('/') {
            let name = trim_rich_text_whitespace(name);
            let after_marker = inside_start.checked_add('/'.len_utf8())?;
            let name_start = after_marker.checked_add(subslice_offset(&inside[1..], name))?;
            return Some(Self {
                start: open,
                end,
                unterminated_quote,
                body: RichTextTagBody::End {
                    name_range: TextRange::new(name_start, name_start.checked_add(name.len())?),
                },
            });
        }

        let (source_name, attrs, name_start) = if let Some(attrs) = inside.strip_prefix('!') {
            ("!", trim_rich_text_whitespace(attrs), inside_start)
        } else {
            let (source_name, attrs) = split_tag_head(inside);
            (
                source_name,
                attrs,
                inside_start.checked_add(subslice_offset(inside, source_name))?,
            )
        };
        if source_name.is_empty() {
            return None;
        }
        let attrs_start = inside_start.checked_add(subslice_offset(inside, attrs))?;
        Some(Self {
            start: open,
            end,
            unterminated_quote,
            body: RichTextTagBody::Open(OpenTagSurface {
                source_name,
                name_range: TextRange::new(name_start, name_start.checked_add(source_name.len())?),
                attrs,
                attrs_range: TextRange::new(attrs_start, attrs_start.checked_add(attrs.len())?),
            }),
        })
    }
}

struct EmittedOpenTag {
    identity: SyntaxRichTextTagIdentity,
    arguments: Vec<SyntaxRichTextArgumentProjection>,
    payload: SyntaxRichTextTagPayloadProjection,
    node: SyntaxDialogueNodeProjection,
    components: Vec<PendingExpressionComponent>,
}

fn emit_open_tag(
    parser: &mut DocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    ordinal: u32,
    scanned_arguments: Option<ScannedTagArguments>,
    content_argument_count: &mut usize,
    argument_limit_exhausted: &mut bool,
) -> EmittedOpenTag {
    let identity = tag_identity_with_arguments(open.source_name, scanned_arguments.as_ref());
    let inferred = open.source_name.starts_with('.');
    let mut components = open_tag_components(surface, open, ordinal, inferred);
    parser.start(SyntaxKind::RichTextTag, SyntaxRole::RichTextTag(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    bump_to_range_start(parser, open.name_range);
    emit_range_node(
        parser,
        SyntaxKind::RichTextTagName,
        SyntaxRole::Name,
        open.name_range,
    );
    let mut state = OpenTagPayloadState {
        ordinal,
        content_argument_count,
        argument_limit_exhausted,
        components: &mut components,
    };
    let (arguments, payload) = emit_open_tag_payload(
        parser,
        surface,
        open,
        &identity,
        scanned_arguments,
        &mut state,
    );
    emit_open_tag_close(parser, surface);
    let node = if inferred {
        SyntaxDialogueNodeProjection::InferredStartTag { tag: ordinal }
    } else {
        SyntaxDialogueNodeProjection::AuthoredStartTag { tag: ordinal }
    };
    EmittedOpenTag {
        identity,
        arguments,
        payload,
        node,
        components,
    }
}

fn open_tag_components(
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    ordinal: u32,
    inferred: bool,
) -> Vec<PendingExpressionComponent> {
    let whole = SourceRange::new(surface.start, surface.end);
    let mut components = vec![
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::Whole,
            },
            whole,
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::OpenDelimiter,
            },
            SourceRange::new(
                whole.start(),
                whole
                    .start()
                    .checked_add('['.len_utf8())
                    .expect("RichText opening delimiter remains representable"),
            ),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::Name,
            },
            SourceRange::new(open.name_range.start(), open.name_range.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::Payload,
            },
            SourceRange::new(open.attrs_range.start(), open.attrs_range.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::CloseDelimiter,
            },
            SourceRange::new(
                surface
                    .end
                    .checked_sub(']'.len_utf8())
                    .expect("RichText closing delimiter follows its opening delimiter"),
                surface.end,
            ),
        ),
    ];
    if inferred {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: ordinal,
                part: SyntaxRichTextTagSourcePart::InferenceInsertion,
            },
            SourceRange::new(open.name_range.start(), open.name_range.start()),
        ));
    }
    components
}

struct OpenTagPayloadState<'a> {
    ordinal: u32,
    content_argument_count: &'a mut usize,
    argument_limit_exhausted: &'a mut bool,
    components: &'a mut Vec<PendingExpressionComponent>,
}

fn emit_open_tag_payload(
    parser: &mut DocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    identity: &SyntaxRichTextTagIdentity,
    scanned_arguments: Option<ScannedTagArguments>,
    state: &mut OpenTagPayloadState<'_>,
) -> (
    Vec<SyntaxRichTextArgumentProjection>,
    SyntaxRichTextTagPayloadProjection,
) {
    let mut arguments = Vec::new();
    let mut payload = SyntaxRichTextTagPayloadProjection::None;
    if !open.attrs.is_empty() {
        bump_to_range_start(parser, open.attrs_range);
        match open.source_name {
            "fx" => {
                payload = SyntaxRichTextTagPayloadProjection::FxCall(emit_expression_payload(
                    parser,
                    open.attrs_range,
                    SyntaxKind::RichTextFxCallPayload,
                    SyntaxRole::Operand,
                ));
            }
            "call" | "!" => {
                payload =
                    SyntaxRichTextTagPayloadProjection::DialogueCall(emit_expression_payload(
                        parser,
                        open.attrs_range,
                        SyntaxKind::RichTextDialogueCallPayload,
                        SyntaxRole::Operand,
                    ));
            }
            "if" => {
                payload = SyntaxRichTextTagPayloadProjection::Condition(emit_expression_payload(
                    parser,
                    open.attrs_range,
                    SyntaxKind::RichTextConditionPayload,
                    SyntaxRole::Condition,
                ));
            }
            _ => {
                let scanned = scanned_arguments
                    .expect("ordinary RichText attributes are scanned once before emission");
                *state.content_argument_count = state
                    .content_argument_count
                    .checked_add(scanned.entries().len())
                    .expect("retained RichText argument count remains grammar-bounded");
                arguments = scanned
                    .entries()
                    .iter()
                    .map(|argument| syntax_argument(parser.source(), argument))
                    .collect();
                state
                    .components
                    .extend(argument_components(state.ordinal, scanned.entries()));
                emit_argument_payload(
                    parser,
                    open.attrs_range,
                    &scanned,
                    surface.unterminated_quote.is_some(),
                    state.argument_limit_exhausted,
                );
                payload = SyntaxRichTextTagPayloadProjection::Arguments;
            }
        }
    } else if !matches!(
        identity,
        SyntaxRichTextTagIdentity::Builtin(
            SyntaxBuiltinRichTextTag::Page
                | SyntaxBuiltinRichTextTag::LineWait
                | SyntaxBuiltinRichTextTag::HardBreak
                | SyntaxBuiltinRichTextTag::Clear
                | SyntaxBuiltinRichTextTag::Reset
        )
    ) {
        payload = SyntaxRichTextTagPayloadProjection::Arguments;
    }
    (arguments, payload)
}

fn emit_open_tag_close(parser: &mut DocumentParser<'_, '_>, surface: RichTextTagSurface<'_>) {
    let close = surface
        .end
        .checked_sub(']'.len_utf8())
        .expect("RichText closing delimiter follows its opening delimiter");
    bump_until_offset(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.rich_text.tag.missing_close",
    );
    emit_unterminated_quote_diagnostic(parser, surface.unterminated_quote);
    parser.finish();
}

fn tag_identity(source: &str) -> SyntaxRichTextTagIdentity {
    SyntaxRichTextTagIdentity::from_source_name(source)
}

fn tag_identity_with_arguments(
    source: &str,
    arguments: Option<&ScannedTagArguments>,
) -> SyntaxRichTextTagIdentity {
    let Some(selector) = arguments
        .and_then(|arguments| arguments.entries().first())
        .and_then(|argument| match argument {
            ScannedTagArgument::Positional { value, .. } => Some(value.decoded()),
            ScannedTagArgument::Named { .. } | ScannedTagArgument::Invalid { .. } => None,
        })
    else {
        return tag_identity(source);
    };
    let Some(builtin) = SyntaxBuiltinRichTextTag::from_source_name(selector) else {
        return tag_identity(source);
    };
    let belongs_to_family = matches!(
        (source, builtin),
        ("style", SyntaxBuiltinRichTextTag::Style(_))
            | ("layout", SyntaxBuiltinRichTextTag::Layout(_))
            | ("transform", SyntaxBuiltinRichTextTag::Transform(_))
            | ("effect", SyntaxBuiltinRichTextTag::Fx(_))
    );
    if belongs_to_family {
        SyntaxRichTextTagIdentity::Builtin(builtin)
    } else {
        tag_identity(source)
    }
}

fn rich_text_end_matches(authored_name: &str, open: &SyntaxRichTextTagIdentity) -> bool {
    match (authored_name, open) {
        ("style", SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Style(_)))
        | ("layout", SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Layout(_)))
        | (
            "transform",
            SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Transform(_)),
        )
        | ("effect", SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Fx(_)))
        | ("object", SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::Object(_))) => {
            true
        }
        _ => &tag_identity(authored_name) == open,
    }
}

fn syntax_argument(
    source: &str,
    argument: &ScannedTagArgument,
) -> SyntaxRichTextArgumentProjection {
    match argument {
        ScannedTagArgument::Positional { value, .. } => {
            SyntaxRichTextArgumentProjection::Positional {
                value: SyntaxRichTextValue::new(value.decoded()),
            }
        }
        ScannedTagArgument::Named {
            name_range, value, ..
        } => SyntaxRichTextArgumentProjection::Named {
            name: SyntaxName::try_new(
                source
                    .get(name_range.as_range())
                    .expect("RichText argument name remains inside source"),
            ),
            value: SyntaxRichTextValue::new(value.decoded()),
        },
        ScannedTagArgument::Invalid { issue, parts, .. } => {
            SyntaxRichTextArgumentProjection::Invalid {
                issue: *issue,
                authored_parts: syntax_argument_parts(*parts),
            }
        }
    }
}

const fn syntax_argument_parts(parts: ScannedTagArgumentParts) -> SyntaxRichTextArgumentParts {
    SyntaxRichTextArgumentParts::new(
        parts.name().is_some(),
        parts.equals().is_some(),
        parts.value().is_some(),
    )
}

fn argument_components(
    tag: u32,
    arguments: &[ScannedTagArgument],
) -> Vec<PendingExpressionComponent> {
    arguments
        .iter()
        .enumerate()
        .flat_map(|(ordinal, _)| {
            let argument = u16::try_from(ordinal).expect("RichText tag argument limit fits u16");
            let mut parts = vec![PendingExpressionComponent::new(
                ExpressionComponentRole::RichTextArgument {
                    tag,
                    argument,
                    part: SyntaxRichTextArgumentSourcePart::Whole,
                },
                source_range(arguments[ordinal].range()),
            )];
            match &arguments[ordinal] {
                ScannedTagArgument::Positional { value, .. } => {
                    parts.push(PendingExpressionComponent::new(
                        ExpressionComponentRole::RichTextArgument {
                            tag,
                            argument,
                            part: SyntaxRichTextArgumentSourcePart::Value,
                        },
                        source_range(value.content_range()),
                    ));
                }
                ScannedTagArgument::Named {
                    name_range,
                    equals_range,
                    value,
                    ..
                } => {
                    parts.extend([
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::RichTextArgument {
                                tag,
                                argument,
                                part: SyntaxRichTextArgumentSourcePart::Name,
                            },
                            source_range(*name_range),
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::RichTextArgument {
                                tag,
                                argument,
                                part: SyntaxRichTextArgumentSourcePart::Equals,
                            },
                            source_range(*equals_range),
                        ),
                        PendingExpressionComponent::new(
                            ExpressionComponentRole::RichTextArgument {
                                tag,
                                argument,
                                part: SyntaxRichTextArgumentSourcePart::Value,
                            },
                            source_range(value.content_range()),
                        ),
                    ]);
                }
                ScannedTagArgument::Invalid {
                    parts: authored, ..
                } => {
                    for (part, range) in [
                        (SyntaxRichTextArgumentSourcePart::Name, authored.name()),
                        (SyntaxRichTextArgumentSourcePart::Equals, authored.equals()),
                        (SyntaxRichTextArgumentSourcePart::Value, authored.value()),
                    ] {
                        if let Some(range) = range {
                            parts.push(PendingExpressionComponent::new(
                                ExpressionComponentRole::RichTextArgument {
                                    tag,
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

fn emit_end_tag(
    parser: &mut DocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    name_range: TextRange,
    ordinal: u32,
) {
    parser.start(SyntaxKind::RichTextEndTag, SyntaxRole::RichTextTag(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    if parser.at("/") {
        let _ = parser.bump();
    }
    bump_to_range_start(parser, name_range);
    if name_range.start() != name_range.end() {
        emit_range_node(
            parser,
            SyntaxKind::RichTextTagName,
            SyntaxRole::Name,
            name_range,
        );
    }
    let close = surface
        .end
        .checked_sub(']'.len_utf8())
        .expect("RichText end tag closing delimiter follows its opening delimiter");
    bump_until_offset(parser, close);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.rich_text.tag.missing_close",
    );
    emit_unterminated_quote_diagnostic(parser, surface.unterminated_quote);
    parser.finish();
}

fn emit_expression_payload(
    parser: &mut DocumentParser<'_, '_>,
    range: TextRange,
    kind: SyntaxKind,
    expression_role: SyntaxRole,
) -> SyntaxExpressionSlot {
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
    scanned: &ScannedTagArguments,
    tag_reports_unterminated_quote: bool,
    argument_limit_exhausted: &mut bool,
) {
    parser.start(SyntaxKind::RichTextArgumentPayload, SyntaxRole::Payload);
    parser.start(SyntaxKind::RichTextArgumentList, SyntaxRole::Element(0));
    for (ordinal, argument) in scanned.entries().iter().enumerate() {
        bump_to_range_start(parser, argument.range());
        emit_argument(
            parser,
            argument,
            u16::try_from(ordinal).expect("RichText tag argument limit fits u16"),
        );
    }
    bump_until_offset(parser, range.end());
    parser.finish();
    for diagnostic in scanned.diagnostics() {
        if tag_reports_unterminated_quote
            && diagnostic.code() == DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote
        {
            continue;
        }
        if diagnostic.code() == DialogueTextDiagnosticCode::RichTextContentArgumentLimit
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

fn emit_argument(parser: &mut DocumentParser<'_, '_>, argument: &ScannedTagArgument, ordinal: u16) {
    match argument {
        ScannedTagArgument::Positional { value, range } => {
            parser.start(
                SyntaxKind::RichTextPositionalArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            emit_present_value(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedTagArgument::Named {
            name_range,
            equals_range,
            value,
            range,
        } => {
            parser.start(
                SyntaxKind::RichTextNamedArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.start(SyntaxKind::RichTextArgumentKey, SyntaxRole::Key);
            cursor.emit_to(name_range.end());
            cursor.finish();
            cursor.start(SyntaxKind::RichTextArgumentEquals, SyntaxRole::Equals);
            cursor.emit_to_as(equals_range.end(), SyntaxKind::PunctuationToken);
            cursor.finish();
            emit_present_value(&mut cursor, value);
            cursor.finish_at(range.end());
            parser.finish();
        }
        ScannedTagArgument::Invalid {
            range, issue_range, ..
        } => {
            parser.start(
                SyntaxKind::RichTextInvalidArgument,
                SyntaxRole::Argument(ordinal),
            );
            let mut cursor = PartitionedEventCursor::new(parser, range.start());
            cursor.emit_to(issue_range.start());
            cursor.start(SyntaxKind::RichTextInvalidArgumentIssue, SyntaxRole::Issue);
            cursor.emit_to(issue_range.end());
            cursor.finish();
            cursor.emit_to(range.end());
            cursor.finish_at(range.end());
            parser.finish();
        }
    }
}

fn emit_present_value(cursor: &mut PartitionedEventCursor<'_, '_, '_>, value: &ScannedTagArgValue) {
    cursor.start(SyntaxKind::RichTextArgumentValue, SyntaxRole::Value);
    cursor.start(SyntaxKind::RichTextArgumentToken, SyntaxRole::Token);
    if let Some(opening) = value.opening_quote_range() {
        cursor.start(SyntaxKind::RichTextArgumentQuote, SyntaxRole::OpeningQuote);
        cursor.emit_to_as(opening.end(), SyntaxKind::PunctuationToken);
        cursor.finish();
    }
    cursor.start(SyntaxKind::RichTextArgumentContent, SyntaxRole::Content);
    cursor.emit_to(value.content_range().end());
    cursor.finish();
    if let Some(closing) = value.closing_quote_range() {
        cursor.start(SyntaxKind::RichTextArgumentQuote, SyntaxRole::ClosingQuote);
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
        DialogueTextDiagnosticCode::RichTextAttributeUnterminatedQuote.as_str(),
        SourceRange::new(range.start(), range.end()),
        "unterminated quote in dialogue tag arguments",
    )));
}

fn split_tag_head(source: &str) -> (&str, &str) {
    source
        .char_indices()
        .find_map(|(index, character)| is_rich_text_whitespace(character).then_some(index))
        .map_or((source, &source[source.len()..]), |index| {
            (
                &source[..index],
                trim_rich_text_whitespace(&source[index..]),
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
