use arcweft_source::SourceRange;

use super::{CompletedNode, completed_slot, parse_binding_power};
use crate::expressions::{
    ExpressionComponentRole, ExpressionProjection, PendingExpressionComponent,
    PendingExpressionProjection, SyntaxBracketTerminator, SyntaxCandidateQuality,
    SyntaxDialogueApplicationForm, SyntaxDialogueApplicationProjection,
    SyntaxDialogueContentProjection, SyntaxDialogueContentRecoveryBoundary, SyntaxExpressionSlot,
    SyntaxIndexProjection, SyntaxPostfixBracketProjection, SyntaxPostfixBracketRecoveryBoundary,
    SyntaxPostfixCandidateFailure, SyntaxPostfixCandidateFailureKind,
    SyntaxPostfixCandidateFailureSite, SyntaxPostfixDialogueCandidate, SyntaxPostfixIndexCandidate,
};
use crate::grammar::kinds::{SyntaxKind, SyntaxRole};
use crate::parser::cursor::{
    CandidateTokenInterval, DocumentParser, StagedParserEvents, is_trivia_kind,
};
use crate::parser::rich_text_grammar::emit_dialogue_content;
use crate::parser::shadow_recovery::{
    bump_until, emit_close_delimiter, emit_open_delimiter, find_top_level_boundary,
};

pub(super) fn emit_postfix_bracket(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    left: CompletedNode,
    role: SyntaxRole,
) -> CompletedNode {
    let target_range = parser
        .completed_range(left.start_event)
        .expect("completed postfix target retains one exact source range");
    let open_range = parser
        .current()
        .expect("postfix bracket dispatch retains its opening token")
        .range();
    let owner =
        parser.insert_projected_start(left.start_event, SyntaxKind::PostfixBracketExpression, role);
    parser.set_start_role(left.start_event + 1, SyntaxRole::Target);
    emit_open_delimiter(parser, SyntaxKind::OpenBracketNode, "[");
    let staged = match stage_postfix_bracket_candidates(parser, end, target_range, open_range) {
        Ok(staged) => staged,
        Err(interval) => return finish_postfix_limit(parser, interval, left),
    };
    let mut projection = select_postfix_bracket_projection(
        parser,
        staged.interval,
        staged.index,
        staged.dialogue,
        &staged.sources,
    );
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_postfix_bracket_close",
    );
    if matches!(
        projection.projection(),
        ExpressionProjection::DialogueContentApplication(_)
    ) && let Some((with, _, "with")) = parser.next_significant()
        && with < end
    {
        bump_until(parser, with);
        let plan = crate::parser::statement::dialogue_plan::emit_dialogue_line_plan(
            parser,
            end,
            SyntaxKind::FlowItem,
        );
        let ExpressionProjection::DialogueContentApplication(application) =
            projection.projection().clone()
        else {
            unreachable!("selected Dialogue projection remains Dialogue-owned")
        };
        let mut components = projection.components().to_vec();
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::Plan,
            plan,
        ));
        projection = PendingExpressionProjection::new(
            ExpressionProjection::DialogueContentApplication(
                SyntaxDialogueApplicationProjection::new(
                    application.form().clone(),
                    application.content().clone(),
                    true,
                ),
            ),
            components,
        );
    }
    if matches!(
        projection.projection(),
        ExpressionProjection::DialogueContentApplication(_)
    ) && let Some(owner) = owner
    {
        parser.set_start_kind(owner, SyntaxKind::DialogueContentApplicationExpression);
    }
    parser.set_expression_projection(owner, projection);
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

struct StagedPostfixBracket {
    interval: CandidateTokenInterval,
    index: StagedIndexAttempt,
    dialogue: StagedDialogueAttempt,
    sources: PostfixBracketSources,
}

