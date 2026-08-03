//! Call projection, child, receiver, and type-argument validation.

use arcweft_lang_syntax::attachment::AttachedExpressionNode;
use arcweft_lang_syntax::expressions::{
    ExpressionProjection, SyntaxAssociatedCallSyntax, SyntaxCallArgumentListTerminator,
    SyntaxCallArgumentProjection, SyntaxCallCalleeProjection, SyntaxCallProjection,
    SyntaxCallTypeApplicationSpelling, SyntaxCallTypeApplicationTerminator,
    SyntaxCallTypeArgumentProjection, SyntaxCallTypeChildRole, SyntaxExpressionSlot,
    SyntaxRequiredTokenState,
};
use arcweft_lang_syntax::incremental::ParsedSource;
use arcweft_lang_syntax::name::SyntaxNameIssue;

use super::projection::{expression_child_matches, poison_state_matches};
use crate::arena::ArenaSnapshot;
use crate::expr::{
    HirAssociatedCallSyntax, HirAssociatedReceiver, HirCallArgument, HirCallArgumentListTerminator,
    HirCallArgumentOrdinal, HirCallCallee, HirCallChildPoison, HirCallChildStates,
    HirCallTypeApplication, HirCallTypeApplicationSpelling, HirCallTypeApplicationTerminator,
    HirCallTypeArgument, HirCallValue, HirExpr, HirRecoveredName, HirRecoveryIssue,
    HirRequiredTokenState,
};
use crate::identity::{ExprId, TypeId};
use crate::slot::{HirOrigin, SlotSnapshot};
use crate::source_index::{HirCallArgumentSourcePart, HirExprSourceRole};
use crate::type_ref::HirType;

pub(super) fn call_projection_matches(
    actual: &crate::expr::HirCallExpr,
    expected: &SyntaxCallProjection,
) -> bool {
    if let SyntaxCallProjection::CallbackBlock(expected) = expected {
        let [HirCallArgument::Positional { value }] = actual.arguments() else {
            return false;
        };
        return matches!(actual.callee(), HirCallCallee::Value { .. })
            && matches!(
                actual.explicit_type_application(),
                HirCallTypeApplication::Absent
            )
            && call_value_projection_matches(value, expected.callback())
            && matches!(
                (actual.terminator(), expected.terminator()),
                (
                    HirCallArgumentListTerminator::Closed,
                    SyntaxCallArgumentListTerminator::Closed
                ) | (
                    HirCallArgumentListTerminator::RecoveredMissing,
                    SyntaxCallArgumentListTerminator::RecoveredMissing
                )
            );
    }
    let SyntaxCallProjection::Parenthesized(expected) = expected else {
        return false;
    };
    let callee_matches = match (actual.callee(), expected.callee()) {
        (HirCallCallee::Value { .. }, SyntaxCallCalleeProjection::Ordinary) => true,
        (
            HirCallCallee::UnresolvedDot { member: actual, .. },
            SyntaxCallCalleeProjection::UnresolvedDot { member: expected },
        ) => recovered_call_name_matches(actual, expected),
        (
            HirCallCallee::Associated {
                member: actual_member,
                syntax: actual_syntax,
                ..
            },
            SyntaxCallCalleeProjection::Associated {
                member: expected_member,
                syntax: expected_syntax,
            },
        ) => {
            recovered_call_name_matches(actual_member, expected_member)
                && matches!(
                    (actual_syntax, expected_syntax),
                    (
                        HirAssociatedCallSyntax::DotFallback,
                        SyntaxAssociatedCallSyntax::DotFallback
                    ) | (
                        HirAssociatedCallSyntax::ExplicitDoubleColon,
                        SyntaxAssociatedCallSyntax::ExplicitDoubleColon
                    )
                )
        }
        _ => false,
    };
    if !callee_matches
        || !call_type_application_matches(
            actual.explicit_type_application(),
            expected.explicit_type_application(),
        )
        || actual.arguments().len() != expected.arguments().len()
        || !matches!(
            (actual.terminator(), expected.terminator()),
            (
                HirCallArgumentListTerminator::Closed,
                SyntaxCallArgumentListTerminator::Closed
            ) | (
                HirCallArgumentListTerminator::RecoveredMissing,
                SyntaxCallArgumentListTerminator::RecoveredMissing
            )
        )
    {
        return false;
    }

    actual
        .arguments()
        .iter()
        .zip(expected.arguments())
        .all(|(actual, expected)| match (actual, expected) {
            (
                HirCallArgument::Positional { value: actual },
                SyntaxCallArgumentProjection::Positional { value: expected },
            ) => call_value_projection_matches(actual, *expected),
            (
                HirCallArgument::Named {
                    name: actual_name,
                    equals: actual_equals,
                    value: actual_value,
                },
                SyntaxCallArgumentProjection::Named {
                    name: expected_name,
                    equals: expected_equals,
                    value: expected_value,
                },
            ) => {
                recovered_call_name_matches(actual_name, expected_name)
                    && required_call_token_matches(*actual_equals, *expected_equals)
                    && call_value_projection_matches(actual_value, *expected_value)
            }
            (
                HirCallArgument::Spread {
                    value: actual_value,
                    ellipsis: actual_ellipsis,
                },
                SyntaxCallArgumentProjection::Spread {
                    value: expected_value,
                    ellipsis: expected_ellipsis,
                },
            ) => {
                required_call_token_matches(*actual_ellipsis, *expected_ellipsis)
                    && call_value_projection_matches(actual_value, *expected_value)
            }
            _ => false,
        })
}

