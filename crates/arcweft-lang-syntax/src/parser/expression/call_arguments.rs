//! Parenthesized Call argument ownership on the shared expression cursor.

use super::{
    DocumentParser, ExpressionComponentRole, PendingExpressionComponent, SourceRange,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
    SyntaxKind, SyntaxName, SyntaxRequiredTokenState, SyntaxRole, bump_until, completed_slot,
    emit_close_delimiter, emit_expression_node, emit_open_delimiter, find_top_level_boundary,
    trimmed_end,
};

pub(in crate::parser) struct EmittedParenthesizedCallTail {
    pub(super) arguments: Vec<SyntaxCallArgumentProjection>,
    pub(super) terminator: SyntaxCallArgumentListTerminator,
    pub(super) components: Vec<PendingExpressionComponent>,
}

impl EmittedParenthesizedCallTail {
    pub(in crate::parser) fn into_parts(
        self,
    ) -> (
        Vec<SyntaxCallArgumentProjection>,
        SyntaxCallArgumentListTerminator,
        Vec<PendingExpressionComponent>,
    ) {
        (self.arguments, self.terminator, self.components)
    }
}

pub(in crate::parser) fn emit_parenthesized_call_tail(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
) -> EmittedParenthesizedCallTail {
    let open = parser
        .current()
        .expect("postfix Call dispatch retains the opening parenthesis")
        .range();
    emit_open_delimiter(parser, SyntaxKind::OpenParenNode, "(");
    parser.start(SyntaxKind::ArgumentList, SyntaxRole::Element(0));
    let mut arguments = Vec::new();
    let mut argument_components = Vec::new();
    let mut separators = Vec::new();
    loop {
        parser.bump_trivia_before(end);
        if parser.cursor() >= end || parser.at(")") {
            break;
        }
        let ordinal = u16::try_from(arguments.len())
            .expect("document grammar budget keeps Call argument ordinals in u16");
        let argument_end = find_top_level_boundary(parser, parser.cursor(), end, &[",", ")"]);
        let argument = emit_call_argument(parser, argument_end, ordinal);
        arguments.push(argument.projection);
        argument_components.extend(argument.components);
        if parser.at(",") {
            separators.push(
                parser
                    .bump()
                    .expect("Call separator dispatch retains one comma")
                    .range(),
            );
        } else {
            break;
        }
    }
    parser.finish();
    let (terminator, terminator_role, terminator_range) = if parser.at(")") {
        (
            SyntaxCallArgumentListTerminator::Closed,
            ExpressionComponentRole::CallArgumentListClose,
            parser
                .current()
                .expect("closed Call retains one closing parenthesis")
                .range(),
        )
    } else {
        let at = parser.current_offset();
        (
            SyntaxCallArgumentListTerminator::RecoveredMissing,
            ExpressionComponentRole::CallArgumentListRecoveryEnd,
            SourceRange::new(at, at),
        )
    };
    emit_close_delimiter(
        parser,
        SyntaxKind::CloseParenNode,
        ")",
        "syntax.expression.missing_call_close",
    );
    let mut components = vec![PendingExpressionComponent::new(
        ExpressionComponentRole::CallArgumentListOpen,
        open,
    )];
    components.append(&mut argument_components);
    for (after, separator) in separators.into_iter().enumerate() {
        if after + 1 < arguments.len() {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallArgumentSeparator {
                    following: u16::try_from(after + 1)
                        .expect("Call argument separator ordinal fits u16"),
                },
                separator,
            ));
        } else {
            components.push(PendingExpressionComponent::new(
                ExpressionComponentRole::CallArgumentTrailingSeparator,
                separator,
            ));
        }
    }
    if arguments.is_empty() {
        components.push(PendingExpressionComponent::new(
            ExpressionComponentRole::CallArgumentListEmptyInsertion,
            SourceRange::new(open.end(), open.end()),
        ));
    }
    components.push(PendingExpressionComponent::new(
        terminator_role,
        terminator_range,
    ));
    EmittedParenthesizedCallTail {
        arguments,
        terminator,
        components,
    }
}