fn stage_postfix_bracket_candidates(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    target: SourceRange,
    open: SourceRange,
) -> Result<StagedPostfixBracket, CandidateTokenInterval> {
    let payload_end = find_top_level_boundary(parser, parser.cursor(), end, &["]"]);
    let interval = parser
        .candidate_interval(payload_end)
        .expect("postfix candidates share one validated lexer interval");
    let payload_range = SourceRange::new(
        parser
            .offset_at_token_boundary(interval.start())
            .expect("postfix payload starts at a lexer boundary"),
        parser
            .offset_at_token_boundary(interval.end())
            .expect("postfix payload ends at a lexer boundary"),
    );
    let close_range = parser
        .token_at(payload_end)
        .filter(|token| parser.text_of(*token) == "]")
        .map_or_else(
            || SourceRange::new(payload_range.end(), payload_range.end()),
            super::super::lexer::LexToken::range,
        );
    let terminator = if parser
        .token_at(payload_end)
        .is_some_and(|token| parser.text_of(token) == "]")
    {
        SyntaxBracketTerminator::Closed
    } else {
        SyntaxBracketTerminator::RecoveredMissing(
            SyntaxPostfixBracketRecoveryBoundary::EndOfExpression {
                anchor: payload_range.end(),
            },
        )
    };

    let index = stage_index_candidate(parser, interval);
    if matches!(index, StagedIndexAttempt::LimitExceeded) {
        return Err(interval);
    }
    let missing_content_boundary = if matches!(terminator, SyntaxBracketTerminator::Closed) {
        SyntaxDialogueContentRecoveryBoundary::CloseBracket { range: close_range }
    } else {
        SyntaxDialogueContentRecoveryBoundary::MissingBracketClose {
            insertion: payload_range.end(),
        }
    };
    let dialogue = stage_dialogue_candidate(parser, interval, missing_content_boundary);
    if matches!(dialogue, StagedDialogueAttempt::LimitExceeded) {
        return Err(interval);
    }
    Ok(StagedPostfixBracket {
        interval,
        index,
        dialogue,
        sources: PostfixBracketSources {
            target,
            open,
            close: close_range,
            payload: payload_range,
            terminator,
        },
    })
}

struct PostfixBracketSources {
    target: SourceRange,
    open: SourceRange,
    close: SourceRange,
    payload: SourceRange,
    terminator: SyntaxBracketTerminator,
}

fn select_postfix_bracket_projection(
    parser: &mut DocumentParser<'_, '_>,
    interval: CandidateTokenInterval,
    index: StagedIndexAttempt,
    dialogue: StagedDialogueAttempt,
    sources: &PostfixBracketSources,
) -> PendingExpressionProjection {
    match (index, dialogue) {
        (
            StagedIndexAttempt::Viable {
                events,
                index,
                range,
                ..
            },
            StagedDialogueAttempt::Failed(_),
        ) => {
            parser.commit_selected(events);
            PendingExpressionProjection::new(
                ExpressionProjection::Index(SyntaxIndexProjection::new(
                    SyntaxExpressionSlot::Authored,
                    index,
                    sources.terminator.clone(),
                )),
                vec![
                    PendingExpressionComponent::new(
                        ExpressionComponentRole::Target,
                        sources.target,
                    ),
                    PendingExpressionComponent::new(ExpressionComponentRole::Index, range),
                ],
            )
        }
        (
            StagedIndexAttempt::Failed(_),
            StagedDialogueAttempt::Viable {
                events,
                content,
                components,
                ..
            },
        ) => {
            parser.commit_selected(events);
            let mut source_components = dialogue_application_components(sources);
            source_components.extend(components);
            PendingExpressionProjection::new(
                ExpressionProjection::DialogueContentApplication(
                    SyntaxDialogueApplicationProjection::new(
                        SyntaxDialogueApplicationForm::Bracket {
                            terminator: sources.terminator.clone(),
                        },
                        content,
                        false,
                    ),
                ),
                source_components,
            )
        }
        (
            index @ StagedIndexAttempt::Viable { .. },
            dialogue @ StagedDialogueAttempt::Viable { .. },
        ) => select_ambiguous_postfix_projection(parser, interval, index, dialogue, sources),
        (StagedIndexAttempt::Failed(index), StagedDialogueAttempt::Failed(dialogue)) => {
            parser.start(SyntaxKind::PostfixBracketPayload, SyntaxRole::Payload);
            parser.emit_raw_interval(interval);
            parser.finish();
            PendingExpressionProjection::new(
                ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Invalid {
                    index,
                    dialogue,
                }),
                postfix_bracket_components(
                    sources.target,
                    sources.open,
                    sources.close,
                    sources.payload,
                ),
            )
        }
        (StagedIndexAttempt::LimitExceeded, _) | (_, StagedDialogueAttempt::LimitExceeded) => {
            unreachable!("candidate limit exhaustion returns before classification")
        }
    }
}