fn call_type_application_matches(
    actual: &HirCallTypeApplication,
    expected: Option<&arcweft_lang_syntax::expressions::SyntaxCallTypeApplicationProjection>,
) -> bool {
    match (actual, expected) {
        (HirCallTypeApplication::Absent, None) => true,
        (
            HirCallTypeApplication::Present {
                spelling,
                arguments,
                terminator,
            },
            Some(expected),
        ) => {
            matches!(
                (spelling, expected.spelling()),
                (
                    HirCallTypeApplicationSpelling::DirectAngle,
                    SyntaxCallTypeApplicationSpelling::DirectAngle
                ) | (
                    HirCallTypeApplicationSpelling::Turbofish,
                    SyntaxCallTypeApplicationSpelling::Turbofish
                )
            ) && matches!(
                (terminator, expected.terminator()),
                (
                    HirCallTypeApplicationTerminator::Closed,
                    SyntaxCallTypeApplicationTerminator::Closed
                ) | (
                    HirCallTypeApplicationTerminator::RecoveredMissing,
                    SyntaxCallTypeApplicationTerminator::RecoveredMissing
                ) | (
                    HirCallTypeApplicationTerminator::InvalidPresent,
                    SyntaxCallTypeApplicationTerminator::InvalidPresent
                )
            ) && arguments.len() == expected.arguments().len()
                && arguments
                    .iter()
                    .zip(expected.arguments())
                    .all(|(actual, expected)| {
                        matches!(
                            (actual, expected),
                            (
                                HirCallTypeArgument::Resolved { .. },
                                SyntaxCallTypeArgumentProjection::Present
                            ) | (
                                HirCallTypeArgument::InvalidPresent { .. },
                                SyntaxCallTypeArgumentProjection::InvalidPresent
                            ) | (
                                HirCallTypeArgument::Missing,
                                SyntaxCallTypeArgumentProjection::Missing
                            )
                        )
                    })
        }
        _ => false,
    }
}

fn call_value_projection_matches(actual: &HirCallValue, expected: SyntaxExpressionSlot) -> bool {
    matches!(
        (actual, expected),
        (HirCallValue::Present { .. }, SyntaxExpressionSlot::Authored)
            | (HirCallValue::Missing { .. }, SyntaxExpressionSlot::Missing)
    )
}

fn recovered_call_name_matches(
    actual: &HirRecoveredName,
    expected: &Result<arcweft_lang_syntax::name::SyntaxName, SyntaxNameIssue>,
) -> bool {
    match (actual, expected) {
        (HirRecoveredName::Valid(actual), Ok(expected)) => actual.as_str() == expected.as_str(),
        (HirRecoveredName::Missing, Err(SyntaxNameIssue::Missing)) => true,
        (
            HirRecoveredName::InvalidPresent,
            Err(SyntaxNameIssue::InvalidStart { .. } | SyntaxNameIssue::InvalidContinuation { .. }),
        ) => true,
        _ => false,
    }
}