struct EmittedCallArgument {
    projection: SyntaxCallArgumentProjection,
    components: Vec<PendingExpressionComponent>,
}

fn emit_call_argument(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> EmittedCallArgument {
    let start_event = parser.event_position();
    parser.start(SyntaxKind::CallArgument, SyntaxRole::Argument(ordinal));
    let assignment = find_top_level_boundary(parser, parser.cursor(), end, &["="]);
    let (projection, mut components) = if assignment < end {
        emit_named_call_argument(parser, end, assignment, ordinal)
    } else {
        emit_unnamed_call_argument(parser, end, ordinal)
    };
    bump_until(parser, end);
    parser.finish();
    let whole = parser
        .completed_range(start_event)
        .expect("Call argument retains one exact whole range");
    components.push(PendingExpressionComponent::new(
        ExpressionComponentRole::CallArgument {
            argument: ordinal,
            part: SyntaxCallArgumentPart::Whole,
        },
        whole,
    ));
    EmittedCallArgument {
        projection,
        components,
    }
}

fn emit_named_call_argument(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    assignment: usize,
    ordinal: u16,
) -> (
    SyntaxCallArgumentProjection,
    Vec<PendingExpressionComponent>,
) {
    let name_start = parser.current_offset();
    let name_end_index = trimmed_end(parser, parser.cursor(), assignment);
    let name_end = parser
        .offset_at_token_boundary(name_end_index)
        .expect("Call name end remains at a token boundary");
    let name_range = SourceRange::new(name_start, name_end);
    let source_name = SyntaxName::try_new(&parser.source()[name_range.as_range()]);
    parser.start(SyntaxKind::NameReference, SyntaxRole::Name);
    bump_until(parser, name_end_index);
    parser.finish();
    bump_until(parser, assignment);
    let equals = parser
        .bump()
        .expect("named Call argument retains one equals token")
        .range();
    parser.bump_trivia_before(end);
    let value = emit_expression_node(parser, end, SyntaxRole::Operand);
    let value_range = parser
        .completed_range(value.start_event)
        .expect("named Call value retains one exact source range");
    (
        SyntaxCallArgumentProjection::Named {
            name: source_name,
            equals: SyntaxRequiredTokenState::Present,
            value: completed_slot(parser, value),
        },
        vec![
            call_argument_component(ordinal, SyntaxCallArgumentPart::Name, name_range),
            call_argument_component(ordinal, SyntaxCallArgumentPart::Equals, equals),
            call_argument_component(ordinal, SyntaxCallArgumentPart::Value, value_range),
        ],
    )
}

fn emit_unnamed_call_argument(
    parser: &mut DocumentParser<'_, '_>,
    end: usize,
    ordinal: u16,
) -> (
    SyntaxCallArgumentProjection,
    Vec<PendingExpressionComponent>,
) {
    let spread = find_top_level_boundary(parser, parser.cursor(), end, &["..."]);
    let value = emit_expression_node(parser, spread, SyntaxRole::Operand);
    let value_range = parser
        .completed_range(value.start_event)
        .expect("Call argument value retains one exact source range");
    let value_slot = completed_slot(parser, value);
    bump_until(parser, spread);
    let mut components = vec![call_argument_component(
        ordinal,
        SyntaxCallArgumentPart::Value,
        value_range,
    )];
    let projection = if parser.at("...") {
        let ellipsis = parser
            .bump()
            .expect("spread Call argument retains one ellipsis token")
            .range();
        components.push(call_argument_component(
            ordinal,
            SyntaxCallArgumentPart::Spread,
            ellipsis,
        ));
        SyntaxCallArgumentProjection::Spread {
            value: value_slot,
            ellipsis: SyntaxRequiredTokenState::Present,
        }
    } else {
        SyntaxCallArgumentProjection::Positional { value: value_slot }
    };
    (projection, components)
}

fn call_argument_component(
    ordinal: u16,
    part: SyntaxCallArgumentPart,
    range: SourceRange,
) -> PendingExpressionComponent {
    PendingExpressionComponent::new(
        ExpressionComponentRole::CallArgument {
            argument: ordinal,
            part,
        },
        range,
    )
}