fn select_ambiguous_postfix_projection(
    parser: &mut DocumentParser<'_, '_>,
    interval: CandidateTokenInterval,
    index: StagedIndexAttempt,
    dialogue: StagedDialogueAttempt,
    sources: &PostfixBracketSources,
) -> PendingExpressionProjection {
    let StagedIndexAttempt::Viable {
        events: index_events,
        quality: index_quality,
        ..
    } = index
    else {
        unreachable!("ambiguous postfix selection requires a viable index candidate")
    };
    let StagedDialogueAttempt::Viable {
        events: dialogue_events,
        quality: dialogue_quality,
        content,
        components,
    } = dialogue
    else {
        unreachable!("ambiguous postfix selection requires a viable dialogue candidate")
    };
    let index_graph = index_events.into_candidate_graph();
    let index_node = *index_graph
        .roots()
        .first()
        .expect("viable index candidate retains one root expression");
    let dialogue_graph = dialogue_events.into_candidate_graph();
    parser.start(SyntaxKind::PostfixBracketPayload, SyntaxRole::Payload);
    parser.emit_raw_interval(interval);
    parser.finish();
    PendingExpressionProjection::new(
        ExpressionProjection::PostfixBracket(SyntaxPostfixBracketProjection::Ambiguous {
            index: Box::new(SyntaxPostfixIndexCandidate::new(
                index_quality,
                index_node,
                index_graph,
            )),
            dialogue: Box::new(SyntaxPostfixDialogueCandidate::new(
                dialogue_quality,
                content,
                components,
                dialogue_graph,
            )),
        }),
        postfix_bracket_components(sources.target, sources.open, sources.close, sources.payload),
    )
}

fn dialogue_application_components(
    sources: &PostfixBracketSources,
) -> Vec<PendingExpressionComponent> {
    vec![
        PendingExpressionComponent::new(ExpressionComponentRole::Target, sources.target),
        PendingExpressionComponent::new(ExpressionComponentRole::OpenBracket, sources.open),
        PendingExpressionComponent::new(ExpressionComponentRole::CloseBracket, sources.close),
        PendingExpressionComponent::new(ExpressionComponentRole::Content, sources.payload),
        PendingExpressionComponent::new(ExpressionComponentRole::ContentBody, sources.payload),
    ]
}

fn finish_postfix_limit(
    parser: &mut DocumentParser<'_, '_>,
    interval: CandidateTokenInterval,
    left: CompletedNode,
) -> CompletedNode {
    parser.emit_raw_interval(interval);
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseBracketNode,
        "]",
        "syntax.expression.missing_postfix_bracket_close",
    );
    parser.finish();
    CompletedNode {
        start_event: left.start_event,
    }
}

enum StagedIndexAttempt {
    Viable {
        events: StagedParserEvents,
        quality: SyntaxCandidateQuality,
        index: SyntaxExpressionSlot,
        range: SourceRange,
    },
    Failed(SyntaxPostfixCandidateFailure),
    LimitExceeded,
}

enum StagedDialogueAttempt {
    Viable {
        events: StagedParserEvents,
        quality: SyntaxCandidateQuality,
        content: SyntaxDialogueContentProjection,
        components: Vec<PendingExpressionComponent>,
    },
    Failed(SyntaxPostfixCandidateFailure),
    LimitExceeded,
}

