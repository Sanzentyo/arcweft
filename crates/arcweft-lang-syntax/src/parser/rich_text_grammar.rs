//! Private `RichText` descendants emitted inside the shared dialogue grammar.
//!
//! This module consumes the document lexer's existing cursor and the neutral
//! argument scan owned by `text::rich_text_tag`. It never invokes the public
//! dialogue parser, reparses a source substring, or wraps detached AST nodes.

use arcweft_source::SourceRange;

use super::cursor::ShadowDocumentParser;
use super::expression::{completed_slot, emit_expression_node};
use super::shadow_recovery::{emit_close_delimiter, emit_open_delimiter};
use crate::ast::common::TextRange;
use crate::expressions::{
    ExpressionComponentRole, PendingExpressionComponent, SyntaxBuiltinRichTextTag,
    SyntaxDialogueContent, SyntaxDialogueContentIssue, SyntaxDialogueContentProjection,
    SyntaxDialogueContentRecoveryBoundary, SyntaxDialogueControl, SyntaxDialogueNodeProjection,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    end: usize,
    missing_boundary: SyntaxDialogueContentRecoveryBoundary,
) -> EmittedDialogueContent {
    let content_end = parser
        .offset_at_token_boundary(end)
        .expect("dialogue content end is a lexer boundary");
    parser.start(SyntaxKind::DialogueContent, SyntaxRole::Content);
    let mut nodes = Vec::new();
    let mut tags = Vec::new();
    let mut components = Vec::new();
    let mut open_spans = Vec::<OpenRichTextSpan>::new();
    let mut content_tag_count = 0_usize;
    let mut argument_count = 0_usize;
    let mut tag_limit_exhausted = false;
    let mut argument_limit_exhausted = false;
    let mut has_real_atom = false;
    let mut saw_nontrivia = false;

    while parser.cursor() < end {
        let start = parser.current_offset();
        if parser
            .current_kind()
            .is_some_and(super::cursor::is_trivia_kind)
        {
            if parser.current_kind() == Some(SyntaxKind::NewlineToken) {
                let range = parser
                    .current()
                    .expect("dialogue newline remains inside the content interval")
                    .range();
                emit_line_break_node(
                    parser,
                    &mut nodes,
                    &mut components,
                    range,
                    SyntaxLineBreakKind::Line,
                );
                has_real_atom = true;
                saw_nontrivia = true;
            } else {
                let _ = parser
                    .bump()
                    .expect("dialogue trivia remains inside the content interval");
            }
            continue;
        }
        saw_nontrivia = true;
        if emit_tag_after_content_limit(
            parser,
            start,
            content_end,
            content_tag_count,
            &mut tag_limit_exhausted,
            &mut nodes,
            &mut components,
        ) {
            has_real_atom = true;
            continue;
        }
        if emit_typed_dialogue_surface(
            parser,
            start,
            content_end,
            &mut content_tag_count,
            &mut argument_count,
            &mut tag_limit_exhausted,
            &mut argument_limit_exhausted,
            &mut nodes,
            &mut tags,
            &mut components,
        ) {
            has_real_atom = true;
            continue;
        }
        if emit_overlong_tag(parser, start, content_end, &mut nodes, &mut components) {
            has_real_atom = true;
            continue;
        }

        let Some(surface) = RichTextTagSurface::scan(parser, content_end) else {
            let plain_end = next_dialogue_surface_start(parser, end);
            let plain_range = SourceRange::new(start, plain_end);
            let source = parser
                .source()
                .get(plain_range.as_range())
                .expect("plain dialogue text remains inside its source");
            if is_real_dialogue_text(source) {
                emit_text_node(
                    parser,
                    plain_range,
                    source.into(),
                    &mut nodes,
                    &mut components,
                );
                has_real_atom = true;
            } else {
                emit_error_node(
                    parser,
                    plain_range,
                    SyntaxDialogueContentIssue::UnclassifiedToken,
                    &mut nodes,
                    &mut components,
                );
            }
            continue;
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
                &mut nodes,
                &mut components,
            );
            continue;
        }

        let tag_ordinal = u32::try_from(tags.len()).expect("RichText tag limit fits u32");
        match surface.body {
            RichTextTagBody::Open(open) => {
                let scanned_arguments = (!open.attrs.is_empty()
                    && !matches!(open.source_name, "mark" | "fx" | "call" | "!" | "if"))
                .then(|| {
                    let remaining = MAX_RICH_TEXT_CONTENT_ARGUMENTS
                        .checked_sub(argument_count)
                        .expect("retained RichText argument count stays inside its limit");
                    scan_tag_arguments(open.attrs, open.attrs_range.start(), remaining)
                });
                if let Some(diagnostic) = scanned_arguments.as_ref().and_then(|scanned| {
                    scanned.diagnostics().iter().find(|diagnostic| {
                        matches!(
                            diagnostic.code(),
                            DialogueTextDiagnosticCode::RichTextAttributeTooMany
                                | DialogueTextDiagnosticCode::RichTextAttributeKeyTooLong
                                | DialogueTextDiagnosticCode::RichTextAttributeValueTooLong
                                | DialogueTextDiagnosticCode::RichTextContentArgumentLimit
                        )
                    })
                }) {
                    let publish = diagnostic.code()
                        != DialogueTextDiagnosticCode::RichTextContentArgumentLimit
                        || !core::mem::replace(&mut argument_limit_exhausted, true);
                    if publish {
                        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
                            diagnostic.code().as_str(),
                            source_range(*diagnostic.range()),
                            diagnostic.message(),
                        )));
                    }
                    let range = SourceRange::new(start, surface.end);
                    let text = parser.source()[range.as_range()].into();
                    emit_text_node(parser, range, text, &mut nodes, &mut components);
                    content_tag_count = content_tag_count
                        .checked_add(1)
                        .expect("RichText tag count remains grammar-bounded");
                    has_real_atom = true;
                    continue;
                }
                let emitted = emit_open_tag(
                    parser,
                    surface,
                    open,
                    tag_ordinal,
                    scanned_arguments,
                    &mut argument_count,
                    &mut argument_limit_exhausted,
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
                    open_spans.push(OpenRichTextSpan {
                        tag: tag_ordinal,
                        identity,
                    });
                }
            }
            RichTextTagBody::End { name_range } => {
                let identity = tag_identity(
                    parser
                        .source()
                        .get(name_range.as_range())
                        .expect("RichText end tag name remains in source"),
                );
                let node = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
                let matched = open_spans
                    .iter()
                    .rposition(|span| span.identity == identity)
                    .map(|position| open_spans.remove(position));
                emit_end_tag(
                    parser,
                    surface,
                    name_range,
                    matched.as_ref().map_or(tag_ordinal, |span| span.tag),
                );
                let issue = matched
                    .is_none()
                    .then_some(SyntaxRichTextIssue::InvalidNesting);
                nodes.push(SyntaxDialogueNodeProjection::AuthoredEndTag(
                    SyntaxRichTextEndTagProjection::new(Some(identity), false, issue),
                ));
                components.push(PendingExpressionComponent::new(
                    ExpressionComponentRole::DialogueNode {
                        ordinal: node,
                        part: SyntaxDialogueNodeSourcePart::Whole,
                    },
                    SourceRange::new(start, surface.end),
                ));
                if let Some(span) = matched {
                    let paired = tags
                        .get_mut(span.tag as usize)
                        .expect("open span tag ordinal remains live")
                        .pair_with_end_node(node);
                    assert!(paired, "one RichText start tag pairs with one end node");
                    components.push(PendingExpressionComponent::new(
                        ExpressionComponentRole::RichTextTag {
                            tag: span.tag,
                            part: SyntaxRichTextTagSourcePart::EndTag,
                        },
                        SourceRange::new(start, surface.end),
                    ));
                }
            }
        }
        content_tag_count = content_tag_count
            .checked_add(1)
            .expect("RichText tag count remains grammar-bounded");
        has_real_atom = true;
    }

    for span in open_spans {
        let node = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
        parser.start(SyntaxKind::DialogueError, SyntaxRole::DialogueNode(node));
        parser.finish();
        nodes.push(SyntaxDialogueNodeProjection::Error(
            SyntaxDialogueContentIssue::UnclosedTag,
        ));
        components.extend([
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialogueNode {
                    ordinal: node,
                    part: SyntaxDialogueNodeSourcePart::Whole,
                },
                SourceRange::new(content_end, content_end),
            ),
            PendingExpressionComponent::new(
                ExpressionComponentRole::DialogueNode {
                    ordinal: node,
                    part: SyntaxDialogueNodeSourcePart::Error,
                },
                SourceRange::new(content_end, content_end),
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
            .map_or(SourceRange::new(content_end, content_end), |component| {
                component.range()
            });
        parser.push(SyntaxEvent::Diagnostic(PendingSyntaxDiagnostic::new(
            "syntax.rich_text.tag.unclosed",
            tag_range,
            "RichText start tag has no matching end tag",
        )));
    }
    parser.finish();

    let projection = if saw_nontrivia {
        SyntaxDialogueContentProjection::Present(SyntaxDialogueContent::new(nodes, tags))
    } else {
        SyntaxDialogueContentProjection::Missing {
            boundary: missing_boundary,
        }
    };
    EmittedDialogueContent {
        projection,
        components,
        has_real_atom,
    }
}

const fn dialogue_node_source_part(
    node: &SyntaxDialogueNodeProjection,
) -> Option<SyntaxDialogueNodeSourcePart> {
    match node {
        SyntaxDialogueNodeProjection::Control(_) => Some(SyntaxDialogueNodeSourcePart::Control),
        SyntaxDialogueNodeProjection::Mark(_) => Some(SyntaxDialogueNodeSourcePart::Mark),
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

fn next_dialogue_surface_start(parser: &ShadowDocumentParser<'_, '_>, end: usize) -> usize {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    surface: &ScannedDialogueSurface,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    tags: &mut Vec<SyntaxRichTextTagProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let whole = SourceRange::new(surface.range().start(), surface.range().end());
    let ordinal = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    match surface.kind() {
        ScannedDialogueSurfaceKind::Escape { escaped, value, .. } => {
            emit_dialogue_range_owner(
                parser,
                SyntaxKind::DialogueEscape,
                SyntaxRole::DialogueNode(ordinal),
                whole,
            );
            nodes.push(SyntaxDialogueNodeProjection::Escape(*value));
            components.extend(dialogue_node_components(
                ordinal,
                whole,
                SyntaxDialogueNodeSourcePart::Escape,
                SourceRange::new(escaped.start(), escaped.end()),
            ));
        }
        ScannedDialogueSurfaceKind::Ruby(ruby) => {
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
        ScannedDialogueSurfaceKind::Raw { body, .. } => {
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
        ScannedDialogueSurfaceKind::Interpolation {
            open,
            payload,
            close,
            ..
        } => {
            parser.start(
                SyntaxKind::DialogueInterpolation,
                SyntaxRole::DialogueNode(ordinal),
            );
            emit_range_node(
                parser,
                SyntaxKind::OpenBracketNode,
                SyntaxRole::OpenDelimiter,
                *open,
            );
            let expression_end = parser
                .token_boundary_index(payload.end())
                .expect("interpolation payload ends at a lexer boundary");
            let expression = emit_expression_node(parser, expression_end, SyntaxRole::Operand);
            let slot = completed_slot(parser, expression);
            bump_until_offset(parser, close.start());
            emit_range_node(
                parser,
                SyntaxKind::CloseBracketNode,
                SyntaxRole::CloseDelimiter,
                *close,
            );
            parser.finish();
            nodes.push(SyntaxDialogueNodeProjection::Interpolation(slot));
            components.extend(dialogue_node_components(
                ordinal,
                whole,
                SyntaxDialogueNodeSourcePart::Interpolation,
                SourceRange::new(payload.start(), payload.end()),
            ));
        }
        ScannedDialogueSurfaceKind::InlineStyle(style) => {
            emit_inline_style(parser, surface, style, nodes, tags, components);
        }
    }
}

fn emit_dialogue_range_owner(
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    surface: &ScannedDialogueSurface,
    style: &ScannedInlineStyle,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    tags: &mut Vec<SyntaxRichTextTagProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) {
    let tag_ordinal = u32::try_from(tags.len()).expect("RichText tag limit fits u32");
    let start_node = u32::try_from(nodes.len()).expect("dialogue node limit fits u32");
    let text_node = start_node
        .checked_add(1)
        .expect("dialogue node ordinal fits u32");
    let end_node = text_node
        .checked_add(1)
        .expect("dialogue node ordinal fits u32");
    let whole = surface.range();
    let prefix = TextRange::new(whole.start(), style.body().range().start());
    let suffix = TextRange::new(style.body().range().end(), whole.end());
    let identity = SyntaxRichTextTagIdentity::Builtin(SyntaxBuiltinRichTextTag::DirectStyle(
        match style.style() {
            ScannedInlineStyleKind::Emphasis => SyntaxRichTextDirectStyle::Emphasis,
            ScannedInlineStyleKind::Strong => SyntaxRichTextDirectStyle::Strong,
            ScannedInlineStyleKind::Color => SyntaxRichTextDirectStyle::Color,
        },
    ));
    let mut cursor = PartitionedEventCursor::new(parser, whole.start());
    cursor.start(
        SyntaxKind::RichTextTag,
        SyntaxRole::RichTextTag(tag_ordinal),
    );
    cursor.emit_to(style.name().start());
    cursor.start(SyntaxKind::RichTextTagName, SyntaxRole::Name);
    cursor.emit_to(style.name().end());
    cursor.finish();
    let (arguments, argument_components) = match style.value() {
        Some(value) => {
            cursor.emit_to(value.token_range().start());
            cursor.start(
                SyntaxKind::RichTextPositionalArgument,
                SyntaxRole::Argument(0),
            );
            emit_present_value(&mut cursor, value);
            cursor.finish();
            (
                vec![SyntaxRichTextArgumentProjection::Positional {
                    value: SyntaxRichTextValue::new(value.decoded()),
                }],
                vec![
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::RichTextArgument {
                            tag: tag_ordinal,
                            argument: 0,
                            part: SyntaxRichTextArgumentSourcePart::Whole,
                        },
                        SourceRange::new(value.token_range().start(), value.token_range().end()),
                    ),
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::RichTextArgument {
                            tag: tag_ordinal,
                            argument: 0,
                            part: SyntaxRichTextArgumentSourcePart::Value,
                        },
                        SourceRange::new(
                            value.content_range().start(),
                            value.content_range().end(),
                        ),
                    ),
                ],
            )
        }
        None => (Vec::new(), Vec::new()),
    };
    cursor.emit_to(prefix.end());
    cursor.finish();

    cursor.start(
        SyntaxKind::DialogueText,
        SyntaxRole::DialogueNode(text_node),
    );
    cursor.emit_to(style.body().range().end());
    cursor.finish();
    cursor.start(
        SyntaxKind::RichTextEndTag,
        SyntaxRole::RichTextTag(tag_ordinal),
    );
    cursor.emit_to(suffix.end());
    cursor.finish();
    cursor.finish_at(whole.end());

    tags.push(SyntaxRichTextTagProjection::new(
        identity.clone(),
        arguments,
        SyntaxRichTextTagPayloadProjection::Arguments,
        Some(end_node),
    ));
    nodes.extend([
        SyntaxDialogueNodeProjection::InferredStartTag { tag: tag_ordinal },
        SyntaxDialogueNodeProjection::Text(style.body().value().into()),
        SyntaxDialogueNodeProjection::InferredEndTag(SyntaxRichTextEndTagProjection::new(
            Some(identity),
            true,
            None,
        )),
    ]);

    components.extend([
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: start_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(prefix.start(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: text_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(style.body().range().start(), style.body().range().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: text_node,
                part: SyntaxDialogueNodeSourcePart::Text,
            },
            SourceRange::new(style.body().range().start(), style.body().range().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::DialogueNode {
                ordinal: end_node,
                part: SyntaxDialogueNodeSourcePart::Whole,
            },
            SourceRange::new(suffix.start(), suffix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::Whole,
            },
            SourceRange::new(prefix.start(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
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
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::Name,
            },
            SourceRange::new(style.name().start(), style.name().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::Payload,
            },
            SourceRange::new(style.name().end(), prefix.end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::CloseDelimiter,
            },
            SourceRange::new(style.separator().start(), style.separator().end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::InferenceInsertion,
            },
            SourceRange::new(style.inferred_end(), style.inferred_end()),
        ),
        PendingExpressionComponent::new(
            ExpressionComponentRole::RichTextTag {
                tag: tag_ordinal,
                part: SyntaxRichTextTagSourcePart::EndTag,
            },
            SourceRange::new(suffix.start(), suffix.end()),
        ),
    ]);
    components.extend(argument_components);
}

#[derive(Clone, Copy)]
enum RichTextContentLimit {
    Tags,
    Arguments,
}

fn emit_tag_after_content_limit(
    parser: &mut ShadowDocumentParser<'_, '_>,
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

fn emit_typed_dialogue_surface(
    parser: &mut ShadowDocumentParser<'_, '_>,
    start: usize,
    content_end: usize,
    content_tag_count: &mut usize,
    argument_count: &mut usize,
    tag_limit_exhausted: &mut bool,
    argument_limit_exhausted: &mut bool,
    nodes: &mut Vec<SyntaxDialogueNodeProjection>,
    tags: &mut Vec<SyntaxRichTextTagProjection>,
    components: &mut Vec<PendingExpressionComponent>,
) -> bool {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    fn scan(parser: &ShadowDocumentParser<'source, '_>, content_end: usize) -> Option<Self> {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    surface: RichTextTagSurface<'_>,
    open: OpenTagSurface<'_>,
    ordinal: u32,
    scanned_arguments: Option<ScannedTagArguments>,
    content_argument_count: &mut usize,
    argument_limit_exhausted: &mut bool,
) -> EmittedOpenTag {
    let identity = tag_identity(open.source_name);
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
    parser.start(SyntaxKind::RichTextTag, SyntaxRole::RichTextTag(ordinal));
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    bump_to_range_start(parser, open.name_range);
    emit_range_node(
        parser,
        SyntaxKind::RichTextTagName,
        SyntaxRole::Name,
        open.name_range,
    );

    let mut arguments = Vec::new();
    let mut payload = SyntaxRichTextTagPayloadProjection::None;
    if !open.attrs.is_empty() {
        bump_to_range_start(parser, open.attrs_range);
        match open.source_name {
            "mark" => bump_until_offset(parser, open.attrs_range.end()),
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
                *content_argument_count = content_argument_count
                    .checked_add(scanned.entries().len())
                    .expect("retained RichText argument count remains grammar-bounded");
                arguments = scanned
                    .entries()
                    .iter()
                    .map(|argument| syntax_argument(parser.source(), argument))
                    .collect();
                components.extend(argument_components(ordinal, scanned.entries()));
                emit_argument_payload(
                    parser,
                    open.attrs_range,
                    &scanned,
                    surface.unterminated_quote.is_some(),
                    argument_limit_exhausted,
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

    let node = match open.source_name {
        "p" | "page" => SyntaxDialogueNodeProjection::LineBreak(SyntaxLineBreakKind::Page),
        "r" | "nl" | "br" => SyntaxDialogueNodeProjection::LineBreak(SyntaxLineBreakKind::Line),
        "mark" => SyntaxDialogueNodeProjection::Mark(SyntaxName::try_new(
            open.attrs.trim_start_matches('.'),
        )),
        source => SyntaxDialogueControl::from_source_name(source).map_or(
            SyntaxDialogueNodeProjection::AuthoredStartTag { tag: ordinal },
            SyntaxDialogueNodeProjection::Control,
        ),
    };
    EmittedOpenTag {
        identity,
        arguments,
        payload,
        node,
        components,
    }
}

fn tag_identity(source: &str) -> SyntaxRichTextTagIdentity {
    SyntaxRichTextTagIdentity::from_source_name(source)
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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

fn emit_argument(
    parser: &mut ShadowDocumentParser<'_, '_>,
    argument: &ScannedTagArgument,
    ordinal: u16,
) {
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
    parser: &'parser mut ShadowDocumentParser<'source, 'events>,
    offset: usize,
}

impl<'parser, 'source, 'events> PartitionedEventCursor<'parser, 'source, 'events> {
    fn new(parser: &'parser mut ShadowDocumentParser<'source, 'events>, offset: usize) -> Self {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
    kind: SyntaxKind,
    role: SyntaxRole,
    range: TextRange,
) {
    parser.start(kind, role);
    bump_until_offset(parser, range.end());
    parser.finish();
}

fn bump_to_range_start(parser: &mut ShadowDocumentParser<'_, '_>, range: TextRange) {
    bump_until_offset(parser, range.start());
}

fn bump_until_offset(parser: &mut ShadowDocumentParser<'_, '_>, end: usize) {
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
    parser: &mut ShadowDocumentParser<'_, '_>,
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
