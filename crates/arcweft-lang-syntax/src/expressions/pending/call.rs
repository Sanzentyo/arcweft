//! Source-component validation for ordinary Call projections.

use std::collections::HashSet;

use arcweft_source::SourceRange;

use super::super::{
    ExpressionComponentRole, SyntaxAssociatedReceiver, SyntaxAssociatedSeparator,
    SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart, SyntaxCallArgumentProjection,
    SyntaxCallCalleeProjection, SyntaxCallProjection, SyntaxCallTypeApplicationComponentRole,
    SyntaxCallTypeApplicationProjection, SyntaxCallTypeApplicationSpelling,
    SyntaxCallTypeApplicationTerminator, SyntaxCallTypeArgumentPart,
    SyntaxCallTypeArgumentProjection, SyntaxCallbackBlockCallProjection,
    SyntaxParenthesizedCallProjection,
};
use super::{PendingExpressionComponent, component_range, exact_component_roles};
use crate::name::SyntaxNameIssue;

pub(super) fn components_validate(
    call: &SyntaxCallProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    match call {
        SyntaxCallProjection::CallbackBlock(callback) => {
            callback_components_validate(callback, owner, roles, components)
        }
        SyntaxCallProjection::Parenthesized(call) => {
            parenthesized_components_validate(call, owner, roles, components)
        }
    }
}

fn callback_components_validate(
    callback: &SyntaxCallbackBlockCallProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let terminal = argument_list_terminal_role(callback.terminator());
    let expected = [
        ExpressionComponentRole::CallCallee,
        ExpressionComponentRole::CallArgumentListOpen,
        terminal,
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Whole,
        },
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Value,
        },
    ];
    if !exact_component_roles(roles, components, &expected) {
        return false;
    }
    let callee = component_range(components, ExpressionComponentRole::CallCallee);
    let open = component_range(components, ExpressionComponentRole::CallArgumentListOpen);
    let tail = component_range(components, terminal);
    let argument = component_range(
        components,
        ExpressionComponentRole::CallArgument {
            argument: 0,
            part: SyntaxCallArgumentPart::Whole,
        },
    );
    callee
        .zip(open)
        .zip(tail.zip(argument))
        .is_some_and(|((callee, open), (tail, argument))| {
            callee.start() == owner.start()
                && callee.end() <= open.start()
                && open.start() < open.end()
                && argument.start() == open.start()
                && argument.end() == owner.end()
                && tail.end() == owner.end()
                && (callback.terminator() == SyntaxCallArgumentListTerminator::Closed
                    || tail.start() == tail.end())
        })
}

fn parenthesized_components_validate(
    call: &SyntaxParenthesizedCallProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let Some(expected) = expected_parenthesized_roles(call, roles) else {
        return false;
    };
    if !exact_component_roles(roles, components, &expected)
        || !call_head_components_validate(call.callee(), components)
    {
        return false;
    }
    let head_role = match call.callee() {
        SyntaxCallCalleeProjection::Ordinary => ExpressionComponentRole::CallCallee,
        SyntaxCallCalleeProjection::UnresolvedDot { .. }
        | SyntaxCallCalleeProjection::Associated { .. } => {
            ExpressionComponentRole::CallAssociatedReceiver
        }
    };
    if component_range(components, head_role)
        .is_none_or(|range| range.start() != owner.start() || range.end() > owner.end())
        || call.explicit_type_application().is_some_and(|application| {
            !type_application_ranges_validate(application, owner, components)
        })
    {
        return false;
    }
    argument_ranges_validate(call, owner, components)
}

fn expected_parenthesized_roles(
    call: &SyntaxParenthesizedCallProjection,
    roles: &HashSet<ExpressionComponentRole>,
) -> Option<Vec<ExpressionComponentRole>> {
    let mut expected = Vec::new();
    match call.callee() {
        SyntaxCallCalleeProjection::Ordinary => {
            expected.push(ExpressionComponentRole::CallCallee);
        }
        SyntaxCallCalleeProjection::UnresolvedDot { .. }
        | SyntaxCallCalleeProjection::Associated { .. } => {
            expected.extend([
                ExpressionComponentRole::CallAssociatedReceiver,
                ExpressionComponentRole::CallAssociatedSeparator,
                ExpressionComponentRole::CallAssociatedMember,
            ]);
        }
    }
    if let Some(application) = call.explicit_type_application() {
        append_type_application_roles(application, roles, &mut expected)?;
    }
    expected.push(ExpressionComponentRole::CallArgumentListOpen);
    expected.push(argument_list_terminal_role(call.terminator()));
    if call.arguments().is_empty() {
        expected.push(ExpressionComponentRole::CallArgumentListEmptyInsertion);
    }

    for (argument, projection) in call.arguments().iter().enumerate() {
        let argument = u16::try_from(argument).ok()?;
        append_argument_roles(argument, projection, &mut expected);
    }
    if roles.contains(&ExpressionComponentRole::CallArgumentTrailingSeparator) {
        expected.push(ExpressionComponentRole::CallArgumentTrailingSeparator);
    }
    Some(expected)
}