const fn required_call_token_matches(
    actual: HirRequiredTokenState,
    expected: SyntaxRequiredTokenState,
) -> bool {
    matches!(
        (actual, expected),
        (
            HirRequiredTokenState::Present,
            SyntaxRequiredTokenState::Present
        ) | (
            HirRequiredTokenState::Missing,
            SyntaxRequiredTokenState::Missing
        ) | (
            HirRequiredTokenState::InvalidPresent,
            SyntaxRequiredTokenState::InvalidPresent
        )
    )
}

pub(super) fn call_children_match(
    parsed: &ParsedSource,
    slots: &SlotSnapshot,
    expressions: &ArenaSnapshot<HirExpr, ExprId>,
    types: &ArenaSnapshot<HirType, TypeId>,
    parent: ExprId,
    payload: &HirExpr,
    expression: &crate::expr::HirCallExpr,
    attached: &AttachedExpressionNode,
) -> bool {
    let ExpressionProjection::Call(projection) = attached.projection() else {
        return false;
    };
    let expected_argument_count = match projection {
        SyntaxCallProjection::Parenthesized(projection) => projection.arguments().len(),
        SyntaxCallProjection::CallbackBlock(_) => 1,
    };
    let value_child_count = usize::from(expression.callee().value_expression().is_some());
    if expected_argument_count != expression.arguments().len()
        || attached.children().len() != expression.arguments().len() + value_child_count
    {
        return false;
    }

    let callee_state = match expression.callee().value_expression() {
        Some(callee) => {
            let Some(attached_callee) = attached
                .children()
                .iter()
                .find(|child| child.ordinal() == 0)
            else {
                return false;
            };
            let role = match expression.callee() {
                HirCallCallee::Value { .. } => HirExprSourceRole::CallCallee,
                HirCallCallee::UnresolvedDot { .. } => HirExprSourceRole::CallAssociatedReceiver,
                HirCallCallee::Associated { .. } => return false,
            };
            if !expression_child_matches(
                parsed,
                slots,
                expressions,
                parent,
                payload.scope(),
                attached_callee,
                callee,
                role,
            ) {
                return false;
            }
            let Ok(callee_payload) = expressions.resolve_prepared(slots, callee) else {
                return false;
            };
            if callee_payload.is_poisoned() {
                HirCallChildPoison::Poisoned
            } else {
                HirCallChildPoison::Clean
            }
        }
        None => {
            if attached.children().iter().any(|child| child.ordinal() == 0) {
                return false;
            }
            HirCallChildPoison::Clean
        }
    };
    let receiver_matches = match expression.callee() {
        HirCallCallee::Value { .. } => attached.call_type_children().iter().all(|child| {
            matches!(
                child.role(),
                SyntaxCallTypeChildRole::ExplicitCallTypeArgument { .. }
            )
        }),
        HirCallCallee::UnresolvedDot {
            nominal_receiver, ..
        } => call_associated_receiver_matches(
            slots,
            types,
            attached,
            SyntaxCallTypeChildRole::DotNominalReceiver,
            nominal_receiver,
        ),
        HirCallCallee::Associated { receiver, .. } => call_associated_receiver_matches(
            slots,
            types,
            attached,
            SyntaxCallTypeChildRole::AssociatedReceiver,
            receiver,
        ),
    };
    if !receiver_matches {
        return false;
    }

    let Some(type_argument_states) = call_type_arguments_match(
        slots,
        types,
        attached,
        expression.explicit_type_application(),
    ) else {
        return false;
    };

    let mut argument_states = Vec::with_capacity(expression.arguments().len());
    for (position, argument) in expression.arguments().iter().enumerate() {
        let Ok(child_ordinal) = u32::try_from(position + 1) else {
            return false;
        };
        let Some(attached_argument) = attached
            .children()
            .iter()
            .find(|child| child.ordinal() == child_ordinal)
        else {
            return false;
        };
        let Ok(argument_ordinal) = HirCallArgumentOrdinal::try_new(position) else {
            return false;
        };
        if !expression_child_matches(
            parsed,
            slots,
            expressions,
            parent,
            payload.scope(),
            attached_argument,
            argument.value(),
            HirExprSourceRole::CallArgument {
                argument: argument_ordinal,
                part: HirCallArgumentSourcePart::Value,
            },
        ) {
            return false;
        }
        let Ok(argument_payload) = expressions.resolve_prepared(slots, argument.value()) else {
            return false;
        };
        argument_states.push(if argument_payload.is_poisoned() {
            HirCallChildPoison::Poisoned
        } else {
            HirCallChildPoison::Clean
        });
    }

    let expected = expression
        .primary_issue(HirCallChildStates::new(
            callee_state,
            &argument_states,
            &type_argument_states,
        ))
        .map(HirRecoveryIssue::InvalidCall);
    poison_state_matches(payload.state(), expected)
}