fn stage_index_candidate(
    parser: &mut DocumentParser<'_, '_>,
    interval: CandidateTokenInterval,
) -> StagedIndexAttempt {
    let checkpoint = parser.checkpoint_candidate(interval);
    parser.start(SyntaxKind::PostfixBracketPayload, SyntaxRole::Payload);
    bump_candidate_trivia(parser, interval.end());
    if parser.cursor() == interval.end() {
        parser.finish();
        return match parser.stage_candidate(checkpoint) {
            Ok(_) => StagedIndexAttempt::Failed(SyntaxPostfixCandidateFailure::new(
                SyntaxPostfixCandidateFailureKind::EmptyPayload,
                SyntaxPostfixCandidateFailureSite::Insertion(parser.current_offset()),
            )),
            Err(_) => StagedIndexAttempt::LimitExceeded,
        };
    }

    let expression = parse_binding_power(parser, interval.end(), 0, SyntaxRole::Argument(0));
    if parser.budget_failed() {
        bump_until(parser, interval.end());
        parser.finish();
        let _ = parser.stage_candidate(checkpoint);
        return StagedIndexAttempt::LimitExceeded;
    }
    let kind = parser
        .completed_kind(expression.start_event)
        .expect("bounded index candidate completes one expression node");
    let range = parser
        .completed_range(expression.start_event)
        .expect("bounded index candidate retains one exact expression range");
    let index = completed_slot(parser, expression);
    bump_candidate_trivia(parser, interval.end());
    let trailing = (parser.cursor() < interval.end()).then(|| {
        parser
            .current()
            .expect("trailing candidate token remains in the interval")
            .range()
    });
    bump_until(parser, interval.end());
    parser.finish();
    let Ok(staged) = parser.stage_candidate(checkpoint) else {
        return StagedIndexAttempt::LimitExceeded;
    };
    if let Some(range) = trailing {
        return StagedIndexAttempt::Failed(SyntaxPostfixCandidateFailure::new(
            SyntaxPostfixCandidateFailureKind::TrailingToken,
            SyntaxPostfixCandidateFailureSite::Span(range),
        ));
    }
    if matches!(
        kind,
        SyntaxKind::MissingExpression | SyntaxKind::ErrorExpression
    ) {
        let (kind, site) = if matches!(kind, SyntaxKind::MissingExpression) {
            (
                SyntaxPostfixCandidateFailureKind::MissingOperand,
                SyntaxPostfixCandidateFailureSite::Insertion(range.start()),
            )
        } else {
            (
                SyntaxPostfixCandidateFailureKind::UnexpectedToken,
                SyntaxPostfixCandidateFailureSite::Span(range),
            )
        };
        return StagedIndexAttempt::Failed(SyntaxPostfixCandidateFailure::new(kind, site));
    }
    let quality = if staged.has_recovery() {
        SyntaxCandidateQuality::Recovered
    } else {
        SyntaxCandidateQuality::Clean
    };
    StagedIndexAttempt::Viable {
        events: staged,
        quality,
        index,
        range,
    }
}

fn stage_dialogue_candidate(
    parser: &mut DocumentParser<'_, '_>,
    interval: CandidateTokenInterval,
    missing_boundary: SyntaxDialogueContentRecoveryBoundary,
) -> StagedDialogueAttempt {
    let has_nontrivia = (interval.start()..interval.end()).any(|index| {
        parser
            .token_at(index)
            .is_some_and(|token| !is_trivia_kind(token.kind()))
    });
    let checkpoint = parser.checkpoint_candidate(interval);
    parser.start(SyntaxKind::PostfixBracketPayload, SyntaxRole::Payload);
    let emitted = emit_dialogue_content(parser, interval.end(), missing_boundary);
    parser.finish();
    if parser.budget_failed() {
        let _ = parser.stage_candidate(checkpoint);
        return StagedDialogueAttempt::LimitExceeded;
    }
    let (content, components, has_real_atom) = emitted.into_parts();
    let Ok(staged) = parser.stage_candidate(checkpoint) else {
        return StagedDialogueAttempt::LimitExceeded;
    };
    if has_nontrivia && !has_real_atom {
        let source = SourceRange::new(
            parser
                .offset_at_token_boundary(interval.start())
                .expect("dialogue candidate starts at a lexer boundary"),
            parser
                .offset_at_token_boundary(interval.end())
                .expect("dialogue candidate ends at a lexer boundary"),
        );
        let site = if source.is_empty() {
            SyntaxPostfixCandidateFailureSite::Insertion(source.start())
        } else {
            SyntaxPostfixCandidateFailureSite::Span(source)
        };
        return StagedDialogueAttempt::Failed(SyntaxPostfixCandidateFailure::new(
            SyntaxPostfixCandidateFailureKind::InvalidDialogueAtom,
            site,
        ));
    }
    let quality = if staged.has_recovery() || content.has_recovery() {
        SyntaxCandidateQuality::Recovered
    } else {
        SyntaxCandidateQuality::Clean
    };
    StagedDialogueAttempt::Viable {
        events: staged,
        quality,
        content,
        components,
    }
}

fn bump_candidate_trivia(parser: &mut DocumentParser<'_, '_>, end: usize) {
    while parser.cursor() < end && parser.current_kind().is_some_and(is_trivia_kind) {
        let _ = parser.bump();
    }
}

fn postfix_bracket_components(
    target: SourceRange,
    open: SourceRange,
    close: SourceRange,
    content: SourceRange,
) -> Vec<PendingExpressionComponent> {
    vec![
        PendingExpressionComponent::new(ExpressionComponentRole::Target, target),
        PendingExpressionComponent::new(ExpressionComponentRole::OpenBracket, open),
        PendingExpressionComponent::new(ExpressionComponentRole::CloseBracket, close),
        PendingExpressionComponent::new(ExpressionComponentRole::Content, content),
    ]
}