fn append_type_application_roles(
    application: &SyntaxCallTypeApplicationProjection,
    roles: &HashSet<ExpressionComponentRole>,
    expected: &mut Vec<ExpressionComponentRole>,
) -> Option<()> {
    expected.extend([
        ExpressionComponentRole::CallTypeApplication(SyntaxCallTypeApplicationComponentRole::Whole),
        ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::OpenAngle,
        ),
    ]);
    if application.spelling() == SyntaxCallTypeApplicationSpelling::Turbofish {
        expected.push(ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::TurbofishSeparator,
        ));
    }
    expected.push(ExpressionComponentRole::CallTypeApplication(
        type_application_terminal_role(application.terminator()),
    ));
    let empty = application.arguments().len() == 1
        && matches!(
            application.arguments()[0],
            SyntaxCallTypeArgumentProjection::Missing
        )
        && !roles.contains(&ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::Separator { following: 1 },
        ));
    if empty {
        expected.push(ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::EmptyInsertion,
        ));
    }
    for argument in 0..application.arguments().len() {
        let argument = u16::try_from(argument).ok()?;
        for part in [
            SyntaxCallTypeArgumentPart::Whole,
            SyntaxCallTypeArgumentPart::Type,
        ] {
            expected.push(ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::Argument { argument, part },
            ));
        }
        if argument > 0 {
            expected.push(ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::Separator {
                    following: argument,
                },
            ));
        }
    }
    if roles.contains(&ExpressionComponentRole::CallTypeApplication(
        SyntaxCallTypeApplicationComponentRole::TrailingSeparator,
    )) {
        expected.push(ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::TrailingSeparator,
        ));
    }
    Some(())
}

fn append_argument_roles(
    argument: u16,
    projection: &SyntaxCallArgumentProjection,
    expected: &mut Vec<ExpressionComponentRole>,
) {
    expected.extend([
        ExpressionComponentRole::CallArgument {
            argument,
            part: SyntaxCallArgumentPart::Whole,
        },
        ExpressionComponentRole::CallArgument {
            argument,
            part: SyntaxCallArgumentPart::Value,
        },
    ]);
    match projection {
        SyntaxCallArgumentProjection::Positional { .. } => {}
        SyntaxCallArgumentProjection::Named { .. } => expected.extend([
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Name,
            },
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Equals,
            },
        ]),
        SyntaxCallArgumentProjection::Spread { .. } => {
            expected.push(ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Spread,
            });
        }
    }
    if argument > 0 {
        expected.push(ExpressionComponentRole::CallArgumentSeparator {
            following: argument,
        });
    }
}

fn type_application_ranges_validate(
    application: &SyntaxCallTypeApplicationProjection,
    owner: SourceRange,
    components: &[PendingExpressionComponent],
) -> bool {
    let whole_role =
        ExpressionComponentRole::CallTypeApplication(SyntaxCallTypeApplicationComponentRole::Whole);
    let open_role = ExpressionComponentRole::CallTypeApplication(
        SyntaxCallTypeApplicationComponentRole::OpenAngle,
    );
    let terminal_role = ExpressionComponentRole::CallTypeApplication(
        type_application_terminal_role(application.terminator()),
    );
    let Some(whole) = component_range(components, whole_role) else {
        return false;
    };
    let Some(type_open) = component_range(components, open_role) else {
        return false;
    };
    let Some(type_tail) = component_range(components, terminal_role) else {
        return false;
    };
    if whole.start() < owner.start()
        || type_open.start() < whole.start()
        || type_tail.end() != whole.end()
        || matches!(
            application.terminator(),
            SyntaxCallTypeApplicationTerminator::RecoveredMissing
        ) && type_tail.start() != type_tail.end()
    {
        return false;
    }
    let mut previous_end = type_open.end();
    for argument in 0..application.arguments().len() {
        let argument = u16::try_from(argument).expect("validated Call type argument ordinal");
        let whole = component_range(
            components,
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::Argument {
                    argument,
                    part: SyntaxCallTypeArgumentPart::Whole,
                },
            ),
        )
        .expect("validated Call type argument whole component");
        let ty = component_range(
            components,
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::Argument {
                    argument,
                    part: SyntaxCallTypeArgumentPart::Type,
                },
            ),
        )
        .expect("validated Call type argument type component");
        if whole.start() < previous_end || ty.start() < whole.start() || ty.end() > whole.end() {
            return false;
        }
        previous_end = whole.end();
    }
    true
}