fn call_associated_receiver_matches(
    slots: &SlotSnapshot,
    types: &ArenaSnapshot<HirType, TypeId>,
    attached: &AttachedExpressionNode,
    expected_role: SyntaxCallTypeChildRole,
    receiver: &HirAssociatedReceiver,
) -> bool {
    let Some(attached_receiver) = attached
        .call_type_children()
        .iter()
        .find(|child| child.role() == expected_role)
    else {
        return false;
    };
    let receiver_id = receiver.type_id();
    let Ok(metadata) = slots.resolve_prepared(receiver_id) else {
        return false;
    };
    let HirOrigin::Source(source) = metadata.origin() else {
        return false;
    };
    if source.syntax() != attached_receiver.node().id() {
        return false;
    }
    let Ok(payload) = types.resolve_prepared(slots, receiver_id) else {
        return false;
    };
    match receiver {
        HirAssociatedReceiver::Resolved { .. } => !payload.is_poisoned(),
        HirAssociatedReceiver::InvalidPresent { .. } => payload.is_poisoned(),
        HirAssociatedReceiver::BareGenericArity { .. }
        | HirAssociatedReceiver::NominalError { .. } => true,
    }
}

fn call_type_arguments_match(
    slots: &SlotSnapshot,
    types: &ArenaSnapshot<HirType, TypeId>,
    attached: &AttachedExpressionNode,
    application: &HirCallTypeApplication,
) -> Option<Vec<HirCallChildPoison>> {
    let arguments = application.arguments();
    let expected_present = arguments
        .iter()
        .filter(|argument| !matches!(argument, HirCallTypeArgument::Missing))
        .count();
    if attached
        .call_type_children()
        .iter()
        .filter(|child| {
            matches!(
                child.role(),
                SyntaxCallTypeChildRole::ExplicitCallTypeArgument { .. }
            )
        })
        .count()
        != expected_present
    {
        return None;
    }
    let mut states = Vec::with_capacity(expected_present);
    for (position, argument) in arguments.iter().enumerate() {
        let role = SyntaxCallTypeChildRole::ExplicitCallTypeArgument {
            ordinal: u16::try_from(position).ok()?,
        };
        let attached_argument = attached
            .call_type_children()
            .iter()
            .find(|child| child.role() == role);
        let Some(type_id) = argument.type_id() else {
            if attached_argument.is_some() {
                return None;
            }
            continue;
        };
        let attached_argument = attached_argument?;
        let metadata = slots.resolve_prepared(type_id).ok()?;
        let HirOrigin::Source(source) = metadata.origin() else {
            return None;
        };
        if source.syntax() != attached_argument.node().id() {
            return None;
        }
        let payload = types.resolve_prepared(slots, type_id).ok()?;
        let poisoned = payload.is_poisoned();
        if poisoned != matches!(argument, HirCallTypeArgument::InvalidPresent { .. }) {
            return None;
        }
        states.push(if poisoned {
            HirCallChildPoison::Poisoned
        } else {
            HirCallChildPoison::Clean
        });
    }
    Some(states)
}
