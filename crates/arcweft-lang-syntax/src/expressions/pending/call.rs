//! Source-component validation for ordinary Call projections.

use std::collections::HashSet;

use arcweft_source::SourceRange;

use super::super::{
    ExpressionComponentRole, SyntaxCallArgumentListTerminator, SyntaxCallArgumentPart,
    SyntaxCallArgumentProjection, SyntaxCallCalleeProjection, SyntaxCallProjection,
    SyntaxCallTypeApplicationComponentRole, SyntaxCallTypeApplicationSpelling,
    SyntaxCallTypeApplicationTerminator, SyntaxCallTypeArgumentPart,
    SyntaxCallTypeArgumentProjection,
};
use super::{PendingExpressionComponent, component_range, exact_component_roles};

pub(super) fn components_validate(
    call: &SyntaxCallProjection,
    owner: SourceRange,
    roles: &HashSet<ExpressionComponentRole>,
    components: &[PendingExpressionComponent],
) -> bool {
    let SyntaxCallProjection::Parenthesized(call) = call else {
        let SyntaxCallProjection::CallbackBlock(callback) = call else {
            return false;
        };
        let terminal = match callback.terminator() {
            SyntaxCallArgumentListTerminator::Closed => {
                ExpressionComponentRole::CallArgumentListClose
            }
            SyntaxCallArgumentListTerminator::RecoveredMissing => {
                ExpressionComponentRole::CallArgumentListRecoveryEnd
            }
        };
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
        let Some(callee) = component_range(components, ExpressionComponentRole::CallCallee) else {
            return false;
        };
        let Some(open) = component_range(components, ExpressionComponentRole::CallArgumentListOpen)
        else {
            return false;
        };
        let Some(tail) = component_range(components, terminal) else {
            return false;
        };
        let Some(argument) = component_range(
            components,
            ExpressionComponentRole::CallArgument {
                argument: 0,
                part: SyntaxCallArgumentPart::Whole,
            },
        ) else {
            return false;
        };
        return callee.start() == owner.start()
            && callee.end() <= open.start()
            && open.start() < open.end()
            && argument.start() == open.start()
            && argument.end() == owner.end()
            && tail.end() == owner.end()
            && (callback.terminator() == SyntaxCallArgumentListTerminator::Closed
                || tail.start() == tail.end());
    };

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
        expected.extend([
            ExpressionComponentRole::CallTypeApplication(
                SyntaxCallTypeApplicationComponentRole::Whole,
            ),
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
            match application.terminator() {
                SyntaxCallTypeApplicationTerminator::Closed
                | SyntaxCallTypeApplicationTerminator::InvalidPresent => {
                    SyntaxCallTypeApplicationComponentRole::CloseAngle
                }
                SyntaxCallTypeApplicationTerminator::RecoveredMissing => {
                    SyntaxCallTypeApplicationComponentRole::RecoveryEnd
                }
            },
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
            let Ok(argument) = u16::try_from(argument) else {
                return false;
            };
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
    }
    expected.push(ExpressionComponentRole::CallArgumentListOpen);
    expected.push(match call.terminator() {
        SyntaxCallArgumentListTerminator::Closed => ExpressionComponentRole::CallArgumentListClose,
        SyntaxCallArgumentListTerminator::RecoveredMissing => {
            ExpressionComponentRole::CallArgumentListRecoveryEnd
        }
    });
    if call.arguments().is_empty() {
        expected.push(ExpressionComponentRole::CallArgumentListEmptyInsertion);
    }

    for (argument, projection) in call.arguments().iter().enumerate() {
        let Ok(argument) = u16::try_from(argument) else {
            return false;
        };
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
    if roles.contains(&ExpressionComponentRole::CallArgumentTrailingSeparator) {
        expected.push(ExpressionComponentRole::CallArgumentTrailingSeparator);
    }
    if !exact_component_roles(roles, components, &expected) {
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
    {
        return false;
    }
    if let Some(application) = call.explicit_type_application() {
        let whole_role = ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::Whole,
        );
        let open_role = ExpressionComponentRole::CallTypeApplication(
            SyntaxCallTypeApplicationComponentRole::OpenAngle,
        );
        let terminal_role =
            ExpressionComponentRole::CallTypeApplication(match application.terminator() {
                SyntaxCallTypeApplicationTerminator::Closed
                | SyntaxCallTypeApplicationTerminator::InvalidPresent => {
                    SyntaxCallTypeApplicationComponentRole::CloseAngle
                }
                SyntaxCallTypeApplicationTerminator::RecoveredMissing => {
                    SyntaxCallTypeApplicationComponentRole::RecoveryEnd
                }
            });
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
            if whole.start() < previous_end || ty.start() < whole.start() || ty.end() > whole.end()
            {
                return false;
            }
            previous_end = whole.end();
        }
    }
    let open = component_range(components, ExpressionComponentRole::CallArgumentListOpen);
    let tail = component_range(
        components,
        match call.terminator() {
            SyntaxCallArgumentListTerminator::Closed => {
                ExpressionComponentRole::CallArgumentListClose
            }
            SyntaxCallArgumentListTerminator::RecoveredMissing => {
                ExpressionComponentRole::CallArgumentListRecoveryEnd
            }
        },
    );
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