fn argument_ranges_validate(
    call: &SyntaxParenthesizedCallProjection,
    owner: SourceRange,
    components: &[PendingExpressionComponent],
) -> bool {
    let open = component_range(components, ExpressionComponentRole::CallArgumentListOpen);
    let tail = component_range(components, argument_list_terminal_role(call.terminator()));
    if open.is_none_or(|range| range.start() < owner.start() || range.start() == range.end())
        || tail.is_none_or(|range| {
            range.end() != owner.end()
                || matches!(
                    call.terminator(),
                    SyntaxCallArgumentListTerminator::RecoveredMissing
                ) && range.start() != range.end()
        })
    {
        return false;
    }

    let mut previous_end = open.expect("validated open component").end();
    for argument in 0..call.arguments().len() {
        let argument = u16::try_from(argument).expect("validated Call argument ordinal");
        let whole = component_range(
            components,
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Whole,
            },
        )
        .expect("validated Call argument whole component");
        let value = component_range(
            components,
            ExpressionComponentRole::CallArgument {
                argument,
                part: SyntaxCallArgumentPart::Value,
            },
        )
        .expect("validated Call argument value component");
        if whole.start() < previous_end
            || value.start() < whole.start()
            || value.end() > whole.end()
        {
            return false;
        }
        previous_end = whole.end();
    }
    true
}

const fn argument_list_terminal_role(
    terminator: SyntaxCallArgumentListTerminator,
) -> ExpressionComponentRole {
    match terminator {
        SyntaxCallArgumentListTerminator::Closed => ExpressionComponentRole::CallArgumentListClose,
        SyntaxCallArgumentListTerminator::RecoveredMissing => {
            ExpressionComponentRole::CallArgumentListRecoveryEnd
        }
    }
}

const fn type_application_terminal_role(
    terminator: SyntaxCallTypeApplicationTerminator,
) -> SyntaxCallTypeApplicationComponentRole {
    match terminator {
        SyntaxCallTypeApplicationTerminator::Closed
        | SyntaxCallTypeApplicationTerminator::InvalidPresent => {
            SyntaxCallTypeApplicationComponentRole::CloseAngle
        }
        SyntaxCallTypeApplicationTerminator::RecoveredMissing => {
            SyntaxCallTypeApplicationComponentRole::RecoveryEnd
        }
    }
}

fn call_head_components_validate(
    callee: &SyntaxCallCalleeProjection,
    components: &[PendingExpressionComponent],
) -> bool {
    match callee {
        SyntaxCallCalleeProjection::Ordinary => {
            let Some(callee_source) =
                component_range(components, ExpressionComponentRole::CallCallee)
            else {
                return false;
            };
            !callee_source.is_empty()
        }
        SyntaxCallCalleeProjection::UnresolvedDot { separator, member } => {
            matches!(separator, SyntaxAssociatedSeparator::Present(_))
                && member.is_ok()
                && associated_component_ranges_validate(member, components)
        }
        SyntaxCallCalleeProjection::Associated {
            receiver,
            separator,
            member,
        } => {
            let SyntaxAssociatedReceiver::Present = receiver;
            let SyntaxAssociatedSeparator::Present(_) = separator;
            associated_component_ranges_validate(member, components)
        }
    }
}

fn associated_component_ranges_validate(
    member: &Result<crate::name::SyntaxName, SyntaxNameIssue>,
    components: &[PendingExpressionComponent],
) -> bool {
    let Some(receiver_source) =
        component_range(components, ExpressionComponentRole::CallAssociatedReceiver)
    else {
        return false;
    };
    let Some(separator_source) =
        component_range(components, ExpressionComponentRole::CallAssociatedSeparator)
    else {
        return false;
    };
    let Some(member_source) =
        component_range(components, ExpressionComponentRole::CallAssociatedMember)
    else {
        return false;
    };
    let member_empty = matches!(member, Err(SyntaxNameIssue::Missing));
    let member_invalid_present = matches!(
        member,
        Err(SyntaxNameIssue::InvalidStart { .. } | SyntaxNameIssue::InvalidContinuation { .. })
    );

    !receiver_source.is_empty()
        && !separator_source.is_empty()
        && member_source.is_empty() == member_empty
        && (!member_invalid_present || !member_source.is_empty())
        && receiver_source.end() <= separator_source.start()
        && separator_source.end() <= member_source.start()
}
